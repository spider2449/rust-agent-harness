//! Experimental neutral runtime seam. These contracts are not used by Desktop.
//! Provider identities and continuation data remain private to adapters. A
//! `ConversationId` is RAH routing identity; `SessionId` identifies one turn.

use std::sync::Arc;

use async_trait::async_trait;
use rah_protocol::{AgentRequest, SessionId, ToolDefinition, ToolInput, ToolName, ToolOutput};

use crate::{RuntimeEventStream, RuntimeFailure};

/// A bounded, closed description of behavior, never a grant of authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Capabilities {
    pub discovery: bool,
    pub native_continuation: bool,
    pub text_replay: bool,
    pub tool_calls: bool,
    pub cancellation: bool,
    pub streaming: bool,
}

/// Model selector scoped to the already configured runtime.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ModelSelection {
    Explicit(String),
    RuntimeDefault,
}

/// Inert discovery data; no provider configuration or authority is embedded.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelDescriptor {
    pub id: String,
    pub display_label: Option<String>,
}

/// A catalog may not be complete enough to prove a model absent.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ModelDiscovery {
    Unsupported,
    Catalog {
        models: Vec<ModelDescriptor>,
        complete: bool,
    },
}

/// RAH-owned process-local identity, distinct from provider state and SessionId.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ConversationId(String);

impl ConversationId {
    /// Construct from a host-assigned routing key. It carries no authority.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The host's read-only Tool advertisement, with no registry mutation handle.
#[derive(Clone, Debug)]
pub struct ToolSnapshot(pub Vec<ToolDefinition>);

/// One untrusted request. The host allocates its own ToolCallId internally.
#[derive(Clone, Debug)]
pub struct ToolRequest {
    pub session_id: SessionId,
    pub name: ToolName,
    pub input: ToolInput,
}

/// Host-produced Tool lifecycle and the result returned to the adapter.
pub struct ToolReply {
    pub events: Vec<crate::RuntimeEvent>,
    pub output: ToolOutput,
}

/// A scoped host port. Its implementation owns lookup, authorization,
/// repository binding, dispatch and uncertain-effect lifetime.
#[async_trait]
pub trait HostToolPort: Send + Sync {
    fn definitions(&self) -> ToolSnapshot;
    async fn request(&self, request: ToolRequest) -> Result<ToolReply, RuntimeFailure>;

    /// Requests host admission, never grants the adapter turn exclusivity policy.
    /// The host assigns the operation identity and live lifecycle receiver.
    async fn admit_turn(&self) -> Result<HostTurnLease, RuntimeFailure> {
        Err(crate::experimental_host::unavailable())
    }

    /// Live mode: lifecycle facts are delivered through the host-owned lease.
    /// Buffered legacy ports are deliberately not usable through this mode.
    async fn request_live(&self, _request: ToolRequest) -> Result<ToolOutput, RuntimeFailure> {
        Err(crate::experimental_host::unavailable())
    }
}

/// Host-issued admission. Dropping this owner withdraws new Tool submissions;
/// already admitted effects remain host-owned until their result is accounted for.
pub struct HostTurnLease {
    pub session_id: SessionId,
    pub events: RuntimeEventStream,
    pub lifetime: Box<dyn HostTurnLifetime>,
}

/// Withdrawal-only lease control. It cannot admit or renew a host turn.
pub trait HostTurnLifetime: Send + Sync {
    fn stop_requests(&self);
}

/// Only the host supplies the conversation ID and scoped Tool port.
pub struct ConversationSeed {
    pub id: ConversationId,
    pub model: ModelSelection,
    pub tools: Arc<dyn HostToolPort>,
}

/// Text replay sends a complete host snapshot. Native continuation is distinct.
pub enum TurnInput {
    TextReplay(AgentRequest),
    NativeContinuation(AgentRequest),
}

/// Explicit cancellation outcome; absence never claims a successful stop.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CancelOutcome {
    Unsupported,
    AlreadyTerminal,
    Stopped,
}

#[async_trait]
pub trait TurnControl: Send + Sync {
    async fn cancel(&self) -> Result<CancelOutcome, RuntimeFailure>;
}

/// One owner of a turn's stream and a separate shareable control handle.
pub struct TurnHandle {
    pub session_id: SessionId,
    pub events: RuntimeEventStream,
    pub control: Arc<dyn TurnControl>,
}

#[async_trait]
pub trait RuntimeConversation: Send + Sync {
    fn id(&self) -> &ConversationId;
    async fn send(&self, input: TurnInput) -> Result<TurnHandle, RuntimeFailure>;
    async fn close(&self) -> Result<(), RuntimeFailure>;
}

#[async_trait]
pub trait RuntimeInstance: Send + Sync {
    fn capabilities(&self) -> Capabilities;
    fn is_alive(&self) -> bool;
    async fn discover_models(&self) -> Result<ModelDiscovery, RuntimeFailure>;
    async fn open(
        &self,
        seed: ConversationSeed,
    ) -> Result<Arc<dyn RuntimeConversation>, RuntimeFailure>;
    async fn shutdown(&self) -> Result<(), RuntimeFailure>;
}

/// Configuration is adapter-owned; this boundary receives no repository or ToolRegistry.
#[async_trait]
pub trait ConfiguredRuntimeFactory: Send + Sync {
    fn validate(&self) -> Result<(), RuntimeFailure>;
    async fn create(&self) -> Result<Arc<dyn RuntimeInstance>, RuntimeFailure>;
}

#[cfg(test)]
mod tests {
    use std::{
        error::Error,
        fmt,
        sync::{
            Arc,
            atomic::{AtomicBool, AtomicUsize, Ordering},
        },
    };

    use async_trait::async_trait;
    use futures::StreamExt;
    use rah_protocol::{
        AgentErrorCode, AgentEvent, AgentInput, AgentOptions, AgentOutput, AgentRequest, Message,
        MessageRole, PermissionLevel, RequestId, RuntimeDiagnostic, RuntimeFailureKind,
        RuntimeOperation, ToolCall, ToolCallId, ToolContent, ToolDefinition, ToolInput, ToolName,
        ToolOutput,
    };
    use tokio_util::sync::CancellationToken;

    use super::*;
    use crate::{RuntimeEvent, RuntimeFailure};

    const SECRET: &str = "fake-provider-secret-sentinel";

    #[derive(Debug)]
    struct FakeAdapterError;
    impl fmt::Display for FakeAdapterError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str(SECRET)
        }
    }
    impl Error for FakeAdapterError {}

    fn failure(operation: RuntimeOperation) -> RuntimeFailure {
        RuntimeFailure::new(
            RuntimeDiagnostic {
                operation,
                kind: RuntimeFailureKind::Operation,
                rpc_code: None,
            },
            FakeAdapterError,
        )
    }

    #[derive(Default)]
    struct Host {
        requests: AtomicUsize,
        authorized: AtomicBool,
    }
    #[async_trait]
    impl HostToolPort for Host {
        fn definitions(&self) -> ToolSnapshot {
            ToolSnapshot(vec![ToolDefinition {
                name: ToolName::new("fake.echo"),
                description: "echo".into(),
                input_schema: serde_json::json!({"type":"object"}),
                permission: PermissionLevel::None,
            }])
        }
        async fn request(&self, request: ToolRequest) -> Result<ToolReply, RuntimeFailure> {
            self.requests.fetch_add(1, Ordering::SeqCst);
            if !self.authorized.load(Ordering::SeqCst) || request.name.as_str() != "fake.echo" {
                return Err(failure(RuntimeOperation::Turn));
            }
            let call = ToolCall {
                id: ToolCallId::new(),
                name: request.name,
                input: request.input,
            };
            let output = ToolOutput {
                content: vec![ToolContent::Json(call.input.0.clone())],
                is_error: false,
            };
            Ok(ToolReply {
                events: vec![
                    RuntimeEvent::from(AgentEvent::ToolRequested {
                        session_id: request.session_id.clone(),
                        tool_call: call.clone(),
                    }),
                    RuntimeEvent::from(AgentEvent::ToolStarted {
                        session_id: request.session_id.clone(),
                        tool_call_id: call.id.clone(),
                    }),
                    RuntimeEvent::from(AgentEvent::ToolFinished {
                        session_id: request.session_id,
                        tool_call_id: call.id,
                        output: output.clone(),
                    }),
                ],
                output,
            })
        }
    }

    struct FakeFactory {
        capabilities: Capabilities,
        fail: bool,
    }
    struct FakeRuntime {
        capabilities: Capabilities,
        alive: Arc<AtomicBool>,
        fail: bool,
    }
    struct FakeConversation {
        id: ConversationId,
        provider_identity: String,
        model: ModelSelection,
        capabilities: Capabilities,
        alive: Arc<AtomicBool>,
        closed: AtomicBool,
        turns: AtomicUsize,
        native_turns: AtomicUsize,
        tools: Arc<dyn HostToolPort>,
        fail: bool,
    }
    struct FakeControl {
        token: CancellationToken,
        supported: bool,
        terminal: Arc<AtomicBool>,
    }

    #[async_trait]
    impl ConfiguredRuntimeFactory for FakeFactory {
        fn validate(&self) -> Result<(), RuntimeFailure> {
            Ok(())
        }
        async fn create(&self) -> Result<Arc<dyn RuntimeInstance>, RuntimeFailure> {
            Ok(Arc::new(FakeRuntime {
                capabilities: self.capabilities,
                alive: Arc::new(AtomicBool::new(true)),
                fail: self.fail,
            }))
        }
    }
    #[async_trait]
    impl RuntimeInstance for FakeRuntime {
        fn capabilities(&self) -> Capabilities {
            self.capabilities
        }
        fn is_alive(&self) -> bool {
            self.alive.load(Ordering::SeqCst)
        }
        async fn discover_models(&self) -> Result<ModelDiscovery, RuntimeFailure> {
            if !self.capabilities.discovery {
                return Ok(ModelDiscovery::Unsupported);
            }
            Ok(ModelDiscovery::Catalog {
                complete: true,
                models: vec![
                    ModelDescriptor {
                        id: "fake-small".into(),
                        display_label: Some("Fake Small".into()),
                    },
                    ModelDescriptor {
                        id: "fake-large".into(),
                        display_label: None,
                    },
                ],
            })
        }
        async fn open(
            &self,
            seed: ConversationSeed,
        ) -> Result<Arc<dyn RuntimeConversation>, RuntimeFailure> {
            if !self.is_alive() {
                return Err(failure(RuntimeOperation::SessionStart));
            }
            if matches!(&seed.model, ModelSelection::Explicit(id) if id != "fake-small" && id != "fake-large")
            {
                return Err(failure(RuntimeOperation::SessionStart));
            }
            Ok(Arc::new(FakeConversation {
                id: seed.id,
                provider_identity: "provider-private-42".into(),
                model: seed.model,
                capabilities: self.capabilities,
                alive: Arc::clone(&self.alive),
                closed: AtomicBool::new(false),
                turns: AtomicUsize::new(0),
                native_turns: AtomicUsize::new(0),
                tools: seed.tools,
                fail: self.fail,
            }))
        }
        async fn shutdown(&self) -> Result<(), RuntimeFailure> {
            self.alive.store(false, Ordering::SeqCst);
            Ok(())
        }
    }
    #[async_trait]
    impl RuntimeConversation for FakeConversation {
        fn id(&self) -> &ConversationId {
            &self.id
        }
        async fn send(&self, input: TurnInput) -> Result<TurnHandle, RuntimeFailure> {
            if !self.alive.load(Ordering::SeqCst) || self.closed.load(Ordering::SeqCst) {
                return Err(failure(RuntimeOperation::Turn));
            }
            if matches!(input, TurnInput::NativeContinuation(_))
                && !self.capabilities.native_continuation
            {
                return Err(failure(RuntimeOperation::SessionResume));
            }
            if matches!(input, TurnInput::TextReplay(_)) && !self.capabilities.text_replay {
                return Err(failure(RuntimeOperation::Turn));
            }
            assert_ne!(self.provider_identity, self.id.as_str());
            let (request, answer) = match input {
                TurnInput::TextReplay(r) => (r, "replay".to_string()),
                TurnInput::NativeContinuation(r) => {
                    let count = self.native_turns.fetch_add(1, Ordering::SeqCst) + 1;
                    (r, format!("native-{count}"))
                }
            };
            let answer = match &self.model {
                ModelSelection::Explicit(id) => format!("{answer}:{id}"),
                ModelSelection::RuntimeDefault => format!("{answer}:default"),
            };
            self.turns.fetch_add(1, Ordering::SeqCst);
            let session_id = SessionId::new();
            let token = CancellationToken::new();
            let terminal = Arc::new(AtomicBool::new(false));
            let control = Arc::new(FakeControl {
                token: token.clone(),
                supported: self.capabilities.cancellation,
                terminal: Arc::clone(&terminal),
            });
            let tools = Arc::clone(&self.tools);
            let fail = self.fail;
            let streaming = self.capabilities.streaming;
            let tool_calls = self.capabilities.tool_calls;
            let session = session_id.clone();
            let events = Box::pin(async_stream::stream! {
                yield RuntimeEvent::from(AgentEvent::Started { session_id: session.clone(), request_id: request.request_id });
                if fail {
                    terminal.store(true, Ordering::SeqCst);
                    yield RuntimeEvent::failed(session, AgentErrorCode::Internal, failure(RuntimeOperation::Turn));
                    return;
                }
                if streaming {
                    yield RuntimeEvent::from(AgentEvent::ModelDelta { session_id: session.clone(),
                        model_request_id: rah_protocol::ModelRequestId::new(), delta: "hello".into() });
                }
                let mut answer = answer;
                if tool_calls {
                    let result = tools.request(ToolRequest { session_id: session.clone(), name: ToolName::new("fake.echo"),
                        input: ToolInput(serde_json::json!({"value": 7})) }).await;
                    match result {
                        Ok(reply) if reply.output.content == vec![ToolContent::Json(serde_json::json!({"value": 7}))] => {
                            for event in reply.events { yield event; }
                            answer.push_str(":tool-result");
                        },
                        _ => {
                            terminal.store(true, Ordering::SeqCst);
                            yield RuntimeEvent::failed(session, AgentErrorCode::Tool, failure(RuntimeOperation::Turn));
                            return;
                        }
                    }
                }
                if request.input.messages.iter().any(|m| m.content == "wait") {
                    token.cancelled().await;
                    terminal.store(true, Ordering::SeqCst);
                    yield RuntimeEvent::from(AgentEvent::Cancelled { session_id: session });
                    return;
                }
                terminal.store(true, Ordering::SeqCst);
                yield RuntimeEvent::from(AgentEvent::Completed { session_id: session,
                    output: AgentOutput { message: Message { role: MessageRole::Assistant,
                        content: answer } } });
            });
            Ok(TurnHandle {
                session_id,
                events,
                control,
            })
        }
        async fn close(&self) -> Result<(), RuntimeFailure> {
            self.closed.store(true, Ordering::SeqCst);
            Ok(())
        }
    }
    #[async_trait]
    impl TurnControl for FakeControl {
        async fn cancel(&self) -> Result<CancelOutcome, RuntimeFailure> {
            if !self.supported {
                return Ok(CancelOutcome::Unsupported);
            }
            if self.terminal.load(Ordering::SeqCst) {
                return Ok(CancelOutcome::AlreadyTerminal);
            }
            self.token.cancel();
            Ok(CancelOutcome::Stopped)
        }
    }

    fn capabilities() -> Capabilities {
        Capabilities {
            discovery: true,
            native_continuation: true,
            text_replay: true,
            tool_calls: true,
            cancellation: true,
            streaming: true,
        }
    }
    fn request(content: &str) -> AgentRequest {
        AgentRequest {
            request_id: RequestId::new(),
            input: AgentInput {
                messages: vec![Message {
                    role: MessageRole::User,
                    content: content.into(),
                }],
            },
            options: AgentOptions::default(),
        }
    }
    async fn setup(
        c: Capabilities,
        fail: bool,
        host: Arc<Host>,
        model: ModelSelection,
    ) -> (Arc<dyn RuntimeInstance>, Arc<dyn RuntimeConversation>) {
        let factory: Arc<dyn ConfiguredRuntimeFactory> = Arc::new(FakeFactory {
            capabilities: c,
            fail,
        });
        factory.validate().unwrap();
        let runtime = factory.create().await.unwrap();
        let conversation = runtime
            .open(ConversationSeed {
                id: ConversationId::new("rah-conversation-1"),
                model,
                tools: host,
            })
            .await
            .unwrap();
        (runtime, conversation)
    }

    #[tokio::test]
    async fn discovery_selection_and_identity_are_neutral() {
        let host = Arc::new(Host::default());
        host.authorized.store(true, Ordering::SeqCst);
        let (runtime, conversation) = setup(
            capabilities(),
            false,
            host.clone(),
            ModelSelection::Explicit("fake-small".into()),
        )
        .await;
        assert_eq!(
            runtime.discover_models().await.unwrap(),
            ModelDiscovery::Catalog {
                complete: true,
                models: vec![
                    ModelDescriptor {
                        id: "fake-small".into(),
                        display_label: Some("Fake Small".into())
                    },
                    ModelDescriptor {
                        id: "fake-large".into(),
                        display_label: None
                    }
                ]
            }
        );
        assert_eq!(conversation.id().as_str(), "rah-conversation-1");
        assert_ne!(conversation.id().as_str(), "provider-private-42");
        let turn = conversation
            .send(TurnInput::TextReplay(request("go")))
            .await
            .unwrap();
        assert_ne!(turn.session_id.to_string(), conversation.id().as_str());
        let events: Vec<_> = turn.events.collect().await;
        assert!(events.iter().any(
            |e| matches!(e.event(), AgentEvent::ModelDelta { delta, .. } if delta == "hello")
        ));
        assert!(
            matches!(events.last().unwrap().event(), AgentEvent::Completed { output, .. }
            if output.message.content == "replay:fake-small:tool-result")
        );
        assert_eq!(host.requests.load(Ordering::SeqCst), 1);
        let (_, default_conversation) = setup(
            capabilities(),
            false,
            Arc::new(Host::default()),
            ModelSelection::RuntimeDefault,
        )
        .await;
        assert_eq!(default_conversation.id().as_str(), "rah-conversation-1");
        let mut absent = capabilities();
        absent.discovery = false;
        absent.tool_calls = false;
        let (runtime, _) = setup(
            absent,
            false,
            Arc::new(Host::default()),
            ModelSelection::RuntimeDefault,
        )
        .await;
        assert_eq!(
            runtime.discover_models().await.unwrap(),
            ModelDiscovery::Unsupported
        );
    }

    #[tokio::test]
    async fn continuation_and_replay_are_distinct() {
        let host = Arc::new(Host::default());
        host.authorized.store(true, Ordering::SeqCst);
        let (_, conversation) =
            setup(capabilities(), false, host, ModelSelection::RuntimeDefault).await;
        let first = conversation
            .send(TurnInput::NativeContinuation(request("one")))
            .await
            .unwrap();
        assert!(
            matches!(first.events.collect::<Vec<_>>().await.last().unwrap().event(),
            AgentEvent::Completed { output, .. } if output.message.content == "native-1:default:tool-result")
        );
        let second = conversation
            .send(TurnInput::NativeContinuation(request("two")))
            .await
            .unwrap();
        assert_ne!(first.session_id, second.session_id);
        assert!(
            matches!(second.events.collect::<Vec<_>>().await.last().unwrap().event(),
            AgentEvent::Completed { output, .. } if output.message.content == "native-2:default:tool-result")
        );
        let mut absent = capabilities();
        absent.native_continuation = false;
        absent.tool_calls = false;
        let (_, conversation) = setup(
            absent,
            false,
            Arc::new(Host::default()),
            ModelSelection::RuntimeDefault,
        )
        .await;
        assert!(
            conversation
                .send(TurnInput::NativeContinuation(request("two")))
                .await
                .is_err()
        );
        assert!(
            conversation
                .send(TurnInput::TextReplay(request("two")))
                .await
                .is_ok()
        );
    }

    #[tokio::test]
    async fn host_port_mediates_tools_and_denial() {
        let host = Arc::new(Host::default());
        let (_, conversation) = setup(
            capabilities(),
            false,
            host.clone(),
            ModelSelection::RuntimeDefault,
        )
        .await;
        let turn = conversation
            .send(TurnInput::TextReplay(request("go")))
            .await
            .unwrap();
        let events = turn.events.collect::<Vec<_>>().await;
        assert!(matches!(
            events.last().unwrap().event(),
            AgentEvent::Failed {
                code: AgentErrorCode::Tool,
                ..
            }
        ));
        assert_eq!(host.requests.load(Ordering::SeqCst), 1);
        host.authorized.store(true, Ordering::SeqCst);
        let turn = conversation
            .send(TurnInput::TextReplay(request("go")))
            .await
            .unwrap();
        let events = turn.events.collect::<Vec<_>>().await;
        assert!(
            events
                .iter()
                .any(|e| matches!(e.event(), AgentEvent::ToolRequested { .. }))
        );
        assert!(
            events
                .iter()
                .any(|e| matches!(e.event(), AgentEvent::ToolStarted { .. }))
        );
        assert!(
            events
                .iter()
                .any(|e| matches!(e.event(), AgentEvent::ToolFinished { .. }))
        );
        assert!(matches!(events.last().unwrap().event(),
            AgentEvent::Completed { output, .. } if output.message.content.ends_with(":tool-result")));
        assert_eq!(host.requests.load(Ordering::SeqCst), 2);
        // The adapter has only definitions() and request(); Host owns the
        // private authorization bit and emits no forged execution lifecycle.
    }

    #[tokio::test]
    async fn cancellation_failure_and_shutdown() {
        let mut c = capabilities();
        c.tool_calls = false;
        let (runtime, conversation) = setup(
            c,
            false,
            Arc::new(Host::default()),
            ModelSelection::RuntimeDefault,
        )
        .await;
        let turn = conversation
            .send(TurnInput::TextReplay(request("wait")))
            .await
            .unwrap();
        assert_eq!(turn.control.cancel().await.unwrap(), CancelOutcome::Stopped);
        assert!(matches!(
            turn.events
                .collect::<Vec<_>>()
                .await
                .last()
                .unwrap()
                .event(),
            AgentEvent::Cancelled { .. }
        ));
        let mut absent = c;
        absent.cancellation = false;
        let (_, conversation2) = setup(
            absent,
            false,
            Arc::new(Host::default()),
            ModelSelection::RuntimeDefault,
        )
        .await;
        let turn = conversation2
            .send(TurnInput::TextReplay(request("wait")))
            .await
            .unwrap();
        assert_eq!(
            turn.control.cancel().await.unwrap(),
            CancelOutcome::Unsupported
        );
        drop(turn);
        let (_, failing) = setup(
            c,
            true,
            Arc::new(Host::default()),
            ModelSelection::RuntimeDefault,
        )
        .await;
        let turn = failing
            .send(TurnInput::TextReplay(request("go")))
            .await
            .unwrap();
        let events = turn.events.collect::<Vec<_>>().await;
        let failure = events.last().unwrap().failure().unwrap();
        assert_eq!(failure.diagnostic().operation, RuntimeOperation::Turn);
        assert!(
            failure
                .source()
                .unwrap()
                .downcast_ref::<FakeAdapterError>()
                .is_some()
        );
        assert!(
            !serde_json::to_string(failure.diagnostic())
                .unwrap()
                .contains(SECRET)
        );
        runtime.shutdown().await.unwrap();
        assert!(!runtime.is_alive());
        assert!(
            conversation
                .send(TurnInput::TextReplay(request("later")))
                .await
                .is_err()
        );
    }
}
