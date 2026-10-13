//! Tools and the [`ToolInvoker`].
//!
//! Tool providers register through [`provide_tools`]; the service key is
//! private, so the only way to reach a tool is [`ToolInvoker::invoke`], which
//! always runs the [`ToolBeforeInvoke`] chain first.

use std::sync::Arc;

use async_trait::async_trait;
use rah_kernel::{KernelHandle, Scope, ScopeError, ServiceKey};
use rah_protocol::{
    IMAGE_MEDIA_TYPES, ImageContent, PermissionLevel, SessionId, ToolCall, ToolContent,
    ToolDefinition, ToolEffect, ToolName, ToolOutput,
};
use serde_json::Value;
use tokio_util::sync::CancellationToken;

use crate::hooks::{
    ApprovalRequest, ToolAfterInvoke, ToolApproval, ToolBeforeInvoke, ToolCompletion, ToolDecision,
    ToolRequestContext,
};

/// Service name tool providers declare in their manifest's `provides`.
pub const TOOLS_SERVICE: &str = "tools";

#[async_trait]
pub trait Tool: Send + Sync {
    fn definition(&self) -> ToolDefinition;

    /// Runs the tool. `input` is model-supplied and untrusted; validate it.
    async fn call(&self, input: Value, cx: ToolContext) -> Result<ToolOutput, ToolError>;
}

#[derive(Clone, Debug)]
pub struct ToolContext {
    pub session: SessionId,
    pub cancel: CancellationToken,
}

/// How a tool call failed. `InvalidInput` and `NotStarted` mean nothing was
/// done, so the effect is `none`; `Execution` and `Cancelled` on a mutating
/// tool leave its effect `uncertain`.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ToolError {
    #[error("invalid input: {0}")]
    InvalidInput(String),
    /// The call was refused or could not start (for example a busy or
    /// missing backend); nothing was attempted.
    #[error("not started: {0}")]
    NotStarted(String),
    #[error("execution failed: {0}")]
    Execution(String),
    #[error("cancelled")]
    Cancelled,
}

struct ToolSet(Vec<Arc<dyn Tool>>);

const TOOLS: ServiceKey<ToolSet> = ServiceKey::new(TOOLS_SERVICE);

/// Registers this plugin's tools. The manifest must declare `provides = ["tools"]`.
pub fn provide_tools(scope: &mut Scope, tools: Vec<Arc<dyn Tool>>) -> Result<(), ScopeError> {
    scope.provide(TOOLS, Arc::new(ToolSet(tools)))
}

/// One requested invocation. `call.id` is host-allocated.
#[derive(Clone, Debug)]
pub struct Invocation {
    pub session: SessionId,
    pub call: ToolCall,
    pub cancel: CancellationToken,
}

#[derive(Clone, Debug, PartialEq)]
pub struct InvocationOutcome {
    pub output: ToolOutput,
    pub effect: ToolEffect,
    /// Whether policy allowed the call (it may still have failed).
    pub authorized: bool,
}

/// The single path from a model tool request to a tool.
#[derive(Clone)]
pub struct ToolInvoker {
    kernel: KernelHandle,
}

impl ToolInvoker {
    pub fn new(kernel: KernelHandle) -> Self {
        Self { kernel }
    }

    /// Whether any authorization handler is mounted. Hosts must warn loudly
    /// when this is `false`.
    pub fn is_guarded(&self) -> bool {
        self.kernel.has_handlers::<ToolBeforeInvoke>()
    }

    /// Definitions of all mounted tools; on name clashes the oldest provider
    /// wins.
    pub fn definitions(&self) -> Vec<ToolDefinition> {
        let mut definitions: Vec<ToolDefinition> = Vec::new();
        for tool in self.tools() {
            let definition = tool.definition();
            if !definitions.iter().any(|d| d.name == definition.name) {
                definitions.push(definition);
            }
        }
        definitions
    }

    fn tools(&self) -> Vec<Arc<dyn Tool>> {
        self.kernel
            .get_all(TOOLS)
            .into_iter()
            .flat_map(|(_, set)| set.0.clone())
            .collect()
    }

    fn find(&self, name: &ToolName) -> Option<(Arc<dyn Tool>, ToolDefinition)> {
        self.tools().into_iter().find_map(|tool| {
            let definition = tool.definition();
            (definition.name == *name).then_some((tool, definition))
        })
    }

    async fn decide(&self, request: &ToolRequestContext) -> Result<(), String> {
        let decision = match self.kernel.bail::<ToolBeforeInvoke>(request.clone()).await {
            Ok(Some(decision)) => decision,
            Ok(None) => ToolDecision::Allow,
            Err(error) => return Err(format!("policy failure: {error}")),
        };
        match decision {
            ToolDecision::Allow => Ok(()),
            ToolDecision::Deny { reason } => Err(reason),
            ToolDecision::Ask { reason } => {
                let approval = ApprovalRequest {
                    request: request.clone(),
                    reason,
                };
                match self.kernel.bail::<ToolApproval>(approval).await {
                    Ok(Some(true)) => Ok(()),
                    Ok(Some(false)) => Err("rejected by the user".to_owned()),
                    Ok(None) => Err("approval required but no approver is available".to_owned()),
                    Err(error) => Err(format!("approval failure: {error}")),
                }
            }
        }
    }

    /// Authorizes and runs one call. `on_authorized` runs after the policy
    /// allowed the call and before the tool starts. Never retries.
    pub async fn invoke(
        &self,
        invocation: Invocation,
        on_authorized: impl FnOnce() + Send,
    ) -> InvocationOutcome {
        let Invocation {
            session,
            call,
            cancel,
        } = invocation;
        let Some((tool, definition)) = self.find(&call.name) else {
            return rejected(format!("unknown tool `{}`", call.name));
        };
        let request = ToolRequestContext {
            session: session.clone(),
            call: call.clone(),
            definition: definition.clone(),
        };
        // Cancellation while the policy or an approver decides (an approval
        // can wait for minutes) ends the call; nothing has started yet.
        let decision = tokio::select! {
            biased;
            () = cancel.cancelled() => None,
            decision = self.decide(&request) => Some(decision),
        };
        match decision {
            Some(Err(reason)) => return rejected(format!("denied: {reason}")),
            None => return rejected("cancelled before the tool started".to_owned()),
            Some(Ok(())) if cancel.is_cancelled() => {
                return rejected("cancelled before the tool started".to_owned());
            }
            Some(Ok(())) => {}
        }
        on_authorized();

        let mutating = matches!(
            definition.permission,
            PermissionLevel::Write | PermissionLevel::Execute
        );
        let cx = ToolContext {
            session,
            cancel: cancel.clone(),
        };
        let result = tokio::select! {
            biased;
            () = cancel.cancelled() => Err(ToolError::Cancelled),
            result = tool.call(call.input.0, cx) => result,
        };
        let uncertain = if mutating {
            ToolEffect::Uncertain
        } else {
            ToolEffect::None
        };
        let (output, effect) = match result {
            Ok(output) if mutating => (output, ToolEffect::Completed),
            Ok(output) => (output, ToolEffect::None),
            Err(error @ (ToolError::InvalidInput(_) | ToolError::NotStarted(_))) => {
                (error_output(error.to_string()), ToolEffect::None)
            }
            Err(error) => (error_output(error.to_string()), uncertain),
        };
        let completion = ToolCompletion {
            request,
            output,
            effect,
        };
        let output = match self.kernel.waterfall::<ToolAfterInvoke>(completion).await {
            Ok(completion) => completion.output,
            Err(error) => error_output(format!("output post-processing failed: {error}")),
        };
        InvocationOutcome {
            output,
            effect,
            authorized: true,
        }
    }
}

/// The most base64 characters one image may have (3 MiB of image bytes).
pub const MAX_IMAGE_DATA: usize = 4 * 1024 * 1024;

/// Checks an image from a tool, such as an MCP server or a process plugin,
/// and returns it as content. The media type must be one of
/// [`IMAGE_MEDIA_TYPES`] and the data padded standard base64 of at most
/// [`MAX_IMAGE_DATA`] characters. The bytes themselves are not decoded.
pub fn image(media_type: &str, data: &str) -> Result<ToolContent, String> {
    let media_type = media_type.trim().to_ascii_lowercase();
    if !IMAGE_MEDIA_TYPES.contains(&media_type.as_str()) {
        return Err(format!("unsupported image type `{media_type}`"));
    }
    if data.is_empty() || data.len() > MAX_IMAGE_DATA {
        return Err(format!(
            "the image has {} base64 characters; 1 to {MAX_IMAGE_DATA} are allowed",
            data.len()
        ));
    }
    let body = data.trim_end_matches('=');
    let valid = data.len().is_multiple_of(4)
        && data.len() - body.len() <= 2
        && body
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'+' || byte == b'/');
    if !valid {
        return Err("the image data is not standard base64".into());
    }
    Ok(ToolContent::Image(ImageContent {
        media_type,
        data: data.to_owned(),
    }))
}

/// A short text that stands for an image where the image cannot be shown,
/// such as for a model without image input.
pub fn describe_image(image: &ImageContent) -> String {
    let bytes = image.data.len() / 4 * 3;
    let size = if bytes >= 1024 * 1024 {
        format!("{:.1} MiB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{} KiB", bytes.div_ceil(1024))
    };
    format!("[{} image, {size}]", image.media_type)
}

fn error_output(text: String) -> ToolOutput {
    ToolOutput {
        content: vec![ToolContent::Text(text)],
        is_error: true,
    }
}

fn rejected(text: String) -> InvocationOutcome {
    InvocationOutcome {
        output: error_output(text),
        effect: ToolEffect::None,
        authorized: false,
    }
}
