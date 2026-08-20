use serde::{Deserialize, Serialize};

use crate::{
    AgentOutput, ApprovalId, ModelRequestId, RequestId, SessionId, ToolCall, ToolCallId, ToolOutput,
};

/// A request for host approval before a tool call proceeds.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ApprovalRequest {
    /// Correlates the approval response with this request.
    pub id: ApprovalId,
    /// The tool call awaiting approval.
    pub tool_call: ToolCall,
    /// Human-readable reason that approval is required.
    pub reason: String,
}

/// Stable category for an agent operation failure.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentErrorCode {
    /// The request was structurally or semantically invalid.
    InvalidRequest,
    /// A model backend operation failed.
    Model,
    /// Tool lookup or execution failed.
    Tool,
    /// Required permission was denied.
    PermissionDenied,
    /// Sandbox validation or execution failed.
    Sandbox,
    /// Session loading or persistence failed.
    Session,
    /// The operation failed for an uncategorized internal reason.
    Internal,
}

/// An externally consumable event emitted by an agent runtime.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentEvent {
    /// The runtime accepted the request and created a session.
    Started {
        /// Session associated with the operation.
        session_id: SessionId,
        /// Request that started the operation.
        request_id: RequestId,
    },
    /// A model backend request started.
    ModelRequestStarted {
        /// Session associated with the operation.
        session_id: SessionId,
        /// Model request associated with subsequent model events.
        model_request_id: ModelRequestId,
    },
    /// Incremental model text became available.
    ModelDelta {
        /// Session associated with the operation.
        session_id: SessionId,
        /// Model request that produced the text.
        model_request_id: ModelRequestId,
        /// Incremental UTF-8 text.
        delta: String,
    },
    /// The model requested a tool call.
    ToolRequested {
        /// Session associated with the operation.
        session_id: SessionId,
        /// Parsed, untrusted tool call.
        tool_call: ToolCall,
    },
    /// An approved tool call started execution.
    ToolStarted {
        /// Session associated with the operation.
        session_id: SessionId,
        /// Tool call being executed.
        tool_call_id: ToolCallId,
    },
    /// A tool call finished execution.
    ToolFinished {
        /// Session associated with the operation.
        session_id: SessionId,
        /// Tool call that produced the output.
        tool_call_id: ToolCallId,
        /// Transport-neutral tool output.
        output: ToolOutput,
    },
    /// A tool call requires approval before execution.
    ApprovalRequired {
        /// Session associated with the operation.
        session_id: SessionId,
        /// Approval request presented to the host.
        request: ApprovalRequest,
    },
    /// The operation completed successfully.
    Completed {
        /// Session associated with the operation.
        session_id: SessionId,
        /// Final agent output.
        output: AgentOutput,
    },
    /// The operation terminated with an error.
    Failed {
        /// Session associated with the operation.
        session_id: SessionId,
        /// Stable failure category.
        code: AgentErrorCode,
        /// Human-readable failure detail.
        message: String,
    },
    /// The operation was cancelled.
    Cancelled {
        /// Session associated with the operation.
        session_id: SessionId,
    },
}
