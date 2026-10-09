//! Native Responses adapter behind the experimental neutral runtime seam.
//! Desktop selects it by default; deterministic coverage is not live API
//! certification. Only the scoped host port can execute a Tool.
//! Continuation uses host-owned text replay; native continuation is unsupported.
//! Official OpenAI discovery is unsupported; the separate llama.cpp factory lists
//! local server models without resolving official OpenAI credentials.
mod error;
mod llamacpp;
pub use llamacpp::{LlamaCppError, LlamaCppFactory};
mod protocol;
mod sse;
pub use error::OpenAiAdapterError;

use OpenAiAdapterError as E;
use async_trait::async_trait;
use futures::{FutureExt, StreamExt};
use rah_protocol::{
    AgentErrorCode, AgentEvent, AgentOutput, Message, MessageRole, ModelRequestId,
    RuntimeOperation, ToolInput, ToolName,
};
use rah_runtime::{RuntimeEvent, RuntimeFailure, experimental::*};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    fmt,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tokio::{
    sync::mpsc,
    task::{AbortHandle, JoinHandle},
};
use tokio_util::sync::CancellationToken;

const ORIGIN: &str = "https://api.openai.com/v1/responses";
const MAX_EVENTS: usize = 32_768;

/// Backend-only configuration: intentionally no serde, Clone or credential accessor.
pub struct OpenAiFactory {
    key: String,
    #[cfg(any(test, feature = "fixture-support"))]
    endpoint: Option<String>,
}
impl fmt::Debug for OpenAiFactory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OpenAiFactory").finish_non_exhaustive()
    }
}
impl OpenAiFactory {
    /// Deterministic local integration seam. Loopback only, fixed fake credential.
    /// Ordinary production builds do not include this constructor.
    #[cfg(feature = "fixture-support")]
    pub fn local_fixture(address: std::net::SocketAddr) -> Result<Self, RuntimeFailure> {
        if !address.ip().is_loopback() {
            return Err(E::Configuration.into_runtime_failure(RuntimeOperation::Connection));
        }
        Ok(Self {
            key: "RAH-LOCAL-FIXTURE-FAKE-KEY".into(),
            endpoint: Some(format!("http://{address}/v1/responses")),
        })
    }
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            key: api_key.into(),
            #[cfg(any(test, feature = "fixture-support"))]
            endpoint: None,
        }
    }
}
#[async_trait]
impl ConfiguredRuntimeFactory for OpenAiFactory {
    fn validate(&self) -> Result<(), RuntimeFailure> {
        if self.key.trim().is_empty()
            || reqwest::header::HeaderValue::from_str(&format!("Bearer {}", self.key)).is_err()
        {
            return Err(E::Configuration.into_runtime_failure(RuntimeOperation::Connection));
        }
        Ok(())
    }
    async fn create(&self) -> Result<Arc<dyn RuntimeInstance>, RuntimeFailure> {
        self.validate()?;
        let endpoint = ORIGIN.to_owned();
        #[cfg(any(test, feature = "fixture-support"))]
        let endpoint = self.endpoint.clone().unwrap_or(endpoint);
        transport_instance(endpoint, self.key.clone(), None)
    }
}
fn transport_instance(
    endpoint: String,
    key: String,
    models: Option<Vec<String>>,
) -> Result<Arc<dyn RuntimeInstance>, RuntimeFailure> {
    let client = reqwest::Client::builder()
        .https_only(endpoint == ORIGIN)
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .no_proxy()
        .connect_timeout(Duration::from_secs(30))
        .read_timeout(Duration::from_secs(60))
        .timeout(Duration::from_secs(300))
        .build()
        .map_err(|_| E::Transport.into_runtime_failure(RuntimeOperation::Connection))?;
    Ok(Arc::new(Instance {
        shared: Arc::new(Shared {
            client,
            key,
            endpoint,
            models,
            stop: CancellationToken::new(),
            tasks: Mutex::new(Vec::new()),
        }),
    }))
}
struct Shared {
    models: Option<Vec<String>>,
    client: reqwest::Client,
    key: String,
    endpoint: String,
    stop: CancellationToken,
    tasks: Mutex<Vec<JoinHandle<()>>>,
}
impl Drop for Shared {
    fn drop(&mut self) {
        self.stop.cancel();
        for task in self
            .tasks
            .get_mut()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .drain(..)
        {
            task.abort();
        }
    }
}
struct Instance {
    shared: Arc<Shared>,
}
#[async_trait]
impl RuntimeInstance for Instance {
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            discovery: self.shared.models.is_some(),
            native_continuation: false,
            text_replay: true,
            tool_calls: true,
            cancellation: true,
            streaming: true,
        }
    }
    fn is_alive(&self) -> bool {
        !self.shared.stop.is_cancelled()
    }
    async fn discover_models(&self) -> Result<ModelDiscovery, RuntimeFailure> {
        if !self.is_alive() {
            return Err(E::Shutdown.into_runtime_failure(RuntimeOperation::ModelDiscovery));
        }
        Ok(match &self.shared.models {
            Some(models) => ModelDiscovery::Catalog {
                models: models
                    .iter()
                    .map(|id| ModelDescriptor {
                        id: id.clone(),
                        display_label: None,
                    })
                    .collect(),
                complete: true,
            },
            None => ModelDiscovery::Unsupported,
        })
    }
    async fn open(
        &self,
        seed: ConversationSeed,
    ) -> Result<Arc<dyn RuntimeConversation>, RuntimeFailure> {
        if !self.is_alive() {
            return Err(E::Shutdown.into_runtime_failure(RuntimeOperation::SessionStart));
        }
        let model = match seed.model {
            ModelSelection::Explicit(model) => model,
            ModelSelection::RuntimeDefault => match self.shared.models.as_deref() {
                Some([model]) => model.clone(),
                Some(_) => {
                    return Err(
                        LlamaCppError::ModelUnavailable.failure(RuntimeOperation::SessionStart)
                    );
                }
                _ => {
                    return Err(
                        E::Configuration.into_runtime_failure(RuntimeOperation::SessionStart)
                    );
                }
            },
        };
        if self
            .shared
            .models
            .as_ref()
            .is_some_and(|models| !models.contains(&model))
        {
            return Err(LlamaCppError::ModelUnavailable.failure(RuntimeOperation::SessionStart));
        }
        if model.trim().is_empty() || model.len() > 256 {
            return Err(E::Configuration.into_runtime_failure(RuntimeOperation::SessionStart));
        }
        let definitions = protocol::tools(&seed.tools.definitions().0)
            .map_err(|e| e.into_runtime_failure(RuntimeOperation::SessionStart))?;
        Ok(Arc::new(Conversation {
            id: seed.id,
            shared: self.shared.clone(),
            model,
            tools: seed.tools,
            definitions,
            stop: self.shared.stop.child_token(),
        }))
    }
    async fn shutdown(&self) -> Result<(), RuntimeFailure> {
        self.shared.stop.cancel();
        let tasks = std::mem::take(
            &mut *self
                .shared
                .tasks
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        );
        for task in tasks {
            let _ = task.await;
        }
        Ok(())
    }
}
struct Conversation {
    id: ConversationId,
    shared: Arc<Shared>,
    model: String,
    tools: Arc<dyn HostToolPort>,
    definitions: Vec<Value>,
    stop: CancellationToken,
}
impl Drop for Conversation {
    fn drop(&mut self) {
        self.stop.cancel();
    }
}
struct Control {
    stop: CancellationToken,
    terminal: Arc<AtomicBool>,
    gate: Arc<Mutex<()>>,
}
#[async_trait]
impl TurnControl for Control {
    async fn cancel(&self) -> Result<CancelOutcome, RuntimeFailure> {
        {
            let _gate = self
                .gate
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if self.terminal.load(Ordering::SeqCst) {
                return Ok(CancelOutcome::AlreadyTerminal);
            }
            self.stop.cancel();
        }
        // The worker drops HTTP/host response futures before publishing terminal.
        while !self.terminal.load(Ordering::SeqCst) {
            tokio::task::yield_now().await;
        }
        Ok(CancelOutcome::Stopped)
    }
}
struct StreamOwner {
    abort: AbortHandle,
    stop: CancellationToken,
}
#[derive(Default)]
struct Correlation {
    pending: Option<String>,
    calls: HashMap<rah_protocol::ToolCallId, String>,
}
struct TerminalGuard(Arc<AtomicBool>);
impl Drop for TerminalGuard {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}
impl Drop for StreamOwner {
    fn drop(&mut self) {
        self.stop.cancel();
        self.abort.abort();
    }
}
#[async_trait]
impl RuntimeConversation for Conversation {
    fn id(&self) -> &ConversationId {
        &self.id
    }
    async fn close(&self) -> Result<(), RuntimeFailure> {
        self.stop.cancel();
        Ok(())
    }
    async fn send(&self, input: TurnInput) -> Result<TurnHandle, RuntimeFailure> {
        if self.stop.is_cancelled() {
            return Err(E::Shutdown.into_runtime_failure(RuntimeOperation::SessionStart));
        }
        let TurnInput::TextReplay(request) = input else {
            return Err(E::Configuration.into_runtime_failure(RuntimeOperation::SessionResume));
        };
        let mut history = protocol::input(&request)
            .map_err(|e| e.into_runtime_failure(RuntimeOperation::SessionStart))?;
        if self.shared.models.is_some() {
            // llama.cpp distinguishes replayed assistant output from easy input
            // messages using this discriminator, even with string content.
            for item in &mut history {
                if item["role"] == "assistant" {
                    item["type"] = Value::String("message".into());
                }
            }
            protocol::bounded(&history)
                .map_err(|e| e.into_runtime_failure(RuntimeOperation::SessionStart))?;
        }
        let lease = self.tools.admit_turn().await?;
        let session = lease.session_id.clone();
        let stop = self.stop.child_token();
        let terminal = Arc::new(AtomicBool::new(false));
        let gate = Arc::new(Mutex::new(()));
        let control = Arc::new(Control {
            stop: stop.clone(),
            terminal: terminal.clone(),
            gate: gate.clone(),
        });
        let (sender, mut receiver) = mpsc::unbounded_channel();
        let correlation = Arc::new(Mutex::new(Correlation::default()));
        let job = Job {
            llamacpp: self.shared.models.is_some(),
            client: self.shared.client.clone(),
            key: self.shared.key.clone(),
            endpoint: self.shared.endpoint.clone(),
            model: self.model.clone(),
            definitions: self.definitions.clone(),
            tools: self.tools.clone(),
            session: session.clone(),
            history,
            correlation: correlation.clone(),
        };
        let task_stop = stop.clone();
        let task_session = session.clone();
        // Serialize registration against shutdown; no worker owns Shared.
        let mut tasks = self
            .shared
            .tasks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if self.stop.is_cancelled() {
            return Err(E::Shutdown.into_runtime_failure(RuntimeOperation::SessionStart));
        }
        tasks.retain(|t| !t.is_finished());
        let terminal_guard = TerminalGuard(terminal.clone());
        let task = tokio::spawn(async move {
            let _terminal_guard = terminal_guard;
            let HostTurnLease {
                mut events,
                lifetime,
                ..
            } = lease;
            let _ = sender.send(
                AgentEvent::Started {
                    session_id: task_session.clone(),
                    request_id: request.request_id,
                }
                .into(),
            );
            let mut count = 0;
            let mut host_failed = false;
            let mut host_open = true;
            {
                let mut stream = job.run();
                loop {
                    let item = tokio::select! {
                        biased;
                        _ = task_stop.cancelled() => Some(Err(E::Cancelled.into_runtime_failure(RuntimeOperation::Turn))),
                        event = events.next(), if host_open => {
                            if let Some(event) = event {
                                if let AgentEvent::ToolRequested { tool_call,.. } = event.event() {
                                    let mut state = correlation.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
                                    if let Some(provider) = state.pending.clone() {
                                        state.calls.insert(tool_call.id.clone(),provider);
                                    }
                                }
                                host_failed |= event.failure().is_some(); Some(Ok(event))
                            }
                            else { host_open=false; continue; }
                        },
                        item = stream.next() => item,
                    };
                    let Some(item) = item else {
                        break;
                    };
                    match item {
                        Ok(event) => {
                            count += 1;
                            if count > MAX_EVENTS {
                                if !host_failed {
                                    let _ = sender.send(RuntimeEvent::failed(
                                        task_session.clone(),
                                        AgentErrorCode::Model,
                                        E::Protocol.into_runtime_failure(RuntimeOperation::Turn),
                                    ));
                                }
                                break;
                            }
                            let completed = matches!(event.event(), AgentEvent::Completed { .. });
                            if completed {
                                let _gate = gate
                                    .lock()
                                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                                if task_stop.is_cancelled() {
                                    let _ = sender.send(RuntimeEvent::failed(
                                        task_session.clone(),
                                        AgentErrorCode::Model,
                                        turn_failure(E::Cancelled),
                                    ));
                                } else {
                                    terminal.store(true, Ordering::SeqCst);
                                    let _ = sender.send(event);
                                }
                                break;
                            }
                            if sender.send(event).is_err() || host_failed {
                                break;
                            }
                        }
                        Err(failure) => {
                            // A synchronously rejected host request may have queued
                            // lifecycle facts in the same poll as its returned error.
                            while let Some(Some(event)) = events.next().now_or_never() {
                                host_failed |= event.failure().is_some();
                                let _ = sender.send(event);
                            }
                            if !host_failed {
                                let _ = sender.send(RuntimeEvent::failed(
                                    task_session.clone(),
                                    AgentErrorCode::Model,
                                    failure,
                                ));
                            }
                            break;
                        }
                    }
                }
                // Drops network stream / response future; host-admitted effects
                // remain host-owned under ADR 0033, never retried here.
            }
            lifetime.stop_requests();
            drop(lifetime);
            terminal.store(true, Ordering::SeqCst);
        });
        let owner = StreamOwner {
            abort: task.abort_handle(),
            stop,
        };
        tasks.push(task);
        drop(tasks);
        Ok(TurnHandle {
            session_id: session,
            control,
            events: Box::pin(async_stream::stream! {
                let _owner = owner;
                while let Some(event) = receiver.recv().await { yield event; }
            }),
        })
    }
}
struct Job {
    llamacpp: bool,
    client: reqwest::Client,
    key: String,
    endpoint: String,
    model: String,
    definitions: Vec<Value>,
    tools: Arc<dyn HostToolPort>,
    session: rah_protocol::SessionId,
    history: Vec<Value>,
    correlation: Arc<Mutex<Correlation>>,
}
impl Job {
    fn run(
        mut self,
    ) -> std::pin::Pin<Box<dyn futures::Stream<Item = Result<RuntimeEvent, RuntimeFailure>> + Send>>
    {
        Box::pin(async_stream::try_stream! {
            let mut final_text = String::new();
            loop {
                protocol::bounded(&self.history).map_err(turn_failure)?;
                let model_request_id = ModelRequestId::new();
                yield AgentEvent::ModelRequestStarted { session_id:self.session.clone(),model_request_id:model_request_id.clone() }.into();
                let body = json!({"model":self.model,"input":self.history,"tools":self.definitions,"stream":true,"store":false});
                let request = self.client.post(&self.endpoint);
                let request = if self.key.is_empty() { request } else { request.bearer_auth(&self.key) };
                let response = request
                    .header(reqwest::header::CONTENT_TYPE,"application/json")
                    .header(reqwest::header::ACCEPT,"text/event-stream")
                    .body(body.to_string()).send().await.map_err(|_| turn_failure(E::Transport))?;
                if !response.status().is_success() { Err(turn_failure(E::HttpStatus(response.status().as_u16())))?; }
                if response.headers().get(reqwest::header::CONTENT_TYPE).and_then(|v|v.to_str().ok()).and_then(|v|v.split(';').next()) != Some("text/event-stream") { Err(turn_failure(E::Sse))?; }
                let mut bytes = response.bytes_stream();
                let mut parser = sse::Sse::default();
                let mut assembled = protocol::Response::default();
                let mut local_events = self.llamacpp.then(llamacpp::EventNormalizer::default);
                let mut total = 0;
                while let Some(chunk) = bytes.next().await {
                    let chunk = chunk.map_err(|_| turn_failure(E::Transport))?;
                    total += chunk.len();
                    if total > protocol::MAX_BYTES * 2 { Err(turn_failure(E::Sse))?; }
                    for event in parser.push(&chunk).map_err(turn_failure)? {
                        let events = match &mut local_events {
                            Some(local) => local.normalize(event).map_err(turn_failure)?,
                            None => vec![event],
                        };
                        for event in events {
                        match assembled.event(event).map_err(turn_failure)? {
                            protocol::Update::Text(delta) => yield AgentEvent::ModelDelta { session_id:self.session.clone(),model_request_id:model_request_id.clone(),delta }.into(),
                            protocol::Update::Complete | protocol::Update::None => {},
                        }
                        }
                    }
                    if assembled.output.is_some() { break; }
                }
                parser.finish().map_err(turn_failure)?;
                let output = assembled.output.as_ref().ok_or_else(||turn_failure(E::Protocol))?;
                if final_text.len() + assembled.text.len() > protocol::MAX_BYTES { Err(turn_failure(E::Protocol))?; }
                final_text.push_str(&assembled.text);
                if assembled.calls().next().is_none() {
                    yield AgentEvent::Completed { session_id:self.session.clone(),output:AgentOutput { message:Message { role:MessageRole::Assistant,content:final_text } } }.into();
                    return;
                }
                self.history.extend(output.iter().cloned());
                // Serialized execution uses exactly the same live host semantics
                // as concurrent calls. The host allocates every neutral ToolCallId.
                for call in assembled.calls() {
                    let name = protocol::string(call,"name").map_err(turn_failure)?;
                    if !self.definitions.iter().any(|d|d["name"] == name) { Err(turn_failure(E::Protocol))?; }
                    let arguments = serde_json::from_str(protocol::string(call,"arguments").map_err(turn_failure)?).map_err(|_|turn_failure(E::EventJson))?;
                    let provider_call = protocol::string(call,"call_id").map_err(turn_failure)?.to_owned();
                    {
                        let mut state = self.correlation.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
                        state.calls.clear();
                        state.pending = Some(provider_call.clone());
                    }
                    let result = self.tools.request_live(ToolRequest { session_id:self.session.clone(),name:ToolName::new(name),input:ToolInput(arguments) }).await?;
                    // Task 502 live host contract emits ToolRequested before result.
                    let correlated = self.correlation.lock().unwrap_or_else(std::sync::PoisonError::into_inner).calls.values().filter(|id| **id == provider_call).count();
                    if correlated != 1 { Err(turn_failure(E::Protocol))?; }
                    let output = serde_json::to_string(&result).map_err(|_|turn_failure(E::Protocol))?;
                    self.history.push(json!({"type":"function_call_output","call_id":call["call_id"],"output":output}));
                    protocol::bounded(&self.history).map_err(turn_failure)?;
                }
            }
        })
    }
}
fn turn_failure(e: E) -> RuntimeFailure {
    e.into_runtime_failure(RuntimeOperation::Turn)
}

#[cfg(test)]
mod tests;
