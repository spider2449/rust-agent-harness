use std::sync::Arc;

use rah_api::{Kernel, MountOptions};
use rah_protocol::{
    AgentOutput, ApprovalId, ApprovalRequest, Message, MessageRole, RequestId, ToolCall, ToolInput,
    ToolName, ToolOutput,
};
use serde_json::json;

use super::*;

struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("rah-telemetry-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        Self(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn usage(input: u64, output: u64, cached: Option<u64>) -> ModelUsage {
    ModelUsage {
        input_tokens: Some(input),
        output_tokens: Some(output),
        cached_input_tokens: cached,
    }
}

fn tool_call(id: &ToolCallId, name: &str) -> ToolCall {
    ToolCall {
        id: id.clone(),
        name: ToolName::new(name),
        input: ToolInput(json!({ "path": "secret.txt" })),
    }
}

fn finished(session: &SessionId, id: &ToolCallId, is_error: bool) -> AgentEvent {
    AgentEvent::ToolFinished {
        session_id: session.clone(),
        tool_call_id: id.clone(),
        output: ToolOutput {
            content: Vec::new(),
            is_error,
        },
    }
}

/// A turn with two model requests and four tool calls, one per decision.
fn turn_events(session: &SessionId) -> Vec<AgentEvent> {
    let first = ModelRequestId::new();
    let second = ModelRequestId::new();
    let [allowed, approved, denied, rejected] = std::array::from_fn(|_| ToolCallId::new());
    let mut events = vec![
        AgentEvent::Started {
            session_id: session.clone(),
            request_id: RequestId::new(),
        },
        AgentEvent::ModelRequestStarted {
            session_id: session.clone(),
            model_request_id: first.clone(),
        },
        AgentEvent::ModelReasoningStarted {
            session_id: session.clone(),
            model_request_id: first.clone(),
        },
        AgentEvent::ModelUsage {
            session_id: session.clone(),
            model_request_id: first,
            usage: usage(100, 10, Some(40)),
        },
    ];
    for (id, name) in [
        (&allowed, "fs.read"),
        (&approved, "repo.patch"),
        (&denied, "shell.exec"),
        (&rejected, "repo.create-file"),
    ] {
        events.push(AgentEvent::ToolRequested {
            session_id: session.clone(),
            tool_call: tool_call(id, name),
        });
    }
    events.extend([
        AgentEvent::ToolStarted {
            session_id: session.clone(),
            tool_call_id: allowed.clone(),
        },
        finished(session, &allowed, false),
        AgentEvent::ApprovalRequired {
            session_id: session.clone(),
            request: ApprovalRequest {
                id: ApprovalId::new(),
                tool_call: tool_call(&approved, "repo.patch"),
                reason: "write".into(),
            },
        },
        AgentEvent::ToolStarted {
            session_id: session.clone(),
            tool_call_id: approved.clone(),
        },
        finished(session, &approved, false),
        finished(session, &denied, true),
        AgentEvent::ApprovalRequired {
            session_id: session.clone(),
            request: ApprovalRequest {
                id: ApprovalId::new(),
                tool_call: tool_call(&rejected, "repo.create-file"),
                reason: "write".into(),
            },
        },
        finished(session, &rejected, true),
        AgentEvent::ModelRequestStarted {
            session_id: session.clone(),
            model_request_id: second.clone(),
        },
        AgentEvent::ModelDelta {
            session_id: session.clone(),
            model_request_id: second.clone(),
            delta: "private answer text".into(),
        },
        AgentEvent::ModelUsage {
            session_id: session.clone(),
            model_request_id: second,
            usage: ModelUsage {
                input_tokens: Some(150),
                output_tokens: Some(20),
                cached_input_tokens: None,
            },
        },
        AgentEvent::Completed {
            session_id: session.clone(),
            output: AgentOutput {
                message: Message {
                    role: MessageRole::Assistant,
                    content: "private answer text".into(),
                },
            },
        },
    ]);
    events
}

fn track(include_arguments: bool, events: &[AgentEvent]) -> Vec<Record> {
    let (sender, mut receiver) = mpsc::unbounded_channel();
    let mut tracker = Tracker::new(include_arguments, sender);
    let start = Instant::now();
    for (index, event) in events.iter().enumerate() {
        tracker.observe(event, start + Duration::from_millis(10 * index as u64));
    }
    let mut records = Vec::new();
    while let Ok(record) = receiver.try_recv() {
        records.push(record);
    }
    records
}

fn json_of(records: &[Record]) -> Vec<Value> {
    records
        .iter()
        .map(|record| serde_json::to_value(record).unwrap())
        .collect()
}

#[test]
fn a_turn_yields_model_tool_and_turn_records() {
    let session = SessionId::new();
    let records = json_of(&track(false, &turn_events(&session)));
    let kinds: Vec<&str> = records
        .iter()
        .map(|r| r["kind"].as_str().unwrap())
        .collect();
    assert_eq!(
        kinds,
        [
            "model_request",
            "tool",
            "tool",
            "tool",
            "tool",
            "model_request",
            "turn"
        ]
    );
    let first = &records[0];
    assert_eq!(first["reasoning"], true);
    assert_eq!(first["input_tokens"], 100);
    assert_eq!(first["cached_input_tokens"], 40);
    assert_eq!(first["duration_ms"], 30);
    assert!(first.get("first_text_ms").is_none());

    let decisions: Vec<(&str, &str)> = records[1..5]
        .iter()
        .map(|r| (r["tool"].as_str().unwrap(), r["decision"].as_str().unwrap()))
        .collect();
    assert_eq!(
        decisions,
        [
            ("fs.read", "allowed"),
            ("repo.patch", "approved"),
            ("shell.exec", "denied"),
            ("repo.create-file", "rejected"),
        ]
    );
    assert_eq!(records[1]["duration_ms"], 10);
    assert!(records.iter().all(|r| r.get("arguments").is_none()));

    let second = &records[5];
    assert_eq!(second["first_text_ms"], 10);
    assert_eq!(second["reasoning"], false);
    assert!(second.get("cached_input_tokens").is_none());

    let turn = &records[6];
    assert_eq!(turn["outcome"], "completed");
    assert_eq!(turn["model_requests"], 2);
    assert_eq!(turn["tool_calls"], 4);
    assert_eq!(turn["input_tokens"], 250);
    assert_eq!(turn["output_tokens"], 30);
    assert_eq!(turn["cached_input_tokens"], 40);
    assert_eq!(
        turn["duration_ms"],
        10 * (turn_events(&session).len() as u64 - 1)
    );
    // Message text never reaches a record.
    let text = serde_json::to_string(&records).unwrap();
    assert!(!text.contains("private answer text"), "{text}");
    assert!(!text.contains("secret.txt"), "{text}");
}

#[test]
fn arguments_are_recorded_only_when_configured() {
    let session = SessionId::new();
    let records = json_of(&track(true, &turn_events(&session)));
    assert_eq!(records[1]["arguments"], "{\"path\":\"secret.txt\"}");
}

#[test]
fn failed_and_cancelled_turns_are_recorded() {
    let session = SessionId::new();
    let model = ModelRequestId::new();
    let events = [
        AgentEvent::Started {
            session_id: session.clone(),
            request_id: RequestId::new(),
        },
        AgentEvent::ModelRequestStarted {
            session_id: session.clone(),
            model_request_id: model,
        },
        AgentEvent::Failed {
            session_id: session.clone(),
            code: AgentErrorCode::Model,
            message: "x".repeat(800),
        },
        AgentEvent::Started {
            session_id: session.clone(),
            request_id: RequestId::new(),
        },
        AgentEvent::Cancelled {
            session_id: session.clone(),
        },
        // Events of a turn that was never started are ignored.
        AgentEvent::Cancelled {
            session_id: SessionId::new(),
        },
    ];
    let records = json_of(&track(false, &events));
    let kinds: Vec<&str> = records
        .iter()
        .map(|r| r["kind"].as_str().unwrap())
        .collect();
    assert_eq!(kinds, ["model_request", "turn", "turn"]);
    assert_eq!(records[1]["outcome"], "failed");
    assert_eq!(records[1]["error_code"], "model");
    assert_eq!(records[1]["error"].as_str().unwrap().chars().count(), 501);
    assert!(records[1].get("input_tokens").is_none());
    assert_eq!(records[2]["outcome"], "cancelled");
    assert_eq!(records[2]["model_requests"], 0);
}

#[test]
fn timestamps_and_days_are_utc_civil_dates() {
    assert_eq!(timestamp(UNIX_EPOCH), "1970-01-01T00:00:00.000Z");
    let leap = UNIX_EPOCH + Duration::from_millis(951_782_400_123);
    assert_eq!(timestamp(leap), "2000-02-29T00:00:00.123Z");
    let later = UNIX_EPOCH + Duration::from_secs(1_791_849_600 + 3_723);
    assert_eq!(timestamp(later), "2026-10-13T01:02:03.000Z");
    assert_eq!(days_of("1970-01-01"), Some(0));
    assert_eq!(days_of("2000-02-29.jsonl"), Some(11_016));
    assert_eq!(days_of("2026-13-01"), None);
    assert!(is_record_file("2026-10-13.jsonl"));
    assert!(is_record_file("2026-10-13.2.jsonl"));
    assert!(!is_record_file("2026-10-13.x.jsonl"));
    assert!(!is_record_file("notes.jsonl"));
    assert!(!is_record_file("2026-10-13.json"));
}

fn record(ts: &str) -> Record {
    Record::Turn {
        ts: ts.into(),
        session: SessionId::new(),
        request: "r".into(),
        outcome: "completed",
        error_code: None,
        error: None,
        duration_ms: 1,
        model_requests: 0,
        tool_calls: 0,
        tokens: Tokens::default(),
    }
}

#[tokio::test]
async fn files_roll_over_by_size_and_old_days_are_deleted() {
    let dir = TempDir::new("files");
    std::fs::create_dir_all(&dir.0).unwrap();
    for old in ["2026-09-01.jsonl", "2026-09-01.1.jsonl", "2026-10-12.jsonl"] {
        std::fs::write(dir.0.join(old), "{}\n").unwrap();
    }
    std::fs::write(dir.0.join("keep-me.txt"), "mine").unwrap();
    let line = serde_json::to_vec(&record("2026-10-13T00:00:00.000Z"))
        .unwrap()
        .len() as u64
        + 1;
    let mut writer = Writer {
        dir: dir.0.clone(),
        max_file_bytes: line * 2,
        keep_days: 14,
        open: None,
    };
    for _ in 0..5 {
        writer.write(&record("2026-10-13T00:00:00.000Z")).await;
    }
    let mut names: Vec<String> = std::fs::read_dir(&dir.0)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(
        names,
        [
            "2026-10-12.jsonl",
            "2026-10-13.1.jsonl",
            "2026-10-13.2.jsonl",
            "2026-10-13.jsonl",
            "keep-me.txt"
        ]
    );
    assert_eq!(read_records(&dir.0.join("2026-10-13.jsonl")).len(), 2);
    assert_eq!(read_records(&dir.0.join("2026-10-13.2.jsonl")).len(), 1);

    // The next day starts a new file and prunes again.
    writer.write(&record("2026-10-26T00:00:00.000Z")).await;
    assert!(!dir.0.join("2026-10-12.jsonl").exists());
    assert!(dir.0.join("2026-10-13.jsonl").exists());
    assert!(dir.0.join("2026-10-26.jsonl").exists());
}

#[tokio::test]
async fn the_plugin_records_turn_events_from_the_bus() {
    let dir = TempDir::new("plugin");
    let kernel = Kernel::new();
    kernel
        .mount(
            Arc::new(TelemetryPlugin::new("telemetry")),
            MountOptions::with_config(json!({ "dir": dir.0 })),
        )
        .await
        .unwrap();
    let session = SessionId::new();
    for event in turn_events(&session) {
        kernel.handle().emit(&TurnEvent { event });
    }
    let file = dir
        .0
        .join(format!("{}.jsonl", day_of(&timestamp(SystemTime::now()))));
    let mut records = Vec::new();
    for _ in 0..200 {
        records = read_records(&file);
        if records.len() == 7 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(records.len(), 7, "{records:?}");
    assert_eq!(records[6]["kind"], "turn");
    assert_eq!(records[6]["session"], json!(session));
}

/// The labelled values and the tool table of a usage view.
fn usage_of(view: &rah_api::ui::View) -> (Vec<(String, String)>, Vec<Vec<String>>) {
    use rah_api::ui::Node;
    let mut values = Vec::new();
    let mut rows = Vec::new();
    for node in &view.nodes {
        match node {
            Node::KeyValue { items } => {
                values.extend(items.iter().map(|kv| (kv.key.clone(), kv.value.clone())));
            }
            Node::Table { rows: table, .. } => rows.clone_from(table),
            _ => {}
        }
    }
    (values, rows)
}

fn value<'a>(values: &'a [(String, String)], key: &str) -> &'a str {
    &values.iter().find(|(k, _)| k == key).unwrap().1
}

#[test]
fn usage_sums_the_records_of_a_turn() {
    let session = SessionId::new();
    let mut totals = usage::Totals::new();
    for record in track(true, &turn_events(&session)) {
        totals.add(&record);
    }
    let views = usage::UsageViews(Arc::new(Mutex::new(totals)));
    let view = futures_render(&views);
    assert!(view.check().is_ok());
    let (values, rows) = usage_of(&view);
    assert_eq!(
        value(&values, "Turns"),
        "1 (1 completed, 0 failed, 0 cancelled)"
    );
    assert_eq!(value(&values, "Model requests"), "2 (0 retries)");
    assert_eq!(value(&values, "Input tokens"), "250");
    assert_eq!(value(&values, "Cached input tokens"), "40");
    assert_eq!(value(&values, "Output tokens"), "30");
    assert_eq!(value(&values, "Tool calls"), "4");
    let row = |cells: [&str; 4]| cells.map(str::to_owned).to_vec();
    assert_eq!(
        rows,
        [
            row(["fs.read", "1", "0", "0"]),
            row(["repo.create-file", "1", "1", "1"]),
            row(["repo.patch", "1", "0", "0"]),
            row(["shell.exec", "1", "1", "1"]),
        ]
    );
    // No argument, message or output text reaches the view.
    let text = serde_json::to_string(&view).unwrap();
    assert!(
        !text.contains("secret.txt") && !text.contains("private answer"),
        "{text}"
    );
}

fn futures_render(views: &usage::UsageViews) -> rah_api::ui::View {
    use rah_api::ui::{UiViews, ViewContext};
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(views.render(usage::VIEW, &ViewContext::default()))
        .unwrap()
}

#[tokio::test]
async fn the_plugin_shows_usage_as_a_view_and_resets_it() {
    use rah_api::ui::{UI_VIEWS, UiError, ViewContext, ViewSpec};
    let dir = TempDir::new("usage-view");
    let kernel = Kernel::new();
    kernel
        .mount(
            Arc::new(TelemetryPlugin::new("telemetry")),
            MountOptions::with_config(json!({ "dir": dir.0 })),
        )
        .await
        .unwrap();
    let all = kernel.handle().get_all(UI_VIEWS);
    assert_eq!(all.len(), 1);
    let (instance, views) = &all[0];
    assert_eq!(instance, "telemetry");
    assert_eq!(views.views(), [ViewSpec::new("usage", "Usage")]);

    let session = SessionId::new();
    for event in turn_events(&session) {
        kernel.handle().emit(&TurnEvent { event });
    }
    let cx = ViewContext::default();
    let mut turns = String::new();
    for _ in 0..200 {
        let view = views.render("usage", &cx).await.unwrap();
        turns = value(&usage_of(&view).0, "Turns").to_owned();
        if turns.starts_with('1') {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(turns, "1 (1 completed, 0 failed, 0 cancelled)");

    let outcome = views.act("usage", "reset", Value::Null, &cx).await.unwrap();
    assert!(outcome.rerender);
    let view = views.render("usage", &cx).await.unwrap();
    let (values, rows) = usage_of(&view);
    assert_eq!(
        value(&values, "Turns"),
        "0 (0 completed, 0 failed, 0 cancelled)"
    );
    assert_eq!(value(&values, "Input tokens"), "not reported");
    assert!(rows.is_empty());
    assert_eq!(
        views.act("usage", "nope", Value::Null, &cx).await,
        Err(UiError::UnknownAction("nope".into()))
    );
    assert_eq!(
        views.render("other", &cx).await,
        Err(UiError::UnknownView("other".into()))
    );
}

#[tokio::test]
async fn written_records_announce_that_the_usage_view_changed() {
    use rah_api::ui::UiChanged;
    let dir = TempDir::new("usage-changed");
    let kernel = Kernel::new();
    kernel
        .mount(
            Arc::new(TelemetryPlugin::new("telemetry")),
            MountOptions::with_config(json!({ "dir": dir.0 })),
        )
        .await
        .unwrap();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let _subscription = {
        let seen = seen.clone();
        kernel
            .handle()
            .subscribe(move |changed: &UiChanged| seen.lock().unwrap().push(changed.clone()))
    };
    let session = SessionId::new();
    for event in turn_events(&session) {
        kernel.handle().emit(&TurnEvent { event });
    }
    for _ in 0..200 {
        if !seen.lock().unwrap().is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let seen = seen.lock().unwrap();
    assert!(!seen.is_empty());
    // Coalesced: at most one per record, usually one per drained queue.
    assert!(seen.len() <= 7, "{seen:?}");
    assert!(seen.iter().all(|changed| *changed
        == UiChanged {
            instance: "telemetry".into(),
            view: "usage".into(),
        }));
}
