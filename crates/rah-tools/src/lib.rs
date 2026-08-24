//! Tool abstractions for RAH.

use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use rah_protocol::{ToolCall, ToolDefinition, ToolInput, ToolName, ToolOutput};
use thiserror::Error;

mod cargo_version;
mod echo;
mod external_permissions;
mod fs_read;
mod git_stage;
mod git_status;
mod git_support;
mod git_unstage;
mod host_execute;
mod repository_diff;
mod repository_diff_staged;
mod repository_file_info;
mod repository_mutation;
mod repository_observer;
mod repository_status;
mod repository_worktree_patch;
mod shell_exec;
mod trusted_profile;
mod trusted_profile_source;

pub use cargo_version::{CARGO_VERSION_TOOL_NAME, CargoVersionTool};
pub use echo::EchoTool;
pub use external_permissions::{
    ExternalToolIdentity, ExternalToolPermissionError, ExternalToolPermissionPolicy,
};
pub use fs_read::FsReadTool;
pub use git_stage::{GIT_STAGE_TOOL_NAME, GitStageTool};
pub use git_status::{GIT_STATUS_TOOL_NAME, GitStatusTool};
pub use git_unstage::{GIT_UNSTAGE_TOOL_NAME, GitUnstageTool};
pub use host_execute::{HostArgumentPolicy, HostExecutionPolicy, HostExecutionTool};
pub use repository_diff::{REPOSITORY_DIFF_TOOL_NAME, RepositoryDiffTool};
pub use repository_diff_staged::{REPOSITORY_DIFF_STAGED_TOOL_NAME, RepositoryDiffStagedTool};
pub use repository_file_info::{REPOSITORY_FILE_INFO_TOOL_NAME, RepositoryFileInfoTool};
pub use repository_mutation::{RepositoryMutationFixtureTestMode, RepositoryMutationFixtureTool};
pub use repository_status::{REPOSITORY_STATUS_TOOL_NAME, RepositoryStatusTool};
pub use repository_worktree_patch::{
    REPOSITORY_WORKTREE_PATCH_TOOL_NAME, RepositoryWorktreePatchTool,
};
#[cfg(feature = "live-test-support")]
pub use repository_worktree_patch::{
    live_test_replacement_attempts, reset_live_test_replacement_attempts,
};
pub use shell_exec::ShellExecTool;
pub use trusted_profile::{
    EffectiveCapability, EffectiveProfile, EffectiveProvider, McpExpectedToolProfile,
    McpProviderProfile, ProcessPluginExpectedToolProfile, ProcessPluginProfile, ProfileError,
    RepositoryObserverProfile, RepositoryWorktreePatchProfile, TrustedStaticProfile,
};

/// Neutral execution context supplied by the runtime to a tool.
#[derive(Clone, Debug, Default)]
pub struct ToolContext {}

/// Error returned by tool registration, lookup, or execution.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ToolError {
    /// A tool with the same name is already registered.
    #[error("tool `{name}` is already registered")]
    DuplicateTool {
        /// Conflicting tool name.
        name: ToolName,
    },
    /// No registered tool has the requested name.
    #[error("tool `{name}` is not registered")]
    UnknownTool {
        /// Requested tool name.
        name: ToolName,
    },
    /// The supplied tool input is invalid.
    #[error("invalid tool input: {message}")]
    InvalidInput {
        /// Validation failure detail.
        message: String,
    },
    /// The tool failed during execution.
    #[error("tool execution failed: {message}")]
    Execution {
        /// Execution failure detail.
        message: String,
    },
}

/// Provider-neutral executable capability registered with RAH.
#[async_trait]
pub trait Tool: Send + Sync {
    /// Returns the serializable definition exposed through RAH boundaries.
    fn definition(&self) -> ToolDefinition;

    /// Executes validated input within the runtime-supplied context.
    async fn execute(
        &self,
        input: ToolInput,
        context: ToolContext,
    ) -> Result<ToolOutput, ToolError>;
}

/// Registry that dispatches all built-in and external tools through one boundary.
#[derive(Default)]
pub struct ToolRegistry {
    tools: HashMap<ToolName, Arc<dyn Tool>>,
}

impl ToolRegistry {
    /// Creates an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a tool, rejecting duplicate names deterministically.
    pub fn register(&mut self, tool: Arc<dyn Tool>) -> Result<(), ToolError> {
        let name = tool.definition().name;
        if self.tools.contains_key(&name) {
            return Err(ToolError::DuplicateTool { name });
        }

        self.tools.insert(name, tool);
        Ok(())
    }

    /// Returns a registered tool by name.
    #[must_use]
    pub fn get(&self, name: &ToolName) -> Option<Arc<dyn Tool>> {
        self.tools.get(name).cloned()
    }

    /// Returns definitions sorted by tool name for deterministic callers.
    #[must_use]
    pub fn definitions(&self) -> Vec<ToolDefinition> {
        let mut definitions = self
            .tools
            .values()
            .map(|tool| tool.definition())
            .collect::<Vec<_>>();
        definitions.sort_by(|left, right| left.name.as_str().cmp(right.name.as_str()));
        definitions
    }

    /// Dispatches a parsed call to the registered tool abstraction.
    pub async fn execute(
        &self,
        call: ToolCall,
        context: ToolContext,
    ) -> Result<ToolOutput, ToolError> {
        let tool = self.get(&call.name).ok_or_else(|| ToolError::UnknownTool {
            name: call.name.clone(),
        })?;
        tool.execute(call.input, context).await
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use async_trait::async_trait;
    use futures::executor::block_on;
    use rah_protocol::{
        PermissionLevel, ToolCall, ToolCallId, ToolContent, ToolDefinition, ToolInput, ToolName,
        ToolOutput,
    };

    use super::{Tool, ToolContext, ToolError, ToolRegistry};

    struct TestTool {
        name: &'static str,
    }

    #[async_trait]
    impl Tool for TestTool {
        fn definition(&self) -> ToolDefinition {
            ToolDefinition {
                name: ToolName::new(self.name),
                description: "Test tool".to_owned(),
                input_schema: Default::default(),
                permission: PermissionLevel::None,
            }
        }

        async fn execute(
            &self,
            _input: ToolInput,
            _context: ToolContext,
        ) -> Result<ToolOutput, ToolError> {
            Ok(ToolOutput {
                content: vec![ToolContent::Text(self.name.to_owned())],
                is_error: false,
            })
        }
    }

    fn call(name: &str) -> ToolCall {
        ToolCall {
            id: ToolCallId::new(),
            name: ToolName::new(name),
            input: ToolInput(Default::default()),
        }
    }

    #[test]
    fn registers_and_gets_tool() {
        let mut registry = ToolRegistry::new();
        registry
            .register(Arc::new(TestTool { name: "alpha" }))
            .expect("unique tool should register");

        assert!(registry.get(&ToolName::new("alpha")).is_some());
    }

    #[test]
    fn duplicate_registration_is_rejected() {
        let mut registry = ToolRegistry::new();
        registry
            .register(Arc::new(TestTool { name: "alpha" }))
            .expect("first tool should register");

        let error = registry
            .register(Arc::new(TestTool { name: "alpha" }))
            .expect_err("duplicate tool should fail");

        assert_eq!(
            error,
            ToolError::DuplicateTool {
                name: ToolName::new("alpha")
            }
        );
        let output = block_on(registry.execute(call("alpha"), ToolContext::default()))
            .expect("duplicate rejection must preserve the first registered tool");
        assert_eq!(output.content, vec![ToolContent::Text("alpha".to_owned())]);
    }

    #[test]
    fn unknown_tool_is_reported() {
        block_on(async {
            let registry = ToolRegistry::new();

            let error = registry
                .execute(call("missing"), ToolContext::default())
                .await
                .expect_err("unknown tool should fail");

            assert_eq!(
                error,
                ToolError::UnknownTool {
                    name: ToolName::new("missing")
                }
            );
        });
    }

    #[test]
    fn executes_registered_tool() {
        block_on(async {
            let mut registry = ToolRegistry::new();
            registry
                .register(Arc::new(TestTool { name: "alpha" }))
                .expect("unique tool should register");

            let output = registry
                .execute(call("alpha"), ToolContext::default())
                .await
                .expect("registered tool should execute");

            assert_eq!(
                output,
                ToolOutput {
                    content: vec![ToolContent::Text("alpha".to_owned())],
                    is_error: false
                }
            );
        });
    }

    #[test]
    fn definitions_are_sorted_by_name() {
        let mut registry = ToolRegistry::new();
        registry
            .register(Arc::new(TestTool { name: "zeta" }))
            .expect("unique tool should register");
        registry
            .register(Arc::new(TestTool { name: "alpha" }))
            .expect("unique tool should register");

        let names = registry
            .definitions()
            .into_iter()
            .map(|definition| definition.name)
            .collect::<Vec<_>>();

        assert_eq!(names, vec![ToolName::new("alpha"), ToolName::new("zeta")]);
    }
}
