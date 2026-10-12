use serde::{Deserialize, Serialize};

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
