//! Opt-in live validation of a Codex dynamic tool call through a RAH MCP tool.
//!
//! This example requires the exactly supported Codex CLI version, live model
//! access, and a prebuilt `rah-mcp-echo-server` binary. It is intentionally
//! excluded from the normal deterministic test suite.

use std::{env, path::PathBuf, process::ExitCode, sync::Arc, time::Duration};

use futures::StreamExt;
use rah_protocol::{
    AgentEvent, AgentInput, AgentOptions, AgentRequest, Message, MessageRole, PermissionLevel,
    RequestId, ToolName,
};
use rah_runtime::AgentRuntime;
use rah_runtime_codex::{CodexRuntime, SUPPORTED_CODEX_VERSION};
use rah_tools::ToolRegistry;
use rah_tools_mcp::{MCP_PROTOCOL_VERSION, McpAdapter, McpServerConfig};
use serde_json::{Value, json};

#[path = "support/live_gate_contract.rs"]
mod live_gate_contract;
use live_gate_contract::{
    CopiedFixture, ProofEvent, certification_token, require_token_hidden, verify_tool_proof,
};

const ADVERTISED_ALIAS: &str = "rah_tool_0";
const EXPECTED_RAH_TOOL: &str = "mcp.test.echo";
const CERTIFICATION_REQUEST: &str = "certification-token";
const PROMPT: &str = "Call the available RAH certification tool exactly once with exactly {\"request\":\"certification-token\"}. After receiving the tool result, continue with a brief confirmation. Do not request any other tool.";
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
    let token = certification_token();
    let fixture = CopiedFixture::create(&mcp_executable, "mcp-certification")?;
    let mcp_executable = fixture.executable().to_owned();

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
            .with_arg("--live-certification")
            .with_arg("--certification-token")
            .with_arg(&token)
            .with_expected_tool(
                "echo",
                json!({
                    "type": "object",
                    "properties": {"request": {"type": "string", "enum": [CERTIFICATION_REQUEST]}},
                    "required": ["request"],
                    "additionalProperties": false
                }),
                PermissionLevel::None,
            )
            .map_err(|error| format!("invalid MCP tool permission configuration: {error}"))?,
    )
    .await
    .map_err(|error| format!("MCP connection failed: {error}"))?;
    let tools = adapter.tools();
    verify_discovery(&tools)?;

    let definition = tools[0].definition();
    require_token_hidden(
        &token,
        PROMPT,
        &definition.description,
        &definition.input_schema,
        &json!({"fixture": true}),
    )?;
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
            let turn_result = run_turn(Arc::clone(&runtime), &token).await;
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
    let provider_lifecycle_calls = fixture.finish()?;
    println!("MCP_PROVIDER_LIFECYCLE_CALLS {provider_lifecycle_calls}");
    println!("MCP_CHILD_REAPED_AND_UNLOCKED true");
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

async fn run_turn(runtime: Arc<CodexRuntime>, token: &str) -> Result<(), String> {
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
    let mut server_audit = None;
    let mut proof_events = Vec::new();

    tokio::time::timeout(TERMINAL_TIMEOUT, async {
        while let Some(event) = events.next().await {
            println!(
                "RAH_EVENT {}",
                serde_json::to_string(&event).map_err(|error| error.to_string())?
            );
            observed.push(event_name(&event));
            match event {
                AgentEvent::ToolRequested { tool_call, .. } => {
                    requested += 1;
                    proof_events.push(ProofEvent::ToolRequested {
                        name: tool_call.name.to_string(),
                        arguments: tool_call.input.0.clone(),
                    });
                    println!(
                        "RESOLVED_RAH_TOOL_CALL name={} input={}",
                        tool_call.name, tool_call.input.0
                    );
                    if tool_call.name.as_str() != EXPECTED_RAH_TOOL
                        || tool_call.input.0 != json!({"request": CERTIFICATION_REQUEST})
                    {
                        return Err(format!("unexpected RAH ToolCall: {tool_call:?}"));
                    }
                }
                AgentEvent::ToolStarted { .. } => {
                    started += 1;
                    proof_events.push(ProofEvent::ToolStarted);
                }
                AgentEvent::ToolFinished { output, .. } => {
                    finished += 1;
                    if output.is_error {
                        return Err("MCP echo returned an error output".to_owned());
                    }
                    let output_text = output
                        .content
                        .iter()
                        .find_map(|content| match content {
                            rah_protocol::ToolContent::Text(text) => Some(text.clone()),
                            _ => None,
                        })
                        .ok_or_else(|| "MCP ToolOutput did not contain text".to_owned())?;
                    server_audit = output.content.iter().find_map(|content| match content {
                        rah_protocol::ToolContent::Json(audit) => Some(audit.clone()),
                        _ => None,
                    });
                    proof_events.push(ProofEvent::ToolFinished {
                        is_error: output.is_error,
                        output_text,
                    });
                }
                AgentEvent::ModelDelta { .. } => {
                    proof_events.push(ProofEvent::ModelDelta);
                }
                AgentEvent::Completed { output, .. } => {
                    final_text = Some(output.message.content);
                    proof_events.push(ProofEvent::Completed);
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
                AgentEvent::Started { .. } | AgentEvent::ModelRequestStarted { .. } => {}
            }
        }
        Ok::<(), String>(())
    })
    .await
    .map_err(|_| "timed out waiting for terminal RAH event".to_owned())??;

    let provider_execution_count = server_audit
        .as_ref()
        .and_then(|audit| audit["bridgeCertificationCalls"].as_u64())
        .unwrap_or_default();
    println!("TOOL_LIFECYCLE_COUNTS requested={requested} started={started} finished={finished}");
    println!("MCP_EXECUTION_COUNT {provider_execution_count}");
    verify_tool_proof(
        "MCP",
        EXPECTED_RAH_TOOL,
        &json!({"request": CERTIFICATION_REQUEST}),
        token,
        &proof_events,
        provider_execution_count,
    )?;
    let audit = server_audit.ok_or_else(|| "missing MCP server audit result".to_owned())?;
    verify_server_audit(&audit)?;
    let final_text = final_text.ok_or_else(|| "missing Completed output".to_owned())?;
    if observed.last() != Some(&"Completed") {
        return Err(format!("Completed was not terminal: {observed:?}"));
    }

    println!("RAH_EVENT_SEQUENCE {}", observed.join(" -> "));
    println!(
        "MCP_SERVER_RECEIVED_ARGUMENTS {}",
        audit["receivedArguments"]
    );
    println!("RAH_TOOL_OUTPUT hidden_token_verified=true is_error=false structured_audit=true");
    println!("DYNAMIC_TOOL_CALL_RESPONSE success=true content_items=2 types=inputText,inputText");
    println!("CODEX_CONTINUED_AFTER_MCP_RESULT true");
    println!("FINAL_ASSISTANT_TEXT {final_text:?}");
    println!("TERMINAL_RAH_EVENT Completed");
    println!("PROHIBITED_ACTIONS none observed; restricted bridge configuration remained active");
    Ok(())
}

fn verify_server_audit(audit: &Value) -> Result<(), String> {
    if audit["bridgeCertificationCalls"] != 1
        || audit["receivedArguments"] != json!({"request": CERTIFICATION_REQUEST})
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
