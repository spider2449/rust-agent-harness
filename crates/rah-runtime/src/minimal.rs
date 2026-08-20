use std::sync::Arc;

use async_trait::async_trait;
use futures::{StreamExt, stream};
use rah_model::{GenerationOptions, ModelBackend, ModelEvent, ModelRequest};
use rah_protocol::{
    AgentEvent, AgentOutput, AgentRequest, Message, MessageRole, ModelRequestId, PermissionLevel,
    SessionId, ToolContent, ToolOutput,
};
use rah_tools::{ToolContext, ToolRegistry};

use crate::{AgentError, AgentHandle, AgentRuntime};

/// Minimal deterministic runtime for tests and examples.
pub struct MinimalTestRuntime {
    backend: Arc<dyn ModelBackend>,
    tools: Arc<ToolRegistry>,
}

impl MinimalTestRuntime {
    /// Creates a runtime from RAH-owned model and tool abstractions.
    #[must_use]
    pub fn new(backend: Arc<dyn ModelBackend>, tools: Arc<ToolRegistry>) -> Self {
        Self { backend, tools }
    }

    async fn run(
        &self,
        request: AgentRequest,
        session_id: SessionId,
    ) -> Result<Vec<AgentEvent>, AgentError> {
        let mut events = vec![AgentEvent::Started {
            session_id: session_id.clone(),
            request_id: request.request_id,
        }];
        let mut messages = request.input.messages;

        loop {
            let model_request_id = ModelRequestId::new();
            events.push(AgentEvent::ModelRequestStarted {
                session_id: session_id.clone(),
                model_request_id: model_request_id.clone(),
            });
            let model_request = ModelRequest {
                id: model_request_id.clone(),
                messages: messages.clone(),
                tools: self.tools.definitions(),
                options: GenerationOptions::default(),
            };
            let mut model_stream = self
                .backend
                .complete(model_request)
                .await
                .map_err(model_error)?;
            let mut requested_tool = false;
            let mut final_text = String::new();

            while let Some(model_event) = model_stream.next().await {
                match model_event.map_err(model_error)? {
                    ModelEvent::TextDelta { text } => {
                        final_text.push_str(&text);
                        events.push(AgentEvent::ModelDelta {
                            session_id: session_id.clone(),
                            model_request_id: model_request_id.clone(),
                            delta: text,
                        });
                    }
                    ModelEvent::ToolCall { call } => {
                        requested_tool = true;
                        events.push(AgentEvent::ToolRequested {
                            session_id: session_id.clone(),
                            tool_call: call.clone(),
                        });
                        let definition = self
                            .tools
                            .get(&call.name)
                            .ok_or_else(|| AgentError::Runtime {
                                message: format!("tool `{}` is not registered", call.name),
                            })?
                            .definition();
                        if definition.permission != PermissionLevel::None {
                            return Err(AgentError::Runtime {
                                message: format!(
                                    "minimal test runtime cannot authorize tool `{}`",
                                    call.name
                                ),
                            });
                        }

                        events.push(AgentEvent::ToolStarted {
                            session_id: session_id.clone(),
                            tool_call_id: call.id.clone(),
                        });
                        let output = self
                            .tools
                            .execute(call.clone(), ToolContext::default())
                            .await
                            .map_err(|error| AgentError::Runtime {
                                message: error.to_string(),
                            })?;
                        events.push(AgentEvent::ToolFinished {
                            session_id: session_id.clone(),
                            tool_call_id: call.id,
                            output: output.clone(),
                        });
                        messages.push(Message {
                            role: MessageRole::Tool,
                            content: tool_output_text(&output),
                        });
                    }
                    ModelEvent::Usage { .. } | ModelEvent::Completed => {}
                }
            }

            if requested_tool {
                continue;
            }

            let message = Message {
                role: MessageRole::Assistant,
                content: final_text,
            };
            events.push(AgentEvent::Completed {
                session_id,
                output: AgentOutput { message },
            });
            return Ok(events);
        }
    }
}

#[async_trait]
impl AgentRuntime for MinimalTestRuntime {
    async fn start(&self, request: AgentRequest) -> Result<AgentHandle, AgentError> {
        let session_id = SessionId::new();
        let events = self.run(request, session_id.clone()).await?;
        Ok(AgentHandle::new(session_id, Box::pin(stream::iter(events))))
    }

    async fn resume(&self, session_id: SessionId) -> Result<AgentHandle, AgentError> {
        Err(AgentError::SessionNotFound { session_id })
    }

    async fn cancel(&self, session_id: SessionId) -> Result<(), AgentError> {
        Err(AgentError::SessionNotFound { session_id })
    }
}

fn model_error(error: rah_model::ModelError) -> AgentError {
    AgentError::Runtime {
        message: error.to_string(),
    }
}

fn tool_output_text(output: &ToolOutput) -> String {
    output
        .content
        .iter()
        .map(|content| match content {
            ToolContent::Text(text) => text.clone(),
            ToolContent::Json(value) => value.to_string(),
        })
        .collect::<Vec<_>>()
        .join("\n")
}
