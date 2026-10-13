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
    /// Token counts the provider reported for this request, sent at most
    /// once, before `Completed` (ADR 0040). Provider metadata: shown and
    /// recorded, never used for decisions.
    Usage {
        usage: ModelUsage,
    },
    Completed,
}

/// Provider-reported token counts of one model request. A provider fills in
/// what it reports; a missing count is `None`, never zero.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct ModelUsage {
    /// Input tokens, including cached ones.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_tokens: Option<u64>,
    /// Output tokens, including reasoning.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_tokens: Option<u64>,
    /// Input tokens served from the provider's prompt cache.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cached_input_tokens: Option<u64>,
}

impl ModelUsage {
    /// Whether no count is known.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.input_tokens.is_none()
            && self.output_tokens.is_none()
            && self.cached_input_tokens.is_none()
    }
}
