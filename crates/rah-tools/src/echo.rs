use async_trait::async_trait;
use rah_protocol::{PermissionLevel, ToolContent, ToolDefinition, ToolInput, ToolName, ToolOutput};
use serde_json::json;

use crate::{Tool, ToolContext, ToolError};

/// Safe built-in tool that returns its input text unchanged.
#[derive(Debug, Default)]
pub struct EchoTool;

impl EchoTool {
    /// Creates an echo tool.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for EchoTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: ToolName::new("echo"),
            description: "Returns the supplied text unchanged.".to_owned(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "text": {"type": "string"}
                },
                "required": ["text"],
                "additionalProperties": false
            }),
            permission: PermissionLevel::None,
        }
    }

    async fn execute(
        &self,
        input: ToolInput,
        _context: ToolContext,
    ) -> Result<ToolOutput, ToolError> {
        let text = input
            .0
            .as_object()
            .and_then(|object| object.get("text"))
            .and_then(|value| value.as_str())
            .ok_or_else(|| ToolError::InvalidInput {
                message: "`text` must be a string".to_owned(),
            })?;

        Ok(ToolOutput {
            content: vec![ToolContent::Text(text.to_owned())],
            is_error: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use futures::executor::block_on;
    use rah_protocol::{
        PermissionLevel, ToolCall, ToolCallId, ToolContent, ToolInput, ToolName, ToolOutput,
    };
    use serde_json::json;

    use super::EchoTool;
    use crate::{Tool, ToolContext, ToolError, ToolRegistry};

    #[test]
    fn definition_is_safe_and_provider_neutral() {
        let definition = EchoTool::new().definition();

        assert_eq!(definition.name, ToolName::new("echo"));
        assert_eq!(definition.permission, PermissionLevel::None);
        assert_eq!(definition.input_schema["required"], json!(["text"]));
    }

    #[test]
    fn returns_input_text_through_registry() {
        block_on(async {
            let mut registry = ToolRegistry::new();
            registry
                .register(Arc::new(EchoTool::new()))
                .expect("echo should register");

            let output = registry
                .execute(
                    ToolCall {
                        id: ToolCallId::new(),
                        name: ToolName::new("echo"),
                        input: ToolInput(json!({"text": "hello"})),
                    },
                    ToolContext::default(),
                )
                .await
                .expect("valid echo input should succeed");

            assert_eq!(
                output,
                ToolOutput {
                    content: vec![ToolContent::Text("hello".to_owned())],
                    is_error: false
                }
            );
        });
    }

    #[test]
    fn rejects_non_string_text() {
        block_on(async {
            let error = EchoTool::new()
                .execute(ToolInput(json!({"text": 7})), ToolContext::default())
                .await
                .expect_err("non-string text should fail");

            assert_eq!(
                error,
                ToolError::InvalidInput {
                    message: "`text` must be a string".to_owned()
                }
            );
        });
    }
}
