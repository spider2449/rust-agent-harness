//! P6 acceptance for replaceable loops: `loop-plan` runs where `loop-react`
//! ran, from configuration or swapped at runtime, on the same sessions.

mod common;

use common::{TempDir, entries, headless, kernel, kinds, new_session, reply, turn};
use rah_api::{
    PluginId,
    admin::{ADMIN, MountRequest},
};
use rah_protocol::AgentEvent;
use serde_json::{Value, json};

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

/// Records how many tools each model request offers.
struct RequestTools {
    manifest: rah_api::PluginManifest,
    seen: std::sync::Arc<std::sync::Mutex<Vec<usize>>>,
}

#[rah_api::async_trait]
impl rah_api::Plugin for RequestTools {
    fn manifest(&self) -> &rah_api::PluginManifest {
        &self.manifest
    }

    async fn apply(&self, scope: &mut rah_api::Scope) -> Result<(), rah_api::PluginError> {
        let seen = self.seen.clone();
        scope.hook_waterfall::<rah_api::hooks::ModelBeforeRequest, _, _>(-100, move |request| {
            seen.lock().unwrap().push(request.tools.len());
            async move { request }
        });
        Ok(())
    }
}

/// Boots `loop_kind` with `config`, records the tools offered per request,
/// and runs one turn that uses up the tool rounds.
async fn exhaust(
    loop_kind: &str,
    config: Value,
) -> (Vec<AgentEvent>, Vec<&'static str>, Vec<usize>) {
    let dir = TempDir::new(&format!("{loop_kind}-exhaust"));
    let mut distribution = headless(&dir);
    for spec in &mut distribution.plugin {
        if spec.kind == "loop-react" {
            spec.kind = loop_kind.into();
            spec.config = config.clone();
        }
    }
    let kernel = kernel(&distribution).await;
    let handle = kernel.handle();
    let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    kernel
        .mount(
            std::sync::Arc::new(RequestTools {
                manifest: rah_api::PluginManifest::new("request-tools", "0"),
                seen: seen.clone(),
            }),
            rah_api::MountOptions::default(),
        )
        .await
        .unwrap();
    let session = new_session(&handle).await;
    let events = turn(&handle, &session, "hello").await;
    let entries = entries(&handle, &session).await;
    let seen = seen.lock().unwrap().clone();
    (events, entries, seen)
}

#[tokio::test]
async fn loop_react_answers_without_tools_once_its_rounds_are_used_up() {
    // The mock requests `echo` in the only tool round; the extra round
    // offers no tools and ends on the limit note, which the mock answers
    // with `ready`.
    let (events, entries, tools) = exhaust("loop-react", json!({ "max_rounds": 1 })).await;
    assert_eq!(answer(&events), "[model-mock] ready");
    assert!(
        tools.len() == 2 && tools[0] > 0 && tools[1] == 0,
        "{tools:?}"
    );
    // The note is not recorded; the answer is.
    assert_eq!(
        entries,
        [
            "turn-started",
            "tool-call",
            "tool-result",
            "assistant",
            "turn-completed"
        ]
    );
}

#[tokio::test]
async fn loop_plan_ends_a_step_without_tools_once_its_rounds_are_used_up() {
    let (events, entries, tools) = exhaust("loop-plan", json!({ "max_rounds_per_step": 1 })).await;
    assert_eq!(answer(&events), "[model-mock] ready");
    // Plan (no tools), one tool round, then the extra round without tools.
    assert!(
        tools.len() == 3 && tools[0] == 0 && tools[1] > 0 && tools[2] == 0,
        "{tools:?}"
    );
    assert_eq!(
        entries,
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
