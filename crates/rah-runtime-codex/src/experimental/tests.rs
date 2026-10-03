use super::*;
use crate::test_support::{FakePeer, fake_transport};
use rah_protocol::{
    AgentInput, AgentOptions, AgentRequest, Message, MessageRole, PermissionLevel, RequestId,
    ToolContent, ToolDefinition, ToolName, ToolOutput,
};
use rah_runtime::experimental_host::HostToolScope;
use rah_tools::{Tool, ToolContext, ToolError, ToolRegistry};
use std::{error::Error, sync::atomic::AtomicUsize};

fn request(text: &str) -> AgentRequest {
    AgentRequest {
        request_id: RequestId::new(),
        input: AgentInput {
            messages: vec![Message {
                role: MessageRole::User,
                content: text.into(),
            }],
        },
        options: AgentOptions::default(),
    }
}
async fn initialize(peer: &mut FakePeer) {
    peer.respond("initialize", json!({})).await;
    peer.expect_notification("initialized").await;
}
async fn wait_stopped(peer: FakePeer) {
    while !peer.stopped.load(Ordering::SeqCst) {
        tokio::task::yield_now().await;
    }
}
struct Echo {
    effects: Arc<AtomicUsize>,
    barrier: Arc<tokio::sync::Barrier>,
}
#[async_trait]
impl Tool for Echo {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: ToolName::new("test.echo"),
            description: "echo".into(),
            input_schema: json!({"type":"object"}),
            permission: PermissionLevel::None,
        }
    }
    async fn execute(&self, input: ToolInput, _: ToolContext) -> Result<ToolOutput, ToolError> {
        self.effects.fetch_add(1, Ordering::SeqCst);
        self.barrier.wait().await;
        Ok(ToolOutput {
            content: vec![ToolContent::Json(input.0)],
            is_error: false,
        })
    }
}
fn host(parties: usize) -> (HostToolScope, Arc<AtomicUsize>) {
    let effects = Arc::new(AtomicUsize::new(0));
    let mut registry = ToolRegistry::new();
    registry
        .register(Arc::new(Echo {
            effects: effects.clone(),
            barrier: Arc::new(tokio::sync::Barrier::new(parties)),
        }))
        .unwrap();
    (
        HostToolScope::new(Arc::new(registry), vec![PermissionLevel::None]),
        effects,
    )
}
#[tokio::test]
async fn default_catalog_identity_native_continuation_and_replay() {
    assert_eq!(
        model_config(ModelSelection::RuntimeDefault, &CodexModelProvider::OpenAi).unwrap(),
        CodexModelConfig::Inherit
    );
    let (transport, mut peer) = fake_transport();
    let (go, mut ready) = mpsc::unbounded_channel();
    let peer_task = tokio::spawn(async move {
        initialize(&mut peer).await;
        peer.respond(
            "model/list",
            json!({"data":[{"id":"private","model":"example"}],"nextCursor":null}),
        )
        .await;
        for (index, native) in [(0, false), (1, true), (2, false)] {
            if native {
                let r = peer
                    .respond("thread/resume", json!({"thread":{"id":"provider-0"}}))
                    .await;
                assert_eq!(r["params"]["threadId"], "provider-0");
            } else {
                let r = peer
                    .respond(
                        "thread/start",
                        json!({"thread":{"id":format!("provider-{index}")}}),
                    )
                    .await;
                assert!(r["params"].get("model").is_none());
            }
            peer.respond("turn/start", json!({"turn":{"id":format!("turn-{index}")}}))
                .await;
            ready.recv().await.unwrap();
            let thread = if native {
                "provider-0".to_owned()
            } else {
                format!("provider-{index}")
            };
            peer.notify(
                "item/agentMessage/delta",
                json!({"threadId":thread,"turnId":format!("turn-{index}"),"delta":"answer"}),
            );
            peer.notify("turn/completed", json!({"threadId":thread,"turn":{"id":format!("turn-{index}"),"status":"completed"}}));
        }
        wait_stopped(peer).await;
    });
    let factory = CodexFactory::new(std::env::current_exe().unwrap(), CodexModelProvider::OpenAi);
    *factory.fixture.lock().unwrap() = Some(transport);
    let configured: &dyn ConfiguredRuntimeFactory = &factory;
    configured.validate().unwrap();
    let runtime = configured.create().await.unwrap();
    assert!(runtime.capabilities().native_continuation && runtime.capabilities().cancellation);
    assert_eq!(
        runtime.discover_models().await.unwrap(),
        ModelDiscovery::Catalog {
            complete: true,
            models: vec![ModelDescriptor {
                id: "example".into(),
                display_label: None
            }]
        }
    );
    let (scope, _) = host(1);
    let conversation = runtime
        .open(ConversationSeed {
            id: ConversationId::new("rah-routing"),
            model: ModelSelection::RuntimeDefault,
            tools: scope.port(),
        })
        .await
        .unwrap();
    let mut sessions = Vec::new();
    for native in [false, true, false] {
        let handle = conversation
            .send(if native {
                TurnInput::NativeContinuation(request("next"))
            } else {
                TurnInput::TextReplay(request("full history"))
            })
            .await
            .unwrap();
        assert_ne!(conversation.id().as_str(), handle.session_id.to_string());
        sessions.push(handle.session_id);
        go.send(()).unwrap();
        let events = handle.events.collect::<Vec<_>>().await;
        assert!(
            events.iter().any(
                |e| matches!(e.event(),AgentEvent::ModelDelta {delta,..} if delta == "answer")
            )
        );
        assert!(matches!(
            events.last().unwrap().event(),
            AgentEvent::Completed { .. }
        ));
    }
    assert!(sessions.windows(2).all(|p| p[0] != p[1]));
    conversation.close().await.unwrap();
    assert!(
        conversation
            .send(TurnInput::TextReplay(request("stale")))
            .await
            .is_err()
    );
    runtime.shutdown().await.unwrap();
    assert!(!runtime.is_alive());
    peer_task.await.unwrap();
}
#[tokio::test]
async fn concurrent_tool_round_trip_duplicate_correlation_and_live_events() {
    let (transport, mut peer) = fake_transport();
    let (go, mut ready) = mpsc::unbounded_channel();
    let peer_task = tokio::spawn(async move {
        initialize(&mut peer).await;
        let start = peer.respond("thread/start", json!({"thread":{"id":"private-thread"},"model":"example","modelProvider":"openai"})).await;
        assert_eq!(start["params"]["model"], "example");
        let alias = start["params"]["dynamicTools"][0]["name"].clone();
        peer.respond("turn/start", json!({"turn":{"id":"private-turn"}}))
            .await;
        ready.recv().await.unwrap();
        for (id, call, value) in [(71, "call-a", 1), (72, "call-b", 2), (73, "call-a", 1)] {
            peer.send(json!({"id":id,"method":"item/tool/call","params":{"threadId":"private-thread","turnId":"private-turn","callId":call,"tool":alias,"arguments":{"value":value}}}));
        }
        let mut replies = Vec::new();
        for _ in 0..3 {
            replies.push(peer.next_sent().await);
        }
        for (id, value) in [(71, 1), (72, 2), (73, 1)] {
            let reply = replies.iter().find(|r| r["id"] == id).unwrap();
            assert_eq!(reply["result"]["success"], true);
            assert_eq!(
                reply["result"]["contentItems"][0]["text"],
                json!({"value":value}).to_string()
            );
        }
        peer.notify(
            "item/agentMessage/delta",
            json!({"threadId":"private-thread","turnId":"private-turn","delta":"continued"}),
        );
        peer.notify(
            "turn/completed",
            json!({"threadId":"private-thread","turn":{"id":"private-turn","status":"completed"}}),
        );
        wait_stopped(peer).await;
    });
    let runtime = Instance::from_transport(transport, CodexModelProvider::OpenAi)
        .await
        .unwrap();
    let (scope, effects) = host(2);
    let conversation = runtime
        .open(ConversationSeed {
            id: ConversationId::new("rah"),
            model: ModelSelection::Explicit("example".into()),
            tools: scope.port(),
        })
        .await
        .unwrap();
    let handle = conversation
        .send(TurnInput::TextReplay(request("tools")))
        .await
        .unwrap();
    go.send(()).unwrap();
    let events = handle.events.collect::<Vec<_>>().await;
    assert_eq!(effects.load(Ordering::SeqCst), 2);
    for predicate in [0, 1, 2] {
        assert_eq!(
            events
                .iter()
                .filter(|e| match predicate {
                    0 => matches!(e.event(), AgentEvent::ToolRequested { .. }),
                    1 => matches!(e.event(), AgentEvent::ToolStarted { .. }),
                    _ => matches!(e.event(), AgentEvent::ToolFinished { .. }),
                })
                .count(),
            2
        );
    }
    assert!(matches!(
        events.last().unwrap().event(),
        AgentEvent::Completed { .. }
    ));
    scope.revoke();
    scope.drained().await;
    runtime.shutdown().await.unwrap();
    peer_task.await.unwrap();
}
#[tokio::test]
async fn cancellation_and_retained_shutdown_handles_are_inert() {
    let (transport, mut peer) = fake_transport();
    let peer_task = tokio::spawn(async move {
        initialize(&mut peer).await;
        peer.respond("thread/start", json!({"thread":{"id":"t"}}))
            .await;
        peer.respond("turn/start", json!({"turn":{"id":"u"}})).await;
        peer.respond("turn/interrupt", json!({})).await;
        peer.notify(
            "turn/completed",
            json!({"threadId":"t","turn":{"id":"u","status":"interrupted"}}),
        );
        wait_stopped(peer).await;
    });
    let runtime = Instance::from_transport(transport, CodexModelProvider::OpenAi)
        .await
        .unwrap();
    let (scope, effects) = host(1);
    let port = scope.port();
    let conversation = runtime
        .open(ConversationSeed {
            id: ConversationId::new("rah"),
            model: ModelSelection::RuntimeDefault,
            tools: port.clone(),
        })
        .await
        .unwrap();
    let handle = conversation
        .send(TurnInput::TextReplay(request("wait")))
        .await
        .unwrap();
    assert_eq!(
        handle.control.cancel().await.unwrap(),
        CancelOutcome::Stopped
    );
    let events = handle.events.collect::<Vec<_>>().await;
    assert!(matches!(
        events.last().unwrap().event(),
        AgentEvent::Cancelled { .. }
    ));
    scope.revoke();
    runtime.shutdown().await.unwrap();
    assert!(
        conversation
            .send(TurnInput::TextReplay(request("stale")))
            .await
            .is_err()
    );
    assert!(port.admit_turn().await.is_err());
    assert_eq!(effects.load(Ordering::SeqCst), 0);
    peer_task.await.unwrap();
}
#[tokio::test]
async fn unexpected_exit_and_malformed_response_retain_typed_sanitized_errors() {
    for during_turn in [false, true] {
        let (transport, mut peer) = fake_transport();
        let (go, mut ready) = mpsc::unbounded_channel();
        let peer_task = tokio::spawn(async move {
            initialize(&mut peer).await;
            if during_turn {
                peer.respond("thread/start", json!({"thread":{"id":"t"}}))
                    .await;
                peer.respond("turn/start", json!({"turn":{"id":"u"}})).await;
                ready.recv().await.unwrap();
                peer.fail(CodexAdapterError::ProcessExited {
                    status: std::process::ExitStatus::from_raw(7),
                    stderr: "secret-sentinel".into(),
                });
            } else {
                peer.respond("thread/start", json!({"secret":"secret-sentinel"}))
                    .await;
            }
            wait_stopped(peer).await;
        });
        let runtime = Instance::from_transport(transport, CodexModelProvider::OpenAi)
            .await
            .unwrap();
        let (scope, _) = host(1);
        let conversation = runtime
            .open(ConversationSeed {
                id: ConversationId::new("rah"),
                model: ModelSelection::RuntimeDefault,
                tools: scope.port(),
            })
            .await
            .unwrap();
        let result = conversation
            .send(TurnInput::TextReplay(request("failure")))
            .await;
        if during_turn {
            let handle = result.unwrap();
            go.send(()).unwrap();
            let events = handle.events.collect::<Vec<_>>().await;
            let failure = events.last().unwrap().failure().unwrap();
            assert!(
                failure
                    .source()
                    .unwrap()
                    .downcast_ref::<CodexAdapterError>()
                    .is_some()
            );
            assert!(
                !serde_json::to_string(events.last().unwrap().event())
                    .unwrap()
                    .contains("secret-sentinel")
            );
        } else {
            let error = result.err().unwrap();
            assert!(
                error
                    .source()
                    .unwrap()
                    .downcast_ref::<CodexAdapterError>()
                    .is_some()
            );
        }
        scope.revoke();
        let _ = runtime.shutdown().await;
        peer_task.await.unwrap();
    }
}
#[cfg(unix)]
use std::os::unix::process::ExitStatusExt;
#[cfg(windows)]
use std::os::windows::process::ExitStatusExt;

#[tokio::test]
async fn retained_conversation_and_turn_cannot_dispatch_after_host_teardown() {
    let (transport, mut peer) = fake_transport();
    let (go, mut ready) = mpsc::unbounded_channel();
    let peer_task = tokio::spawn(async move {
        initialize(&mut peer).await;
        let start = peer
            .respond("thread/start", json!({"thread":{"id":"old-t"}}))
            .await;
        let alias = start["params"]["dynamicTools"][0]["name"].clone();
        peer.respond("turn/start", json!({"turn":{"id":"old-u"}}))
            .await;
        ready.recv().await.unwrap();
        peer.send(json!({"id":81,"method":"item/tool/call","params":{"threadId":"old-t","turnId":"old-u","callId":"stale","tool":alias,"arguments":{"value":1}}}));
        let reply = peer.next_sent().await;
        assert_eq!(reply["id"], 81);
        assert_eq!(reply["result"]["success"], false);
        peer.notify(
            "turn/completed",
            json!({"threadId":"old-t","turn":{"id":"old-u","status":"completed"}}),
        );
        wait_stopped(peer).await;
    });
    let runtime = Instance::from_transport(transport, CodexModelProvider::OpenAi)
        .await
        .unwrap();
    let (scope, effects) = host(1);
    let conversation = runtime
        .open(ConversationSeed {
            id: ConversationId::new("old-rah"),
            model: ModelSelection::RuntimeDefault,
            tools: scope.port(),
        })
        .await
        .unwrap();
    let handle = conversation
        .send(TurnInput::TextReplay(request("retain")))
        .await
        .unwrap();
    scope.revoke();
    scope.drained().await;
    go.send(()).unwrap();
    let events = handle.events.collect::<Vec<_>>().await;
    assert!(!events.iter().any(|e| matches!(
        e.event(),
        AgentEvent::ToolStarted { .. } | AgentEvent::ToolFinished { .. }
    )));
    assert_eq!(effects.load(Ordering::SeqCst), 0);
    assert!(
        conversation
            .send(TurnInput::NativeContinuation(request("stale")))
            .await
            .is_err()
    );
    runtime.shutdown().await.unwrap();
    peer_task.await.unwrap();
}

#[tokio::test]
async fn cancellation_preserves_completed_uncertain_tool_result_without_replay() {
    let (transport, mut peer) = fake_transport();
    let (go, mut ready) = mpsc::unbounded_channel();
    let peer_task = tokio::spawn(async move {
        initialize(&mut peer).await;
        let start = peer
            .respond("thread/start", json!({"thread":{"id":"t"}}))
            .await;
        let alias = start["params"]["dynamicTools"][0]["name"].clone();
        peer.respond("turn/start", json!({"turn":{"id":"u"}})).await;
        ready.recv().await.unwrap();
        peer.send(json!({"id":91,"method":"item/tool/call","params":{"threadId":"t","turnId":"u","callId":"one-effect","tool":alias,"arguments":{"uncertain":true}}}));
        // Finished may be observed before the provider reply. Neither transport
        // order implies rollback or permission to replay the effect.
        loop {
            let outbound = peer.next_sent().await;
            if outbound["method"] == "turn/interrupt" {
                peer.send(json!({"id":outbound["id"],"result":{}}));
                peer.notify(
                    "turn/completed",
                    json!({"threadId":"t","turn":{"id":"u","status":"interrupted"}}),
                );
                break;
            } else {
                assert_eq!(outbound["id"], 91);
                assert!(
                    outbound["result"]["contentItems"][0]["text"]
                        .as_str()
                        .unwrap()
                        .contains("uncertain")
                );
            }
        }
        wait_stopped(peer).await;
    });
    let runtime = Instance::from_transport(transport, CodexModelProvider::OpenAi)
        .await
        .unwrap();
    let (scope, effects) = host(1);
    let conversation = runtime
        .open(ConversationSeed {
            id: ConversationId::new("rah"),
            model: ModelSelection::RuntimeDefault,
            tools: scope.port(),
        })
        .await
        .unwrap();
    let mut handle = conversation
        .send(TurnInput::TextReplay(request("effect then wait")))
        .await
        .unwrap();
    go.send(()).unwrap();
    loop {
        let event = handle.events.next().await.unwrap();
        if let AgentEvent::ToolFinished { output, .. } = event.event() {
            assert_eq!(
                output.content,
                vec![ToolContent::Json(json!({"uncertain":true}))]
            );
            break;
        }
    }
    assert_eq!(
        handle.control.cancel().await.unwrap(),
        CancelOutcome::Stopped
    );
    let events = handle.events.collect::<Vec<_>>().await;
    assert!(matches!(
        events.last().unwrap().event(),
        AgentEvent::Cancelled { .. }
    ));
    scope.revoke();
    scope.drained().await;
    assert_eq!(effects.load(Ordering::SeqCst), 1);
    runtime.shutdown().await.unwrap();
    peer_task.await.unwrap();
}
