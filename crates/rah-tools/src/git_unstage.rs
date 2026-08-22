use std::path::Path;

use async_trait::async_trait;
use rah_protocol::{PermissionLevel, ToolDefinition, ToolInput, ToolName, ToolOutput};
use serde_json::json;

use crate::{
    Tool, ToolContext, ToolError,
    git_stage::{GitIndexMutation, GitIndexMutationPolicy, reject_input},
};

/// Stable name for the single-target host-authorized Git unstaging capability.
pub const GIT_UNSTAGE_TOOL_NAME: &str = "host.git.unstage";

/// Replaces exactly one host-bound tracked regular file's index entry with its
/// `HEAD` entry, without writing its worktree bytes.
///
/// `symbolic_target` and `target_path` are trusted host configuration, never
/// model input. The only model-visible input accepted by this tool is `{}`.
pub struct GitUnstageTool {
    policy: GitIndexMutationPolicy,
}

impl GitUnstageTool {
    /// Creates an unstaging capability with one canonical repository, native
    /// Git executable, symbolic target, and existing regular target file.
    pub fn new(
        git_executable: impl AsRef<Path>,
        repository_root: impl AsRef<Path>,
        symbolic_target: impl Into<String>,
        target_path: impl AsRef<Path>,
    ) -> Result<Self, ToolError> {
        Ok(Self {
            policy: GitIndexMutationPolicy::new(
                git_executable.as_ref(),
                repository_root.as_ref(),
                symbolic_target.into(),
                target_path.as_ref(),
                GitIndexMutation::Unstage,
            )?,
        })
    }
}

#[async_trait]
impl Tool for GitUnstageTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: ToolName::new(GIT_UNSTAGE_TOOL_NAME),
            description:
                "Unstages one host-authorized tracked file without changing its worktree bytes."
                    .to_owned(),
            input_schema: json!({"type":"object","properties":{},"additionalProperties":false}),
            permission: PermissionLevel::Execute,
        }
    }

    async fn execute(
        &self,
        input: ToolInput,
        _context: ToolContext,
    ) -> Result<ToolOutput, ToolError> {
        reject_input(&input)?;
        let _lease = self.policy.acquire_lease().await;
        self.policy.execute_once(GitIndexMutation::Unstage).await
    }
}
