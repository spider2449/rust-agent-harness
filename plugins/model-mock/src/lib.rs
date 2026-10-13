//! `model-mock`: a deterministic, rule-based model for development and tests.
//!
//! Rules, applied to the last context item:
//! - a tool result: reply `[label] <tool> returned: <output>`;
//! - a user message starting with `/notool `: reply with the text and the
//!   number of earlier context items (useful to observe resumed history);
//! - a user message `read <path>` when `fs.read` is offered: request it;
//! - a user message `/call <tool> <json>` when `<tool>` is offered: request
//!   that tool with the JSON arguments;
//! - any other user message when `echo` is offered: request `echo`;
//! - otherwise: reply `[label] you said: <text> (<n> earlier items)`.
//!
//! Configuration: `{ "label": "..." }` (defaults to the plugin id).

use std::sync::Arc;

use futures::stream;
use rah_api::{
    CancellationToken, Plugin, PluginError, PluginId, PluginManifest, Scope, async_trait,
    model::{MODEL, ModelError, ModelProvider, ModelStream},
    tools::describe_image,
};
use rah_protocol::{ContextItem, ModelEvent, ModelRequest, ToolContent, ToolName, ToolOutput};
use serde::Deserialize;
use serde_json::json;

pub struct MockModelPlugin {
    manifest: PluginManifest,
}

impl MockModelPlugin {
    pub fn new(id: impl Into<PluginId>) -> Self {
        Self {
            manifest: PluginManifest::new(id, env!("CARGO_PKG_VERSION")).provides(["model"]),
        }
    }
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    label: Option<String>,
}

#[async_trait]
impl Plugin for MockModelPlugin {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    async fn apply(&self, scope: &mut Scope) -> Result<(), PluginError> {
        let config: Config = if scope.config().is_null() {
            Config::default()
        } else {
            scope.config_as()?
        };
        let label = config.label.unwrap_or_else(|| scope.id().to_string());
        scope.provide(MODEL, Arc::new(MockModel { label }))?;
        Ok(())
    }
}

pub struct MockModel {
    label: String,
}

impl MockModel {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
        }
    }

    /// The events this model produces for a request.
    pub fn respond(&self, request: &ModelRequest) -> Vec<ModelEvent> {
        let label = &self.label;
        let offers = |name: &str| request.tools.iter().any(|t| t.name.as_str() == name);
        let earlier = request.items.len().saturating_sub(1);
        let mut events = match request.items.last() {
            Some(ContextItem::ToolResult { call_id, output }) => {
                let tool = request
                    .items
                    .iter()
                    .rev()
                    .find_map(|item| match item {
                        ContextItem::ToolCall {
                            call_id: id, name, ..
                        } if id == call_id => Some(name.to_string()),
                        _ => None,
                    })
                    .unwrap_or_else(|| "tool".to_owned());
                text(format!("[{label}] {tool} returned: {}", render(output)))
            }
            Some(ContextItem::User { text: input }) => {
                let call_id = format!(
                    "mock-{}",
                    request
                        .items
                        .iter()
                        .filter(|i| matches!(i, ContextItem::ToolCall { .. }))
                        .count()
                        + 1
                );
                if let Some(rest) = input.strip_prefix("/call ") {
                    let (name, arguments) = rest.split_once(' ').unwrap_or((rest, "{}"));
                    match serde_json::from_str(arguments) {
                        Ok(arguments) if offers(name) => vec![ModelEvent::ToolCall {
                            call_id,
                            name: ToolName::new(name),
                            arguments,
                        }],
                        Ok(_) => text(format!("[{label}] tool `{name}` is not offered")),
                        Err(error) => text(format!("[{label}] invalid arguments: {error}")),
                    }
                } else if let Some(rest) = input.strip_prefix("/notool ") {
                    text(format!(
                        "[{label}] you said: {rest} ({earlier} earlier items)"
                    ))
                } else if let Some(path) = input.strip_prefix("read ").filter(|_| offers("fs.read"))
                {
                    vec![ModelEvent::ToolCall {
                        call_id,
                        name: ToolName::new("fs.read"),
                        arguments: json!({ "path": path.trim() }),
                    }]
                } else if offers("echo") {
                    vec![ModelEvent::ToolCall {
                        call_id,
                        name: ToolName::new("echo"),
                        arguments: json!({ "text": input }),
                    }]
                } else {
                    text(format!(
                        "[{label}] you said: {input} ({earlier} earlier items)"
                    ))
                }
            }
            _ => text(format!("[{label}] ready")),
        };
        events.push(ModelEvent::Completed);
        events
    }
}

fn text(reply: String) -> Vec<ModelEvent> {
    reply
        .split_inclusive(' ')
        .map(|word| ModelEvent::TextDelta {
            text: word.to_owned(),
        })
        .collect()
}

fn render(output: &ToolOutput) -> String {
    output
        .content
        .iter()
        .map(|content| match content {
            ToolContent::Text(text) => text.clone(),
            ToolContent::Json(value) => value.to_string(),
            ToolContent::Image(image) => describe_image(image),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[async_trait]
impl ModelProvider for MockModel {
    async fn stream(
        &self,
        request: ModelRequest,
        cancel: CancellationToken,
    ) -> Result<ModelStream, ModelError> {
        if cancel.is_cancelled() {
            return Err(ModelError::Cancelled);
        }
        Ok(Box::pin(stream::iter(
            self.respond(&request).into_iter().map(Ok),
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rah_protocol::{PermissionLevel, ToolDefinition};

    fn tool(name: &str) -> ToolDefinition {
        ToolDefinition {
            name: ToolName::new(name),
            description: String::new(),
            input_schema: json!({}),
            permission: PermissionLevel::None,
        }
    }

    fn joined(events: &[ModelEvent]) -> String {
        events
            .iter()
            .filter_map(|e| match e {
                ModelEvent::TextDelta { text } => Some(text.as_str()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn requests_echo_then_summarizes_the_result() {
        let model = MockModel::new("A");
        let mut request = ModelRequest {
            items: vec![ContextItem::User {
                text: "hello".into(),
            }],
            tools: vec![tool("echo")],
            ..ModelRequest::default()
        };
        let events = model.respond(&request);
        assert!(matches!(
            &events[0],
            ModelEvent::ToolCall { call_id, name, arguments }
                if call_id == "mock-1" && name.as_str() == "echo" && arguments["text"] == "hello"
        ));
        request.items.push(ContextItem::ToolCall {
            call_id: "mock-1".into(),
            name: ToolName::new("echo"),
            arguments: json!({"text": "hello"}),
        });
        request.items.push(ContextItem::ToolResult {
            call_id: "mock-1".into(),
            output: ToolOutput {
                content: vec![ToolContent::Text("hello".into())],
                is_error: false,
            },
        });
        let events = model.respond(&request);
        assert_eq!(joined(&events), "[A] echo returned: hello");
        assert_eq!(events.last(), Some(&ModelEvent::Completed));
    }

    #[test]
    fn notool_reports_history_size() {
        let model = MockModel::new("B");
        let request = ModelRequest {
            items: vec![
                ContextItem::User { text: "a".into() },
                ContextItem::Assistant { text: "b".into() },
                ContextItem::User {
                    text: "/notool hi".into(),
                },
            ],
            tools: vec![tool("echo")],
            ..ModelRequest::default()
        };
        assert_eq!(
            joined(&model.respond(&request)),
            "[B] you said: hi (2 earlier items)"
        );
    }

    #[test]
    fn call_requests_an_offered_tool_with_json_arguments() {
        let model = MockModel::new("C");
        let respond = |text: &str| {
            model.respond(&ModelRequest {
                items: vec![ContextItem::User { text: text.into() }],
                tools: vec![tool("echo"), tool("shell.exec")],
                ..ModelRequest::default()
            })
        };
        assert!(matches!(
            &respond(r#"/call shell.exec {"program":"git"}"#)[0],
            ModelEvent::ToolCall { name, arguments, .. }
                if name.as_str() == "shell.exec" && arguments["program"] == "git"
        ));
        assert_eq!(
            joined(&respond("/call missing {}")),
            "[C] tool `missing` is not offered"
        );
        assert!(joined(&respond("/call echo {")).starts_with("[C] invalid arguments"));
    }
}
