//! Neutral authorized dispatch for current registered tools.

use rah_protocol::{PermissionLevel, ToolCall, ToolDefinition, ToolName, ToolOutput};
use thiserror::Error;

use crate::{ToolContext, ToolError, ToolRegistry};

/// A pre-dispatch admission rejection from [`authorized_tool_dispatch`].
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum AuthorizedDispatchRejection {
    /// The requested call and host expectation name different public tools.
    #[error("tool call name does not match the expected definition name")]
    NameMismatch {
        /// Public name supplied by the call.
        call_name: ToolName,
        /// Public name supplied by the host expectation.
        expected_name: ToolName,
    },
    /// No current registered tool has the requested public name.
    #[error("tool is not currently registered")]
    UnknownTool {
        /// Requested public tool name.
        name: ToolName,
    },
    /// The current registered definition no longer matches the host expectation.
    #[error("current tool definition does not match the expected definition")]
    DefinitionMismatch {
        /// Host-owned definition expected for this dispatch.
        expected: ToolDefinition,
        /// Current definition returned by the registered tool.
        current: ToolDefinition,
    },
    /// The current tool permission is absent from the host-owned policy.
    #[error("current tool permission is not allowed")]
    PermissionDenied {
        /// Current public tool name.
        name: ToolName,
        /// Current permission required by the registered tool.
        permission: PermissionLevel,
    },
}

/// The result of authorized dispatch admission or underlying tool execution.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum AuthorizedDispatchError {
    /// Dispatch admission was rejected before tool execution.
    #[error("authorized tool dispatch rejected: {0}")]
    Rejected(AuthorizedDispatchRejection),
    /// The admitted tool returned its ordinary execution error.
    #[error("authorized tool dispatch execution failed: {0}")]
    Tool(#[source] ToolError),
}

/// Dispatches one call only after current definition and permission admission.
///
/// This is a neutral dispatch primitive. It does not compose capability
/// authority, choose product eligibility, interpret tool output, or own
/// lifecycle and retry behavior.
pub async fn authorized_tool_dispatch(
    registry: &ToolRegistry,
    expected_definition: &ToolDefinition,
    allowed_permissions: &[PermissionLevel],
    call: ToolCall,
    context: ToolContext,
) -> Result<ToolOutput, AuthorizedDispatchError> {
    if call.name != expected_definition.name {
        return Err(AuthorizedDispatchError::Rejected(
            AuthorizedDispatchRejection::NameMismatch {
                call_name: call.name,
                expected_name: expected_definition.name.clone(),
            },
        ));
    }

    let current = registry.get(&call.name).ok_or_else(|| {
        AuthorizedDispatchError::Rejected(AuthorizedDispatchRejection::UnknownTool {
            name: call.name.clone(),
        })
    })?;
    let current_definition = current.definition();

    if current_definition != *expected_definition {
        return Err(AuthorizedDispatchError::Rejected(
            AuthorizedDispatchRejection::DefinitionMismatch {
                expected: expected_definition.clone(),
                current: current_definition,
            },
        ));
    }

    if !allowed_permissions.contains(&current_definition.permission) {
        return Err(AuthorizedDispatchError::Rejected(
            AuthorizedDispatchRejection::PermissionDenied {
                name: current_definition.name,
                permission: current_definition.permission,
            },
        ));
    }

    registry
        .execute(call, context)
        .await
        .map_err(AuthorizedDispatchError::Tool)
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use async_trait::async_trait;
    use futures::executor::block_on;
    use rah_protocol::{
        PermissionLevel, ToolCall, ToolCallId, ToolContent, ToolDefinition, ToolInput, ToolName,
        ToolOutput,
    };
    use serde_json::json;

    use super::{AuthorizedDispatchError, AuthorizedDispatchRejection, authorized_tool_dispatch};
    use crate::{Tool, ToolContext, ToolError, ToolRegistry};

    struct TestTool {
        registered_definition: ToolDefinition,
        current_definition: ToolDefinition,
        definition_calls: AtomicUsize,
        result: Result<ToolOutput, ToolError>,
        executions: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl Tool for TestTool {
        fn definition(&self) -> ToolDefinition {
            if self.definition_calls.fetch_add(1, Ordering::SeqCst) == 0 {
                self.registered_definition.clone()
            } else {
                self.current_definition.clone()
            }
        }

        async fn execute(
            &self,
            _input: ToolInput,
            _context: ToolContext,
        ) -> Result<ToolOutput, ToolError> {
            self.executions.fetch_add(1, Ordering::SeqCst);
            self.result.clone()
        }
    }

    fn definition(name: &str, permission: PermissionLevel) -> ToolDefinition {
        ToolDefinition {
            name: ToolName::new(name),
            description: "deterministic test tool".to_owned(),
            input_schema: json!({"type": "object", "additionalProperties": false}),
            permission,
        }
    }

    fn call(name: &str) -> ToolCall {
        ToolCall {
            id: ToolCallId::new(),
            name: ToolName::new(name),
            input: ToolInput(json!({})),
        }
    }

    fn registry_with(
        definition: ToolDefinition,
        result: Result<ToolOutput, ToolError>,
    ) -> (ToolRegistry, Arc<AtomicUsize>) {
        registry_with_definitions(definition.clone(), definition, result)
    }

    fn registry_with_definitions(
        registered_definition: ToolDefinition,
        current_definition: ToolDefinition,
        result: Result<ToolOutput, ToolError>,
    ) -> (ToolRegistry, Arc<AtomicUsize>) {
        let executions = Arc::new(AtomicUsize::new(0));
        let mut registry = ToolRegistry::new();
        registry
            .register(Arc::new(TestTool {
                registered_definition,
                current_definition,
                definition_calls: AtomicUsize::new(0),
                result,
                executions: Arc::clone(&executions),
            }))
            .expect("fixture tool should register");
        (registry, executions)
    }

    fn output(is_error: bool) -> ToolOutput {
        ToolOutput {
            content: vec![
                ToolContent::Text("exact text".to_owned()),
                ToolContent::Json(json!({"status": "exact", "count": 1})),
            ],
            is_error,
        }
    }

    #[test]
    fn authorized_dispatch_exact_definition_and_allowed_permission_executes_once() {
        block_on(async {
            let expected = definition("test.tool", PermissionLevel::Read);
            let expected_output = output(false);
            let (registry, executions) =
                registry_with(expected.clone(), Ok(expected_output.clone()));

            let actual = authorized_tool_dispatch(
                &registry,
                &expected,
                &[PermissionLevel::Read],
                call("test.tool"),
                ToolContext::default(),
            )
            .await
            .expect("matching current definition and permission should dispatch");

            assert_eq!(actual, expected_output);
            assert_eq!(executions.load(Ordering::SeqCst), 1);
        });
    }

    #[test]
    fn authorized_dispatch_none_requires_explicit_membership() {
        assert_permission_membership(PermissionLevel::None, &[PermissionLevel::None], true);
        assert_permission_membership(PermissionLevel::None, &[PermissionLevel::Read], false);
    }

    #[test]
    fn authorized_dispatch_read_requires_explicit_membership() {
        assert_permission_membership(PermissionLevel::Read, &[PermissionLevel::Read], true);
        assert_permission_membership(PermissionLevel::Read, &[PermissionLevel::Execute], false);
    }

    #[test]
    fn authorized_dispatch_write_requires_explicit_membership() {
        assert_permission_membership(PermissionLevel::Write, &[PermissionLevel::Write], true);
        assert_permission_membership(PermissionLevel::Write, &[PermissionLevel::Execute], false);
    }

    #[test]
    fn authorized_dispatch_execute_requires_explicit_membership() {
        assert_permission_membership(PermissionLevel::Execute, &[PermissionLevel::Execute], true);
        assert_permission_membership(
            PermissionLevel::Execute,
            &[PermissionLevel::None, PermissionLevel::Read],
            false,
        );
    }

    #[test]
    fn authorized_dispatch_permission_order_and_duplicates_do_not_change_membership() {
        assert_permission_membership(
            PermissionLevel::Read,
            &[
                PermissionLevel::Execute,
                PermissionLevel::Read,
                PermissionLevel::Read,
            ],
            true,
        );
    }

    #[test]
    fn authorized_dispatch_rejects_call_expected_name_mismatch_without_execution() {
        let expected = definition("expected.tool", PermissionLevel::Read);
        let (registry, executions) = registry_with(expected.clone(), Ok(output(false)));

        let error = block_on(authorized_tool_dispatch(
            &registry,
            &expected,
            &[PermissionLevel::Read],
            call("other.tool"),
            ToolContext::default(),
        ))
        .expect_err("mismatched call name should reject");

        assert!(matches!(
            error,
            AuthorizedDispatchError::Rejected(AuthorizedDispatchRejection::NameMismatch { .. })
        ));
        assert_eq!(executions.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn authorized_dispatch_rejects_unknown_tool_without_execution() {
        let expected = definition("missing.tool", PermissionLevel::Read);
        let registry = ToolRegistry::new();

        let error = block_on(authorized_tool_dispatch(
            &registry,
            &expected,
            &[PermissionLevel::Read],
            call("missing.tool"),
            ToolContext::default(),
        ))
        .expect_err("missing tool should reject");

        assert!(matches!(
            error,
            AuthorizedDispatchError::Rejected(AuthorizedDispatchRejection::UnknownTool { .. })
        ));
    }

    #[test]
    fn authorized_dispatch_rejects_description_schema_and_name_definition_changes() {
        for current in [
            ToolDefinition {
                description: "changed".to_owned(),
                ..definition("test.tool", PermissionLevel::Read)
            },
            ToolDefinition {
                input_schema: json!({"type": "string"}),
                ..definition("test.tool", PermissionLevel::Read)
            },
            definition("current.tool", PermissionLevel::Read),
        ] {
            let expected = definition("test.tool", PermissionLevel::Read);
            let (registry, executions) =
                registry_with_definitions(expected.clone(), current, Ok(output(false)));

            let error = block_on(authorized_tool_dispatch(
                &registry,
                &expected,
                &[PermissionLevel::Read],
                call("test.tool"),
                ToolContext::default(),
            ))
            .expect_err("changed current definition should reject");

            assert!(matches!(
                error,
                AuthorizedDispatchError::Rejected(
                    AuthorizedDispatchRejection::DefinitionMismatch { .. }
                )
            ));
            assert_eq!(executions.load(Ordering::SeqCst), 0);
        }
    }

    #[test]
    fn authorized_dispatch_rejects_permission_change_even_when_new_permission_is_allowed() {
        for (expected_permission, current_permission) in [
            (PermissionLevel::Read, PermissionLevel::Execute),
            (PermissionLevel::Execute, PermissionLevel::Read),
        ] {
            let expected = definition("test.tool", expected_permission);
            let current = definition("test.tool", current_permission);
            let (registry, executions) = registry_with(current, Ok(output(false)));

            let error = block_on(authorized_tool_dispatch(
                &registry,
                &expected,
                &[current_permission],
                call("test.tool"),
                ToolContext::default(),
            ))
            .expect_err("permission change must be a stale definition mismatch");

            assert!(matches!(
                error,
                AuthorizedDispatchError::Rejected(
                    AuthorizedDispatchRejection::DefinitionMismatch { .. }
                )
            ));
            assert_eq!(executions.load(Ordering::SeqCst), 0);
        }
    }

    #[test]
    fn authorized_dispatch_rejects_absent_and_empty_permission_policy_without_execution() {
        for allowed_permissions in [vec![PermissionLevel::Write], vec![]] {
            let expected = definition("test.tool", PermissionLevel::Read);
            let (registry, executions) = registry_with(expected.clone(), Ok(output(false)));

            let error = block_on(authorized_tool_dispatch(
                &registry,
                &expected,
                &allowed_permissions,
                call("test.tool"),
                ToolContext::default(),
            ))
            .expect_err("missing current permission should reject");

            assert!(matches!(
                error,
                AuthorizedDispatchError::Rejected(
                    AuthorizedDispatchRejection::PermissionDenied { .. }
                )
            ));
            assert_eq!(executions.load(Ordering::SeqCst), 0);
        }
    }

    #[test]
    fn authorized_dispatch_preserves_underlying_tool_error_without_retry() {
        let expected = definition("test.tool", PermissionLevel::Read);
        let tool_error = ToolError::Execution {
            message: "fixture failure".to_owned(),
        };
        let (registry, executions) = registry_with(expected.clone(), Err(tool_error.clone()));

        let error = block_on(authorized_tool_dispatch(
            &registry,
            &expected,
            &[PermissionLevel::Read],
            call("test.tool"),
            ToolContext::default(),
        ))
        .expect_err("admitted tool error should remain distinct");

        assert_eq!(error, AuthorizedDispatchError::Tool(tool_error));
        assert_eq!(executions.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn authorized_dispatch_returns_error_output_without_reclassifying_it() {
        let expected = definition("test.tool", PermissionLevel::Read);
        let expected_output = output(true);
        let (registry, executions) = registry_with(expected.clone(), Ok(expected_output.clone()));

        let actual = block_on(authorized_tool_dispatch(
            &registry,
            &expected,
            &[PermissionLevel::Read],
            call("test.tool"),
            ToolContext::default(),
        ))
        .expect("tool output is not a dispatch error");

        assert_eq!(actual, expected_output);
        assert!(actual.is_error);
        assert_eq!(executions.load(Ordering::SeqCst), 1);
    }

    fn assert_permission_membership(
        permission: PermissionLevel,
        allowed_permissions: &[PermissionLevel],
        admitted: bool,
    ) {
        let expected = definition("test.tool", permission);
        let (registry, executions) = registry_with(expected.clone(), Ok(output(false)));
        let result = block_on(authorized_tool_dispatch(
            &registry,
            &expected,
            allowed_permissions,
            call("test.tool"),
            ToolContext::default(),
        ));

        assert_eq!(result.is_ok(), admitted);
        assert_eq!(executions.load(Ordering::SeqCst), usize::from(admitted));
    }
}
