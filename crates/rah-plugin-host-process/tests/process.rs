//! Protocol v3 against the Rust fixture plugin (`src/bin/fixture.rs`).

use std::{sync::Arc, time::Duration};

use rah_api::{
    CancellationToken, Kernel, MountOptions, Plugin, PluginError, PluginId, PluginManifest,
    PluginStatus, Scope, async_trait,
    hooks::{ModelBeforeRequest, ToolBeforeInvoke, ToolDecision},
    session::{SESSION, SessionAppended, SessionError, SessionLog},
    tools::{Invocation, ToolInvoker},
};
use rah_plugin_host_process::ProcessPlugin;
use rah_protocol::{
    ContextItem, ModelRequest, PermissionLevel, RequestId, SessionEntry, SessionId, SessionInfo,
    SessionRecord, ToolCall, ToolCallId, ToolContent, ToolEffect, ToolInput, ToolName, ToolOutput,
};
use serde_json::{Value, json};

fn config(args: &[&str], plugin: Value) -> Value {
    json!({
        "program": env!("CARGO_BIN_EXE_rah-process-plugin-fixture"),
        "args": args,
        "plugin": plugin,
        "call_timeout_ms": 500,
        "hook_timeout_ms": 300
    })
}

/// One fixed session with one record.
struct Sessions(SessionId);

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
        Ok(vec![SessionRecord {
            seq: 1,
            entry: SessionEntry::TurnStarted {
                turn: RequestId::new(),
                input: "hello".into(),
            },
        }])
    }

    async fn fork(&self, _: &SessionId, _: u64) -> Result<SessionInfo, SessionError> {
        unreachable!()
    }

    async fn list(&self) -> Result<Vec<SessionInfo>, SessionError> {
        Ok(vec![SessionInfo {
            id: self.0.clone(),
            parent: None,
            created: 0,
            title: None,
            updated: 0,
            preview: None,
            workspace: None,
        }])
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

async fn mount_sessions(kernel: &Kernel) -> SessionId {
    let id = SessionId::new();
    let plugin = SessionPlugin(
        PluginManifest::new("sessions", "1").provides(["session"]),
        Arc::new(Sessions(id.clone())),
    );
    kernel
        .mount(Arc::new(plugin), MountOptions::default())
        .await
        .unwrap();
    id
}

async fn mount_process(kernel: &Kernel, config: Value) -> PluginStatus {
    let plugin = ProcessPlugin::describe(PluginId::new("fixture"), &config)
        .await
        .unwrap();
    kernel
        .mount(Arc::new(plugin), MountOptions::with_config(config))
        .await
        .unwrap()
}

async fn call(kernel: &Kernel, name: &str, input: Value) -> (ToolOutput, ToolEffect) {
    let outcome = ToolInvoker::new(kernel.handle())
        .invoke(
            Invocation {
                session: SessionId::new(),
                call: ToolCall {
                    id: ToolCallId::new(),
                    name: ToolName::new(name),
                    input: ToolInput(input),
                },
                cancel: CancellationToken::new(),
            },
            || {},
        )
        .await;
    (outcome.output, outcome.effect)
}

fn json_of(output: &ToolOutput) -> Value {
    match output.content.last().unwrap() {
        ToolContent::Json(value) => value.clone(),
        ToolContent::Text(text) => Value::String(text.clone()),
        other => panic!("unexpected content: {other:?}"),
    }
}

#[tokio::test]
async fn describe_reads_the_manifest_and_tools() {
    let plugin = ProcessPlugin::describe(
        PluginId::new("fixture"),
        &config(&["--requires", "session"], Value::Null),
    )
    .await
    .unwrap();
    assert_eq!(plugin.manifest().provides, ["tools".into()]);
    assert_eq!(plugin.manifest().requires, ["session".into()]);
    assert_eq!(plugin.manifest().version, "0.1.0");
    let tools = &plugin.described().tools;
    assert_eq!(tools.len(), 7);
    assert_eq!(tools[1].permission, PermissionLevel::Read);
}

#[tokio::test]
async fn tools_run_in_the_plugin_and_may_call_required_services_only() {
    let kernel = Kernel::new();
    let session = mount_sessions(&kernel).await;
    let status = mount_process(&kernel, config(&["--requires", "session"], Value::Null)).await;
    assert_eq!(status, PluginStatus::Active);

    let (output, effect) = call(&kernel, "fixture.echo", json!({ "text": "hi" })).await;
    assert_eq!(output.content[0], ToolContent::Text("hi".into()));
    assert_eq!(effect, ToolEffect::None);

    let (output, _) = call(
        &kernel,
        "fixture.call",
        json!({ "service": "session", "method": "read", "params": { "session": session.to_string() } }),
    )
    .await;
    let answer = json_of(&output);
    assert_eq!(answer["result"][0]["entry"]["input"], "hello", "{answer}");

    for service in ["sandbox", "model", "tools"] {
        let (output, _) = call(
            &kernel,
            "fixture.call",
            json!({ "service": service, "method": "run", "params": {} }),
        )
        .await;
        let answer = json_of(&output);
        assert_eq!(answer["error"]["code"], -32003, "{answer}");
        assert!(
            answer["error"]["message"]
                .as_str()
                .unwrap()
                .contains("requires"),
            "{answer}"
        );
    }
}

#[tokio::test]
async fn services_outside_requires_are_refused_even_when_mounted() {
    let kernel = Kernel::new();
    let session = mount_sessions(&kernel).await;
    // The fixture does not require `session` here.
    mount_process(&kernel, config(&[], Value::Null)).await;
    let (output, _) = call(
        &kernel,
        "fixture.call",
        json!({ "service": "session", "method": "read", "params": { "session": session.to_string() } }),
    )
    .await;
    assert_eq!(json_of(&output)["error"]["code"], -32003);
}

#[tokio::test]
async fn required_services_gate_the_plugin() {
    let kernel = Kernel::new();
    let status = mount_process(&kernel, config(&["--requires", "session"], Value::Null)).await;
    assert!(matches!(status, PluginStatus::Pending { .. }), "{status:?}");
    mount_sessions(&kernel).await;
    assert_eq!(
        kernel.status(&PluginId::new("fixture")),
        Some(PluginStatus::Active)
    );
}

#[tokio::test]
async fn errors_timeouts_and_crashes_map_to_effects() {
    let kernel = Kernel::new();
    mount_process(&kernel, config(&[], Value::Null)).await;

    let (output, effect) = call(&kernel, "fixture.invalid", json!({})).await;
    assert!(output.is_error);
    assert_eq!(effect, ToolEffect::None);

    let (output, effect) = call(&kernel, "fixture.slow", json!({})).await;
    assert_eq!(effect, ToolEffect::Uncertain);
    assert!(json_of(&output).as_str().unwrap().contains("not replayed"));
    tokio::time::sleep(Duration::from_millis(50)).await;
    let (output, _) = call(&kernel, "fixture.cancelled", json!({})).await;
    assert_eq!(json_of(&output).as_array().unwrap().len(), 1);

    let (output, effect) = call(&kernel, "fixture.exit", json!({})).await;
    assert_eq!(effect, ToolEffect::Uncertain, "{output:?}");
    let (output, effect) = call(&kernel, "fixture.echo", json!({ "text": "x" })).await;
    assert_eq!(effect, ToolEffect::None);
    assert!(
        json_of(&output).as_str().unwrap().contains("not started"),
        "{output:?}"
    );
}

#[tokio::test]
async fn invalid_manifests_fail_describe() {
    for (mode, expected) in [
        ("old-protocol", "protocol 1"),
        ("provides-model", "cannot provide `model`"),
    ] {
        let error = ProcessPlugin::describe(
            PluginId::new("fixture"),
            &config(&["--mode", mode], Value::Null),
        )
        .await
        .err()
        .unwrap();
        assert!(error.contains(expected), "{mode}: {error}");
        assert!(
            error.contains("fixture started"),
            "stderr is reported: {error}"
        );
    }
    let error = ProcessPlugin::describe(
        PluginId::new("fixture"),
        &config(&["--requires", "model"], Value::Null),
    )
    .await
    .err()
    .unwrap();
    assert!(error.contains("cannot depend on `model`"), "{error}");
    let error = ProcessPlugin::describe(
        PluginId::new("x"),
        &json!({ "program": "rah-no-such-plugin-x7" }),
    )
    .await
    .err()
    .unwrap();
    assert!(error.contains("cannot start"), "{error}");
}

#[tokio::test]
async fn a_changed_manifest_fails_the_start() {
    let flip = std::env::temp_dir().join(format!("rah-host-process-flip-{}", std::process::id()));
    let _ = std::fs::remove_file(&flip);
    let kernel = Kernel::new();
    mount_sessions(&kernel).await;
    let status = mount_process(&kernel, config(&[], json!({ "flip_file": flip }))).await;
    let _ = std::fs::remove_file(&flip);
    match status {
        PluginStatus::Failed { message } => {
            assert!(message.contains("manifest changed"), "{message}")
        }
        other => panic!("{other:?}"),
    }
}

fn text_of(output: &ToolOutput) -> String {
    match &output.content[0] {
        ToolContent::Text(text) => text.clone(),
        other => panic!("unexpected content: {other:?}"),
    }
}

/// Denies every call at priority 0, like a strict policy.
struct DenyAll(PluginManifest);

#[async_trait]
impl Plugin for DenyAll {
    fn manifest(&self) -> &PluginManifest {
        &self.0
    }

    async fn apply(&self, scope: &mut Scope) -> Result<(), PluginError> {
        scope.hook_bail::<ToolBeforeInvoke, _, _>(0, |_| async {
            Some(ToolDecision::Deny {
                reason: "static policy".into(),
            })
        });
        Ok(())
    }
}

#[tokio::test]
async fn protocol_2_plugins_still_load_but_cannot_hook() {
    let kernel = Kernel::new();
    let status = mount_process(&kernel, config(&["--mode", "v2"], Value::Null)).await;
    assert_eq!(status, PluginStatus::Active);
    let (output, _) = call(&kernel, "fixture.echo", json!({ "text": "hi" })).await;
    assert_eq!(text_of(&output), "hi");

    for args in [
        &["--mode", "v2", "--hook", "tool.before_invoke"][..],
        &["--mode", "v2", "--event", "session.appended"][..],
    ] {
        let error = ProcessPlugin::describe(PluginId::new("fixture"), &config(args, Value::Null))
            .await
            .err()
            .unwrap();
        assert!(error.contains("need protocol 3"), "{args:?}: {error}");
    }
}

#[tokio::test]
async fn invalid_hooks_and_events_fail_describe() {
    for (args, expected) in [
        (
            &["--hook", "tool.approval"][..],
            "cannot attach to `tool.approval`",
        ),
        (
            &[
                "--hook",
                "tool.before_invoke",
                "--hook",
                "tool.before_invoke:5",
            ][..],
            "duplicate hook",
        ),
        (
            &["--event", "plugin.status"][..],
            "cannot subscribe to `plugin.status`",
        ),
        (
            &["--event", "session.appended", "--event", "session.appended"][..],
            "duplicate event",
        ),
    ] {
        let error = ProcessPlugin::describe(PluginId::new("fixture"), &config(args, Value::Null))
            .await
            .err()
            .unwrap();
        assert!(error.contains(expected), "{args:?}: {error}");
    }
}

#[tokio::test]
async fn before_invoke_decides_and_fails_closed() {
    let kernel = Kernel::new();
    mount_process(
        &kernel,
        config(&["--hook", "tool.before_invoke"], Value::Null),
    )
    .await;
    assert!(ToolInvoker::new(kernel.handle()).is_guarded());

    for (text, expected) in [
        ("allow", "allow"),
        ("no decision", "no decision"),
        ("deny", "denied: fixture says no"),
        ("ask", "no approver"),
        ("malformed", "malformed decision"),
        ("hang", "timed out"),
    ] {
        let (output, effect) = call(&kernel, "fixture.echo", json!({ "text": text })).await;
        let shown = text_of(&output);
        assert!(shown.contains(expected), "{text}: {shown}");
        assert_eq!(output.is_error, !matches!(text, "allow" | "no decision"));
        assert_eq!(effect, ToolEffect::None);
    }
    // A hook that failed does not break the connection.
    let (output, _) = call(&kernel, "fixture.echo", json!({ "text": "allow" })).await;
    assert_eq!(text_of(&output), "allow");

    kernel.unmount(&PluginId::new("fixture")).await.unwrap();
    assert!(!ToolInvoker::new(kernel.handle()).is_guarded());
}

#[tokio::test]
async fn hook_priority_orders_process_and_static_handlers() {
    for (hook, expected) in [
        ("tool.before_invoke:10", "allow"),
        ("tool.before_invoke:-10", "denied: static policy"),
    ] {
        let kernel = Kernel::new();
        kernel
            .mount(
                Arc::new(DenyAll(PluginManifest::new("deny-all", "1"))),
                MountOptions::default(),
            )
            .await
            .unwrap();
        mount_process(&kernel, config(&["--hook", hook], Value::Null)).await;
        let (output, _) = call(&kernel, "fixture.echo", json!({ "text": "allow" })).await;
        assert!(text_of(&output).contains(expected), "{hook}: {output:?}");
    }
}

#[tokio::test]
async fn after_invoke_rewrites_output_and_failures_become_errors() {
    let kernel = Kernel::new();
    mount_process(
        &kernel,
        config(&["--hook", "tool.after_invoke"], Value::Null),
    )
    .await;
    let (output, effect) = call(&kernel, "fixture.echo", json!({ "text": "hi" })).await;
    assert_eq!(
        output.content,
        [ToolContent::Text("filtered hi (none)".into())]
    );
    assert!(!output.is_error);
    assert_eq!(effect, ToolEffect::None);

    let (output, effect) = call(&kernel, "fixture.echo", json!({ "text": "hang-after" })).await;
    assert!(output.is_error);
    let shown = text_of(&output);
    assert!(
        shown.contains("`tool.after_invoke` handler of process plugin `fixture` failed"),
        "{shown}"
    );
    assert!(shown.contains("timed out"), "{shown}");
    assert_eq!(effect, ToolEffect::None, "the tool's own effect is kept");
}

#[tokio::test]
async fn model_before_request_extends_the_request_or_fails_it() {
    let kernel = Kernel::new();
    mount_process(
        &kernel,
        config(&["--hook", "model.before_request"], Value::Null),
    )
    .await;
    let request = |text: &str| ModelRequest {
        model: None,
        items: vec![ContextItem::User { text: text.into() }],
        tools: Vec::new(),
    };
    let extended = kernel
        .handle()
        .waterfall::<ModelBeforeRequest>(request("hi"))
        .await
        .unwrap();
    assert_eq!(
        extended.items,
        [
            ContextItem::User { text: "hi".into() },
            ContextItem::System {
                text: "from fixture".into()
            },
        ]
    );
    for text in ["hang", "bad-image"] {
        let result = kernel
            .handle()
            .waterfall::<ModelBeforeRequest>(request(text))
            .await;
        assert!(result.is_err(), "{text}: {result:?}");
    }
    // The connection survives a failed request.
    assert!(
        kernel
            .handle()
            .waterfall::<ModelBeforeRequest>(request("again"))
            .await
            .is_ok()
    );
}

#[tokio::test]
async fn session_appended_events_reach_subscribed_plugins_only() {
    let session = SessionId::new();
    let appended = SessionAppended {
        session: session.clone(),
        seq: 3,
        entry: SessionEntry::TurnStarted {
            turn: RequestId::new(),
            input: "hello".into(),
        },
    };
    for (args, count) in [(&["--event", "session.appended"][..], 1), (&[][..], 0)] {
        let kernel = Kernel::new();
        mount_process(&kernel, config(args, Value::Null)).await;
        kernel.handle().emit(&appended);
        let (output, _) = call(&kernel, "fixture.events", json!({})).await;
        let events = json_of(&output);
        let events = events.as_array().unwrap();
        assert_eq!(events.len(), count, "{args:?}: {events:?}");
        if let Some(event) = events.first() {
            assert_eq!(event["event"], "session.appended");
            assert_eq!(event["data"]["seq"], 3);
            assert_eq!(event["data"]["session"], session.to_string());
            assert_eq!(event["data"]["entry"]["input"], "hello");
        }
    }
}
