//! P6 acceptance for replaceable loops: `loop-plan` runs where `loop-react`
//! ran, from configuration or swapped at runtime, on the same sessions.

mod common;

use common::{TempDir, entries, headless, kernel, kinds, new_session, reply, turn};
use rah_api::{
    PluginId,
    admin::{ADMIN, MountRequest},
};
use rah_protocol::AgentEvent;
use serde_json::Value;

fn answer(events: &[AgentEvent]) -> String {
    events
        .iter()
        .find_map(|event| match event {
            AgentEvent::Completed { output, .. } => Some(output.message.content.clone()),
            _ => None,
        })
        .expect("the turn completed")
}

#[tokio::test]
async fn loop_plan_plans_then_executes_with_tools() {
    let dir = TempDir::new("loop-plan");
    let mut distribution = headless(&dir);
    for spec in &mut distribution.plugin {
        if spec.kind == "loop-react" {
            spec.kind = "loop-plan".into();
        }
    }
    let kernel = kernel(&distribution).await;
    let handle = kernel.handle();
    let session = new_session(&handle).await;
    let events = turn(&handle, &session, "hello").await;
    // The plan request offers no tools, so the mock answers in text.
    assert!(
        reply(&events).starts_with("[model-mock] you said: hello (1 earlier items)"),
        "{}",
        reply(&events)
    );
    assert_eq!(answer(&events), "[model-mock] echo returned: hello");
    assert_eq!(
        kinds(&events),
        [
            "started",
            "model",
            "model",
            "tool-requested",
            "tool-started",
            "tool-finished",
            "model",
            "completed"
        ]
    );
    assert_eq!(
        entries(&handle, &session).await,
        [
            "turn-started",
            "assistant",
            "tool-call",
            "tool-result",
            "assistant",
            "turn-completed"
        ]
    );
}

#[tokio::test]
async fn the_loop_can_be_swapped_at_runtime_on_the_same_session() {
    let dir = TempDir::new("loop-swap");
    let kernel = kernel(&headless(&dir)).await;
    let handle = kernel.handle();
    let session = new_session(&handle).await;
    assert_eq!(
        answer(&turn(&handle, &session, "first").await),
        "[model-mock] echo returned: first"
    );

    let admin = handle.get(ADMIN).unwrap();
    let (id, _) = admin
        .replace(
            &PluginId::new("loop-react"),
            MountRequest {
                kind: "loop-plan".into(),
                config: Value::Null,
                ..MountRequest::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(id.as_str(), "loop-plan");
    let events = turn(&handle, &session, "/notool second").await;
    // Planning sees the earlier turn plus its instruction.
    assert!(
        reply(&events).contains("(5 earlier items)"),
        "{}",
        reply(&events)
    );
    assert!(
        answer(&events).starts_with("[model-mock] you said: second"),
        "{}",
        answer(&events)
    );
    assert_eq!(entries(&handle, &session).await.len(), 5 + 4);
}
