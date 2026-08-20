use std::{
    collections::HashMap,
    sync::{Arc, Mutex, MutexGuard},
};

use async_trait::async_trait;
use futures::StreamExt;
use rah_model::{GenerationOptions, ModelBackend, ModelEvent, ModelRequest};
use rah_protocol::{
    AgentErrorCode, AgentEvent, AgentOutput, AgentRequest, Message, MessageRole, ModelRequestId,
    PermissionLevel, SessionId, ToolContent, ToolOutput,
};
use rah_tools::{ToolContext, ToolRegistry};
use tokio_util::sync::CancellationToken;

use crate::{AgentError, AgentEventStream, AgentHandle, AgentRuntime};

type ActiveSessions = Arc<Mutex<HashMap<SessionId, CancellationToken>>>;

/// Minimal deterministic runtime for tests and examples.
pub struct MinimalTestRuntime {
    backend: Arc<dyn ModelBackend>,
    tools: Arc<ToolRegistry>,
    active_sessions: ActiveSessions,
}

impl MinimalTestRuntime {
    /// Creates a runtime from RAH-owned model and tool abstractions.
    #[must_use]
    pub fn new(backend: Arc<dyn ModelBackend>, tools: Arc<ToolRegistry>) -> Self {
        Self {
            backend,
            tools,
            active_sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn active_sessions(&self) -> MutexGuard<'_, HashMap<SessionId, CancellationToken>> {
        self.active_sessions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn event_stream(
        &self,
        request: AgentRequest,
        session_id: SessionId,
        cancellation: CancellationToken,
    ) -> AgentEventStream {
        let backend = Arc::clone(&self.backend);
        let tools = Arc::clone(&self.tools);
        let session_guard = SessionGuard {
            session_id: session_id.clone(),
            active_sessions: Arc::clone(&self.active_sessions),
        };

        Box::pin(async_stream::stream! {
            let _session_guard = session_guard;
            let mut messages = request.input.messages;
            yield AgentEvent::Started {
                session_id: session_id.clone(),
                request_id: request.request_id,
            };

            'agent: loop {
                if cancellation.is_cancelled() {
                    yield cancelled(&session_id);
                    return;
                }

                let model_request_id = ModelRequestId::new();
                yield AgentEvent::ModelRequestStarted {
                    session_id: session_id.clone(),
                    model_request_id: model_request_id.clone(),
                };
                let model_request = ModelRequest {
                    id: model_request_id.clone(),
                    messages: messages.clone(),
                    tools: tools.definitions(),
                    options: GenerationOptions::default(),
                };
                let model_result = tokio::select! {
                    biased;
                    () = cancellation.cancelled() => {
                        yield cancelled(&session_id);
                        return;
                    }
                    result = backend.complete(model_request) => result,
                };
                let mut model_stream = match model_result {
                    Ok(stream) => stream,
                    Err(error) => {
                        yield failed(&session_id, AgentErrorCode::Model, error.to_string());
                        return;
                    }
                };
                let mut requested_tool = false;
                let mut final_text = String::new();

                loop {
                    let next_event = tokio::select! {
                        biased;
                        () = cancellation.cancelled() => {
                            yield cancelled(&session_id);
                            return;
                        }
                        event = model_stream.next() => event,
                    };
                    let Some(model_event) = next_event else {
                        break;
                    };
                    let model_event = match model_event {
                        Ok(event) => event,
                        Err(error) => {
                            yield failed(&session_id, AgentErrorCode::Model, error.to_string());
                            return;
                        }
                    };

                    match model_event {
                        ModelEvent::TextDelta { text } => {
                            final_text.push_str(&text);
                            yield AgentEvent::ModelDelta {
                                session_id: session_id.clone(),
                                model_request_id: model_request_id.clone(),
                                delta: text,
                            };
                        }
                        ModelEvent::ToolCall { call } => {
                            requested_tool = true;
                            yield AgentEvent::ToolRequested {
                                session_id: session_id.clone(),
                                tool_call: call.clone(),
                            };
                            let Some(tool) = tools.get(&call.name) else {
                                yield failed(
                                    &session_id,
                                    AgentErrorCode::Tool,
                                    format!("tool `{}` is not registered", call.name),
                                );
                                return;
                            };
                            if tool.definition().permission != PermissionLevel::None {
                                yield failed(
                                    &session_id,
                                    AgentErrorCode::PermissionDenied,
                                    format!(
                                        "minimal test runtime cannot authorize tool `{}`",
                                        call.name
                                    ),
                                );
                                return;
                            }

                            yield AgentEvent::ToolStarted {
                                session_id: session_id.clone(),
                                tool_call_id: call.id.clone(),
                            };
                            let tool_result = tokio::select! {
                                biased;
                                () = cancellation.cancelled() => {
                                    yield cancelled(&session_id);
                                    return;
                                }
                                result = tools.execute(call.clone(), ToolContext::default()) => result,
                            };
                            let output = match tool_result {
                                Ok(output) => output,
                                Err(error) => {
                                    yield failed(
                                        &session_id,
                                        AgentErrorCode::Tool,
                                        error.to_string(),
                                    );
                                    return;
                                }
                            };
                            yield AgentEvent::ToolFinished {
                                session_id: session_id.clone(),
                                tool_call_id: call.id,
                                output: output.clone(),
                            };
                            messages.push(Message {
                                role: MessageRole::Tool,
                                content: tool_output_text(&output),
                            });
                        }
                        ModelEvent::Usage { .. } | ModelEvent::Completed => {}
                    }
                }

                if requested_tool {
                    continue 'agent;
                }
                if cancellation.is_cancelled() {
                    yield cancelled(&session_id);
                    return;
                }

                yield AgentEvent::Completed {
                    session_id: session_id.clone(),
                    output: AgentOutput {
                        message: Message {
                            role: MessageRole::Assistant,
                            content: final_text,
                        },
                    },
                };
                return;
            }
        })
    }
}

#[async_trait]
impl AgentRuntime for MinimalTestRuntime {
    async fn start(&self, request: AgentRequest) -> Result<AgentHandle, AgentError> {
        let session_id = SessionId::new();
        let cancellation = CancellationToken::new();
        self.active_sessions()
            .insert(session_id.clone(), cancellation.clone());
        let events = self.event_stream(request, session_id.clone(), cancellation);
        Ok(AgentHandle::new(session_id, events))
    }

    async fn resume(&self, session_id: SessionId) -> Result<AgentHandle, AgentError> {
        Err(AgentError::SessionNotFound { session_id })
    }

    async fn cancel(&self, session_id: SessionId) -> Result<(), AgentError> {
        let cancellation = self.active_sessions().get(&session_id).cloned();
        let Some(cancellation) = cancellation else {
            return Err(AgentError::SessionNotFound { session_id });
        };
        cancellation.cancel();
        Ok(())
    }
}

struct SessionGuard {
    session_id: SessionId,
    active_sessions: ActiveSessions,
}

impl Drop for SessionGuard {
    fn drop(&mut self) {
        self.active_sessions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&self.session_id);
    }
}

fn cancelled(session_id: &SessionId) -> AgentEvent {
    AgentEvent::Cancelled {
        session_id: session_id.clone(),
    }
}

fn failed(session_id: &SessionId, code: AgentErrorCode, message: String) -> AgentEvent {
    AgentEvent::Failed {
        session_id: session_id.clone(),
        code,
        message,
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
