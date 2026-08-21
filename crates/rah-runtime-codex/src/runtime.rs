use std::{
    collections::HashMap,
    path::Path,
    sync::{Arc, Mutex, MutexGuard},
};

use async_trait::async_trait;
use futures::Stream;
use rah_protocol::{
    AgentErrorCode, AgentEvent, AgentOutput, AgentRequest, Message, MessageRole, ModelRequestId,
    PermissionLevel, SessionId, ToolName,
};
use rah_runtime::{AgentError, AgentEventStream, AgentHandle, AgentRuntime};
use rah_tools::ToolRegistry;
use serde_json::{Value, json};
use std::{
    pin::Pin,
    sync::atomic::{AtomicBool, Ordering},
    task::{Context, Poll},
};
use tokio::{
    sync::{broadcast, mpsc},
    task::JoinHandle,
};

use crate::{
    CodexAdapterError,
    bridge::{BridgeConfig, BridgeControl, ToolSnapshot, run_bridge, snapshot_tools},
    connection::{AppServerConnection, ConnectionEvent},
    process::ProcessTransport,
    transport::AppServerTransport,
};

#[derive(Clone)]
pub(crate) struct SessionRecord {
    pub(crate) thread_id: String,
    pub(crate) active_turn: Option<String>,
    pub(crate) bridge_tools: HashMap<String, ToolSnapshot>,
    pub(crate) bridge_aliases: HashMap<ToolName, String>,
}

struct BridgeMode {
    config: BridgeConfig,
    controls: mpsc::UnboundedSender<BridgeControl>,
    task: Mutex<Option<JoinHandle<()>>>,
}

struct TurnRoute {
    session_id: SessionId,
    thread_id: String,
    turn_id: String,
}

impl Drop for BridgeMode {
    fn drop(&mut self) {
        if let Some(task) = self
            .task
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
        {
            task.abort();
        }
    }
}

/// Restricted Codex app-server implementation of RAH's runtime contract.
pub struct CodexRuntime {
    connection: Arc<AppServerConnection>,
    sessions: Arc<Mutex<HashMap<SessionId, SessionRecord>>>,
    bridge: Option<BridgeMode>,
}

impl CodexRuntime {
    /// Starts and initializes a compatible Codex app-server executable.
    pub async fn connect(executable: impl AsRef<Path>) -> Result<Self, CodexAdapterError> {
        let transport = ProcessTransport::start(executable.as_ref(), false).await?;
        Self::from_transport(transport).await
    }

    /// Starts Codex with the experimental RAH Tool Bridge explicitly enabled.
    pub async fn connect_tool_bridge(
        executable: impl AsRef<Path>,
        registry: Arc<ToolRegistry>,
        allowed_permissions: Vec<PermissionLevel>,
    ) -> Result<Self, CodexAdapterError> {
        let transport = ProcessTransport::start(executable.as_ref(), true).await?;
        Self::from_transport_bridge(transport, registry, allowed_permissions).await
    }

    pub(crate) async fn from_transport(
        transport: impl AppServerTransport,
    ) -> Result<Self, CodexAdapterError> {
        Self::from_transport_mode(transport, None).await
    }

    pub(crate) async fn from_transport_bridge(
        transport: impl AppServerTransport,
        registry: Arc<ToolRegistry>,
        allowed_permissions: Vec<PermissionLevel>,
    ) -> Result<Self, CodexAdapterError> {
        Self::from_transport_mode(
            transport,
            Some(BridgeConfig {
                registry,
                allowed_permissions: Arc::new(allowed_permissions),
            }),
        )
        .await
    }

    async fn from_transport_mode(
        transport: impl AppServerTransport,
        bridge_config: Option<BridgeConfig>,
    ) -> Result<Self, CodexAdapterError> {
        let bridge_enabled = bridge_config.is_some();
        let connection =
            Arc::new(AppServerConnection::initialize(transport, bridge_enabled).await?);
        let sessions = Arc::new(Mutex::new(HashMap::new()));
        let bridge = if let Some(config) = bridge_config {
            let requests = connection.take_server_requests().ok_or_else(|| {
                CodexAdapterError::ProtocolViolation {
                    message: "dynamic tool responder was already claimed".to_owned(),
                }
            })?;
            let (controls, control_receiver) = mpsc::unbounded_channel();
            let task = tokio::spawn(run_bridge(
                Arc::clone(&connection),
                Arc::clone(&sessions),
                config.clone(),
                requests,
                control_receiver,
            ));
            Some(BridgeMode {
                config,
                controls,
                task: Mutex::new(Some(task)),
            })
        } else {
            None
        };
        Ok(Self {
            connection,
            sessions,
            bridge,
        })
    }

    /// Stops the owned app-server transport and its lifecycle task.
    pub async fn shutdown(&self) -> Result<(), CodexAdapterError> {
        self.connection.shutdown().await?;
        let bridge_task = self.bridge.as_ref().and_then(|bridge| {
            bridge
                .task
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .take()
        });
        if let Some(task) = bridge_task {
            task.await
                .map_err(|error| CodexAdapterError::ProtocolViolation {
                    message: format!("dynamic tool bridge task failed: {error}"),
                })?;
        }
        Ok(())
    }

    fn sessions(&self) -> MutexGuard<'_, HashMap<SessionId, SessionRecord>> {
        self.sessions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

#[async_trait]
impl AgentRuntime for CodexRuntime {
    async fn start(&self, request: AgentRequest) -> Result<AgentHandle, AgentError> {
        if request.input.messages.is_empty() {
            return Err(AgentError::InvalidRequest {
                message: "Codex runtime input must contain at least one message".to_owned(),
            });
        }
        let receiver = self.connection.subscribe();
        let (thread_params, bridge_tools, bridge_aliases) = if let Some(bridge) = &self.bridge {
            let snapshot = snapshot_tools(&bridge.config.registry);
            let params = restricted_thread_params(Some(snapshot.dynamic_tools));
            (params, snapshot.by_alias, snapshot.by_name)
        } else {
            (
                restricted_thread_params(None),
                HashMap::new(),
                HashMap::new(),
            )
        };
        let thread = self
            .connection
            .request("thread/start", thread_params)
            .await
            .map_err(agent_error)?;
        let thread_id = required_string(&thread, &["thread", "id"]).map_err(agent_error)?;
        let turn = self
            .connection
            .request(
                "turn/start",
                json!({
                    "threadId": thread_id,
                    "input": translate_input(&request),
                    "approvalPolicy": "never",
                    "sandboxPolicy": { "type": "readOnly" }
                }),
            )
            .await
            .map_err(agent_error)?;
        let turn_id = required_string(&turn, &["turn", "id"]).map_err(agent_error)?;
        let session_id = SessionId::new();
        self.sessions().insert(
            session_id.clone(),
            SessionRecord {
                thread_id: thread_id.clone(),
                active_turn: Some(turn_id.clone()),
                bridge_tools,
                bridge_aliases,
            },
        );
        let events = event_stream(
            Arc::clone(&self.connection),
            receiver,
            Arc::clone(&self.sessions),
            TurnRoute {
                session_id: session_id.clone(),
                thread_id,
                turn_id,
            },
            request,
            self.bridge.as_ref().map(|bridge| bridge.controls.clone()),
        );
        Ok(AgentHandle::new(session_id, events))
    }

    async fn resume(&self, session_id: SessionId) -> Result<AgentHandle, AgentError> {
        let record = self.sessions().get(&session_id).cloned().ok_or_else(|| {
            AgentError::SessionNotFound {
                session_id: session_id.clone(),
            }
        })?;
        let receiver = self.connection.subscribe();
        self.connection
            .request("thread/resume", json!({ "threadId": record.thread_id }))
            .await
            .map_err(agent_error)?;
        let events = passive_stream(receiver, session_id.clone(), record.thread_id);
        Ok(AgentHandle::new(session_id, events))
    }

    async fn cancel(&self, session_id: SessionId) -> Result<(), AgentError> {
        let record = self.sessions().get(&session_id).cloned().ok_or_else(|| {
            AgentError::SessionNotFound {
                session_id: session_id.clone(),
            }
        })?;
        let turn_id = record.active_turn.ok_or_else(|| AgentError::Runtime {
            message: format!("session `{session_id}` has no active Codex turn"),
        })?;
        if let Some(bridge) = &self.bridge {
            let _ = bridge.controls.send(BridgeControl::Cancel {
                thread_id: record.thread_id.clone(),
                turn_id: turn_id.clone(),
            });
        }
        let mut receiver = self.connection.subscribe();
        self.connection
            .request(
                "turn/interrupt",
                json!({ "threadId": record.thread_id, "turnId": turn_id }),
            )
            .await
            .map_err(agent_error)?;
        loop {
            match receiver.recv().await.map_err(broadcast_error)? {
                ConnectionEvent::Notification { method, params }
                    if method == "turn/completed"
                        && belongs_to(&params, &record.thread_id, &turn_id) =>
                {
                    let status = params.pointer("/turn/status").and_then(Value::as_str);
                    if status == Some("interrupted") {
                        clear_active_turn(&self.sessions, &session_id, &turn_id);
                        return Ok(());
                    }
                    return Err(AgentError::Runtime {
                        message: format!(
                            "Codex turn completed with status `{}` before cancellation",
                            status.unwrap_or("unknown")
                        ),
                    });
                }
                ConnectionEvent::Fault { message } => {
                    return Err(AgentError::Runtime { message });
                }
                _ => {}
            }
        }
    }
}

fn restricted_thread_params(dynamic_tools: Option<Vec<Value>>) -> Value {
    let mut params = json!({
        "approvalPolicy": "never",
        "sandbox": "read-only",
        "serviceName": "rah-runtime-codex",
        "config": {
            "features": { "shell_tool": false, "unified_exec": false },
            "tools": { "web_search": false, "view_image": false },
            "apps": { "_default": { "enabled": false } },
            "mcp_servers": {}
        }
    });
    if let Some(dynamic_tools) = dynamic_tools {
        params["dynamicTools"] = Value::Array(dynamic_tools);
    }
    params
}

fn translate_input(request: &AgentRequest) -> Vec<Value> {
    request
        .input
        .messages
        .iter()
        .map(|message| {
            let role = match message.role {
                MessageRole::System => "system",
                MessageRole::User => "user",
                MessageRole::Assistant => "assistant",
                MessageRole::Tool => "tool",
            };
            json!({ "type": "text", "text": format!("{role}: {}", message.content) })
        })
        .collect()
}

fn event_stream(
    connection: Arc<AppServerConnection>,
    mut receiver: broadcast::Receiver<ConnectionEvent>,
    sessions: Arc<Mutex<HashMap<SessionId, SessionRecord>>>,
    route: TurnRoute,
    request: AgentRequest,
    bridge_controls: Option<mpsc::UnboundedSender<BridgeControl>>,
) -> AgentEventStream {
    let TurnRoute {
        session_id,
        thread_id,
        turn_id,
    } = route;
    let terminal = Arc::new(AtomicBool::new(false));
    let stream_terminal = Arc::clone(&terminal);
    let stream_session_id = session_id.clone();
    let guard_sessions = Arc::clone(&sessions);
    let guard_thread_id = thread_id.clone();
    let stream_turn_id = turn_id.clone();
    let stream_bridge_controls = bridge_controls.clone();
    let bridge_enabled = bridge_controls.is_some();
    let inner: AgentEventStream = Box::pin(async_stream::stream! {
        let model_request_id = ModelRequestId::new();
        let mut final_text = String::new();
        yield AgentEvent::Started {
            session_id: session_id.clone(),
            request_id: request.request_id,
        };
        yield AgentEvent::ModelRequestStarted {
            session_id: session_id.clone(),
            model_request_id: model_request_id.clone(),
        };
        loop {
            match receiver.recv().await {
                Ok(ConnectionEvent::Notification { method, params })
                    if method == "item/agentMessage/delta"
                        && belongs_to(&params, &thread_id, &turn_id) =>
                {
                    let Some(delta) = params.get("delta").and_then(Value::as_str) else {
                        yield failed(&session_id, "agent-message delta is missing `delta`");
                        break;
                    };
                    final_text.push_str(delta);
                    yield AgentEvent::ModelDelta {
                        session_id: session_id.clone(),
                        model_request_id: model_request_id.clone(),
                        delta: delta.to_owned(),
                    };
                }
                Ok(ConnectionEvent::Notification { method, params })
                    if method == "turn/completed" && belongs_to(&params, &thread_id, &turn_id) =>
                {
                    if let Some(controls) = &stream_bridge_controls {
                        let _ = controls.send(BridgeControl::Terminal {
                            thread_id: thread_id.clone(),
                            turn_id: turn_id.clone(),
                        });
                    }
                    clear_active_turn(&sessions, &session_id, &turn_id);
                    stream_terminal.store(true, Ordering::SeqCst);
                    match params.pointer("/turn/status").and_then(Value::as_str) {
                        Some("completed") => yield AgentEvent::Completed {
                            session_id: session_id.clone(),
                            output: AgentOutput {
                                message: Message {
                                    role: MessageRole::Assistant,
                                    content: final_text,
                                },
                            },
                        },
                        Some("interrupted") => yield AgentEvent::Cancelled {
                            session_id: session_id.clone(),
                        },
                        Some("failed") => {
                            let message = params.pointer("/turn/error/message")
                                .and_then(Value::as_str)
                                .unwrap_or("Codex turn failed");
                            yield failed(&session_id, message);
                        }
                        status => yield failed(
                            &session_id,
                            &format!("unknown terminal Codex turn status: {status:?}"),
                        ),
                    }
                    break;
                }
                Ok(ConnectionEvent::Notification { method, params }) => {
                    if unsupported_tool_item(&method, &params, bridge_enabled) {
                        yield failed(
                            &session_id,
                            "Codex-owned tool activity is unsupported by the restricted runtime",
                        );
                        break;
                    }
                    tracing::debug!(target: "rah", codex_method = %method, "ignored additive Codex notification");
                }
                Ok(ConnectionEvent::RahEvent { thread_id: event_thread, turn_id: event_turn, event })
                    if event_thread == thread_id && event_turn == turn_id =>
                {
                    let terminal = matches!(event, AgentEvent::Failed { .. });
                    yield event;
                    if terminal {
                        break;
                    }
                }
                Ok(ConnectionEvent::RahEvent { .. }) => {}
                Ok(ConnectionEvent::UnsupportedRequest { method }) => {
                    yield AgentEvent::Failed {
                        session_id: session_id.clone(),
                        code: AgentErrorCode::PermissionDenied,
                        message: format!("unsupported Codex server request `{method}` was denied"),
                    };
                    break;
                }
                Ok(ConnectionEvent::Fault { message }) => {
                    yield failed(&session_id, &message);
                    break;
                }
                Err(error) => {
                    yield failed(&session_id, &format!("Codex event stream failed: {error}"));
                    break;
                }
            }
        }
    });
    Box::pin(OwnedTurnStream {
        inner,
        _guard: TurnGuard {
            connection,
            sessions: guard_sessions,
            session_id: stream_session_id,
            thread_id: guard_thread_id,
            turn_id: stream_turn_id,
            terminal,
            bridge_controls,
        },
    })
}

struct OwnedTurnStream {
    inner: AgentEventStream,
    _guard: TurnGuard,
}

impl Stream for OwnedTurnStream {
    type Item = AgentEvent;

    fn poll_next(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.inner.as_mut().poll_next(context)
    }
}

struct TurnGuard {
    connection: Arc<AppServerConnection>,
    sessions: Arc<Mutex<HashMap<SessionId, SessionRecord>>>,
    session_id: SessionId,
    thread_id: String,
    turn_id: String,
    terminal: Arc<AtomicBool>,
    bridge_controls: Option<mpsc::UnboundedSender<BridgeControl>>,
}

impl Drop for TurnGuard {
    fn drop(&mut self) {
        if !self.terminal.load(Ordering::SeqCst) {
            if let Some(controls) = &self.bridge_controls {
                let _ = controls.send(BridgeControl::Cancel {
                    thread_id: self.thread_id.clone(),
                    turn_id: self.turn_id.clone(),
                });
            }
            self.connection
                .interrupt_now(self.thread_id.clone(), self.turn_id.clone());
            clear_active_turn(&self.sessions, &self.session_id, &self.turn_id);
        }
    }
}

fn passive_stream(
    mut receiver: broadcast::Receiver<ConnectionEvent>,
    session_id: SessionId,
    thread_id: String,
) -> AgentEventStream {
    Box::pin(async_stream::stream! {
        loop {
            match receiver.recv().await {
                Ok(ConnectionEvent::Notification { method, params }) => {
                    if params.get("threadId").and_then(Value::as_str) != Some(&thread_id) {
                        continue;
                    }
                    if unsupported_tool_item(&method, &params, false) {
                        yield failed(&session_id, "Codex-owned tool activity is unsupported by the restricted runtime");
                        break;
                    }
                    tracing::debug!(target: "rah", codex_method = %method, "ignored resumed-thread notification");
                }
                Ok(ConnectionEvent::RahEvent { .. }) => {}
                Ok(ConnectionEvent::UnsupportedRequest { method }) => {
                    yield AgentEvent::Failed {
                        session_id: session_id.clone(),
                        code: AgentErrorCode::PermissionDenied,
                        message: format!("unsupported Codex server request `{method}` was denied"),
                    };
                    break;
                }
                Ok(ConnectionEvent::Fault { message }) => {
                    yield failed(&session_id, &message);
                    break;
                }
                Err(error) => {
                    yield failed(&session_id, &format!("Codex event stream failed: {error}"));
                    break;
                }
            }
        }
    })
}

fn required_string(value: &Value, path: &[&str]) -> Result<String, CodexAdapterError> {
    let mut current = value;
    for segment in path {
        current = current
            .get(segment)
            .ok_or_else(|| CodexAdapterError::ProtocolViolation {
                message: format!("response is missing `{}`", path.join(".")),
            })?;
    }
    current
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| CodexAdapterError::ProtocolViolation {
            message: format!("response field `{}` is not a string", path.join(".")),
        })
}

fn belongs_to(params: &Value, thread_id: &str, turn_id: &str) -> bool {
    params.get("threadId").and_then(Value::as_str) == Some(thread_id)
        && (params.get("turnId").and_then(Value::as_str) == Some(turn_id)
            || params.pointer("/turn/id").and_then(Value::as_str) == Some(turn_id))
}

fn unsupported_tool_item(method: &str, params: &Value, allow_dynamic: bool) -> bool {
    if method != "item/started" && method != "item/completed" {
        return false;
    }
    match params.pointer("/item/type").and_then(Value::as_str) {
        Some("dynamicToolCall") => !allow_dynamic,
        Some("commandExecution" | "fileChange" | "mcpToolCall") => true,
        _ => false,
    }
}

fn clear_active_turn(
    sessions: &Mutex<HashMap<SessionId, SessionRecord>>,
    session_id: &SessionId,
    turn_id: &str,
) {
    let mut sessions = sessions
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(record) = sessions.get_mut(session_id)
        && record.active_turn.as_deref() == Some(turn_id)
    {
        record.active_turn = None;
    }
}

fn failed(session_id: &SessionId, message: &str) -> AgentEvent {
    AgentEvent::Failed {
        session_id: session_id.clone(),
        code: AgentErrorCode::Internal,
        message: message.to_owned(),
    }
}

fn agent_error(error: CodexAdapterError) -> AgentError {
    AgentError::Runtime {
        message: error.to_string(),
    }
}

fn broadcast_error(error: broadcast::error::RecvError) -> AgentError {
    AgentError::Runtime {
        message: format!("Codex event stream failed: {error}"),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use futures::StreamExt;
    use rah_protocol::{
        AgentEvent, AgentInput, AgentOptions, AgentRequest, Message, MessageRole, RequestId,
    };
    use rah_runtime::AgentRuntime;
    use serde_json::json;

    use crate::test_support::fake_transport;

    use super::CodexRuntime;

    #[tokio::test]
    async fn start_streams_restricted_agent_lifecycle() {
        let (transport, mut peer) = fake_transport();
        let connecting = tokio::spawn(CodexRuntime::from_transport(transport));
        peer.respond("initialize", json!({})).await;
        peer.expect_notification("initialized").await;
        let runtime = Arc::new(connecting.await.expect("connection task").expect("runtime"));

        let starting = {
            let runtime = Arc::clone(&runtime);
            tokio::spawn(async move {
                runtime
                    .start(AgentRequest {
                        request_id: RequestId::new(),
                        input: AgentInput {
                            messages: vec![Message {
                                role: MessageRole::User,
                                content: "hello".to_owned(),
                            }],
                        },
                        options: AgentOptions::default(),
                    })
                    .await
            })
        };
        let thread_request = peer
            .respond(
                "thread/start",
                json!({ "thread": { "id": "codex-thread" } }),
            )
            .await;
        assert_eq!(thread_request["params"]["approvalPolicy"], "never");
        assert_eq!(
            thread_request["params"]["config"]["features"]["shell_tool"],
            false
        );
        let turn_request = peer
            .respond("turn/start", json!({ "turn": { "id": "codex-turn" } }))
            .await;
        assert_eq!(turn_request["params"]["threadId"], "codex-thread");
        assert_eq!(turn_request["params"]["input"][0]["text"], "user: hello");
        let handle = starting.await.expect("start task").expect("agent handle");
        let session_id = handle.session_id().clone();

        peer.notify(
            "item/agentMessage/delta",
            json!({
                "threadId": "codex-thread",
                "turnId": "codex-turn",
                "itemId": "message",
                "delta": "hello back"
            }),
        );
        peer.notify(
            "turn/completed",
            json!({
                "threadId": "codex-thread",
                "turn": { "id": "codex-turn", "status": "completed", "items": [] }
            }),
        );
        let events = handle.into_events().collect::<Vec<_>>().await;
        assert!(matches!(events[0], AgentEvent::Started { .. }));
        assert!(matches!(events[1], AgentEvent::ModelRequestStarted { .. }));
        assert!(
            matches!(events[2], AgentEvent::ModelDelta { ref delta, .. } if delta == "hello back")
        );
        assert!(matches!(
            events[3],
            AgentEvent::Completed { session_id: ref completed_id, .. } if completed_id == &session_id
        ));
        assert!(!events.iter().any(|event| matches!(
            event,
            AgentEvent::ToolRequested { .. }
                | AgentEvent::ToolStarted { .. }
                | AgentEvent::ToolFinished { .. }
        )));

        runtime.shutdown().await.expect("shutdown");
        assert!(peer.stopped.load(std::sync::atomic::Ordering::SeqCst));
    }
}
