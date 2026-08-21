use std::{path::PathBuf, sync::Arc, time::Duration};

use rah_protocol::{
    PermissionLevel, ToolCall, ToolCallId, ToolContent, ToolInput, ToolName, ToolOutput,
};
use rah_tools::{Tool, ToolContext, ToolError, ToolRegistry};
use rah_tools_plugin::{PLUGIN_PROTOCOL_VERSION, PluginAdapter, PluginConfig, PluginLimits};
use serde_json::json;
use tokio::time::{sleep, timeout};

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_rah-plugin-echo"))
}

fn config(call_timeout: Duration) -> PluginConfig {
    PluginConfig::stdio("test", "0.1.0", fixture())
        .expect("fixture configuration should be valid")
        .with_tool_permission("echo", PermissionLevel::None)
        .expect("echo permission should be assigned once")
        .with_call_timeout(call_timeout)
}

async fn adapter() -> PluginAdapter {
    PluginAdapter::connect(config(Duration::from_secs(1)))
        .await
        .expect("fixture should connect")
}

async fn execute(tool: &Arc<dyn Tool>, value: serde_json::Value) -> Result<ToolOutput, ToolError> {
    tool.execute(ToolInput(json!({"value": value})), ToolContext::default())
        .await
}

fn call(value: serde_json::Value) -> ToolCall {
    ToolCall {
        id: ToolCallId::new(),
        name: ToolName::new("plugin.test.echo"),
        input: ToolInput(json!({"value": value})),
    }
}

#[tokio::test]
async fn handshakes_discovers_names_permissions_and_registers() {
    let adapter = adapter().await;
    assert_eq!(PLUGIN_PROTOCOL_VERSION, "1");
    let tool = adapter.tools().remove(0);
    assert_eq!(
        tool.definition(),
        rah_protocol::ToolDefinition {
            name: ToolName::new("plugin.test.echo"),
            description: "Returns the supplied value.".to_owned(),
            input_schema: json!({
                "type": "object",
                "properties": {"value": {}},
                "required": ["value"],
                "additionalProperties": false
            }),
            permission: PermissionLevel::None,
        }
    );

    let mut registry = ToolRegistry::new();
    registry
        .register(Arc::clone(&tool))
        .expect("plugin proxy should be an ordinary RAH tool");
    let output = registry
        .execute(call(json!("hello")), ToolContext::default())
        .await
        .expect("registry should dispatch plugin echo");
    assert_eq!(
        output,
        ToolOutput {
            content: vec![ToolContent::Text("hello".to_owned())],
            is_error: false,
        }
    );

    let lifecycle = execute(&tool, json!("__lifecycle__"))
        .await
        .expect("fixture should expose lifecycle audit data");
    assert_eq!(
        lifecycle.content,
        [ToolContent::Text(
            "initialize,initialized,tools/list".to_owned()
        )]
    );
    adapter
        .shutdown()
        .await
        .expect("shutdown should reap child");
}

#[tokio::test]
async fn rejects_protocol_and_reported_identity_mismatches() {
    for arg in ["--wrong-protocol", "--wrong-id", "--wrong-version"] {
        let configuration = config(Duration::from_secs(1)).with_arg(arg);
        let error = PluginAdapter::connect(configuration)
            .await
            .expect_err("mismatched handshake must fail closed");
        assert!(error.to_string().contains("initialization failed"));
    }
}

#[tokio::test]
async fn missing_permission_fails_closed_and_metadata_cannot_escalate() {
    let missing =
        PluginConfig::stdio("test", "0.1.0", fixture()).expect("configuration should be valid");
    let error = PluginAdapter::connect(missing)
        .await
        .expect_err("missing permission must fail the complete generation");
    assert!(error.to_string().contains("no explicit host permission"));

    let configured = PluginConfig::stdio("test", "0.1.0", fixture())
        .expect("configuration should be valid")
        .with_arg("--contradictory-metadata")
        .with_tool_permission("echo", PermissionLevel::None)
        .expect("host permission should be valid");
    let adapter = PluginAdapter::connect(configured)
        .await
        .expect("untrusted metadata must not affect authorization");
    assert_eq!(
        adapter.tools()[0].definition().permission,
        PermissionLevel::None
    );
    adapter.shutdown().await.expect("shutdown should succeed");
}

#[tokio::test]
async fn maps_json_and_plugin_declared_error_results() {
    let adapter = adapter().await;
    let tool = adapter.tools().remove(0);
    let structured = execute(&tool, json!({"nested": true}))
        .await
        .expect("JSON echo should succeed");
    assert_eq!(
        structured,
        ToolOutput {
            content: vec![ToolContent::Json(json!({"nested": true}))],
            is_error: false,
        }
    );
    let declared = execute(&tool, json!("__tool_error__"))
        .await
        .expect("declared tool failure is a completed output");
    assert!(declared.is_error);
    assert_eq!(
        declared.content,
        [ToolContent::Text("deterministic plugin error".to_owned())]
    );

    for marker in ["__remote_error__", "__malformed_result__"] {
        let error = execute(&tool, json!(marker))
            .await
            .expect_err("remote protocol errors and malformed results must fail closed");
        assert!(matches!(error, ToolError::Execution { .. }));
        assert!(!error.to_string().contains("untrusted remote detail"));
    }
    let invalid = tool
        .execute(ToolInput(json!("not an object")), ToolContext::default())
        .await
        .expect_err("local non-object arguments must be rejected before IPC");
    assert!(matches!(invalid, ToolError::InvalidInput { .. }));
    adapter.shutdown().await.expect("shutdown should succeed");
}

#[tokio::test]
async fn malformed_oversized_unknown_and_duplicate_messages_fail_connection() {
    for trigger in [
        "__malformed_message__",
        "__oversized_message__",
        "__oversized_result__",
        "__unknown_response__",
    ] {
        let adapter = adapter().await;
        let tool = adapter.tools().remove(0);
        let error = execute(&tool, json!(trigger))
            .await
            .expect_err("protocol violation should fail the call");
        assert!(matches!(error, ToolError::Execution { .. }));
        let second = execute(&tool, json!("must not reconnect"))
            .await
            .expect_err("invalid generation must remain disconnected");
        assert!(matches!(second, ToolError::Execution { .. }));
        adapter
            .shutdown()
            .await
            .expect("failed child should be reaped");
    }

    let adapter = adapter().await;
    let tool = adapter.tools().remove(0);
    let first = execute(&tool, json!("__duplicate_response__")).await;
    assert!(
        first.is_ok() || matches!(first, Err(ToolError::Execution { .. })),
        "the first correlated response may win the scheduling race"
    );
    sleep(Duration::from_millis(30)).await;
    execute(&tool, json!("duplicate must invalidate generation"))
        .await
        .expect_err("a duplicate response must close the connection");
    adapter
        .shutdown()
        .await
        .expect("duplicate-failed child should be reaped");
}

#[tokio::test]
async fn outstanding_limit_is_bounded() {
    let adapter = adapter().await;
    let tool = adapter.tools().remove(0);
    let mut calls = Vec::new();
    for _ in 0..PluginLimits::default().max_outstanding {
        let tool = Arc::clone(&tool);
        calls.push(tokio::spawn(async move {
            execute(&tool, json!("__hang__")).await
        }));
    }
    sleep(Duration::from_millis(50)).await;
    let busy = execute(&tool, json!("__hang__"))
        .await
        .expect_err("request beyond outstanding limit must be rejected");
    assert!(busy.to_string().contains("busy"));
    for call in calls {
        call.abort();
        let _ = call.await;
    }
    adapter.shutdown().await.expect("shutdown should succeed");
}

#[tokio::test]
async fn timeout_and_drop_cancel_without_replay_and_ignore_late_response() {
    let adapter = PluginAdapter::connect(config(Duration::from_millis(75)))
        .await
        .expect("fixture should connect");
    let tool = adapter.tools().remove(0);
    let error = execute(&tool, json!("__timeout_late__"))
        .await
        .expect_err("call should time out");
    assert!(error.to_string().contains("not replayed"));

    let running_tool = Arc::clone(&tool);
    let running =
        tokio::spawn(async move { execute(&running_tool, json!("__cancel_late__")).await });
    sleep(Duration::from_millis(30)).await;
    running.abort();
    let _ = running.await;

    let audit = wait_for_audit(&tool, 2, 2).await;
    assert_eq!(audit["execution_count"], 2);
    assert_eq!(audit["cancellation_count"], 2);
    assert_eq!(audit["call_counts"]["__timeout_late__"], 1);
    assert_eq!(audit["call_counts"]["__cancel_late__"], 1);
    adapter.shutdown().await.expect("shutdown should succeed");
}

#[tokio::test]
async fn crash_and_disconnect_are_not_replayed_or_restarted() {
    for trigger in ["__crash__", "__disconnect__"] {
        let adapter = adapter().await;
        let tool = adapter.tools().remove(0);
        execute(&tool, json!(trigger))
            .await
            .expect_err("lost process should fail uncertain call");
        execute(&tool, json!("would restart"))
            .await
            .expect_err("dead generation must not automatically restart");
        adapter.shutdown().await.expect("child should be reaped");
    }
}

#[tokio::test]
async fn stderr_environment_and_cwd_are_bounded_and_isolated() {
    unsafe { std::env::set_var("RAH_PLUGIN_PARENT_SECRET", "must-not-leak") };
    let repository = std::env::current_dir().expect("repository cwd should exist");
    let adapter = PluginAdapter::connect(
        config(Duration::from_secs(1))
            .with_environment("RAH_PLUGIN_ALLOWED", "visible")
            .expect("explicit environment should be valid"),
    )
    .await
    .expect("fixture should connect");
    let tool = adapter.tools().remove(0);
    let audit = execute(&tool, json!("__audit__"))
        .await
        .expect("audit should succeed");
    let ToolContent::Json(audit) = &audit.content[0] else {
        panic!("audit should be JSON")
    };
    assert_eq!(audit["environment"]["RAH_PLUGIN_ALLOWED"], "visible");
    assert!(
        audit["environment"]
            .get("RAH_PLUGIN_PARENT_SECRET")
            .is_none()
    );
    assert_eq!(audit["environment"]["RAH_PLUGIN_PROTOCOL"], "1");
    let environment = audit["environment"]
        .as_object()
        .expect("fixture environment should be an object");
    assert!(environment.keys().all(|name| matches!(
        name.to_ascii_uppercase().as_str(),
        "RAH_PLUGIN_ALLOWED" | "RAH_PLUGIN_PROTOCOL" | "SYSTEMROOT"
    )));
    let child_cwd = PathBuf::from(audit["cwd"].as_str().expect("fixture cwd"));
    assert_ne!(child_cwd, repository);
    assert_eq!(
        std::fs::canonicalize(&child_cwd).expect("fixture cwd should canonicalize"),
        adapter.diagnostics().cwd
    );
    assert!(
        child_cwd
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("rah-process-plugin-"))
    );

    execute(&tool, json!("__stderr_flood__"))
        .await
        .expect("stderr flood must not block protocol");
    timeout(Duration::from_secs(1), async {
        loop {
            let diagnostics = adapter.diagnostics();
            if diagnostics.truncated_bytes > 0 {
                assert!(diagnostics.stderr.len() <= PluginLimits::default().max_stderr_bytes);
                break;
            }
            sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("stderr should be drained into bounded diagnostics");
    let isolated_cwd = adapter.diagnostics().cwd;
    adapter
        .shutdown()
        .await
        .expect("shutdown should reap child");
    assert!(
        !isolated_cwd.exists(),
        "isolated cwd cleanup proves actor exit"
    );
    unsafe { std::env::remove_var("RAH_PLUGIN_PARENT_SECRET") };
}

#[tokio::test]
async fn live_text_mode_emits_host_only_single_call_audit() {
    let adapter =
        PluginAdapter::connect(config(Duration::from_secs(1)).with_arg("--live-text-audit"))
            .await
            .expect("live fixture mode should connect");
    let tool = adapter.tools().remove(0);
    assert_eq!(
        tool.definition().input_schema,
        json!({
            "type": "object",
            "properties": {"text": {"type": "string"}},
            "required": ["text"],
            "additionalProperties": false
        })
    );
    let output = tool
        .execute(
            ToolInput(json!({"text": "RAH_PLUGIN_BRIDGE_OK"})),
            ToolContext::default(),
        )
        .await
        .expect("live fixture echo should succeed");
    assert_eq!(
        output,
        ToolOutput {
            content: vec![ToolContent::Text("RAH_PLUGIN_BRIDGE_OK".to_owned())],
            is_error: false,
        }
    );

    let audit = timeout(Duration::from_secs(1), async {
        loop {
            let diagnostics = adapter.diagnostics();
            if let Some(payload) = diagnostics
                .stderr
                .lines()
                .find_map(|line| line.strip_prefix("RAH_PLUGIN_AUDIT "))
            {
                break serde_json::from_str::<serde_json::Value>(payload)
                    .expect("fixture audit should be JSON");
            }
            sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("fixture audit should be drained");
    assert_eq!(audit["execution_count"], 1);
    assert_eq!(
        audit["received_arguments"],
        json!([{"text": "RAH_PLUGIN_BRIDGE_OK"}])
    );
    assert_eq!(audit["tools_call"]["method"], "tools/call");
    assert_eq!(
        audit["tools_call"]["params"]["arguments"],
        json!({"text": "RAH_PLUGIN_BRIDGE_OK"})
    );
    adapter.shutdown().await.expect("shutdown should succeed");
}

#[test]
fn codex_bridge_has_no_production_plugin_dependency_or_plugin_specific_code() {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("plugin crate should be nested under workspace/crates")
        .to_path_buf();
    let manifest = std::fs::read_to_string(workspace.join("crates/rah-runtime-codex/Cargo.toml"))
        .expect("Codex adapter manifest should be readable");
    let production_manifest = manifest
        .split("[dev-dependencies]")
        .next()
        .expect("manifest should have a production section");
    assert!(!production_manifest.contains("rah-tools-plugin"));
    let bridge = std::fs::read_to_string(workspace.join("crates/rah-runtime-codex/src/bridge.rs"))
        .expect("generic bridge source should be readable");
    assert!(!bridge.contains("PluginAdapter"));
    assert!(!bridge.contains("plugin.test.echo"));
}

async fn wait_for_audit(
    tool: &Arc<dyn Tool>,
    executions: u64,
    cancellations: u64,
) -> serde_json::Value {
    timeout(Duration::from_secs(1), async {
        loop {
            let output = execute(tool, json!("__audit__"))
                .await
                .expect("audit should remain available after late responses");
            let ToolContent::Json(value) = &output.content[0] else {
                panic!("audit should be JSON")
            };
            if value["execution_count"].as_u64().unwrap_or_default() >= executions
                && value["cancellation_count"].as_u64().unwrap_or_default() >= cancellations
            {
                break value.clone();
            }
            sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("fixture should observe calls and cancellations")
}
