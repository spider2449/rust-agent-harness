//! Experimental neutral Codex composition, using adapter-owned artifact and workspace configuration.
use crate::{
    CodexAdapterError, CodexModelConfig, CodexModelProvider, CodexModelSelection,
    bridge::{output_response, snapshot_definitions},
    connection::{AppServerConnection, ConnectionEvent, ServerRequest},
    process::ProcessTransport,
    runtime::{self, SessionRecord, TurnRoute},
    transport::AppServerTransport,
};
use async_trait::async_trait;
use futures::StreamExt;
use rah_protocol::{AgentEvent, RuntimeOperation, SessionId, ToolInput};
use rah_runtime::{RuntimeFailure, experimental::*};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
use tokio::{
    sync::{broadcast, mpsc},
    task::{JoinHandle, JoinSet},
};

/// Adapter-owned immutable configuration. Executable/provider details never enter
/// rah-runtime's factory contract.
pub struct CodexFactory {
    executable: PathBuf,
    provider: CodexModelProvider,
    workspace: Option<PathBuf>,
    #[cfg(test)]
    fixture: Mutex<Option<crate::test_support::FakeTransport>>,
}
impl CodexFactory {
    pub fn new(executable: PathBuf, provider: CodexModelProvider) -> Self {
        Self {
            executable,
            provider,
            workspace: None,
            #[cfg(test)]
            fixture: Mutex::new(None),
        }
    }
    /// Host-selected workspace, never derived from a model request.
    pub fn with_workspace(
        mut self,
        workspace: impl AsRef<std::path::Path>,
    ) -> Result<Self, RuntimeFailure> {
        self.workspace = Some(workspace.as_ref().canonicalize().map_err(|source| {
            CodexAdapterError::WorkspaceContext { source }
                .into_runtime_failure(RuntimeOperation::Connection)
        })?);
        Ok(self)
    }
}
fn failure(message: &str, operation: RuntimeOperation) -> RuntimeFailure {
    CodexAdapterError::ProtocolViolation {
        message: message.into(),
    }
    .into_runtime_failure(operation)
}
fn model_config(
    selection: ModelSelection,
    provider: &CodexModelProvider,
) -> Result<CodexModelConfig, RuntimeFailure> {
    match selection {
        ModelSelection::RuntimeDefault => Ok(CodexModelConfig::Inherit),
        ModelSelection::Explicit(model) => CodexModelSelection::new(model, provider.clone())
            .map(CodexModelConfig::Explicit)
            .map_err(|e| e.into_runtime_failure(RuntimeOperation::SessionStart)),
    }
}
#[async_trait]
impl ConfiguredRuntimeFactory for CodexFactory {
    fn validate(&self) -> Result<(), RuntimeFailure> {
        if !self.executable.is_absolute() {
            return Err(failure(
                "executable must be absolute",
                RuntimeOperation::Connection,
            ));
        }
        Ok(())
    }
    async fn create(&self) -> Result<Arc<dyn RuntimeInstance>, RuntimeFailure> {
        self.validate()?;
        #[cfg(test)]
        {
            let fixture = self
                .fixture
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .take();
            if let Some(transport) = fixture {
                return Ok(Instance::from_transport(
                    transport,
                    self.provider.clone(),
                    self.workspace.clone(),
                )
                .await?);
            }
        }
        let transport = ProcessTransport::start(&self.executable, true)
            .await
            .map_err(|e| e.into_runtime_failure(RuntimeOperation::Connection))?;
        Ok(
            Instance::from_transport(transport, self.provider.clone(), self.workspace.clone())
                .await?,
        )
    }
}
struct Route {
    session: SessionId,
    thread: String,
    turn: String,
    tools: Arc<dyn HostToolPort>,
    aliases: HashMap<String, crate::bridge::ToolSnapshot>,
    accepting: AtomicBool,
    lease: Mutex<Option<Box<dyn HostTurnLifetime>>>,
}
struct Shared {
    connection: Arc<AppServerConnection>,
    alive: AtomicBool,
    routes: Mutex<HashMap<SessionId, Arc<Route>>>,
    sessions: Arc<Mutex<HashMap<SessionId, SessionRecord>>>,
    stop: mpsc::UnboundedSender<()>,
    router: Mutex<Option<JoinHandle<()>>>,
}
impl Drop for Shared {
    fn drop(&mut self) {
        let _ = self.stop.send(());
        // The router owns its worker JoinSet, whose Drop aborts workers.
        if let Some(task) = self
            .router
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
        {
            task.abort();
        }
    }
}
struct Instance {
    shared: Arc<Shared>,
    provider: CodexModelProvider,
    workspace: Option<PathBuf>,
}
impl Instance {
    async fn from_transport(
        transport: impl AppServerTransport,
        provider: CodexModelProvider,
        workspace: Option<PathBuf>,
    ) -> Result<Arc<Self>, RuntimeFailure> {
        let connection = Arc::new(
            AppServerConnection::initialize(transport, true)
                .await
                .map_err(|e| e.into_runtime_failure(RuntimeOperation::Connection))?,
        );
        let requests = connection
            .take_server_requests()
            .ok_or_else(|| failure("responder already claimed", RuntimeOperation::Connection))?;
        let (stop, stopping) = mpsc::unbounded_channel();
        let shared = Arc::new(Shared {
            connection,
            alive: AtomicBool::new(true),
            routes: Mutex::new(HashMap::new()),
            sessions: Arc::new(Mutex::new(HashMap::new())),
            stop,
            router: Mutex::new(None),
        });
        let task = tokio::spawn(router(Arc::downgrade(&shared), requests, stopping));
        *shared
            .router
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(task);
        Ok(Arc::new(Self {
            shared,
            provider,
            workspace,
        }))
    }
}
#[async_trait]
impl RuntimeInstance for Instance {
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            discovery: self.provider == CodexModelProvider::OpenAi,
            native_continuation: true,
            text_replay: true,
            tool_calls: true,
            cancellation: true,
            streaming: true,
        }
    }
    fn is_alive(&self) -> bool {
        self.shared.alive.load(Ordering::SeqCst)
    }
    async fn discover_models(&self) -> Result<ModelDiscovery, RuntimeFailure> {
        if !self.is_alive() {
            return Err(failure("runtime stopped", RuntimeOperation::ModelDiscovery));
        }
        if self.provider != CodexModelProvider::OpenAi {
            return Ok(ModelDiscovery::Unsupported);
        }
        let catalog = crate::catalog::discover(&self.shared.connection)
            .await
            .map_err(|e| e.into_runtime_failure(RuntimeOperation::ModelDiscovery))?;
        Ok(ModelDiscovery::Catalog {
            complete: true,
            models: catalog
                .models
                .into_iter()
                .map(|id| ModelDescriptor {
                    id,
                    display_label: None,
                })
                .collect(),
        })
    }
    async fn open(
        &self,
        seed: ConversationSeed,
    ) -> Result<Arc<dyn RuntimeConversation>, RuntimeFailure> {
        if !self.is_alive() {
            return Err(failure("runtime stopped", RuntimeOperation::SessionStart));
        }
        Ok(Arc::new(Conversation {
            shared: self.shared.clone(),
            id: seed.id,
            model: model_config(seed.model, &self.provider)?,
            workspace: self.workspace.clone(),
            tools: seed.tools.clone(),
            snapshot: snapshot_definitions(seed.tools.definitions().0),
            state: tokio::sync::Mutex::new(ConversationState {
                closed: false,
                thread: None,
                active: None,
            }),
        }))
    }
    async fn shutdown(&self) -> Result<(), RuntimeFailure> {
        self.shared.alive.store(false, Ordering::SeqCst);
        for route in self
            .shared
            .routes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .values()
        {
            route.accepting.store(false, Ordering::SeqCst);
            route
                .lease
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .take();
        }
        let _ = self.shared.stop.send(());
        let result = self
            .shared
            .connection
            .shutdown()
            .await
            .map_err(|e| e.into_runtime_failure(RuntimeOperation::Shutdown));
        let task = self
            .shared
            .router
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        if let Some(task) = task {
            task.await
                .map_err(|_| failure("router failed", RuntimeOperation::Shutdown))?;
        }
        self.shared
            .routes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clear();
        self.shared
            .sessions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clear();
        result
    }
}
struct ConversationState {
    closed: bool,
    thread: Option<String>,
    active: Option<Arc<Route>>,
}
struct Conversation {
    shared: Arc<Shared>,
    id: ConversationId,
    model: CodexModelConfig,
    workspace: Option<PathBuf>,
    tools: Arc<dyn HostToolPort>,
    snapshot: crate::bridge::ThreadToolSnapshot,
    state: tokio::sync::Mutex<ConversationState>,
}
struct TurnGuard {
    shared: Arc<Shared>,
    route: Arc<Route>,
}
impl Drop for TurnGuard {
    fn drop(&mut self) {
        if self.route.accepting.swap(false, Ordering::SeqCst) {
            self.shared
                .connection
                .interrupt_now(self.route.thread.clone(), self.route.turn.clone());
        }
        self.shared
            .routes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&self.route.session);
        self.shared
            .sessions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&self.route.session);
        self.route
            .lease
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
    }
}
#[async_trait]
impl RuntimeConversation for Conversation {
    fn id(&self) -> &ConversationId {
        &self.id
    }
    async fn send(&self, input: TurnInput) -> Result<TurnHandle, RuntimeFailure> {
        let mut state = self.state.lock().await;
        if state.closed
            || !self.shared.alive.load(Ordering::SeqCst)
            || state
                .active
                .as_ref()
                .is_some_and(|r| r.accepting.load(Ordering::SeqCst))
        {
            return Err(failure(
                "conversation unavailable or busy",
                RuntimeOperation::Turn,
            ));
        }
        let (request, native) = match input {
            TurnInput::TextReplay(r) => (r, false),
            TurnInput::NativeContinuation(r) => (r, true),
        };
        if request.input.messages.is_empty() {
            return Err(failure("empty input", RuntimeOperation::Turn));
        }
        let lease = self.tools.admit_turn().await?;
        let snapshot = self.snapshot.clone();
        let receiver = self.shared.connection.subscribe();
        let thread = if native {
            let thread = state.thread.clone().ok_or_else(|| {
                failure(
                    "native continuation requires a previous thread",
                    RuntimeOperation::SessionResume,
                )
            })?;
            let response = self
                .shared
                .connection
                .request("thread/resume", json!({"threadId":thread}))
                .await
                .map_err(|e| e.into_runtime_failure(RuntimeOperation::SessionResume))?;
            if runtime::required_string(&response, &["thread", "id"])
                .map_err(|e| e.into_runtime_failure(RuntimeOperation::SessionResume))?
                != thread
            {
                return Err(failure(
                    "resumed thread mismatch",
                    RuntimeOperation::SessionResume,
                ));
            }
            thread
        } else {
            let response = self
                .shared
                .connection
                .request(
                    "thread/start",
                    runtime::restricted_thread_params(
                        Some(snapshot.dynamic_tools),
                        &self.model,
                        self.workspace.as_deref(),
                    ),
                )
                .await
                .map_err(|e| e.into_runtime_failure(RuntimeOperation::SessionStart))?;
            runtime::verify_effective_workspace_context(&response, self.workspace.as_deref())
                .map_err(|e| e.into_runtime_failure(RuntimeOperation::SessionStart))?;
            runtime::verify_effective_model_config(&response, &self.model)
                .map_err(|e| e.into_runtime_failure(RuntimeOperation::SessionStart))?;
            runtime::required_string(&response, &["thread", "id"])
                .map_err(|e| e.into_runtime_failure(RuntimeOperation::SessionStart))?
        };
        let response = self.shared.connection.request("turn/start", json!({"threadId":thread,
            "input":runtime::translate_input(&request), "approvalPolicy":"never", "sandboxPolicy":{"type":"readOnly"}})).await
            .map_err(|e| e.into_runtime_failure(RuntimeOperation::Turn))?;
        let turn = runtime::required_string(&response, &["turn", "id"])
            .map_err(|e| e.into_runtime_failure(RuntimeOperation::Turn))?;
        let route = Arc::new(Route {
            session: lease.session_id.clone(),
            thread: thread.clone(),
            turn: turn.clone(),
            tools: self.tools.clone(),
            aliases: snapshot.by_alias.clone(),
            accepting: AtomicBool::new(true),
            lease: Mutex::new(Some(lease.lifetime)),
        });
        state.thread = Some(thread.clone());
        state.active = Some(route.clone());
        self.shared
            .routes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(lease.session_id.clone(), route.clone());
        self.shared
            .sessions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(
                lease.session_id.clone(),
                SessionRecord {
                    thread_id: thread.clone(),
                    active_turn: Some(turn.clone()),
                    bridge_tools: snapshot.by_alias,
                    bridge_aliases: snapshot.by_name,
                },
            );
        let (controls, _receiver) = mpsc::unbounded_channel();
        let provider_events = runtime::event_stream(
            self.shared.connection.clone(),
            receiver,
            self.shared.sessions.clone(),
            TurnRoute {
                session_id: lease.session_id.clone(),
                thread_id: thread,
                turn_id: turn,
            },
            request,
            Some(controls),
        );
        let guard = TurnGuard {
            shared: self.shared.clone(),
            route: route.clone(),
        };
        let events = merged_events(provider_events, lease.events, guard);
        Ok(TurnHandle {
            session_id: lease.session_id,
            events,
            control: Arc::new(Control {
                shared: self.shared.clone(),
                route,
            }),
        })
    }
    async fn close(&self) -> Result<(), RuntimeFailure> {
        let mut state = self.state.lock().await;
        state.closed = true;
        if let Some(route) = state.active.take()
            && route.accepting.load(Ordering::SeqCst)
        {
            Control {
                shared: self.shared.clone(),
                route,
            }
            .cancel()
            .await?;
        }
        state.thread = None;
        Ok(())
    }
}
fn merged_events(
    mut provider: rah_runtime::RuntimeEventStream,
    mut host: rah_runtime::RuntimeEventStream,
    guard: TurnGuard,
) -> rah_runtime::RuntimeEventStream {
    Box::pin(async_stream::stream! {
        let guard = guard;
        let mut host_open = true;
        loop {
            tokio::select! { biased;
                event = host.next(), if host_open => match event {
                    Some(event) => { let terminal = matches!(event.event(), AgentEvent::Failed { .. }); yield event; if terminal { break; } },
                    None => host_open = false,
                },
                event = provider.next() => match event {
                    Some(event) => { let terminal = matches!(event.event(), AgentEvent::Completed { .. } | AgentEvent::Failed { .. } | AgentEvent::Cancelled { .. });
                        if terminal { guard.route.accepting.store(false, Ordering::SeqCst); guard.route.lease.lock().unwrap_or_else(std::sync::PoisonError::into_inner).take(); }
                        yield event; if terminal { break; } },
                    None => break,
                }
            }
        }
        drop(guard);
    })
}
struct Control {
    shared: Arc<Shared>,
    route: Arc<Route>,
}
#[async_trait]
impl TurnControl for Control {
    async fn cancel(&self) -> Result<CancelOutcome, RuntimeFailure> {
        if !self.route.accepting.swap(false, Ordering::SeqCst) {
            return Ok(CancelOutcome::AlreadyTerminal);
        }
        if let Some(lease) = self
            .route
            .lease
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
        {
            lease.stop_requests();
        }
        let mut receiver = self.shared.connection.subscribe();
        self.shared
            .connection
            .request(
                "turn/interrupt",
                json!({"threadId":self.route.thread,"turnId":self.route.turn}),
            )
            .await
            .map_err(|e| e.into_runtime_failure(RuntimeOperation::Cancellation))?;
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                match receiver.recv().await {
                    Ok(ConnectionEvent::Notification { method, params })
                        if method == "turn/completed"
                            && params["threadId"] == self.route.thread
                            && params["turn"]["id"] == self.route.turn =>
                    {
                        if params["turn"]["status"] == "interrupted" {
                            self.route
                                .lease
                                .lock()
                                .unwrap_or_else(std::sync::PoisonError::into_inner)
                                .take();
                            return Ok(CancelOutcome::Stopped);
                        }
                        return Err(failure(
                            "cancellation not confirmed",
                            RuntimeOperation::Cancellation,
                        ));
                    }
                    Ok(ConnectionEvent::Fault { failure }) => {
                        return Err(failure.at_operation(RuntimeOperation::Cancellation));
                    }
                    Err(_) => {
                        return Err(failure("connection closed", RuntimeOperation::Cancellation));
                    }
                    _ => {}
                }
            }
        })
        .await
        .map_err(|_| failure("cancellation deadline", RuntimeOperation::Cancellation))?
    }
}
struct Entry {
    tool: String,
    input: Value,
    ids: Vec<Value>,
    result: Option<Value>,
}
async fn router(
    shared: std::sync::Weak<Shared>,
    mut requests: mpsc::UnboundedReceiver<ServerRequest>,
    mut stop: mpsc::UnboundedReceiver<()>,
) {
    let mut workers: JoinSet<((SessionId, String), Value)> = JoinSet::new();
    let mut calls: HashMap<(SessionId, String), Entry> = HashMap::new();
    let mut fault_events: Option<broadcast::Receiver<ConnectionEvent>> =
        shared.upgrade().map(|s| s.connection.subscribe());
    loop {
        tokio::select! { biased;
            _ = stop.recv() => break,
            event = async { match &mut fault_events { Some(r) => r.recv().await, None => futures::future::pending().await } } => {
                if matches!(event, Ok(ConnectionEvent::Fault { .. }) | Err(_)) {
                    if let Some(s) = shared.upgrade() { s.alive.store(false, Ordering::SeqCst);
                        for route in s.routes.lock().unwrap_or_else(std::sync::PoisonError::into_inner).values() { route.accepting.store(false, Ordering::SeqCst); route.lease.lock().unwrap_or_else(std::sync::PoisonError::into_inner).take(); }
                    }
                    break;
                }
            }
            result = workers.join_next(), if !workers.is_empty() => {
                if let Some(Ok((key, response))) = result
                    && let Some(entry) = calls.get_mut(&key) {
                        if let Some(s) = shared.upgrade() { for id in entry.ids.drain(..) { s.connection.respond_result(id, response.clone()); } }
                        entry.result = Some(response);
                }
            }
            request = requests.recv() => {
                let Some(request) = request else { break };
                let Some(s) = shared.upgrade() else { break };
                let p = &request.params;
                let route = s.routes.lock().unwrap_or_else(std::sync::PoisonError::into_inner).values()
                    .find(|r| p["threadId"] == r.thread && p["turnId"] == r.turn && r.accepting.load(Ordering::SeqCst)).cloned();
                let Some(route) = route else { s.connection.respond_error(request.id, -32602, "inactive Tool route"); continue };
                let Some(tool) = p["tool"].as_str() else { s.connection.respond_error(request.id, -32602, "malformed Tool request"); continue };
                let Some(call) = p["callId"].as_str().filter(|id| !id.is_empty()) else { s.connection.respond_error(request.id, -32602, "missing call identity"); continue };
                let Some(input) = p.get("arguments") else { s.connection.respond_error(request.id, -32602, "missing arguments"); continue };
                if request.method != "item/tool/call" || p.get("namespace").is_some_and(|v| !v.is_null()) {
                    s.connection.respond_error(request.id, -32602, "unsupported Tool namespace"); continue;
                }
                let Some(definition) = route.aliases.get(tool) else { s.connection.respond_error(request.id, -32602, "unadvertised Tool"); continue };
                let key = (route.session.clone(), call.to_owned());
                if let Some(entry) = calls.get_mut(&key) {
                    if entry.tool != tool || entry.input != *input { s.connection.respond_error(request.id, -32602, "conflicting Tool replay"); }
                    else if let Some(response) = &entry.result { s.connection.respond_result(request.id, response.clone()); }
                    else { entry.ids.push(request.id); }
                    continue;
                }
                calls.retain(|(session, _), _| s.routes.lock().unwrap_or_else(std::sync::PoisonError::into_inner).contains_key(session));
                if calls.len() >= 128 || p.to_string().len() > 1024 * 1024 { s.connection.respond_error(request.id, -32602, "Tool request bound"); continue; }
                let name = definition.definition.name.clone();
                let input = input.clone();
                calls.insert(key.clone(), Entry { tool: tool.to_owned(), input: input.clone(), ids: vec![request.id], result: None });
                workers.spawn(async move {
                    let response = if route.accepting.load(Ordering::SeqCst) {
                        match route.tools.request_live(ToolRequest { session_id: route.session.clone(), name, input: ToolInput(input) }).await {
                            Ok(output) => output_response(&output),
                            Err(_) => json!({"contentItems":[{"type":"inputText","text":"host Tool request failed"}],"success":false}),
                        }
                    } else { json!({"contentItems":[],"success":false}) };
                    (key, response)
                });
            }
        }
    }
    workers.abort_all();
    while workers.join_next().await.is_some() {}
}

#[cfg(test)]
mod tests;
