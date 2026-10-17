//! Standard hooks. Policy, approval, post-processing and context plugins
//! attach here instead of patching loops or tools.

use rah_kernel::{BailHook, WaterfallHook};
use rah_protocol::{ModelRequest, SessionId, ToolCall, ToolDefinition, ToolEffect, ToolOutput};

/// What is about to be invoked. The definition comes from the tool provider;
/// the call comes from the model and is untrusted.
#[derive(Clone, Debug, PartialEq)]
pub struct ToolRequestContext {
    pub session: SessionId,
    pub call: ToolCall,
    pub definition: ToolDefinition,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ToolDecision {
    Allow,
    Deny {
        reason: String,
    },
    /// Requires confirmation through [`ToolApproval`]; no approver means deny.
    Ask {
        reason: String,
    },
}

/// Authorization chain run before every tool call. A handler error (panic)
/// is a denial. With no handler at all the call is allowed and the host must
/// warn that it runs unguarded.
pub struct ToolBeforeInvoke;

impl BailHook for ToolBeforeInvoke {
    const NAME: &'static str = "tool.before_invoke";
    type Input = ToolRequestContext;
    type Output = ToolDecision;
}

#[derive(Clone, Debug, PartialEq)]
pub struct ApprovalRequest {
    pub request: ToolRequestContext,
    pub reason: String,
}

/// Asks a frontend to confirm an `Ask` decision. `Some(true)` approves.
pub struct ToolApproval;

impl BailHook for ToolApproval {
    const NAME: &'static str = "tool.approval";
    type Input = ApprovalRequest;
    type Output = bool;
}

#[derive(Clone, Debug, PartialEq)]
pub struct ToolCompletion {
    pub request: ToolRequestContext,
    pub output: ToolOutput,
    pub effect: ToolEffect,
}

/// Post-processing of tool output (truncation, redaction). Only `output`
/// changes are applied; a handler error replaces the output with an error.
pub struct ToolAfterInvoke;

impl WaterfallHook for ToolAfterInvoke {
    const NAME: &'static str = "tool.after_invoke";
    type Value = ToolCompletion;
}

/// Rewrites every model request (system prompt, memory, skills).
pub struct ModelBeforeRequest;

impl WaterfallHook for ModelBeforeRequest {
    const NAME: &'static str = "model.before_request";
    type Value = ModelRequest;
}

/// Repairs the arguments of a tool call before anything judges them. A
/// handler may change only `call.input`; the invoker keeps the call id, the
/// tool name and the definition it found. It runs before
/// [`ToolBeforeInvoke`], so the policy sees the input the tool will receive
/// (ADR 0049). A handler error (panic) leaves the input as it was.
pub struct ToolInputRepair;

impl WaterfallHook for ToolInputRepair {
    const NAME: &'static str = "tool.repair_input";
    type Value = ToolRequestContext;
}
