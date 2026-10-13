//! Provider-neutral protocol types for RAH.

mod agent;
mod events;
mod identifiers;
mod model;
mod session;
mod tools;

pub use agent::{AgentOutput, Message, MessageRole};
pub use events::{AgentErrorCode, AgentEvent, ApprovalRequest};
pub use identifiers::{ApprovalId, ModelRequestId, RequestId, SessionId, ToolCallId};
pub use model::{ContextItem, ModelEvent, ModelRequest};
pub use session::{
    ForkPoint, MAX_TITLE_CHARS, SessionEntry, SessionInfo, SessionRecord, ToolEffect,
    normalize_title,
};
pub use tools::{
    IMAGE_MEDIA_TYPES, ImageContent, PermissionLevel, ToolCall, ToolContent, ToolDefinition,
    ToolInput, ToolName, ToolOutput,
};

/// The initial version of the serialized RAH protocol.
pub const RAH_PROTOCOL_VERSION: u32 = 1;
