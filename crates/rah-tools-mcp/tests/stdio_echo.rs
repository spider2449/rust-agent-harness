use std::{path::PathBuf, sync::Arc, time::Duration};

use rah_protocol::{
    PermissionLevel, ToolCall, ToolCallId, ToolContent, ToolInput, ToolName, ToolOutput,
};
use rah_tools::{Tool, ToolContext, ToolError, ToolRegistry};
use rah_tools_mcp::{MCP_PROTOCOL_VERSION, McpAdapter, McpServerConfig};
use serde_json::json;
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

    assert!(error.to_string().contains("no explicit host permission"));
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
        "invalid MCP configuration: external tool identity must not be empty"
    );

    let duplicate = McpServerConfig::stdio("test", server_program())
        .expect("server config should be valid")
        .with_tool_permission("echo", PermissionLevel::None)
        .expect("first assignment should succeed")
        .with_tool_permission("echo", PermissionLevel::Read)
        .expect_err("duplicate assignment should fail");
    assert_eq!(
        duplicate.to_string(),
        "invalid MCP configuration: permission for external tool `echo` is configured more than once"
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

    assert!(error.to_string().contains("remote tool `echo`"));
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
