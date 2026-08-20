use serde::{Deserialize, Serialize};

use crate::RequestId;

/// Input supplied to an agent runtime.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentInput {
    /// Conversation messages available to the agent.
    pub messages: Vec<Message>,
}

/// A request to start an agent operation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentRequest {
    /// Correlates the request with its events and output.
    pub request_id: RequestId,
    /// User and conversation input for the operation.
    pub input: AgentInput,
    /// Runtime-level execution options.
    pub options: AgentOptions,
}

/// Provider-neutral controls for an agent operation.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentOptions {}

/// Final output produced by an agent operation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentOutput {
    /// The final assistant message.
    pub message: Message,
}

/// A text message in an agent conversation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Message {
    /// The participant role associated with the message.
    pub role: MessageRole,
    /// UTF-8 text content of the message.
    pub content: String,
}

/// The role of a participant in an agent conversation.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageRole {
    /// Instructions supplied by the host application.
    System,
    /// Input supplied by a user.
    User,
    /// Output supplied by the agent.
    Assistant,
    /// Output returned from a tool execution.
    Tool,
}
