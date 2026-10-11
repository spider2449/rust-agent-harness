//! Provider-neutral protocol types for RAH.

mod agent;
mod events;
mod identifiers;
pub mod live_evidence;
mod model;
mod runtime_diagnostic;
mod session;
mod tools;

pub use agent::{AgentInput, AgentOptions, AgentOutput, AgentRequest, Message, MessageRole};
pub use events::{AgentErrorCode, AgentEvent, ApprovalRequest};
pub use identifiers::{ApprovalId, ModelRequestId, RequestId, SessionId, ToolCallId};
pub use model::{ContextItem, ModelEvent, ModelRequest};
pub use runtime_diagnostic::{RuntimeDiagnostic, RuntimeFailureKind, RuntimeOperation};
pub use session::{ForkPoint, SessionEntry, SessionInfo, SessionRecord, ToolEffect};
pub use tools::{
    PermissionLevel, ToolCall, ToolContent, ToolDefinition, ToolInput, ToolName, ToolOutput,
};

/// The initial version of the serialized RAH protocol.
pub const RAH_PROTOCOL_VERSION: u32 = 1;
