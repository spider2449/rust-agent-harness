use std::{path::PathBuf, sync::Arc};

use futures::StreamExt;
use rah_protocol::{
    AgentEvent, AgentInput, AgentOptions, AgentRequest, Message, MessageRole, RequestId, SessionId,
};
use rah_runtime::{AgentHandle, AgentRuntime};
use serde_json::{Value, json};

use crate::{
    CodexAdapterError, PREFERRED_CURRENT_CODEX_VERSION,
    process::{check_version, validate_captured_contract},
    runtime::CodexRuntime,
    test_support::{FakePeer, fake_transport},
};

#[tokio::test]
async fn restricted_codex_end_to_end_covers_compatibility_resume_and_cancel() {
    check_version(true, PREFERRED_CURRENT_CODEX_VERSION.to_owned()).expect("pinned version");
    validate_captured_contract().expect("captured schema contract");
    let (runtime, mut peer) = connected_runtime().await;

    let first = start_turn(&runtime, &mut peer).await;
    let first_session = first.session_id().clone();
    peer.notify(
        "item/agentMessage/delta",
        json!({
            "threadId": "private-thread",
            "turnId": "private-turn",
            "itemId": "message",
            "delta": "end-to-end"
        }),
    );
    peer.notify("turn/completed", terminal("completed"));
    let events = first.into_events().collect::<Vec<_>>().await;
    assert!(matches!(events.last(), Some(AgentEvent::Completed { .. })));
    assert!(
        !serde_json::to_string(&events)
            .expect("serialize events")
            .contains("private-thread")
    );

    let resuming = {
        let runtime = Arc::clone(&runtime);
        let session_id = first_session.clone();
        tokio::spawn(async move { runtime.resume(session_id).await })
    };
    peer.respond(
        "thread/resume",
        json!({ "thread": { "id": "private-thread" } }),
    )
    .await;
    let resumed = resuming.await.expect("resume task").expect("resume handle");
    assert_eq!(resumed.session_id(), &first_session);
    drop(resumed);

    let second = start_turn(&runtime, &mut peer).await;
    let cancelling = {
        let runtime = Arc::clone(&runtime);
        let session_id = second.session_id().clone();
        tokio::spawn(async move { runtime.cancel(session_id).await })
    };
    peer.respond("turn/interrupt", json!({})).await;
    peer.notify("turn/completed", terminal("interrupted"));
    cancelling
        .await
        .expect("cancel task")
        .expect("confirmed cancellation");
    let events = second.into_events().collect::<Vec<_>>().await;
    assert!(matches!(events.last(), Some(AgentEvent::Cancelled { .. })));

    runtime.shutdown().await.expect("shutdown");
}

async fn connected_runtime() -> (Arc<CodexRuntime>, FakePeer) {
    let (transport, mut peer) = fake_transport();
    let connecting = tokio::spawn(CodexRuntime::from_transport(transport));
    peer.respond("initialize", json!({})).await;
    peer.expect_notification("initialized").await;
    let runtime = connecting
        .await
        .expect("connection task")
        .expect("runtime should initialize");
    (Arc::new(runtime), peer)
}

#[tokio::test]
async fn host_workspace_binds_the_thread_start_request_and_rejects_a_mismatch() {
    let root = PathBuf::from(r"C:\selected-repo");
    let (transport, mut peer) = fake_transport();
    let connecting = tokio::spawn(
        CodexRuntime::from_transport_bridge_with_model_config_and_workspace(
            transport,
            Arc::new(rah_tools::ToolRegistry::new()),
            Vec::new(),
            crate::CodexModelConfig::Inherit,
            root.clone(),
        ),
    );
    peer.respond("initialize", json!({})).await;
    peer.expect_notification("initialized").await;
    let runtime = Arc::new(connecting.await.unwrap().unwrap());
    let starting = {
        let runtime = Arc::clone(&runtime);
        tokio::spawn(async move { runtime.start(sample_request()).await })
    };
    let request = peer
        .respond(
            "thread/start",
            json!({ "thread": { "id": "private-thread" }, "cwd": r"C:\launch-root" }),
        )
        .await;
    assert_eq!(request["params"]["cwd"], root.display().to_string());
    assert!(
        starting.await.unwrap().is_err(),
        "mismatched effective cwd fails closed"
    );
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn pinned_0_149_thread_start_fixture_reaches_turn_start_and_first_event() {
    let root = std::env::current_dir().unwrap().canonicalize().unwrap();
    let (transport, mut peer) = fake_transport();
    let connecting = tokio::spawn(
        CodexRuntime::from_transport_bridge_with_model_config_and_workspace(
            transport,
            Arc::new(rah_tools::ToolRegistry::new()),
            Vec::new(),
            crate::CodexModelConfig::Inherit,
            root.clone(),
        ),
    );
    peer.respond("initialize", json!({})).await;
    peer.expect_notification("initialized").await;
    let runtime = Arc::new(connecting.await.unwrap().unwrap());
    let starting = {
        let runtime = Arc::clone(&runtime);
        tokio::spawn(async move { runtime.start(sample_request()).await })
    };
    let thread = peer
        .respond(
            "thread/start",
            json!({
                "thread": { "id": "private-thread" },
                "cwd": root.display().to_string(),
                "instructionSources": []
            }),
        )
        .await;
    assert_eq!(thread["params"]["cwd"], root.display().to_string());
    let turn = peer
        .respond("turn/start", json!({ "turn": { "id": "private-turn" } }))
        .await;
    assert!(turn["params"].get("cwd").is_none());
    let handle = starting.await.unwrap().unwrap();
    let events = handle.into_events().take(1).collect::<Vec<_>>().await;
    assert!(matches!(events.as_slice(), [AgentEvent::Started { .. }]));
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
#[ignore = "host-only Task 126 direct 0.149.0 runtime.start probe"]
async fn task_126_direct_pinned_runtime_start_probe() {
    let root = std::env::current_dir().unwrap().canonicalize().unwrap();
    let executable = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .expect("LOCALAPPDATA")
        .join("codex-baselines\\0.149.0\\codex.exe");
    let runtime = CodexRuntime::connect_tool_bridge_with_model_config_and_workspace(
        executable,
        Arc::new(rah_tools::ToolRegistry::new()),
        Vec::new(),
        crate::CodexModelConfig::Inherit,
        &root,
    )
    .await
    .expect("pinned app-server connects");
    let handle = runtime
        .start(sample_request())
        .await
        .expect("thread/start and turn/start create an AgentHandle");
    let events = handle.into_events().take(1).collect::<Vec<_>>().await;
    assert!(matches!(events.as_slice(), [AgentEvent::Started { .. }]));
    runtime.shutdown().await.unwrap();
}

async fn start_turn(runtime: &Arc<CodexRuntime>, peer: &mut FakePeer) -> AgentHandle {
    let starting = {
        let runtime = Arc::clone(runtime);
        tokio::spawn(async move { runtime.start(sample_request()).await })
    };
    peer.respond(
        "thread/start",
        json!({ "thread": { "id": "private-thread" } }),
    )
    .await;
    peer.respond("turn/start", json!({ "turn": { "id": "private-turn" } }))
        .await;
    starting
        .await
        .expect("start task")
        .expect("runtime should start")
}

fn sample_request() -> AgentRequest {
    AgentRequest {
        request_id: RequestId::new(),
        input: AgentInput {
            messages: vec![Message {
                role: MessageRole::User,
                content: "test prompt".to_owned(),
            }],
        },
        options: AgentOptions::default(),
    }
}

fn terminal(status: &str) -> Value {
    json!({
        "threadId": "private-thread",
        "turn": {
            "id": "private-turn",
            "status": status,
            "items": [],
            "error": if status == "failed" {
                json!({ "message": "fixture failure" })
            } else {
                Value::Null
            }
        }
    })
}

#[tokio::test]
async fn resume_uses_private_mapping_and_rejects_unknown_session() {
    let (runtime, mut peer) = connected_runtime().await;
    let handle = start_turn(&runtime, &mut peer).await;
    let session_id = handle.session_id().clone();
    let resuming = {
        let runtime = Arc::clone(&runtime);
        let session_id = session_id.clone();
        tokio::spawn(async move { runtime.resume(session_id).await })
    };
    let request = peer
        .respond(
            "thread/resume",
            json!({ "thread": { "id": "private-thread" } }),
        )
        .await;
    assert_eq!(request["params"]["threadId"], "private-thread");
    let resumed = resuming.await.expect("resume task").expect("known session");
    assert_eq!(resumed.session_id(), &session_id);
    let unknown = runtime.resume(SessionId::new()).await;
    assert!(unknown.is_err());

    peer.notify("turn/completed", terminal("completed"));
    let events = handle.into_events().collect::<Vec<_>>().await;
    assert!(matches!(events.last(), Some(AgentEvent::Completed { .. })));
    drop(resumed);
    runtime.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn failed_and_interrupted_turns_map_to_terminal_rah_events() {
    for (status, expected) in [("failed", "failed"), ("interrupted", "cancelled")] {
        let (runtime, mut peer) = connected_runtime().await;
        let handle = start_turn(&runtime, &mut peer).await;
        peer.notify("turn/completed", terminal(status));
        let events = handle.into_events().collect::<Vec<_>>().await;
        match (expected, events.last()) {
            ("failed", Some(AgentEvent::Failed { message, .. })) => {
                assert!(!message.contains("fixture failure"));
            }
            ("cancelled", Some(AgentEvent::Cancelled { .. })) => {}
            _ => panic!("unexpected terminal event for {status}: {events:?}"),
        }
        runtime.shutdown().await.expect("shutdown");
    }
}

#[tokio::test]
async fn cancellation_waits_for_interrupted_terminal_notification() {
    let (runtime, mut peer) = connected_runtime().await;
    let handle = start_turn(&runtime, &mut peer).await;
    let cancelling = {
        let runtime = Arc::clone(&runtime);
        let session_id = handle.session_id().clone();
        tokio::spawn(async move { runtime.cancel(session_id).await })
    };
    let request = peer.respond("turn/interrupt", json!({})).await;
    assert_eq!(request["params"]["turnId"], "private-turn");
    peer.notify("turn/completed", terminal("interrupted"));
    cancelling
        .await
        .expect("cancel task")
        .expect("interruption should be confirmed");
    let events = handle.into_events().collect::<Vec<_>>().await;
    assert!(matches!(events.last(), Some(AgentEvent::Cancelled { .. })));
    runtime.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn cancellation_reports_when_completion_wins_the_race() {
    let (runtime, mut peer) = connected_runtime().await;
    let handle = start_turn(&runtime, &mut peer).await;
    let cancelling = {
        let runtime = Arc::clone(&runtime);
        let session_id = handle.session_id().clone();
        tokio::spawn(async move { runtime.cancel(session_id).await })
    };
    peer.respond("turn/interrupt", json!({})).await;
    peer.notify("turn/completed", terminal("completed"));
    let error = cancelling
        .await
        .expect("cancel task")
        .expect_err("completion should win");
    assert!(
        adapter_source(&error)
            .to_string()
            .contains("before cancellation")
    );
    let events = handle.into_events().collect::<Vec<_>>().await;
    assert!(matches!(events.last(), Some(AgentEvent::Completed { .. })));
    runtime.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn unknown_notifications_are_ignored_without_reordering_deltas() {
    let (runtime, mut peer) = connected_runtime().await;
    let handle = start_turn(&runtime, &mut peer).await;
    peer.notify(
        "future/additiveNotification",
        json!({ "threadId": "private-thread", "turnId": "private-turn" }),
    );
    for delta in ["one", "two"] {
        peer.notify(
            "item/agentMessage/delta",
            json!({
                "threadId": "private-thread",
                "turnId": "private-turn",
                "itemId": "message",
                "delta": delta
            }),
        );
    }
    peer.notify("turn/completed", terminal("completed"));
    let events = handle.into_events().collect::<Vec<_>>().await;
    let deltas = events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::ModelDelta { delta, .. } => Some(delta.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(deltas, ["one", "two"]);
    let encoded = serde_json::to_string(&events).expect("serialize RAH events");
    assert!(!encoded.contains("private-thread"));
    assert!(!encoded.contains("private-turn"));
    runtime.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn codex_tool_items_fail_without_emitting_rah_tool_events() {
    for item_type in [
        "commandExecution",
        "fileChange",
        "mcpToolCall",
        "dynamicToolCall",
    ] {
        let (runtime, mut peer) = connected_runtime().await;
        let handle = start_turn(&runtime, &mut peer).await;
        peer.notify(
            "item/started",
            json!({
                "threadId": "private-thread",
                "turnId": "private-turn",
                "item": { "id": "unsafe", "type": item_type }
            }),
        );
        let events = handle.into_events().collect::<Vec<_>>().await;
        assert!(matches!(events.last(), Some(AgentEvent::Failed { .. })));
        assert!(!events.iter().any(|event| matches!(
            event,
            AgentEvent::ToolRequested { .. }
                | AgentEvent::ToolStarted { .. }
                | AgentEvent::ToolFinished { .. }
        )));
        peer.respond("turn/interrupt", json!({})).await;
        runtime.shutdown().await.expect("shutdown");
    }
}

#[tokio::test]
async fn approval_requests_receive_explicit_errors() {
    let (runtime, mut peer) = connected_runtime().await;
    let handle = start_turn(&runtime, &mut peer).await;
    for (index, method) in [
        "item/commandExecution/requestApproval",
        "item/fileChange/requestApproval",
        "item/permissions/requestApproval",
        "item/tool/call",
        "mcpServer/elicitation/request",
    ]
    .into_iter()
    .enumerate()
    {
        peer.send(json!({
            "id": format!("denied-{index}"),
            "method": method,
            "params": {
                "threadId": "private-thread",
                "turnId": "private-turn"
            }
        }));
        let denial = peer.next_sent().await;
        assert_eq!(denial["id"], format!("denied-{index}"));
        assert_eq!(denial["error"]["code"], -32601);
    }
    let events = handle.into_events().collect::<Vec<_>>().await;
    assert!(matches!(
        events.last(),
        Some(AgentEvent::Failed {
            code: rah_protocol::AgentErrorCode::PermissionDenied,
            ..
        })
    ));
    peer.respond("turn/interrupt", json!({})).await;
    runtime.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn dropping_an_unpolled_turn_stream_enqueues_interrupt() {
    let (runtime, mut peer) = connected_runtime().await;
    let handle = start_turn(&runtime, &mut peer).await;
    drop(handle);
    let request = peer.respond("turn/interrupt", json!({})).await;
    assert_eq!(request["params"]["threadId"], "private-thread");
    assert_eq!(request["params"]["turnId"], "private-turn");
    runtime.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn malformed_response_and_unexpected_exit_are_typed_failures() {
    let (runtime, mut peer) = connected_runtime().await;
    let starting = {
        let runtime = Arc::clone(&runtime);
        tokio::spawn(async move { runtime.start(sample_request()).await })
    };
    let request = peer.next_sent().await;
    assert_eq!(request["method"], "thread/start");
    peer.send(json!({ "id": "not-a-number", "result": {} }));
    let error = match starting.await.expect("start task") {
        Ok(_) => panic!("malformed correlation must fail"),
        Err(error) => error,
    };
    assert!(
        matches!(adapter_source(&error), CodexAdapterError::ProtocolViolation { message } if message.contains("response ID must be"))
    );
    assert!(peer.stopped.load(std::sync::atomic::Ordering::SeqCst));

    let (runtime, mut peer) = connected_runtime().await;
    let starting = {
        let runtime = Arc::clone(&runtime);
        tokio::spawn(async move { runtime.start(sample_request()).await })
    };
    let request = peer.next_sent().await;
    assert_eq!(request["method"], "thread/start");
    peer.fail(process_exit_error("captured stderr"));
    let error = match starting.await.expect("start task") {
        Ok(_) => panic!("unexpected exit must fail"),
        Err(error) => error,
    };
    assert!(
        matches!(adapter_source(&error), CodexAdapterError::ProcessExited { status, stderr } if status.code() == Some(7) && stderr == "captured stderr")
    );
    assert!(peer.stopped.load(std::sync::atomic::Ordering::SeqCst));
}

fn process_exit_error(stderr: &str) -> CodexAdapterError {
    #[cfg(windows)]
    let status = std::process::Command::new("cmd")
        .args(["/C", "exit", "7"])
        .status()
        .expect("obtain fixture exit status");
    #[cfg(not(windows))]
    let status = std::process::Command::new("sh")
        .args(["-c", "exit 7"])
        .status()
        .expect("obtain fixture exit status");
    CodexAdapterError::ProcessExited {
        status,
        stderr: stderr.to_owned(),
    }
}

pub(crate) fn adapter_source<'a>(
    error: &'a (dyn std::error::Error + 'static),
) -> &'a CodexAdapterError {
    let mut current = Some(error);
    while let Some(cause) = current {
        if let Some(typed) = cause.downcast_ref::<CodexAdapterError>() {
            return typed;
        }
        current = cause.source();
    }
    panic!("original adapter source missing");
}

#[tokio::test]
async fn local_turn_failure_retains_source_and_redacts_projection() {
    let (runtime, mut peer) = connected_runtime().await;
    let handle = start_turn(&runtime, &mut peer).await;
    peer.fail(process_exit_error("SECRET_STDERR_498A"));
    let events = handle.into_runtime_events().collect::<Vec<_>>().await;
    let failures: Vec<_> = events.iter().filter_map(|event| event.failure()).collect();
    assert_eq!(failures.len(), 1);
    let failure = failures[0].clone();
    assert!(
        matches!(adapter_source(&failure), CodexAdapterError::ProcessExited { status, stderr }
        if status.code() == Some(7) && stderr == "SECRET_STDERR_498A")
    );
    assert!(!format!("{events:?}").contains("SECRET_STDERR_498A"));
    let projected: Vec<_> = events
        .into_iter()
        .map(rah_runtime::RuntimeEvent::into_event)
        .collect();
    assert!(
        !serde_json::to_string(&projected)
            .unwrap()
            .contains("SECRET_STDERR_498A")
    );
    drop(runtime);
    assert!(matches!(
        adapter_source(&failure),
        CodexAdapterError::ProcessExited { .. }
    ));
}

#[tokio::test]
async fn terminal_provider_failure_is_local_typed_data() {
    let (runtime, mut peer) = connected_runtime().await;
    let handle = start_turn(&runtime, &mut peer).await;
    let mut params = terminal("failed");
    params["turn"]["error"]["message"] = json!("SECRET_PROVIDER_BODY_498A");
    peer.notify("turn/completed", params);
    let events = handle.into_runtime_events().collect::<Vec<_>>().await;
    let failure = events.last().unwrap().failure().unwrap();
    assert!(
        matches!(adapter_source(failure), CodexAdapterError::TurnFailed { message }
        if message == "SECRET_PROVIDER_BODY_498A")
    );
    assert!(
        !serde_json::to_string(events.last().unwrap().event())
            .unwrap()
            .contains("SECRET_PROVIDER_BODY_498A")
    );
    runtime.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn connection_fault_fanout_shares_original_for_all_pending_requests() {
    use crate::connection::{AppServerConnection, ConnectionEvent};
    use std::error::Error;
    let (transport, mut peer) = fake_transport();
    let starting = tokio::spawn(AppServerConnection::initialize(transport, false));
    peer.respond("initialize", json!({})).await;
    peer.expect_notification("initialized").await;
    let connection = Arc::new(starting.await.unwrap().unwrap());
    let mut subscriber = connection.subscribe();
    let first = {
        let connection = Arc::clone(&connection);
        tokio::spawn(async move { connection.request("first", json!({})).await })
    };
    peer.next_sent().await;
    let second = {
        let connection = Arc::clone(&connection);
        tokio::spawn(async move { connection.request("second", json!({})).await })
    };
    peer.next_sent().await;
    peer.fail(process_exit_error("SECRET_FANOUT_498A"));
    let first = first.await.unwrap().unwrap_err();
    let second = second.await.unwrap().unwrap_err();
    let ConnectionEvent::Fault { failure } = subscriber.recv().await.unwrap() else {
        panic!("fault expected")
    };
    let original = failure
        .source()
        .unwrap()
        .downcast_ref::<CodexAdapterError>()
        .unwrap();
    assert!(std::ptr::eq(
        original,
        adapter_source(first.source().unwrap())
    ));
    assert!(std::ptr::eq(
        original,
        adapter_source(second.source().unwrap())
    ));
    assert!(
        matches!(original, CodexAdapterError::ProcessExited { status, stderr }
        if status.code() == Some(7) && stderr == "SECRET_FANOUT_498A")
    );
}

#[test]
fn model_and_provider_mismatches_recover_original_fields() {
    use crate::model_config::{CodexModelConfig, CodexModelProvider};
    let config = CodexModelConfig::Explicit(
        crate::model_config::CodexModelSelection::new("selected", CodexModelProvider::OpenAi)
            .unwrap(),
    );
    for (model, provider) in [("fallback", "openai"), ("selected", "other-provider")] {
        let error = crate::runtime::verify_effective_model_config(
            &json!({
                "model": model, "modelProvider": provider
            }),
            &config,
        )
        .unwrap_err();
        let outer = rah_runtime::AgentError::Failure {
            failure: error.into_runtime_failure(rah_protocol::RuntimeOperation::SessionStart),
        };
        assert!(
            matches!(adapter_source(&outer), CodexAdapterError::ProtocolViolation { message }
            if message.contains("explicit model/provider mismatch") && message.contains(model) && message.contains(provider))
        );
        assert!(!outer.to_string().contains("fallback"));
        assert!(!outer.to_string().contains("other-provider"));
    }
}

#[tokio::test]
async fn explicit_shutdown_retains_transport_error() {
    let (mut transport, mut peer) = fake_transport();
    transport.shutdown_error = Some(CodexAdapterError::Transport {
        source: std::io::Error::other("SECRET_SHUTDOWN_498A"),
    });
    let connecting = tokio::spawn(CodexRuntime::from_transport(transport));
    peer.respond("initialize", json!({})).await;
    peer.expect_notification("initialized").await;
    let runtime = connecting.await.unwrap().unwrap();
    let failure = runtime
        .shutdown()
        .await
        .unwrap_err()
        .into_runtime_failure(rah_protocol::RuntimeOperation::Shutdown);
    assert!(
        matches!(adapter_source(&failure), CodexAdapterError::Transport { source }
        if source.to_string() == "SECRET_SHUTDOWN_498A")
    );
    assert!(!failure.to_string().contains("SECRET_SHUTDOWN_498A"));
    assert!(peer.stopped.load(std::sync::atomic::Ordering::SeqCst));
}

#[tokio::test]
async fn resume_and_cancellation_rejections_preserve_typed_sources() {
    for cancel in [false, true] {
        let (runtime, mut peer) = connected_runtime().await;
        let handle = start_turn(&runtime, &mut peer).await;
        let id = handle.session_id().clone();
        let acting = {
            let runtime = Arc::clone(&runtime);
            tokio::spawn(async move {
                if cancel {
                    runtime.cancel(id).await
                } else {
                    runtime.resume(id).await.map(|_| ())
                }
            })
        };
        let request = peer.next_sent().await;
        assert_eq!(
            request["method"],
            if cancel {
                "turn/interrupt"
            } else {
                "thread/resume"
            }
        );
        peer.send(json!({ "id": request["id"], "error": { "code": -32600, "message": "SECRET_RPC_498A" }}));
        let error = acting.await.unwrap().unwrap_err();
        assert!(
            matches!(adapter_source(&error), CodexAdapterError::JsonRpc { code: -32600, message }
            if message == "SECRET_RPC_498A")
        );
        let rah_runtime::AgentError::Failure { failure } = error else {
            panic!("envelope expected")
        };
        assert_eq!(failure.diagnostic().rpc_code, Some(-32600));
        assert_eq!(
            failure.diagnostic().operation,
            if cancel {
                rah_protocol::RuntimeOperation::Cancellation
            } else {
                rah_protocol::RuntimeOperation::SessionResume
            }
        );
        assert!(
            !serde_json::to_string(failure.diagnostic())
                .unwrap()
                .contains("SECRET_RPC_498A")
        );
        peer.notify("turn/completed", terminal("completed"));
        handle.into_events().collect::<Vec<_>>().await;
        runtime.shutdown().await.unwrap();
    }
}
