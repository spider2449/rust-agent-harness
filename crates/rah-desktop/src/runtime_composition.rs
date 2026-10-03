//! Production composition and host ownership of the experimental neutral seam.
use rah_protocol::{AgentRequest, PermissionLevel, SessionId};
use rah_runtime::{
    ModelCatalog, ModelPreflight, RuntimeFailure,
    experimental::{
        CancelOutcome, ConfiguredRuntimeFactory, ConversationId, ConversationSeed, ModelDiscovery,
        ModelSelection, RuntimeConversation, RuntimeInstance, TurnControl, TurnHandle, TurnInput,
    },
    experimental_host::HostToolScope,
};
use rah_runtime_codex::{CodexModelConfig, CodexModelProvider, experimental::CodexFactory};
use rah_tools::ToolRegistry;
use std::sync::{Arc, Mutex};

/// Host owner of neutral handles. No provider process, thread or RPC identity.
/// The Desktop coordinator remains the authoritative model-turn policy.
pub(crate) struct DesktopRuntime {
    instance: Arc<dyn RuntimeInstance>,
    conversation: Arc<dyn RuntimeConversation>,
    scope: HostToolScope,
    control: Mutex<Option<(SessionId, Arc<dyn TurnControl>)>>,
}
impl DesktopRuntime {
    pub async fn start(&self, request: AgentRequest) -> Result<TurnHandle, RuntimeFailure> {
        let handle = self
            .conversation
            .send(TurnInput::TextReplay(request))
            .await?;
        *self
            .control
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) =
            Some((handle.session_id.clone(), handle.control.clone()));
        Ok(handle)
    }
    pub async fn cancel(&self, session: SessionId) -> Result<(), RuntimeFailure> {
        let control = self
            .control
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .filter(|(id, _)| *id == session)
            .map(|(_, control)| control.clone());
        if let Some(control) = control {
            match control.cancel().await? {
                CancelOutcome::Stopped | CancelOutcome::AlreadyTerminal => Ok(()),
                CancelOutcome::Unsupported => Err(unavailable()),
            }
        } else {
            Err(unavailable())
        }
    }
    /// Called under Desktop lifecycle coordination, before state withdrawal.
    pub fn is_alive(&self) -> bool {
        self.instance.is_alive()
    }
    pub fn revoke(&self) {
        self.scope.revoke();
    }
    pub async fn shutdown(&self) -> Result<(), RuntimeFailure> {
        self.revoke();
        let result = self.instance.shutdown().await;
        self.scope.drained().await;
        self.control
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        result
    }
}
impl Drop for DesktopRuntime {
    fn drop(&mut self) {
        self.revoke();
    }
}

/// Adapter configuration is confined to the production composition root.
pub(crate) fn configured_codex_factory(
    executable: std::path::PathBuf,
    config: CodexModelConfig,
    workspace: &std::path::Path,
) -> Result<(CodexFactory, ModelSelection), RuntimeFailure> {
    let (provider, model) = match config {
        CodexModelConfig::Inherit => (CodexModelProvider::OpenAi, ModelSelection::RuntimeDefault),
        CodexModelConfig::Explicit(selection) => (
            selection.provider().clone(),
            ModelSelection::Explicit(selection.model().to_owned()),
        ),
    };
    Ok((
        CodexFactory::new(executable, provider).with_workspace(workspace)?,
        model,
    ))
}

/// Discovery only: no conversation or inference is opened before this gate.
pub(crate) async fn create_and_preflight(
    factory: &dyn ConfiguredRuntimeFactory,
    model: &ModelSelection,
) -> Result<
    (
        Arc<dyn RuntimeInstance>,
        Result<ModelPreflight, RuntimeFailure>,
    ),
    RuntimeFailure,
> {
    factory.validate()?;
    let instance = factory.create().await?;
    let preflight = match model {
        ModelSelection::RuntimeDefault => Ok(ModelPreflight::NotChecked),
        ModelSelection::Explicit(selected) => match instance.discover_models().await {
            Ok(ModelDiscovery::Catalog {
                models,
                complete: true,
            }) => {
                let catalog = ModelCatalog {
                    models: models.into_iter().map(|m| m.id).collect(),
                };
                if catalog.models.contains(selected) {
                    Ok(ModelPreflight::Advertised(catalog))
                } else {
                    Ok(ModelPreflight::NotAdvertised(catalog))
                }
            }
            Ok(ModelDiscovery::Unsupported) => Ok(ModelPreflight::NotChecked),
            Ok(_) => {
                Err(unavailable().at_operation(rah_protocol::RuntimeOperation::ModelDiscovery))
            }
            Err(error) => Err(error),
        },
    };
    Ok((instance, preflight))
}

/// The port receives the same current registry and permission policy as before.
/// No second Tool execution path is created.
pub(crate) async fn bind_conversation(
    instance: Arc<dyn RuntimeInstance>,
    model: ModelSelection,
    registry: Arc<ToolRegistry>,
    permissions: Vec<PermissionLevel>,
) -> Result<DesktopRuntime, RuntimeFailure> {
    let scope = HostToolScope::new(registry, permissions);
    let conversation = instance
        .open(ConversationSeed {
            id: ConversationId::new(format!("desktop-{}", SessionId::new())),
            model,
            tools: scope.port(),
        })
        .await;
    let conversation = match conversation {
        Ok(conversation) => conversation,
        Err(error) => {
            scope.revoke();
            if let Err(shutdown_error) = instance.shutdown().await {
                tracing::warn!(failure = ?shutdown_error, "conversation binding cleanup failed");
            }
            return Err(error);
        }
    };
    Ok(DesktopRuntime {
        instance,
        conversation,
        scope,
        control: Mutex::new(None),
    })
}

fn unavailable() -> RuntimeFailure {
    RuntimeFailure::without_source(rah_protocol::RuntimeDiagnostic {
        operation: rah_protocol::RuntimeOperation::Turn,
        kind: rah_protocol::RuntimeFailureKind::Unavailable,
        rpc_code: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use futures::StreamExt;
    use rah_protocol::{
        AgentEvent, AgentInput, AgentOptions, Message, MessageRole, RequestId, ToolInput, ToolName,
    };
    use rah_runtime::experimental::{Capabilities, HostToolPort, ModelDescriptor, ToolRequest};
    use std::{
        error::Error,
        sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    };

    struct Fixture {
        alive: AtomicBool,
        discoveries: AtomicUsize,
        opens: AtomicUsize,
        sends: Arc<AtomicUsize>,
        port: Mutex<Option<Arc<dyn HostToolPort>>>,
        cancelled: Arc<AtomicBool>,
        fault: AtomicBool,
    }
    impl Fixture {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                alive: AtomicBool::new(true),
                discoveries: AtomicUsize::new(0),
                opens: AtomicUsize::new(0),
                sends: Arc::new(AtomicUsize::new(0)),
                port: Mutex::new(None),
                cancelled: Arc::new(AtomicBool::new(false)),
                fault: AtomicBool::new(false),
            })
        }
    }
    struct Factory {
        instance: Arc<Fixture>,
        validations: AtomicUsize,
        creates: AtomicUsize,
    }
    #[async_trait]
    impl ConfiguredRuntimeFactory for Factory {
        fn validate(&self) -> Result<(), RuntimeFailure> {
            self.validations.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
        async fn create(&self) -> Result<Arc<dyn RuntimeInstance>, RuntimeFailure> {
            assert_eq!(self.validations.load(Ordering::SeqCst), 1);
            self.creates.fetch_add(1, Ordering::SeqCst);
            Ok(self.instance.clone())
        }
    }
    struct Conversation {
        id: ConversationId,
        tools: Arc<dyn HostToolPort>,
        cancelled: Arc<AtomicBool>,
        sends: Arc<AtomicUsize>,
    }
    struct Control(Arc<AtomicBool>);
    #[async_trait]
    impl TurnControl for Control {
        async fn cancel(&self) -> Result<CancelOutcome, RuntimeFailure> {
            self.0.store(true, Ordering::SeqCst);
            Ok(CancelOutcome::Stopped)
        }
    }
    #[async_trait]
    impl RuntimeConversation for Conversation {
        fn id(&self) -> &ConversationId {
            &self.id
        }
        async fn send(&self, _input: TurnInput) -> Result<TurnHandle, RuntimeFailure> {
            let lease = self.tools.admit_turn().await?;
            self.sends.fetch_add(1, Ordering::SeqCst);
            let result = self
                .tools
                .request_live(ToolRequest {
                    session_id: lease.session_id.clone(),
                    name: ToolName::new("echo"),
                    input: ToolInput(serde_json::json!({"text":"round-trip"})),
                })
                .await?;
            assert!(!result.is_error);
            assert_eq!(
                result.content,
                vec![rah_protocol::ToolContent::Text("round-trip".into())]
            );
            let session_id = lease.session_id.clone();
            let final_id = session_id.clone();
            let cancelled = self.cancelled.clone();
            let lifetime = lease.lifetime;
            let terminal = futures::stream::once(async move {
                if cancelled.load(Ordering::SeqCst) {
                    AgentEvent::Cancelled {
                        session_id: final_id,
                    }
                    .into()
                } else {
                    AgentEvent::Completed {
                        session_id: final_id,
                        output: rah_protocol::AgentOutput {
                            message: Message {
                                role: MessageRole::Assistant,
                                content: "round-trip".into(),
                            },
                        },
                    }
                    .into()
                }
            });
            let events = lease.events.take(3).chain(terminal).map(move |event| {
                let _ = &lifetime;
                event
            });
            Ok(TurnHandle {
                session_id,
                events: Box::pin(events),
                control: Arc::new(Control(self.cancelled.clone())),
            })
        }
        async fn close(&self) -> Result<(), RuntimeFailure> {
            self.cancelled.store(true, Ordering::SeqCst);
            Ok(())
        }
    }
    #[async_trait]
    impl RuntimeInstance for Fixture {
        fn capabilities(&self) -> Capabilities {
            Capabilities {
                discovery: true,
                native_continuation: false,
                text_replay: true,
                tool_calls: true,
                cancellation: true,
                streaming: true,
            }
        }
        fn is_alive(&self) -> bool {
            self.alive.load(Ordering::SeqCst)
        }
        async fn discover_models(&self) -> Result<ModelDiscovery, RuntimeFailure> {
            self.discoveries.fetch_add(1, Ordering::SeqCst);
            if self.fault.load(Ordering::SeqCst) {
                return Err(rah_runtime_codex::CodexAdapterError::JsonRpc {
                    code: -32001,
                    message: "Authorization: Bearer SECRET".into(),
                }
                .into_runtime_failure(rah_protocol::RuntimeOperation::ModelDiscovery));
            }
            Ok(ModelDiscovery::Catalog {
                complete: true,
                models: vec![ModelDescriptor {
                    id: "advertised".into(),
                    display_label: None,
                }],
            })
        }
        async fn open(
            &self,
            seed: ConversationSeed,
        ) -> Result<Arc<dyn RuntimeConversation>, RuntimeFailure> {
            self.opens.fetch_add(1, Ordering::SeqCst);
            *self.port.lock().unwrap() = Some(seed.tools.clone());
            Ok(Arc::new(Conversation {
                id: seed.id,
                tools: seed.tools,
                cancelled: self.cancelled.clone(),
                sends: self.sends.clone(),
            }))
        }
        async fn shutdown(&self) -> Result<(), RuntimeFailure> {
            self.alive.store(false, Ordering::SeqCst);
            Ok(())
        }
    }
    fn factory(instance: Arc<Fixture>) -> Factory {
        Factory {
            instance,
            validations: AtomicUsize::new(0),
            creates: AtomicUsize::new(0),
        }
    }
    fn request() -> AgentRequest {
        AgentRequest {
            request_id: RequestId::new(),
            input: AgentInput {
                messages: vec![Message {
                    role: MessageRole::User,
                    content: "neutral turn".into(),
                }],
            },
            options: AgentOptions::default(),
        }
    }
    fn registry() -> Arc<ToolRegistry> {
        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(rah_tools::EchoTool)).unwrap();
        Arc::new(registry)
    }
    #[tokio::test]
    async fn task504_factory_preflight_gates_open_and_ready() {
        for (model, allowed, discoveries) in [
            (ModelSelection::Explicit("advertised".into()), true, 1),
            (ModelSelection::Explicit("absent".into()), false, 1),
            (ModelSelection::RuntimeDefault, true, 0),
        ] {
            let fixture = Fixture::new();
            let factory = factory(fixture.clone());
            let (instance, outcome) = create_and_preflight(&factory, &model).await.unwrap();
            let selected = match &model {
                ModelSelection::Explicit(id) => Some(id.clone()),
                _ => None,
            };
            let (view, gate) = crate::model_preflight::present(selected, outcome);
            assert_eq!(fixture.discoveries.load(Ordering::SeqCst), discoveries);
            assert_eq!(fixture.opens.load(Ordering::SeqCst), 0);
            assert_eq!(gate.is_ok(), allowed);
            if allowed {
                let runtime =
                    bind_conversation(instance, model, registry(), vec![PermissionLevel::None])
                        .await
                        .unwrap();
                assert_eq!(fixture.opens.load(Ordering::SeqCst), 1);
                let events = runtime
                    .start(request())
                    .await
                    .unwrap()
                    .events
                    .collect::<Vec<_>>()
                    .await;
                assert!(matches!(
                    events.last().unwrap().event(),
                    AgentEvent::Completed { .. }
                ));
                runtime.shutdown().await.unwrap();
            } else {
                assert_eq!(view.presentation.advertised_models, ["advertised"]);
                assert_eq!(fixture.sends.load(Ordering::SeqCst), 0);
                instance.shutdown().await.unwrap();
            }
        }
    }
    #[tokio::test]
    async fn task504_production_binding_round_trip_revoke_and_cancel() {
        for cancel in [false, true] {
            let fixture = Fixture::new();
            let runtime = bind_conversation(
                fixture.clone(),
                ModelSelection::RuntimeDefault,
                registry(),
                vec![PermissionLevel::None],
            )
            .await
            .unwrap();
            let conversation = runtime.conversation.clone();
            let port = fixture.port.lock().unwrap().clone().unwrap();
            let handle = runtime.start(request()).await.unwrap();
            assert_ne!(conversation.id().as_str(), handle.session_id.to_string());
            let session = handle.session_id.clone();
            if cancel {
                runtime.cancel(session.clone()).await.unwrap();
            }
            let events = handle.events.collect::<Vec<_>>().await;
            assert!(matches!(
                events[0].event(),
                AgentEvent::ToolRequested { .. }
            ));
            assert!(matches!(events[1].event(), AgentEvent::ToolStarted { .. }));
            assert!(matches!(events[2].event(), AgentEvent::ToolFinished { .. }));
            assert_eq!(
                events
                    .iter()
                    .filter(|event| matches!(event.event(), AgentEvent::ToolStarted { .. }))
                    .count(),
                1
            );
            assert_eq!(
                matches!(events[3].event(), AgentEvent::Cancelled { .. }),
                cancel
            );
            runtime.revoke();
            assert!(port.admit_turn().await.is_err());
            assert!(
                port.request_live(ToolRequest {
                    session_id: session,
                    name: ToolName::new("echo"),
                    input: ToolInput(serde_json::json!({}))
                })
                .await
                .is_err()
            );
            assert!(
                conversation
                    .send(TurnInput::TextReplay(request()))
                    .await
                    .is_err()
            );
            runtime.shutdown().await.unwrap();
        }
    }
    #[tokio::test]
    async fn task504_neutral_discovery_retains_typed_codex_failure() {
        let fixture = Fixture::new();
        fixture.fault.store(true, Ordering::SeqCst);
        let (instance, outcome) = create_and_preflight(
            &factory(fixture.clone()),
            &ModelSelection::Explicit("advertised".into()),
        )
        .await
        .unwrap();
        let (view, gate) = crate::model_preflight::present(Some("advertised".into()), outcome);
        assert!(gate.is_err());
        assert_eq!(fixture.opens.load(Ordering::SeqCst), 0);
        assert!(
            view.failure
                .as_ref()
                .unwrap()
                .source()
                .unwrap()
                .downcast_ref::<rah_runtime_codex::CodexAdapterError>()
                .is_some()
        );
        assert!(
            !serde_json::to_string(&view.presentation)
                .unwrap()
                .contains("SECRET")
        );
        instance.shutdown().await.unwrap();
    }
    #[tokio::test]
    async fn task504_unexpected_exit_withdraws_ready_and_revokes_production_port() {
        let fixture = Fixture::new();
        let registry = registry();
        let composition =
            crate::desktop_tool_composition_from_registry(registry.clone(), None, false, &[])
                .unwrap();
        let runtime = Arc::new(
            bind_conversation(
                fixture.clone(),
                ModelSelection::RuntimeDefault,
                registry,
                vec![PermissionLevel::None],
            )
            .await
            .unwrap(),
        );
        let retained = runtime.conversation.clone();
        let port = fixture.port.lock().unwrap().clone().unwrap();
        let state = crate::DesktopAppState::new(
            std::env::temp_dir().join(format!("rah-task504-exit-{}", SessionId::new())),
        );
        *state.connection.lock().unwrap() = crate::ConnectionState::Connected {
            runtime: runtime.clone(),
            source: crate::CodexExecutableSource::Path,
            repository_generation: 0,
            model_generation: 0,
            profile_generation: 0,
            connection_generation: 0,
            identity_generation: 0,
            repository_fingerprint: None,
            composition,
            allowed_permissions: vec![PermissionLevel::None],
        };
        fixture.alive.store(false, Ordering::SeqCst);
        assert_eq!(state.status().codex_status, "error");
        assert!(matches!(
            *state.connection.lock().unwrap(),
            crate::ConnectionState::Error(_)
        ));
        assert!(port.admit_turn().await.is_err());
        assert!(
            retained
                .send(TurnInput::TextReplay(request()))
                .await
                .is_err()
        );
        assert_eq!(fixture.sends.load(Ordering::SeqCst), 0);
        runtime.shutdown().await.unwrap();
    }
}
