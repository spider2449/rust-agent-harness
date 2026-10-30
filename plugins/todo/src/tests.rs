use rah_api::{
    CancellationToken, Kernel, MountOptions,
    session::{SessionError, SessionLog},
    tools::{Invocation, InvocationOutcome, ToolInvoker},
};
use rah_protocol::{RequestId, SessionInfo, SessionRecord, ToolCall, ToolCallId, ToolInput};

use super::*;

fn item(content: &str, status: Status) -> Item {
    Item {
        content: content.to_owned(),
        status,
    }
}

#[test]
fn write_input_is_checked_and_cleaned() {
    let items = parse_write(json!({ "todos": [
        { "content": "  read   the\n code ", "status": "completed" },
        { "content": "fix it", "status": "in_progress" },
        { "content": "test it", "status": "pending" },
    ]}))
    .unwrap();
    assert_eq!(
        items,
        [
            item("read the code", Status::Completed),
            item("fix it", Status::InProgress),
            item("test it", Status::Pending),
        ]
    );
    assert_eq!(parse_write(json!({ "todos": [] })).unwrap(), []);

    let too_many: Vec<Value> = (0..=MAX_ITEMS)
        .map(|n| json!({ "content": format!("step {n}"), "status": "pending" }))
        .collect();
    for bad in [
        json!({}),
        json!({ "todos": "a" }),
        json!({ "todos": [], "extra": 1 }),
        json!({ "todos": [{ "content": "a", "status": "done" }] }),
        json!({ "todos": [{ "content": "a", "status": "pending", "id": 1 }] }),
        json!({ "todos": [{ "content": " \n ", "status": "pending" }] }),
        json!({ "todos": [{ "content": "x".repeat(MAX_ITEM_CHARS + 1), "status": "pending" }] }),
        json!({ "todos": too_many }),
    ] {
        assert!(parse_write(bad.clone()).is_err(), "{bad}");
    }
}

/// A fixed log for one session; other sessions are not found.
struct Sessions(SessionId, Vec<SessionEntry>);

#[async_trait]
impl SessionLog for Sessions {
    async fn create(&self) -> Result<SessionInfo, SessionError> {
        unreachable!()
    }

    async fn append(&self, _: &SessionId, _: SessionEntry) -> Result<u64, SessionError> {
        unreachable!()
    }

    async fn read(&self, session: &SessionId) -> Result<Vec<SessionRecord>, SessionError> {
        if *session != self.0 {
            return Err(SessionError::NotFound(session.clone()));
        }
        Ok(self
            .1
            .iter()
            .zip(1..)
            .map(|(entry, seq)| SessionRecord {
                seq,
                entry: entry.clone(),
            })
            .collect())
    }

    async fn fork(&self, _: &SessionId, _: u64) -> Result<SessionInfo, SessionError> {
        unreachable!()
    }

    async fn list(&self) -> Result<Vec<SessionInfo>, SessionError> {
        Ok(Vec::new())
    }
}

struct SessionPlugin(PluginManifest, Arc<Sessions>);

#[async_trait]
impl Plugin for SessionPlugin {
    fn manifest(&self) -> &PluginManifest {
        &self.0
    }

    async fn apply(&self, scope: &mut Scope) -> Result<(), PluginError> {
        scope.provide(SESSION, self.1.clone() as Arc<dyn SessionLog>)?;
        Ok(())
    }
}

async fn mount_todo(kernel: &Kernel) {
    kernel
        .mount(Arc::new(TodoPlugin::new("todo")), MountOptions::default())
        .await
        .unwrap();
}

async fn invoke(kernel: &Kernel, session: &SessionId, name: &str, input: Value) -> ToolOutput {
    invoke_outcome(kernel, session, name, input).await.output
}

async fn invoke_outcome(
    kernel: &Kernel,
    session: &SessionId,
    name: &str,
    input: Value,
) -> InvocationOutcome {
    ToolInvoker::new(kernel.handle())
        .invoke(
            Invocation {
                session: session.clone(),
                call: ToolCall {
                    id: ToolCallId::new(),
                    name: ToolName::new(name),
                    input: ToolInput(input),
                },
                cancel: CancellationToken::new(),
            },
            || {},
        )
        .await
}

fn output_text(output: &ToolOutput) -> &str {
    match output.content.as_slice() {
        [ToolContent::Text(text)] => text,
        other => panic!("unexpected output {other:?}"),
    }
}

async fn render(kernel: &Kernel, session: Option<&SessionId>) -> View {
    let all = kernel.handle().get_all(UI_VIEWS);
    let (_, views) = all.iter().find(|(id, _)| id == "todo").unwrap();
    let view = views
        .render(
            VIEW,
            &ViewContext {
                session: session.cloned(),
            },
        )
        .await
        .unwrap();
    assert!(view.check().is_ok());
    view
}

#[tokio::test]
async fn the_model_writes_and_reads_a_list_the_view_shows() {
    let kernel = Kernel::new();
    mount_todo(&kernel).await;
    let names: Vec<_> = ToolInvoker::new(kernel.handle())
        .definitions()
        .into_iter()
        .map(|definition| (definition.name.as_str().to_owned(), definition.permission))
        .collect();
    assert_eq!(
        names,
        [
            (WRITE_TOOL.to_owned(), PermissionLevel::None),
            (READ_TOOL.to_owned(), PermissionLevel::None),
        ]
    );
    let changed = Arc::new(Mutex::new(Vec::new()));
    let _subscription = {
        let changed = changed.clone();
        kernel
            .handle()
            .subscribe(move |event: &UiChanged| changed.lock().unwrap().push(event.clone()))
    };

    let session = SessionId::new();
    let other = SessionId::new();
    let output = invoke(
        &kernel,
        &session,
        WRITE_TOOL,
        json!({ "todos": [
            { "content": "read", "status": "completed" },
            { "content": "fix", "status": "in_progress" },
        ]}),
    )
    .await;
    assert!(!output.is_error);
    assert_eq!(
        output_text(&output),
        "To-do list updated (1 of 2 completed):\n[x] read\n[>] fix"
    );
    assert_eq!(
        *changed.lock().unwrap(),
        [UiChanged {
            instance: "todo".into(),
            view: VIEW.into(),
        }]
    );

    let read = invoke(&kernel, &session, READ_TOOL, json!({})).await;
    assert_eq!(
        output_text(&read),
        "To-do list (1 of 2 completed):\n[x] read\n[>] fix"
    );
    let empty = invoke(&kernel, &other, READ_TOOL, json!({})).await;
    assert_eq!(output_text(&empty), "To-do list: the list is empty.");

    assert_eq!(
        render(&kernel, Some(&session)).await.nodes,
        [
            Node::Text {
                text: "1 of 2 completed.".into(),
            },
            Node::List {
                items: vec!["[x] read".into(), "[>] fix".into()],
            },
        ]
    );
    assert!(matches!(
        render(&kernel, Some(&other)).await.nodes.as_slice(),
        [Node::Text { text }] if text.starts_with("No to-do items")
    ));
    assert!(matches!(
        render(&kernel, None).await.nodes.as_slice(),
        [Node::Text { text }] if text.starts_with("Open a session")
    ));

    // A refused write changes nothing.
    let refused = invoke(
        &kernel,
        &session,
        WRITE_TOOL,
        json!({ "todos": [{ "content": "", "status": "pending" }] }),
    )
    .await;
    assert!(refused.is_error);
    let read = invoke(&kernel, &session, READ_TOOL, json!({})).await;
    assert!(output_text(&read).contains("[>] fix"));
    assert_eq!(changed.lock().unwrap().len(), 1);

    // An empty list clears it.
    let cleared = invoke(&kernel, &session, WRITE_TOOL, json!({ "todos": [] })).await;
    assert_eq!(
        output_text(&cleared),
        "To-do list updated: the list is empty."
    );
}

#[tokio::test]
async fn unknown_views_are_refused() {
    let kernel = Kernel::new();
    mount_todo(&kernel).await;
    let all = kernel.handle().get_all(UI_VIEWS);
    let (_, views) = &all[0];
    assert_eq!(views.views(), [ViewSpec::new(VIEW, "To-do")]);
    assert_eq!(
        views.render("other", &ViewContext::default()).await,
        Err(UiError::UnknownView("other".into()))
    );
}

fn write_call(call_id: &str, content: &str) -> SessionEntry {
    SessionEntry::ToolCall {
        turn: RequestId::new(),
        call_id: call_id.to_owned(),
        tool_call_id: ToolCallId::new(),
        name: ToolName::new(WRITE_TOOL),
        arguments: json!({ "todos": [{ "content": content, "status": "pending" }] }),
    }
}

fn result(call_id: &str, is_error: bool, effect: ToolEffect) -> SessionEntry {
    SessionEntry::ToolResult {
        turn: RequestId::new(),
        call_id: call_id.to_owned(),
        output: ToolOutput {
            content: Vec::new(),
            is_error,
        },
        effect,
    }
}

#[tokio::test]
async fn an_unknown_list_is_rebuilt_from_the_session_log() {
    // The effect the invoker records for a successful write: the log holds
    // that, not an effect chosen by this test.
    let written = {
        let kernel = Kernel::new();
        mount_todo(&kernel).await;
        let outcome = invoke_outcome(
            &kernel,
            &SessionId::new(),
            WRITE_TOOL,
            json!({ "todos": [{ "content": "x", "status": "pending" }] }),
        )
        .await;
        assert!(!outcome.output.is_error);
        outcome.effect
    };
    let session = SessionId::new();
    let log = Sessions(
        session.clone(),
        vec![
            write_call("a", "first"),
            result("a", false, written),
            write_call("b", "kept"),
            result("b", false, written),
            // Failed, uncertain and unanswered writes do not count.
            write_call("c", "failed"),
            result("c", true, written),
            write_call("d", "uncertain"),
            result("d", false, ToolEffect::Uncertain),
            write_call("e", "unanswered"),
        ],
    );
    let kernel = Kernel::new();
    kernel
        .mount(
            Arc::new(SessionPlugin(
                PluginManifest::new("sessions", "1").provides(["session"]),
                Arc::new(log),
            )),
            MountOptions::default(),
        )
        .await
        .unwrap();
    mount_todo(&kernel).await;

    let read = invoke(&kernel, &session, READ_TOOL, json!({})).await;
    assert_eq!(
        output_text(&read),
        "To-do list (0 of 1 completed):\n[ ] kept"
    );
    // A session the log does not know has an empty list.
    let read = invoke(&kernel, &SessionId::new(), READ_TOOL, json!(null)).await;
    assert_eq!(output_text(&read), "To-do list: the list is empty.");

    // A write replaces the rebuilt list.
    invoke(
        &kernel,
        &session,
        WRITE_TOOL,
        json!({ "todos": [{ "content": "new", "status": "completed" }] }),
    )
    .await;
    assert_eq!(
        render(&kernel, Some(&session)).await.nodes[1],
        Node::List {
            items: vec!["[x] new".into()],
        }
    );
}

#[test]
fn the_plugin_takes_no_configuration() {
    let plugin = TodoPlugin::new("todo");
    let example: Value = serde_json::from_str(TodoPlugin::CONFIG_EXAMPLE).unwrap();
    assert!(plugin.check_config(&example).is_ok());
    assert!(plugin.check_config(&Value::Null).is_ok());
    assert!(plugin.check_config(&json!({ "x": 1 })).is_err());
}
