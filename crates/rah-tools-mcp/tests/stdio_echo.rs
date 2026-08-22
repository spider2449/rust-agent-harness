use std::{
    fs,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use rah_protocol::{
    PermissionLevel, ToolCall, ToolCallId, ToolContent, ToolInput, ToolName, ToolOutput,
};
use rah_tools::{Tool, ToolContext, ToolError, ToolRegistry};
use rah_tools_mcp::{MCP_PROTOCOL_VERSION, McpAdapter, McpLimits, McpServerConfig};
use serde_json::{Value, json};
use tokio::time::{sleep, timeout};

fn server_program() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_rah-mcp-echo-server"))
}

fn config(call_timeout: Duration) -> McpServerConfig {
    McpServerConfig::stdio("test", server_program())
        .expect("test server configuration should be valid")
        .with_tool_permission("echo", PermissionLevel::None)
        .expect("echo permission should be configured once")
        .with_call_timeout(call_timeout)
}

fn mode_config(mode: &str) -> McpServerConfig {
    config(Duration::from_millis(75))
        .with_arg("--mode")
        .with_arg(mode)
}

fn fixture_path(label: &str) -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    std::env::temp_dir().join(format!(
        "rah-mcp-test-{label}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}

async fn connect_error(config: McpServerConfig) -> rah_tools_mcp::McpAdapterError {
    match McpAdapter::connect(config).await {
        Ok(adapter) => {
            adapter
                .shutdown()
                .await
                .expect("unexpected adapter should shut down");
            panic!("fixture must fail connection");
        }
        Err(error) => error,
    }
}

async fn adapter() -> McpAdapter {
    McpAdapter::connect(config(Duration::from_secs(1)))
        .await
        .expect("echo adapter should connect")
}

fn call(name: &str, text: &str) -> ToolCall {
    ToolCall {
        id: ToolCallId::new(),
        name: ToolName::new(name),
        input: ToolInput(json!({"text": text})),
    }
}

async fn execute(tool: &Arc<dyn Tool>, text: &str) -> Result<ToolOutput, ToolError> {
    tool.execute(ToolInput(json!({"text": text})), ToolContext::default())
        .await
}

#[tokio::test]
async fn initializes_then_discovers_and_maps_echo_definition() {
    let adapter = adapter().await;

    assert_eq!(MCP_PROTOCOL_VERSION, "2025-06-18");
    let tools = adapter.tools();
    assert_eq!(tools.len(), 1);
    assert_eq!(
        tools[0].definition(),
        rah_protocol::ToolDefinition {
            name: ToolName::new("mcp.test.echo"),
            description: "Returns the supplied text unchanged.".to_owned(),
            input_schema: json!({
                "type": "object",
                "properties": {"text": {"type": "string"}},
                "required": ["text"],
                "additionalProperties": false
            }),
            permission: PermissionLevel::None,
        }
    );

    let lifecycle = execute(&tools[0], "__lifecycle__")
        .await
        .expect("lifecycle query should execute");
    assert_eq!(
        lifecycle.content,
        [ToolContent::Text(
            "initialize,initialized,tools/list".to_owned()
        )]
    );

    adapter.shutdown().await.expect("adapter should shut down");
}

#[tokio::test]
async fn unconfigured_remote_tool_fails_closed_during_discovery() {
    let configuration =
        McpServerConfig::stdio("test", server_program()).expect("server config should be valid");

    let error = match McpAdapter::connect(configuration).await {
        Ok(adapter) => {
            adapter.shutdown().await.expect("adapter should shut down");
            panic!("an unconfigured remote tool must not become a RAH tool");
        }
        Err(error) => error,
    };

    assert!(error.to_string().contains("exact expected tool set"));
}

#[tokio::test]
async fn exact_schema_expectation_admits_only_the_host_pinned_schema() {
    let schema = json!({
        "type": "object",
        "properties": {"text": {"type": "string"}},
        "required": ["text"],
        "additionalProperties": false
    });
    let adapter = McpAdapter::connect(
        McpServerConfig::stdio("test", server_program())
            .expect("server config")
            .with_expected_tool("echo", schema, PermissionLevel::Read)
            .expect("host expectation"),
    )
    .await
    .expect("exact schema should be admitted");
    assert_eq!(
        adapter.tools()[0].definition().permission,
        PermissionLevel::Read
    );
    adapter.shutdown().await.expect("shutdown");
}

#[test]
fn resource_limits_reject_unbounded_configuration() {
    let error = McpServerConfig::stdio("test", server_program())
        .expect("server config")
        .with_limits(rah_tools_mcp::McpLimits {
            max_outstanding: 33,
            ..Default::default()
        })
        .expect_err("hard maximum must reject expansion");
    assert!(error.to_string().contains("resource limits"));
}

#[tokio::test]
async fn discovered_tools_receive_distinct_host_permissions() {
    let configuration = McpServerConfig::stdio("test", server_program())
        .expect("server config should be valid")
        .with_arg("--two-tools")
        .with_tool_permission("echo", PermissionLevel::Read)
        .expect("echo permission should be valid")
        .with_tool_permission("write", PermissionLevel::Write)
        .expect("write permission should be valid");
    let adapter = McpAdapter::connect(configuration)
        .await
        .expect("both remote identities are explicitly configured");

    let definitions = adapter
        .tools()
        .into_iter()
        .map(|tool| tool.definition())
        .collect::<Vec<_>>();
    assert_eq!(definitions.len(), 2);
    assert_eq!(definitions[0].name, ToolName::new("mcp.test.echo"));
    assert_eq!(definitions[0].permission, PermissionLevel::Read);
    assert_eq!(definitions[1].name, ToolName::new("mcp.test.write"));
    assert_eq!(definitions[1].permission, PermissionLevel::Write);

    adapter.shutdown().await.expect("adapter should shut down");
}

#[tokio::test]
async fn mcp_metadata_cannot_override_host_permission() {
    let configuration = McpServerConfig::stdio("test", server_program())
        .expect("server config should be valid")
        .with_tool_permission("echo", PermissionLevel::Execute)
        .expect("host permission should be valid");
    let adapter = McpAdapter::connect(configuration)
        .await
        .expect("echo adapter should connect");

    assert_eq!(
        adapter.tools()[0].definition().permission,
        PermissionLevel::Execute
    );

    adapter.shutdown().await.expect("adapter should shut down");
}

#[test]
fn duplicate_and_malformed_permission_configuration_fails_deterministically() {
    let malformed = McpServerConfig::stdio("test", server_program())
        .expect("server config should be valid")
        .with_tool_permission("", PermissionLevel::None)
        .expect_err("empty remote identity should fail");
    assert_eq!(
        malformed.to_string(),
        "invalid MCP configuration: remote tool name must contain 1-64 lowercase ASCII letters, digits, `_`, or `-`"
    );

    let duplicate = McpServerConfig::stdio("test", server_program())
        .expect("server config should be valid")
        .with_tool_permission("echo", PermissionLevel::None)
        .expect("first assignment should succeed")
        .with_tool_permission("echo", PermissionLevel::Read)
        .expect_err("duplicate assignment should fail");
    assert_eq!(
        duplicate.to_string(),
        "invalid MCP configuration: permission for external tool `mcp:test:echo` is configured more than once"
    );
}

#[tokio::test]
async fn partially_configured_discovery_does_not_create_implicit_none_tool() {
    let configuration = McpServerConfig::stdio("test", server_program())
        .expect("server config should be valid")
        .with_arg("--two-tools")
        .with_tool_permission("write", PermissionLevel::Write)
        .expect("write permission should be valid");

    let error = match McpAdapter::connect(configuration).await {
        Ok(adapter) => {
            adapter.shutdown().await.expect("adapter should shut down");
            panic!("unconfigured echo must fail the entire discovery generation");
        }
        Err(error) => error,
    };

    assert!(error.to_string().contains("exact expected tool set"));
    assert!(!error.to_string().contains("PermissionLevel::None"));
}

#[tokio::test]
async fn registers_and_dispatches_through_the_generic_registry() {
    let adapter = adapter().await;
    let tool = adapter.tools().remove(0);
    let mut registry = ToolRegistry::new();
    registry
        .register(Arc::clone(&tool))
        .expect("MCP proxy should register as an ordinary tool");

    assert!(registry.get(&ToolName::new("mcp.test.echo")).is_some());
    let output = registry
        .execute(call("mcp.test.echo", "hello"), ToolContext::default())
        .await
        .expect("registry should dispatch the MCP-backed tool");
    assert_eq!(
        output,
        ToolOutput {
            content: vec![ToolContent::Text("hello".to_owned())],
            is_error: false,
        }
    );

    let error = registry
        .execute(call("mcp.test.missing", "hello"), ToolContext::default())
        .await
        .expect_err("unknown RAH tool should fail in the registry");
    assert_eq!(
        error,
        ToolError::UnknownTool {
            name: ToolName::new("mcp.test.missing")
        }
    );

    adapter.shutdown().await.expect("adapter should shut down");
}

#[tokio::test]
async fn reports_live_bridge_echo_arguments_and_execution_count() {
    let adapter = adapter().await;
    let output = execute(&adapter.tools()[0], "RAH_MCP_BRIDGE_OK")
        .await
        .expect("bridge echo should execute");

    assert_eq!(
        output,
        ToolOutput {
            content: vec![
                ToolContent::Text("RAH_MCP_BRIDGE_OK".to_owned()),
                ToolContent::Json(json!({
                    "bridgeEchoCalls": 1,
                    "receivedArguments": {"text": "RAH_MCP_BRIDGE_OK"}
                })),
            ],
            is_error: false,
        }
    );

    adapter.shutdown().await.expect("adapter should shut down");
}

#[tokio::test]
async fn maps_structured_and_completed_tool_error_results() {
    let adapter = adapter().await;
    let tool = &adapter.tools()[0];

    let structured = execute(tool, "__structured__")
        .await
        .expect("structured result should map");
    assert_eq!(
        structured,
        ToolOutput {
            content: vec![
                ToolContent::Text("structured".to_owned()),
                ToolContent::Json(json!({"echo": "structured"})),
            ],
            is_error: false,
        }
    );

    let tool_error = execute(tool, "__tool_error__")
        .await
        .expect("completed MCP tool error should remain output");
    assert_eq!(
        tool_error,
        ToolOutput {
            content: vec![ToolContent::Text("deterministic echo error".to_owned())],
            is_error: true,
        }
    );

    adapter.shutdown().await.expect("adapter should shut down");
}

#[tokio::test]
async fn rejects_invalid_input_and_malformed_results() {
    let adapter = adapter().await;
    let tool = &adapter.tools()[0];

    let invalid = tool
        .execute(ToolInput(json!("not an object")), ToolContext::default())
        .await
        .expect_err("non-object input should fail before transport");
    assert!(matches!(invalid, ToolError::InvalidInput { .. }));

    let malformed = execute(tool, "__malformed_result__")
        .await
        .expect_err("malformed result should fail closed");
    assert!(matches!(malformed, ToolError::Execution { .. }));

    let protocol_error = execute(tool, "__protocol_error__")
        .await
        .expect_err("JSON-RPC error should become a RAH execution error");
    assert!(matches!(protocol_error, ToolError::Execution { .. }));

    adapter.shutdown().await.expect("adapter should shut down");
}

#[tokio::test]
async fn timeout_cancels_without_replaying_uncertain_execution() {
    let adapter = McpAdapter::connect(config(Duration::from_millis(75)))
        .await
        .expect("echo adapter should connect");
    let tool = &adapter.tools()[0];

    let error = execute(tool, "__timeout__")
        .await
        .expect_err("slow call should time out");
    assert!(matches!(error, ToolError::Execution { .. }));

    let counts = wait_for_counts(tool, 1, 1).await;
    assert_eq!(counts, (1, 1));

    adapter.shutdown().await.expect("adapter should shut down");
}

#[tokio::test]
async fn dropping_execution_sends_cancellation_and_ignores_late_success() {
    let adapter = adapter().await;
    let tool = adapter.tools().remove(0);
    let executing_tool = Arc::clone(&tool);
    let execution = tokio::spawn(async move { execute(&executing_tool, "__cancel__").await });

    timeout(Duration::from_secs(1), async {
        loop {
            let output = execute(&tool, "__counts__")
                .await
                .expect("count query should execute");
            if parse_counts(&output).0 == 1 {
                break;
            }
            sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("server should observe the cancellable call");

    execution.abort();
    let join_error = execution.await.expect_err("execution should be aborted");
    assert!(join_error.is_cancelled());

    let counts = wait_for_counts(&tool, 1, 1).await;
    assert_eq!(counts, (1, 1));

    adapter.shutdown().await.expect("adapter should shut down");
}

#[tokio::test]
async fn child_exit_and_disconnect_fail_closed_without_reconnect() {
    for trigger in ["__child_exit__", "__disconnect__"] {
        let adapter = adapter().await;
        let tool = &adapter.tools()[0];

        let error = execute(tool, trigger)
            .await
            .expect_err("lost server should fail the call");
        assert!(matches!(error, ToolError::Execution { .. }));

        let second = execute(tool, "would replay if reconnected")
            .await
            .expect_err("unavailable generation should reject new calls");
        assert!(matches!(second, ToolError::Execution { .. }));

        adapter.shutdown().await.expect("adapter should shut down");
    }
}

#[tokio::test]
async fn child_cwd_and_environment_are_host_isolated_and_cleaned_up() {
    let observation_file = fixture_path("observation.json");
    // This is intentionally process-global: the fixture proves `env_clear` does
    // not pass this ambient parent value to the child.
    unsafe { std::env::set_var("RAH_MCP_PARENT_SECRET_SENTINEL", "parent-only") };
    let adapter = McpAdapter::connect(
        mode_config("echo")
            .with_arg("--observation-file")
            .with_arg(observation_file.to_string_lossy()),
    )
    .await
    .expect("fixture should connect");
    let observation: Value =
        serde_json::from_slice(&fs::read(&observation_file).expect("observation"))
            .expect("valid fixture observation");
    let cwd = PathBuf::from(observation["cwd"].as_str().expect("fixture cwd"));
    assert_ne!(cwd, std::env::current_dir().expect("parent cwd"));
    assert!(cwd.exists(), "host-owned cwd exists while child is active");
    assert!(
        !observation["parentSecretPresent"]
            .as_bool()
            .expect("secret flag")
    );
    #[cfg(windows)]
    assert_eq!(
        observation["systemRootPresent"].as_bool(),
        Some(std::env::var_os("SystemRoot").is_some())
    );

    adapter.shutdown().await.expect("shutdown");
    assert!(!cwd.exists(), "host-owned cwd is reaped with the child");
    let _ = fs::remove_file(observation_file);
    unsafe { std::env::remove_var("RAH_MCP_PARENT_SECRET_SENTINEL") };
}

#[tokio::test]
async fn initialize_and_discovery_timeouts_return_no_partial_provider() {
    for mode in ["hang-initialize", "hang-discovery"] {
        let started = Instant::now();
        let error = connect_error(mode_config(mode)).await;
        assert!(started.elapsed() < Duration::from_secs(3));
        assert!(matches!(
            error,
            rah_tools_mcp::McpAdapterError::Initialization { .. }
        ));
        assert!(error.to_string().contains("not replayed"));
    }
}

#[tokio::test]
async fn oversized_wire_message_fails_before_provider_admission() {
    let error = connect_error(mode_config("oversized-message")).await;
    assert!(matches!(
        error,
        rah_tools_mcp::McpAdapterError::Initialization { .. }
    ));
    assert!(!error.to_string().contains('x'));
}

#[tokio::test]
async fn result_and_model_output_limits_reject_structured_and_text_results() {
    let limits = McpLimits {
        max_message_bytes: 16 * 1024,
        max_result_bytes: 1024,
        ..Default::default()
    };
    for mode in ["__oversized_structured__", "__oversized_text__"] {
        let adapter = McpAdapter::connect(
            config(Duration::from_secs(1))
                .with_limits(limits.clone())
                .expect("small valid limits"),
        )
        .await
        .expect("adapter");
        let error = execute(&adapter.tools()[0], mode)
            .await
            .expect_err("result must exceed output limit");
        assert!(matches!(error, ToolError::Execution { .. }));
        assert!(error.to_string().contains("result exceeded"));
        adapter.shutdown().await.expect("shutdown");
    }
}

#[tokio::test]
async fn bounded_stderr_is_host_diagnostic_only() {
    let adapter = McpAdapter::connect(
        mode_config("stderr-flood")
            .with_limits(McpLimits {
                max_stderr_bytes: 128,
                ..Default::default()
            })
            .expect("small stderr tail"),
    )
    .await
    .expect("adapter");
    let diagnostic = adapter.diagnostics();
    assert!(diagnostic.stderr.len() <= 128);
    assert!(diagnostic.truncated_bytes > 0);
    let output = execute(&adapter.tools()[0], "normal")
        .await
        .expect("normal result");
    assert!(!format!("{output:?}").contains("RAH_MCP_STDERR_SECRET_SENTINEL"));
    adapter.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn exact_discovery_rejects_missing_extra_duplicate_and_schema_drift_atomically() {
    for mode in [
        "missing-tool",
        "extra-tool",
        "duplicate-tool",
        "schema-drift",
    ] {
        let config = if mode == "schema-drift" {
            McpServerConfig::stdio("test", server_program()).expect("config")
                .with_arg("--mode").with_arg(mode)
                .with_expected_tool("echo", json!({"type":"object", "properties":{"text":{"type":"string"}}, "required":["text"], "additionalProperties":false}), PermissionLevel::None)
                .expect("expected tool")
        } else {
            mode_config(mode)
        };
        let error = connect_error(config).await;
        assert!(matches!(
            error,
            rah_tools_mcp::McpAdapterError::Initialization { .. }
        ));
    }
}

#[tokio::test]
async fn schema_normalization_accepts_reordered_objects_but_not_security_relevant_drift() {
    let schema = json!({"additionalProperties":false, "required":["text"], "properties":{"text":{"type":"string"}}, "type":"object"});
    let adapter = McpAdapter::connect(
        McpServerConfig::stdio("test", server_program())
            .expect("config")
            .with_expected_tool("echo", schema, PermissionLevel::None)
            .expect("expected tool"),
    )
    .await
    .expect("object key order is normalized");
    adapter.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn exact_schema_rejects_added_removed_type_required_and_nested_drift() {
    let actual = json!({"type":"object", "properties":{"text":{"type":"string"}}, "required":["text"], "additionalProperties":false});
    let variants = [
        json!({"type":"object", "properties":{"text":{"type":"string"}, "extra":{"type":"string"}}, "required":["text"], "additionalProperties":false}),
        json!({"type":"object", "properties":{}, "required":["text"], "additionalProperties":false}),
        json!({"type":"object", "properties":{"text":{"type":"number"}}, "required":["text"], "additionalProperties":false}),
        json!({"type":"object", "properties":{"text":{"type":"string"}}, "required":[], "additionalProperties":false}),
        json!({"type":"object", "properties":{"text":{"type":"string", "items":{"type":"string"}}}, "required":["text"], "additionalProperties":false}),
    ];
    for schema in variants {
        assert_ne!(schema, actual);
        let error = connect_error(
            McpServerConfig::stdio("test", server_program())
                .expect("config")
                .with_expected_tool("echo", schema, PermissionLevel::None)
                .expect("expected tool"),
        )
        .await;
        assert!(error.to_string().contains("schema or permission"));
    }
}

#[tokio::test]
async fn malformed_discovery_cannot_admit_a_partial_provider() {
    let error = connect_error(mode_config("malformed-discovery")).await;
    assert!(matches!(
        error,
        rah_tools_mcp::McpAdapterError::Initialization { .. }
    ));
    assert!(error.to_string().contains("invalid name"));
}

#[cfg(windows)]
#[tokio::test]
async fn windows_admits_native_exe_and_rejects_script_extensions_without_running_them() {
    let adapter = adapter().await;
    adapter
        .shutdown()
        .await
        .expect("native exe fixture should be admitted");
    for extension in ["cmd", "ps1"] {
        let path = fixture_path(extension).with_extension(extension);
        fs::write(&path, b"not a native executable").expect("script fixture");
        let error = connect_error(
            McpServerConfig::stdio("test", &path)
                .expect("absolute script path config")
                .with_tool_permission("echo", PermissionLevel::None)
                .expect("permission"),
        )
        .await;
        assert!(matches!(
            error,
            rah_tools_mcp::McpAdapterError::InvalidConfiguration { .. }
        ));
        let _ = fs::remove_file(path);
    }
}

#[cfg(windows)]
#[tokio::test]
async fn windows_reparse_executable_alias_is_rejected_when_symlinks_are_available() {
    use std::os::windows::fs::symlink_file;

    let alias = fixture_path("exe-alias").with_extension("exe");
    if let Err(error) = symlink_file(server_program(), &alias) {
        eprintln!("skipped Windows symlink integration variant: {error}");
        return;
    }
    let error = connect_error(
        McpServerConfig::stdio("test", &alias)
            .expect("absolute alias config")
            .with_tool_permission("echo", PermissionLevel::None)
            .expect("permission"),
    )
    .await;
    assert!(matches!(
        error,
        rah_tools_mcp::McpAdapterError::InvalidConfiguration { .. }
    ));
    let _ = fs::remove_file(alias);
}

#[cfg(unix)]
#[tokio::test]
async fn unix_rejects_non_executable_directories_and_symlinks() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let regular = fixture_path("unix-regular");
    fs::write(&regular, b"fixture").expect("regular fixture");
    fs::set_permissions(&regular, fs::Permissions::from_mode(0o600)).expect("permissions");
    let directory = fixture_path("unix-directory");
    fs::create_dir(&directory).expect("directory fixture");
    let link = fixture_path("unix-link");
    symlink(server_program(), &link).expect("symlink fixture");
    for path in [&regular, &directory, &link] {
        let error = connect_error(
            McpServerConfig::stdio("test", path)
                .expect("config")
                .with_tool_permission("echo", PermissionLevel::None)
                .expect("permission"),
        )
        .await;
        assert!(matches!(
            error,
            rah_tools_mcp::McpAdapterError::InvalidConfiguration { .. }
                | rah_tools_mcp::McpAdapterError::Startup { .. }
        ));
    }
    let _ = fs::remove_file(regular);
    let _ = fs::remove_dir(directory);
    let _ = fs::remove_file(link);
}

#[tokio::test]
async fn cancellation_late_response_queue_pressure_and_exit_never_replay_or_restart() {
    let counter = fixture_path("spawns");
    let adapter = McpAdapter::connect(
        mode_config("late-response")
            .with_arg("--spawn-counter-file")
            .with_arg(counter.to_string_lossy())
            .with_limits(McpLimits {
                max_outstanding: 2,
                command_queue: 2,
                ..Default::default()
            })
            .expect("small limits"),
    )
    .await
    .expect("adapter");
    let tool = adapter.tools().remove(0);
    let first_tool = Arc::clone(&tool);
    let first = tokio::spawn(async move { execute(&first_tool, "__cancel__").await });
    let second_tool = Arc::clone(&tool);
    let second = tokio::spawn(async move { execute(&second_tool, "__cancel__").await });
    sleep(Duration::from_millis(20)).await;
    let busy = execute(&tool, "second")
        .await
        .expect_err("outstanding limit must reject");
    assert!(busy.to_string().contains("busy"));
    first.abort();
    second.abort();
    let _ = first.await;
    let _ = second.await;
    sleep(Duration::from_millis(50)).await;
    let output = execute(&tool, "after-cancel")
        .await
        .expect("late reply must not resurrect request");
    assert_eq!(
        output.content,
        [ToolContent::Text("after-cancel".to_owned())]
    );
    adapter.shutdown().await.expect("shutdown");
    assert_eq!(
        fs::read_to_string(&counter)
            .expect("counter")
            .lines()
            .count(),
        1
    );
    let _ = fs::remove_file(counter);
}

#[tokio::test]
async fn child_exits_before_initialize_during_discovery_and_call_without_restart() {
    for mode in ["exit-before-init", "exit-during-discovery"] {
        let counter = fixture_path("exit-spawns");
        let error = connect_error(
            mode_config(mode)
                .with_arg("--spawn-counter-file")
                .with_arg(counter.to_string_lossy()),
        )
        .await;
        assert!(matches!(
            error,
            rah_tools_mcp::McpAdapterError::Initialization { .. }
        ));
        assert_eq!(
            fs::read_to_string(&counter)
                .expect("counter")
                .lines()
                .count(),
            1
        );
        let _ = fs::remove_file(counter);
    }
}

async fn wait_for_counts(
    tool: &Arc<dyn Tool>,
    calls: usize,
    cancellations: usize,
) -> (usize, usize) {
    timeout(Duration::from_secs(1), async {
        loop {
            let output = execute(tool, "__counts__")
                .await
                .expect("count query should execute");
            let counts = parse_counts(&output);
            if counts.0 >= calls && counts.1 >= cancellations {
                break counts;
            }
            sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("server should observe expected calls and cancellations")
}

fn parse_counts(output: &ToolOutput) -> (usize, usize) {
    let ToolContent::Json(value) = &output.content[0] else {
        panic!("count query should return structured JSON");
    };
    (
        value["calls"].as_u64().expect("call count") as usize,
        value["cancellations"].as_u64().expect("cancellation count") as usize,
    )
}
