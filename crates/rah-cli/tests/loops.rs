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
        reply(&events).starts_with("[model-mock] you said: hello (3 earlier items)"),
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
        reply(&events).contains("(7 earlier items)"),
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

/// Provides `gate`, a tool that returns only once it is opened.
struct GateTool {
    manifest: rah_api::PluginManifest,
    open: std::sync::Arc<tokio::sync::Notify>,
}

struct Gate(std::sync::Arc<tokio::sync::Notify>);

#[rah_api::async_trait]
impl rah_api::tools::Tool for Gate {
    fn definition(&self) -> rah_protocol::ToolDefinition {
        rah_protocol::ToolDefinition {
            name: rah_protocol::ToolName::new("gate"),
            description: "returns once the test opens it".into(),
            input_schema: json!({"type": "object"}),
            permission: rah_protocol::PermissionLevel::None,
        }
    }

    async fn call(
        &self,
        _input: Value,
        _cx: rah_api::tools::ToolContext,
    ) -> Result<rah_protocol::ToolOutput, rah_api::tools::ToolError> {
        self.0.notified().await;
        Ok(rah_protocol::ToolOutput {
            content: vec![rah_protocol::ToolContent::Text("opened".into())],
            is_error: false,
        })
    }
}

#[rah_api::async_trait]
impl rah_api::Plugin for GateTool {
    fn manifest(&self) -> &rah_api::PluginManifest {
        &self.manifest
    }

    async fn apply(&self, scope: &mut rah_api::Scope) -> Result<(), rah_api::PluginError> {
        let gate = std::sync::Arc::new(Gate(self.open.clone()));
        rah_api::tools::provide_tools(scope, vec![gate])?;
        Ok(())
    }
}

/// `ToolStarted` arrives while the tool runs, not after it returned: the
/// gate opens only once the event was seen.
#[tokio::test]
async fn both_loops_pin_the_workspace_and_accept_steering_while_a_tool_runs() {
    use futures::StreamExt;
    use rah_api::agent_loop::{LOOP, TurnRequest};

    for loop_kind in ["loop-react", "loop-plan"] {
        let dir = TempDir::new(&format!("tool-started-{loop_kind}"));
        let mut distribution = headless(&dir);
        for spec in &mut distribution.plugin {
            if spec.kind == "loop-react" {
                spec.kind = loop_kind.into();
            }
        }
        distribution.set_config("policy-authority", "decisions", json!({"gate": "allow"}));
        let kernel = kernel(&distribution).await;
        let open = std::sync::Arc::new(tokio::sync::Notify::new());
        let gate = GateTool {
            manifest: rah_api::PluginManifest::new("gate", "1").provides(["tools"]),
            open: open.clone(),
        };
        kernel
            .mount(std::sync::Arc::new(gate), rah_api::MountOptions::default())
            .await
            .unwrap();
        let handle = kernel.handle();
        let session = new_session(&handle).await;
        let (mut events, steering) = handle.get(LOOP).unwrap().run_steerable_turn(TurnRequest {
            origin: rah_api::agent_loop::TurnOrigin::User,
            session: session.clone(),
            input: "/call gate {}".into(),
            cancel: rah_api::CancellationToken::new(),
        });
        let steering = steering.unwrap();
        let other = dir.0.join("other");
        std::fs::create_dir(&other).unwrap();
        let admin = handle.get(ADMIN).unwrap();
        let mut seen = Vec::new();
        let mut receipt = None;
        let run = async {
            while let Some(event) = events.next().await {
                match &event {
                    AgentEvent::ToolStarted { .. } => {
                        assert!(admin.set_workspace(&other).await.is_err());
                        let mut pending = steering.send("/notool revised direction").unwrap();
                        assert_eq!(
                            pending.try_recv(),
                            Err(tokio::sync::oneshot::error::TryRecvError::Empty)
                        );
                        receipt = Some(pending);
                        open.notify_one();
                    }
                    AgentEvent::ToolFinished { output, .. } => {
                        assert!(!output.is_error, "{output:?}");
                    }
                    _ => {}
                }
                seen.push(event);
            }
        };
        tokio::time::timeout(std::time::Duration::from_secs(10), run)
            .await
            .unwrap_or_else(|_| panic!("{loop_kind}: no ToolStarted while the tool ran"));
        let kinds = kinds(&seen);
        assert!(kinds.contains(&"tool-started"), "{loop_kind}: {kinds:?}");
        assert_eq!(kinds.last(), Some(&"completed"), "{loop_kind}: {kinds:?}");
        receipt.unwrap().await.unwrap();
        assert!(
            answer(&seen).contains("you said: revised direction"),
            "{loop_kind}"
        );
        let records = handle.get(SESSION).unwrap().read(&session).await.unwrap();
        let SessionEntry::TurnStarted { turn: turn_id, .. } = &records[0].entry else {
            panic!("no turn start")
        };
        let guidance = records.iter().position(|record| matches!(&record.entry, SessionEntry::UserMessage { turn, input } if turn == turn_id && input == "/notool revised direction")).unwrap();
        assert!(matches!(
            records[guidance - 1].entry,
            SessionEntry::ToolResult { .. }
        ));
        let context = rah_api::session::rebuild_context(&records);
        assert!(context.iter().any(|item| matches!(item, rah_protocol::ContextItem::UserGuidance { text } if text == "/notool revised direction")));
        assert!(matches!(
            steering.send("too late"),
            Err(rah_api::agent_loop::SteeringError::Closed)
        ));
        admin.set_workspace(&other).await.unwrap();
        let before = entries(&handle, &session).await;
        let refused = turn(&handle, &session, "/call fs.read {}").await;
        assert!(
            matches!(&refused[..], [AgentEvent::Failed { message, .. }] if message.contains("workspace"))
        );
        assert_eq!(entries(&handle, &session).await, before);
        admin.set_workspace(&dir.workspace()).await.unwrap();
        assert!(matches!(
            turn(&handle, &session, "hello").await.last(),
            Some(AgentEvent::Completed { .. })
        ));
    }
}

/// Holds the first request so guidance arrives before a text-only reply.
struct RequestGate {
    manifest: rah_api::PluginManifest,
    ready: std::sync::Arc<tokio::sync::Notify>,
    release: std::sync::Arc<tokio::sync::Notify>,
    requests: std::sync::Arc<std::sync::Mutex<Vec<rah_protocol::ModelRequest>>>,
}

#[rah_api::async_trait]
impl rah_api::Plugin for RequestGate {
    fn manifest(&self) -> &rah_api::PluginManifest {
        &self.manifest
    }

    async fn apply(&self, scope: &mut rah_api::Scope) -> Result<(), rah_api::PluginError> {
        let first = std::sync::Mutex::new(Some((self.ready.clone(), self.release.clone())));
        let requests = self.requests.clone();
        scope.hook_waterfall::<rah_api::hooks::ModelBeforeRequest, _, _>(-100, move |request| {
            let first = first.lock().unwrap().take();
            requests.lock().unwrap().push(request.clone());
            async move {
                if let Some((ready, release)) = first {
                    ready.notify_one();
                    release.notified().await;
                }
                request
            }
        });
        Ok(())
    }
}

#[tokio::test]
async fn steering_continues_a_turn_even_when_the_current_reply_has_no_tools() {
    use futures::StreamExt;
    use rah_api::agent_loop::{LOOP, TurnOrigin, TurnRequest};

    let dir = TempDir::new("steer-text-only");
    let kernel = kernel(&headless(&dir)).await;
    let ready = std::sync::Arc::new(tokio::sync::Notify::new());
    let release = std::sync::Arc::new(tokio::sync::Notify::new());
    kernel
        .mount(
            std::sync::Arc::new(RequestGate {
                manifest: rah_api::PluginManifest::new("request-gate", "1"),
                ready: ready.clone(),
                release: release.clone(),
                requests: Default::default(),
            }),
            rah_api::MountOptions::default(),
        )
        .await
        .unwrap();
    let handle = kernel.handle();
    let session = new_session(&handle).await;
    let (events, steering) = handle.get(LOOP).unwrap().run_steerable_turn(TurnRequest {
        origin: TurnOrigin::User,
        session: session.clone(),
        input: "/notool first".into(),
        cancel: rah_api::CancellationToken::new(),
    });
    let steering = steering.unwrap();
    let events: Vec<AgentEvent> = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        ready.notified().await;
        let receipt = steering.send("/notool second").unwrap();
        release.notify_one();
        receipt.await.unwrap();
        events.collect().await
    })
    .await
    .unwrap();
    let text = answer(&events);
    assert!(
        text.contains("you said: first") && text.contains("you said: second"),
        "{text}"
    );
    assert_eq!(
        entries(&handle, &session).await,
        [
            "turn-started",
            "assistant",
            "user-message",
            "assistant",
            "turn-completed"
        ]
    );
}

#[tokio::test]
async fn steering_during_planning_keeps_the_plan_in_the_execution_request() {
    use futures::StreamExt;
    use rah_api::agent_loop::{LOOP, TurnOrigin, TurnRequest};
    use rah_protocol::ContextItem;

    let dir = TempDir::new("steer-during-planning");
    let mut distribution = headless(&dir);
    let spec = distribution
        .plugin
        .iter_mut()
        .find(|spec| spec.kind == "loop-react")
        .unwrap();
    spec.kind = "loop-plan".into();
    let kernel = kernel(&distribution).await;
    let ready = std::sync::Arc::new(tokio::sync::Notify::new());
    let release = std::sync::Arc::new(tokio::sync::Notify::new());
    let requests = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    kernel
        .mount(
            std::sync::Arc::new(RequestGate {
                manifest: rah_api::PluginManifest::new("request-gate", "1"),
                ready: ready.clone(),
                release: release.clone(),
                requests: requests.clone(),
            }),
            rah_api::MountOptions::default(),
        )
        .await
        .unwrap();
    let handle = kernel.handle();
    let session = new_session(&handle).await;
    let (events, steering) = handle.get(LOOP).unwrap().run_steerable_turn(TurnRequest {
        origin: TurnOrigin::User,
        session,
        input: "/notool original plan marker".into(),
        cancel: rah_api::CancellationToken::new(),
    });
    let events: Vec<_> = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        ready.notified().await;
        let receipt = steering.unwrap().send("/notool revised direction").unwrap();
        release.notify_one();
        receipt.await.unwrap();
        events.collect().await
    })
    .await
    .unwrap();
    assert!(answer(&events).contains("you said: revised direction"));
    let requests = requests.lock().unwrap();
    assert!(requests[1].items.iter().any(|item| matches!(
        item, ContextItem::System { text } if text.contains("Earlier plan:\n[model-mock] you said: original plan marker")
    )));
}

/// Inject failures on either side of the durable TurnStarted write.
#[derive(Clone, Copy, PartialEq, Eq)]
enum StartFault {
    None,
    WrongWorkspace,
    BeforeAppend,
    AfterAppend,
}

struct ControlledLog {
    manifest: rah_api::PluginManifest,
    log: std::sync::Arc<dyn rah_api::session::SessionLog>,
}

#[rah_api::async_trait]
impl rah_api::Plugin for ControlledLog {
    fn manifest(&self) -> &rah_api::PluginManifest {
        &self.manifest
    }
    async fn apply(&self, scope: &mut rah_api::Scope) -> Result<(), rah_api::PluginError> {
        scope.provide(rah_api::session::SESSION, self.log.clone())?;
        Ok(())
    }
}

struct FaultyStarts {
    inner: std::sync::Arc<dyn rah_api::session::SessionLog>,
    fault: std::sync::Arc<std::sync::Mutex<StartFault>>,
}
use rah_api::session::{SESSION, SessionError, SessionLog};
use rah_protocol::{SessionEntry, SessionId, SessionInfo, SessionRecord};
#[rah_api::async_trait]
impl SessionLog for FaultyStarts {
    async fn create(&self) -> Result<SessionInfo, SessionError> {
        self.inner.create().await
    }
    async fn read(&self, id: &SessionId) -> Result<Vec<SessionRecord>, SessionError> {
        self.inner.read(id).await
    }
    async fn fork(&self, id: &SessionId, at: u64) -> Result<SessionInfo, SessionError> {
        self.inner.fork(id, at).await
    }
    async fn list(&self) -> Result<Vec<SessionInfo>, SessionError> {
        self.inner.list().await
    }
    async fn turn_history(
        &self,
        id: &SessionId,
    ) -> Result<(Vec<SessionRecord>, Option<String>), SessionError> {
        let (records, mut root) = self.inner.turn_history(id).await?;
        if *self.fault.lock().unwrap() == StartFault::WrongWorkspace {
            root = Some("inactive-workspace".into());
        }
        Ok((records, root))
    }
    async fn append(&self, id: &SessionId, entry: SessionEntry) -> Result<u64, SessionError> {
        let fault = if matches!(entry, SessionEntry::TurnStarted { .. }) {
            *self.fault.lock().unwrap()
        } else {
            StartFault::None
        };
        if fault == StartFault::BeforeAppend {
            return Err(SessionError::Io("injected before append".into()));
        }
        let seq = self.inner.append(id, entry).await?;
        if fault == StartFault::AfterAppend {
            return Err(SessionError::Io("injected after append".into()));
        }
        Ok(seq)
    }
}

#[tokio::test]
async fn user_turns_record_undo_notes_once_across_admission_and_append_failures() {
    use futures::StreamExt;
    use rah_api::agent_loop::{LOOP, TurnOrigin, TurnRequest};
    use std::sync::{Arc, Mutex};
    for kind in ["loop-react", "loop-plan"] {
        let dir = TempDir::new(&format!("notes-{kind}"));
        let mut distribution = headless(&dir);
        distribution.set_config("policy-authority", "approve", json!(["repo.create-file"]));
        let kernel = kernel(&distribution).await;
        let handle = kernel.handle();
        let inner = handle.get(SESSION).unwrap();
        let fault = Arc::new(Mutex::new(StartFault::None));
        kernel
            .mount(
                Arc::new(ControlledLog {
                    manifest: rah_api::PluginManifest::new("controlled-log", "1")
                        .provides(["session"]),
                    log: Arc::new(FaultyStarts {
                        inner: inner.clone(),
                        fault: fault.clone(),
                    }),
                }),
                Default::default(),
            )
            .await
            .unwrap();
        handle
            .get(ADMIN)
            .unwrap()
            .mount(MountRequest {
                kind: kind.into(),
                id: Some("notes-loop".into()),
                bind: [("session".into(), "controlled-log".into())].into(),
                ..Default::default()
            })
            .await
            .unwrap();
        let agent = handle.get_instance(LOOP, "notes-loop").unwrap();
        for failure in [StartFault::BeforeAppend, StartFault::AfterAppend] {
            let sid = new_session(&handle).await;
            turn(
                &handle,
                &sid,
                r#"/call repo.create-file {"path":"note.txt","content":"note"}"#,
            )
            .await;
            rah_api::undo::undo_newest(&handle, &sid)
                .await
                .unwrap()
                .unwrap();
            let start = |origin, input: &str| {
                agent.run_turn(TurnRequest {
                    origin,
                    session: sid.clone(),
                    input: input.into(),
                    cancel: Default::default(),
                })
            };
            let background = start(TurnOrigin::Background, "background");
            let busy: Vec<_> = start(TurnOrigin::User, "busy").collect().await;
            assert!(matches!(busy.as_slice(), [AgentEvent::Failed { .. }]));
            background.collect::<Vec<_>>().await;
            *fault.lock().unwrap() = StartFault::WrongWorkspace;
            let refused = start(TurnOrigin::User, "wrong workspace")
                .collect::<Vec<_>>()
                .await;
            assert!(matches!(refused.last(), Some(AgentEvent::Failed { .. })));
            *fault.lock().unwrap() = failure;
            let failed = start(TurnOrigin::User, "first").collect::<Vec<_>>().await;
            assert!(matches!(failed.last(), Some(AgentEvent::Failed { .. })));
            *fault.lock().unwrap() = StartFault::None;
            assert!(matches!(
                start(TurnOrigin::User, "retry")
                    .collect::<Vec<_>>()
                    .await
                    .last(),
                Some(AgentEvent::Completed { .. })
            ));
            start(TurnOrigin::User, "later").collect::<Vec<_>>().await;
            let inputs: Vec<_> = inner
                .read(&sid)
                .await
                .unwrap()
                .into_iter()
                .filter_map(|r| match r.entry {
                    SessionEntry::TurnStarted { input, .. }
                        if input.contains("[RAH: the user undid") =>
                    {
                        Some(input)
                    }
                    _ => None,
                })
                .collect();
            assert_eq!(inputs.len(), 1, "{kind}: {inputs:?}");
            let expected = if failure == StartFault::BeforeAppend {
                "retry"
            } else {
                "first"
            };
            assert!(inputs[0].ends_with(expected), "{kind}: {inputs:?}");
        }
    }
}
