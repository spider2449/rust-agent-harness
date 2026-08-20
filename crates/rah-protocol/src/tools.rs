use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::ToolCallId;

/// Provider-neutral identity of a tool.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub struct ToolName(String);

impl ToolName {
    /// Creates a tool name without imposing transport-specific validation.
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    /// Returns the tool name as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ToolName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// Serializable description of a tool exposed to a model backend.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ToolDefinition {
    /// Stable tool identity used for dispatch.
    pub name: ToolName,
    /// Human-readable description of the tool's behavior.
    pub description: String,
    /// Provider-neutral JSON Schema describing accepted input.
    pub input_schema: Value,
    /// Permission required before execution.
    pub permission: PermissionLevel,
}

/// A parsed request to execute a registered tool.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ToolCall {
    /// Correlates the request with its execution and output.
    pub id: ToolCallId,
    /// Identifies the requested registered tool.
    pub name: ToolName,
    /// Contains the untrusted model-supplied arguments.
    pub input: ToolInput,
}

/// Untrusted JSON arguments supplied to a tool call.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct ToolInput(pub Value);

/// Output returned through the tool boundary.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ToolOutput {
    /// Ordered content returned by the tool.
    pub content: Vec<ToolContent>,
    /// Indicates that the tool completed with an error result.
    pub is_error: bool,
}

/// A transport-neutral unit of tool output.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum ToolContent {
    /// UTF-8 text content.
    Text(String),
    /// Structured JSON content.
    Json(Value),
}

/// Permission category required by a tool.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionLevel {
    /// No external capability is required.
    None,
    /// Read access to configured resources is required.
    Read,
    /// Write access to configured resources is required.
    Write,
    /// Subprocess execution is required.
    Execute,
}
