use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{ToolDefinition, ToolName, ToolOutput};

/// One provider-neutral item of model context.
///
/// `call_id` values are model-facing correlation identifiers; they are not the
/// host-allocated [`crate::ToolCallId`].
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContextItem {
    System {
        text: String,
    },
    User {
        text: String,
    },
    Assistant {
        text: String,
    },
    ToolCall {
        call_id: String,
        name: ToolName,
        arguments: Value,
    },
    ToolResult {
        call_id: String,
        output: ToolOutput,
    },
}

/// One model request: context plus the tools the model may request.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct ModelRequest {
    /// Provider-specific model identifier; `None` uses the provider default.
    pub model: Option<String>,
    pub items: Vec<ContextItem>,
    pub tools: Vec<ToolDefinition>,
}

/// One streamed model event. A stream ends with `Completed`.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ModelEvent {
    TextDelta {
        text: String,
    },
    /// Content-free signal that the model is reasoning.
    ReasoningStarted,
    /// A tool request. It is untrusted input, never authorization.
    ToolCall {
        call_id: String,
        name: ToolName,
        arguments: Value,
    },
    Completed,
}
