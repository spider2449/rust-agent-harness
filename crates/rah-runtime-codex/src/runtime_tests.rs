use std::sync::Arc;

use futures::StreamExt;
use rah_protocol::{
    AgentEvent, AgentInput, AgentOptions, AgentRequest, Message, MessageRole, RequestId, SessionId,
};
use rah_runtime::{AgentHandle, AgentRuntime};
use serde_json::{Value, json};

use crate::{
    CodexAdapterError,
    runtime::CodexRuntime,
    test_support::{FakePeer, fake_transport},
};

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
                assert_eq!(message, "fixture failure");
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
    assert!(error.to_string().contains("before cancellation"));
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
    assert!(error.to_string().contains("response ID must be"));
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
    assert!(error.to_string().contains("captured stderr"));
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
