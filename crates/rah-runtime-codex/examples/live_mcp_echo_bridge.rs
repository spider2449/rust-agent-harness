//! Opt-in live validation of a Codex dynamic tool call through a RAH MCP tool.
//!
//! This example requires the exactly supported Codex CLI version, live model
//! access, and a prebuilt `rah-mcp-echo-server` binary. It is intentionally
//! excluded from the normal deterministic test suite.

use std::{env, path::PathBuf, process::ExitCode, sync::Arc, time::Duration};

use futures::StreamExt;
use rah_protocol::{
    AgentEvent, AgentInput, AgentOptions, AgentRequest, Message, MessageRole, PermissionLevel,
    RequestId, ToolContent, ToolName,
};
use rah_runtime::AgentRuntime;
use rah_runtime_codex::{CodexRuntime, SUPPORTED_CODEX_VERSION};
use rah_tools::ToolRegistry;
use rah_tools_mcp::{MCP_PROTOCOL_VERSION, McpAdapter, McpServerConfig};
use serde_json::{Value, json};

const ADVERTISED_ALIAS: &str = "rah_tool_0";
const EXPECTED_RAH_TOOL: &str = "mcp.test.echo";
const EXPECTED_TEXT: &str = "RAH_MCP_BRIDGE_OK";
const PROMPT: &str = "Use the available RAH tool exactly once to echo:\nRAH_MCP_BRIDGE_OK\n\nAfter receiving the tool result, reply with exactly:\nRAH_MCP_BRIDGE_OK\n\nDo not request any other tool.";
const TERMINAL_TIMEOUT: Duration = Duration::from_secs(120);

fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter("rah=debug")
        .with_target(true)
        .without_time()
        .init();

    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("LIVE_MCP_ECHO_BRIDGE_FAIL failed to create Tokio runtime: {error}");
            return ExitCode::FAILURE;
        }
    };

    match runtime.block_on(run()) {
        Ok(()) => {
            println!("LIVE_MCP_ECHO_BRIDGE_PASS");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("LIVE_MCP_ECHO_BRIDGE_FAIL {error}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), String> {
    let codex_executable = env::var_os("RAH_CODEX_EXECUTABLE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("codex"));
    let mcp_executable = env::var_os("RAH_MCP_ECHO_SERVER")
        .map(PathBuf::from)
        .unwrap_or_else(default_mcp_executable);

    println!("CODEX_EXECUTABLE {}", codex_executable.display());
    println!("REQUIRED_CODEX_VERSION {SUPPORTED_CODEX_VERSION}");
    println!("MCP_PROTOCOL_VERSION {MCP_PROTOCOL_VERSION}");
    println!("MCP_SERVER_COMMAND {}", mcp_executable.display());
    println!("MCP_TRANSPORT stdio");
    println!("EXPERIMENTAL_API true");
    println!("ADVERTISED_CODEX_ALIAS {ADVERTISED_ALIAS}");
    println!("PRIVATE_ALIAS_MAPPING {ADVERTISED_ALIAS} -> {EXPECTED_RAH_TOOL}");
    println!("HOST_PERMISSION_ALLOWLIST {:?}", [PermissionLevel::None]);
    println!("PROHIBITED_PERMISSIONS Read=false Write=false Execute=false");
    println!(
        "PROMPT {}",
        serde_json::to_string(PROMPT).map_err(|error| error.to_string())?
    );

    let adapter = McpAdapter::connect(
        McpServerConfig::stdio("test", &mcp_executable)
            .map_err(|error| format!("invalid MCP server configuration: {error}"))?
            .with_tool_permission("echo", PermissionLevel::None)
            .map_err(|error| format!("invalid MCP tool permission configuration: {error}"))?,
    )
    .await
    .map_err(|error| format!("MCP connection failed: {error}"))?;
    let tools = adapter.tools();
    verify_discovery(&tools)?;

    let definition = tools[0].definition();
    println!("DISCOVERED_MCP_TOOLS {}", tools.len());
    println!("RESOLVED_RAH_TOOL_NAME {}", definition.name);
    println!("RAH_TOOL_PERMISSION {:?}", definition.permission);
    println!(
        "ADVERTISED_DYNAMIC_TOOL {}",
        json!({
            "type": "function",
            "name": ADVERTISED_ALIAS,
            "description": definition.description,
            "inputSchema": definition.input_schema,
            "deferLoading": false
        })
    );

    let mut registry = ToolRegistry::new();
    registry
        .register(Arc::clone(&tools[0]))
        .map_err(|error| format!("failed to register MCP echo tool: {error}"))?;

    let runtime_result = CodexRuntime::connect_tool_bridge(
        &codex_executable,
        Arc::new(registry),
        vec![PermissionLevel::None],
    )
    .await;
    let live_result = match runtime_result {
        Ok(runtime) => {
            let runtime = Arc::new(runtime);
            let turn_result = run_turn(Arc::clone(&runtime)).await;
            let shutdown_result = runtime.shutdown().await;
            match &shutdown_result {
                Ok(()) => println!("CODEX_PROCESS_CLEANUP app-server shutdown completed"),
                Err(error) => eprintln!("CODEX_PROCESS_CLEANUP_FAIL {error}"),
            }
            shutdown_result
                .map_err(|error| format!("app-server shutdown failed: {error}"))
                .and(turn_result)
        }
        Err(error) => Err(format!("Codex connection failed: {error}")),
    };

    let mcp_shutdown = adapter.shutdown().await;
    match &mcp_shutdown {
        Ok(()) => println!("MCP_PROCESS_CLEANUP echo-server shutdown completed"),
        Err(error) => eprintln!("MCP_PROCESS_CLEANUP_FAIL {error}"),
    }
    mcp_shutdown.map_err(|error| format!("MCP server shutdown failed: {error}"))?;
    live_result
}

fn verify_discovery(tools: &[Arc<dyn rah_tools::Tool>]) -> Result<(), String> {
    if tools.len() != 1 {
        return Err(format!(
            "expected one discovered MCP tool, found {}",
            tools.len()
        ));
    }
    let definition = tools[0].definition();
    if definition.name != ToolName::new(EXPECTED_RAH_TOOL) {
        return Err(format!(
            "unexpected discovered RAH tool: {}",
            definition.name
        ));
    }
    if definition.permission != PermissionLevel::None {
        return Err(format!(
            "unexpected MCP tool permission: {:?}",
            definition.permission
        ));
    }
    Ok(())
}

async fn run_turn(runtime: Arc<CodexRuntime>) -> Result<(), String> {
    let handle = runtime
        .start(AgentRequest {
            request_id: RequestId::new(),
            input: AgentInput {
                messages: vec![Message {
                    role: MessageRole::User,
                    content: PROMPT.to_owned(),
                }],
            },
            options: AgentOptions::default(),
        })
        .await
        .map_err(|error| format!("failed to start agent request: {error}"))?;
    let mut events = handle.into_events();
    let mut observed = Vec::new();
    let mut requested = 0_usize;
    let mut started = 0_usize;
    let mut finished = 0_usize;
    let mut final_text = None;
    let mut finished_index = None;
    let mut continuation_index = None;
    let mut server_audit = None;

    tokio::time::timeout(TERMINAL_TIMEOUT, async {
        while let Some(event) = events.next().await {
            println!(
                "RAH_EVENT {}",
                serde_json::to_string(&event).map_err(|error| error.to_string())?
            );
            let index = observed.len();
            observed.push(event_name(&event));
            match event {
                AgentEvent::ToolRequested { tool_call, .. } => {
                    requested += 1;
                    println!(
                        "RESOLVED_RAH_TOOL_CALL name={} input={}",
                        tool_call.name, tool_call.input.0
                    );
                    if tool_call.name.as_str() != EXPECTED_RAH_TOOL
                        || tool_call.input.0 != json!({"text": EXPECTED_TEXT})
                    {
                        return Err(format!("unexpected RAH ToolCall: {tool_call:?}"));
                    }
                }
                AgentEvent::ToolStarted { .. } => started += 1,
                AgentEvent::ToolFinished { output, .. } => {
                    finished += 1;
                    finished_index = Some(index);
                    if output.is_error {
                        return Err("MCP echo returned an error output".to_owned());
                    }
                    match output.content.as_slice() {
                        [ToolContent::Text(text), ToolContent::Json(audit)]
                            if text == EXPECTED_TEXT =>
                        {
                            server_audit = Some(audit.clone());
                        }
                        content => return Err(format!("unexpected MCP ToolOutput: {content:?}")),
                    }
                }
                AgentEvent::ModelDelta { .. } if finished_index.is_some() => {
                    continuation_index.get_or_insert(index);
                }
                AgentEvent::Completed { output, .. } => {
                    final_text = Some(output.message.content);
                }
                AgentEvent::ApprovalRequired { .. } => {
                    return Err("Codex requested an approval".to_owned());
                }
                AgentEvent::Failed { message, .. } => {
                    return Err(format!("runtime failed closed: {message}"));
                }
                AgentEvent::Cancelled { .. } => {
                    return Err("turn was cancelled".to_owned());
                }
                AgentEvent::Started { .. }
                | AgentEvent::ModelRequestStarted { .. }
                | AgentEvent::ModelDelta { .. } => {}
            }
        }
        Ok::<(), String>(())
    })
    .await
    .map_err(|_| "timed out waiting for terminal RAH event".to_owned())??;

    if requested != 1 || started != 1 || finished != 1 {
        return Err(format!(
            "unexpected tool lifecycle counts: requested={requested}, started={started}, finished={finished}"
        ));
    }
    let audit = server_audit.ok_or_else(|| "missing MCP server audit result".to_owned())?;
    verify_server_audit(&audit)?;
    if continuation_index.is_none() {
        return Err("no Codex model continuation followed ToolFinished".to_owned());
    }
    let final_text = final_text.ok_or_else(|| "missing Completed output".to_owned())?;
    if final_text != EXPECTED_TEXT {
        return Err(format!("unexpected final assistant text: {final_text:?}"));
    }
    if observed.last() != Some(&"Completed") {
        return Err(format!("Completed was not terminal: {observed:?}"));
    }

    println!("RAH_EVENT_SEQUENCE {}", observed.join(" -> "));
    println!("TOOL_LIFECYCLE_COUNTS requested={requested} started={started} finished={finished}");
    println!("MCP_EXECUTION_COUNT {}", audit["bridgeEchoCalls"]);
    println!(
        "MCP_SERVER_RECEIVED_ARGUMENTS {}",
        audit["receivedArguments"]
    );
    println!("RAH_TOOL_OUTPUT text={EXPECTED_TEXT:?} is_error=false structured_audit=true");
    println!("DYNAMIC_TOOL_CALL_RESPONSE success=true content_items=2 types=inputText,inputText");
    println!("CODEX_CONTINUED_AFTER_MCP_RESULT true");
    println!("FINAL_ASSISTANT_TEXT {final_text:?}");
    println!("TERMINAL_RAH_EVENT Completed");
    println!("PROHIBITED_ACTIONS none observed; restricted bridge configuration remained active");
    Ok(())
}

fn verify_server_audit(audit: &Value) -> Result<(), String> {
    if audit["bridgeEchoCalls"] != 1 || audit["receivedArguments"] != json!({"text": EXPECTED_TEXT})
    {
        return Err(format!("unexpected MCP server audit result: {audit}"));
    }
    Ok(())
}

fn default_mcp_executable() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target")
        .join("debug")
        .join(format!("rah-mcp-echo-server{}", env::consts::EXE_SUFFIX))
}

fn event_name(event: &AgentEvent) -> &'static str {
    match event {
        AgentEvent::Started { .. } => "Started",
        AgentEvent::ModelRequestStarted { .. } => "ModelRequestStarted",
        AgentEvent::ModelDelta { .. } => "ModelDelta",
        AgentEvent::ToolRequested { .. } => "ToolRequested",
        AgentEvent::ToolStarted { .. } => "ToolStarted",
        AgentEvent::ToolFinished { .. } => "ToolFinished",
        AgentEvent::ApprovalRequired { .. } => "ApprovalRequired",
        AgentEvent::Completed { .. } => "Completed",
        AgentEvent::Failed { .. } => "Failed",
        AgentEvent::Cancelled { .. } => "Cancelled",
    }
}
