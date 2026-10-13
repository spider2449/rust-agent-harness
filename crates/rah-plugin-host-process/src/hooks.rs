//! Hooks and events of protocol 3 (ADR 0039). The host sends
//! `hooks/call {hook, input}` for each hook in the manifest and an `event`
//! notification for each subscribed event. A failing handler behaves like
//! a failing static handler: a denial, an error output or a failed model
//! request. Hook calls are never retried.

use std::{fmt::Display, time::Duration};

use rah_api::{
    CancellationToken, PluginId, Scope,
    hooks::{
        ModelBeforeRequest, ToolAfterInvoke, ToolBeforeInvoke, ToolCompletion, ToolDecision,
        ToolRequestContext,
    },
    kernel::{BailHook, WaterfallHook},
    session::SessionAppended,
    tools::image,
};
use rah_protocol::{ContextItem, ModelRequest, ToolContent, ToolOutput};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{Peer, tool_output, wire_output};

/// Hooks a process plugin may attach to.
pub const HOOKABLE: [&str; 3] = [
    ToolBeforeInvoke::NAME,
    ToolAfterInvoke::NAME,
    ModelBeforeRequest::NAME,
];
/// Events a process plugin may subscribe to.
pub const EVENTS: [&str; 1] = [SESSION_APPENDED];
const SESSION_APPENDED: &str = "session.appended";

/// One `hooks` entry of a manifest.
#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(deny_unknown_fields)]
pub struct HookSpec {
    pub hook: String,
    #[serde(default)]
    pub priority: i32,
}

/// Registers the manifest's hooks and event handlers on `scope`.
pub(crate) fn attach(
    scope: &mut Scope,
    peer: &Peer,
    hooks: &[HookSpec],
    events: &[String],
    timeout: Duration,
) {
    let caller = Caller {
        plugin: scope.id().clone(),
        peer: peer.clone(),
        timeout,
        cancel: scope.spawner().cancellation(),
    };
    for spec in hooks {
        let caller = caller.clone();
        match spec.hook.as_str() {
            ToolBeforeInvoke::NAME => {
                scope.hook_bail::<ToolBeforeInvoke, _, _>(spec.priority, move |request| {
                    let caller = caller.clone();
                    async move { caller.before_invoke(request).await }
                });
            }
            ToolAfterInvoke::NAME => {
                scope.hook_waterfall::<ToolAfterInvoke, _, _>(spec.priority, move |completion| {
                    let caller = caller.clone();
                    async move { caller.after_invoke(completion).await }
                });
            }
            ModelBeforeRequest::NAME => {
                scope.hook_waterfall::<ModelBeforeRequest, _, _>(spec.priority, move |request| {
                    let caller = caller.clone();
                    async move { caller.before_request(request).await }
                });
            }
            // `parse_initialize` admits only the hooks above.
            _ => {}
        }
    }
    if events.iter().any(|event| event == SESSION_APPENDED) {
        let peer = peer.clone();
        scope.on::<SessionAppended>(move |event| {
            // Best effort: a closed connection drops the event.
            let _ = peer.notify(
                "event",
                json!({
                    "event": SESSION_APPENDED,
                    "data": { "session": event.session, "seq": event.seq, "entry": event.entry },
                }),
            );
        });
    }
}

#[derive(Clone)]
struct Caller {
    plugin: PluginId,
    peer: Peer,
    timeout: Duration,
    cancel: CancellationToken,
}

impl Caller {
    async fn call(&self, hook: &'static str, input: Value) -> Result<Value, String> {
        self.peer
            .request(
                "hooks/call",
                json!({ "hook": hook, "input": input }),
                self.timeout,
                &self.cancel,
            )
            .await
            .map_err(|error| error.to_string())
    }

    fn failure(&self, hook: &str, error: impl Display) -> String {
        format!(
            "the `{hook}` handler of process plugin `{}` failed: {error}",
            self.plugin
        )
    }

    async fn before_invoke(&self, request: ToolRequestContext) -> Option<ToolDecision> {
        let input = json!({
            "session": request.session,
            "call": request.call,
            "definition": request.definition,
        });
        let answer = self.call(ToolBeforeInvoke::NAME, input).await;
        match answer.and_then(|reply| decision(&reply)) {
            Ok(decision) => decision,
            // Fails closed, and stops the chain like a panicking handler.
            Err(error) => Some(ToolDecision::Deny {
                reason: self.failure(ToolBeforeInvoke::NAME, error),
            }),
        }
    }

    async fn after_invoke(&self, mut completion: ToolCompletion) -> ToolCompletion {
        let request = &completion.request;
        let input = json!({
            "session": request.session,
            "call": request.call,
            "definition": request.definition,
            "output": wire_output(&completion.output),
            "effect": completion.effect,
        });
        let answer = self.call(ToolAfterInvoke::NAME, input).await;
        match answer.and_then(|reply| replaced_output(&reply)) {
            Ok(Some(output)) => completion.output = output,
            Ok(None) => {}
            Err(error) => {
                completion.output = ToolOutput {
                    content: vec![ToolContent::Text(
                        self.failure(ToolAfterInvoke::NAME, error),
                    )],
                    is_error: true,
                }
            }
        }
        completion
    }

    async fn before_request(&self, request: ModelRequest) -> ModelRequest {
        let answer = match serde_json::to_value(&request) {
            Ok(input) => self.call(ModelBeforeRequest::NAME, input).await,
            Err(error) => Err(error.to_string()),
        };
        match answer.and_then(|reply| replaced_request(&reply)) {
            Ok(Some(replaced)) => replaced,
            Ok(None) => request,
            // A waterfall handler can fail only by panicking; the kernel
            // contains the unwind and the loop fails the request, so the
            // hook does not fail open. `resume_unwind` skips the panic hook.
            Err(error) => {
                std::panic::resume_unwind(Box::new(self.failure(ModelBeforeRequest::NAME, error)))
            }
        }
    }
}

fn decision(reply: &Value) -> Result<Option<ToolDecision>, String> {
    #[derive(Deserialize)]
    #[serde(tag = "decision", rename_all = "snake_case")]
    enum Reply {
        Allow,
        Deny { reason: String },
        Ask { reason: String },
    }
    if reply.is_null() {
        return Ok(None);
    }
    let reply: Reply = serde_json::from_value(reply.clone())
        .map_err(|error| format!("malformed decision: {error}"))?;
    Ok(Some(match reply {
        Reply::Allow => ToolDecision::Allow,
        Reply::Deny { reason } => ToolDecision::Deny { reason },
        Reply::Ask { reason } => ToolDecision::Ask { reason },
    }))
}

fn replaced_output(reply: &Value) -> Result<Option<ToolOutput>, String> {
    if reply.is_null() {
        return Ok(None);
    }
    let output = reply
        .get("output")
        .ok_or("the result has neither `null` nor `output`")?;
    tool_output(output)
        .map(Some)
        .map_err(|error| error.to_string())
}

fn replaced_request(reply: &Value) -> Result<Option<ModelRequest>, String> {
    if reply.is_null() {
        return Ok(None);
    }
    let request = reply
        .get("request")
        .ok_or("the result has neither `null` nor `request`")?;
    let request: ModelRequest = serde_json::from_value(request.clone())
        .map_err(|error| format!("malformed model request: {error}"))?;
    for item in &request.items {
        if let ContextItem::ToolResult { output, .. } = item {
            for content in &output.content {
                if let ToolContent::Image(picture) = content {
                    image(&picture.media_type, &picture.data)?;
                }
            }
        }
    }
    Ok(Some(request))
}
