//! Opt-in live validation of a Codex dynamic tool call through a RAH process plugin.
//!
//! This example requires the exactly supported Codex CLI version, live model
//! access, and a prebuilt `rah-plugin-echo` fixture. It is intentionally excluded
//! from the normal deterministic test suite.

use std::{
    collections::BTreeSet, env, path::PathBuf, process::ExitCode, sync::Arc, time::Duration,
};

use futures::StreamExt;
use rah_protocol::{
    AgentEvent, AgentInput, AgentOptions, AgentRequest, Message, MessageRole, PermissionLevel,
    RequestId, ToolName,
};
use rah_runtime::AgentRuntime;
use rah_runtime_codex::{CodexRuntime, SUPPORTED_CODEX_VERSION};
use rah_tools::{ExternalToolIdentity, ExternalToolPermissionPolicy, ToolRegistry};
use rah_tools_plugin::{PLUGIN_PROTOCOL_VERSION, PluginAdapter, PluginConfig};
use serde_json::{Value, json};

#[path = "support/live_gate_contract.rs"]
mod live_gate_contract;
use live_gate_contract::{
    CopiedFixture, ProofEvent, certification_token, require_token_hidden, verify_tool_proof,
};

const ADVERTISED_ALIAS: &str = "rah_tool_0";
const EXPECTED_RAH_TOOL: &str = "plugin.test.echo";
const EXTERNAL_IDENTITY: &str = "plugin:test:echo";
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
            eprintln!("LIVE_PLUGIN_ECHO_BRIDGE_FAIL failed to create Tokio runtime: {error}");
            return ExitCode::FAILURE;
        }
    };

    match runtime.block_on(run()) {
        Ok(()) => {
            println!("LIVE_PLUGIN_ECHO_BRIDGE_PASS");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("LIVE_PLUGIN_ECHO_BRIDGE_FAIL {error}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), String> {
    let codex_executable = env::var_os("RAH_CODEX_EXECUTABLE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("codex"));
    let plugin_executable = env::var_os("RAH_PLUGIN_ECHO_EXECUTABLE")
        .map(PathBuf::from)
        .unwrap_or_else(default_plugin_executable);
    let token = certification_token();
    let fixture = CopiedFixture::create(&plugin_executable, "plugin-certification")?;
    let plugin_executable = fixture.executable().to_owned();

    let identity = ExternalToolIdentity::new(EXTERNAL_IDENTITY)
        .map_err(|error| format!("invalid external tool identity: {error}"))?;
    let mut permission_policy = ExternalToolPermissionPolicy::new();
    permission_policy
        .assign(identity.clone(), PermissionLevel::None)
        .map_err(|error| format!("invalid external tool permission: {error}"))?;
    if permission_policy.permission_for(&identity) != Some(PermissionLevel::None) {
        return Err("host permission assignment was not exactly None".to_owned());
    }

    println!("CODEX_EXECUTABLE {}", codex_executable.display());
    println!("REQUIRED_CODEX_VERSION {SUPPORTED_CODEX_VERSION}");
    println!("PLUGIN_PROTOCOL_VERSION {PLUGIN_PROTOCOL_VERSION}");
    println!("PLUGIN_EXECUTABLE {}", plugin_executable.display());
    println!("CONFIGURED_PLUGIN_ID test");
    println!("CONFIGURED_PLUGIN_VERSION 0.1.0");
    println!("REMOTE_TOOL echo");
    println!("EXTERNAL_TOOL_IDENTITY {}", identity.as_str());
    println!("HOST_PERMISSION_ASSIGNMENT {} -> None", identity.as_str());
    println!("HOST_PERMISSION_ALLOWLIST {:?}", [PermissionLevel::None]);
    println!("PROHIBITED_PERMISSIONS Read=false Write=false Execute=false");
    println!("PLUGIN_ENVIRONMENT_ALLOWLIST RAH_PLUGIN_PROTOCOL,SystemRoot(windows-only)");
    println!("PLUGIN_AUTO_RESTART false");
    println!("EXPERIMENTAL_API true");
    println!("ADVERTISED_CODEX_ALIAS {ADVERTISED_ALIAS}");
    println!("PRIVATE_ALIAS_MAPPING {ADVERTISED_ALIAS} -> {EXPECTED_RAH_TOOL}");
    println!(
        "PROMPT {}",
        serde_json::to_string(PROMPT).map_err(|error| error.to_string())?
    );

    let adapter = PluginAdapter::connect(
        PluginConfig::stdio("test", "0.1.0", &plugin_executable)
            .map_err(|error| format!("invalid plugin configuration: {error}"))?
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
            .map_err(|error| format!("invalid plugin tool permission configuration: {error}"))?,
    )
    .await
    .map_err(|error| format!("plugin connection failed: {error}"))?;
    let isolated_cwd = adapter.diagnostics().cwd;
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
    println!("DISCOVERED_PLUGIN_TOOLS {}", tools.len());
    println!("RESOLVED_RAH_TOOL_NAME {}", definition.name);
    println!("RAH_TOOL_PERMISSION {:?}", definition.permission);
    println!("PLUGIN_ISOLATED_CWD {}", isolated_cwd.display());
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
        .map_err(|error| format!("failed to register plugin echo tool: {error}"))?;

    let runtime_result = CodexRuntime::connect_tool_bridge(
        &codex_executable,
        Arc::new(registry),
        vec![PermissionLevel::None],
    )
    .await;
    let live_result = match runtime_result {
        Ok(runtime) => {
            let runtime = Arc::new(runtime);
            let turn_result = run_turn(Arc::clone(&runtime), &adapter, &token).await;
            let shutdown_result = runtime.shutdown().await;
            match &shutdown_result {
                Ok(()) => {
                    println!("CODEX_PROCESS_CLEANUP app-server shutdown completed and reaped")
                }
                Err(error) => eprintln!("CODEX_PROCESS_CLEANUP_FAIL {error}"),
            }
            shutdown_result
                .map_err(|error| format!("app-server shutdown failed: {error}"))
                .and(turn_result)
        }
        Err(error) => Err(format!("Codex connection failed: {error}")),
    };

    let plugin_shutdown = adapter.shutdown().await;
    match &plugin_shutdown {
        Ok(()) => println!("PLUGIN_PROCESS_CLEANUP echo fixture shutdown completed and reaped"),
        Err(error) => eprintln!("PLUGIN_PROCESS_CLEANUP_FAIL {error}"),
    }
    plugin_shutdown.map_err(|error| format!("plugin shutdown failed: {error}"))?;
    if isolated_cwd.exists() {
        return Err(format!(
            "isolated plugin working directory was not removed: {}",
            isolated_cwd.display()
        ));
    }
    let provider_lifecycle_calls = fixture.finish()?;
    println!("PLUGIN_PROVIDER_LIFECYCLE_CALLS {provider_lifecycle_calls}");
    println!("PLUGIN_CHILD_REAPED_AND_UNLOCKED true");
    println!("PLUGIN_TEMPORARY_DIRECTORY_CLEANUP removed");
    live_result
}

fn verify_discovery(tools: &[Arc<dyn rah_tools::Tool>]) -> Result<(), String> {
    if tools.len() != 1 {
        return Err(format!(
            "expected one discovered plugin tool, found {}",
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
            "unexpected plugin tool permission: {:?}",
            definition.permission
        ));
    }
    if definition.input_schema
        != json!({
            "type": "object",
            "properties": {"request": {"type": "string", "enum": [CERTIFICATION_REQUEST]}},
            "required": ["request"],
            "additionalProperties": false
        })
    {
        return Err(format!(
            "unexpected plugin tool input schema: {}",
            definition.input_schema
        ));
    }
    Ok(())
}

async fn run_turn(
    runtime: Arc<CodexRuntime>,
    adapter: &PluginAdapter,
    token: &str,
) -> Result<(), String> {
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
    let mut tool_output = None;
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
                        return Err("plugin echo returned an error output".to_owned());
                    }
                    let output_text = output
                        .content
                        .iter()
                        .find_map(|content| match content {
                            rah_protocol::ToolContent::Text(text) => Some(text.clone()),
                            _ => None,
                        })
                        .ok_or_else(|| "plugin ToolOutput did not contain text".to_owned())?;
                    proof_events.push(ProofEvent::ToolFinished {
                        is_error: output.is_error,
                        output_text,
                    });
                    tool_output = Some(output);
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
                AgentEvent::Cancelled { .. } => return Err("turn was cancelled".to_owned()),
                AgentEvent::Started { .. } | AgentEvent::ModelRequestStarted { .. } => {}
            }
        }
        Ok::<(), String>(())
    })
    .await
    .map_err(|_| "timed out waiting for terminal RAH event".to_owned())??;

    let audit = wait_for_plugin_audit(adapter).await?;
    println!("TOOL_LIFECYCLE_COUNTS requested={requested} started={started} finished={finished}");
    println!(
        "PLUGIN_EXECUTION_COUNT {}",
        audit
            .as_ref()
            .and_then(|audit| audit["execution_count"].as_u64())
            .unwrap_or_default()
    );
    verify_tool_proof(
        "Process Plugin",
        EXPECTED_RAH_TOOL,
        &json!({"request": CERTIFICATION_REQUEST}),
        token,
        &proof_events,
        audit
            .as_ref()
            .and_then(|audit| audit["execution_count"].as_u64())
            .unwrap_or_default(),
    )?;
    let audit = audit.ok_or_else(|| "missing plugin provider audit result".to_owned())?;
    verify_plugin_audit(&audit, adapter)?;
    let final_text = final_text.ok_or_else(|| "missing Completed output".to_owned())?;
    if observed.last() != Some(&"Completed") {
        return Err(format!("Completed was not terminal: {observed:?}"));
    }

    println!("CODEX_ITEM_TOOL_CALL_COUNT {requested}");
    println!("RAH_EVENT_SEQUENCE {}", observed.join(" -> "));
    println!("PLUGIN_TOOLS_CALL_PAYLOAD {}", audit["tools_call"]);
    println!("PLUGIN_RECEIVED_ARGUMENTS {}", audit["received_arguments"]);
    println!("PLUGIN_AUDIT_CWD {}", audit["cwd"]);
    println!(
        "PLUGIN_AUDIT_ENVIRONMENT_NAMES {}",
        environment_names(&audit["environment"])?
    );
    let _ = tool_output.expect("validated ToolOutput");
    println!("RAH_TOOL_OUTPUT hidden_token_verified=true is_error=false");
    println!("DYNAMIC_TOOL_CALL_RESPONSE success=true content_items=1 type=inputText");
    println!("CODEX_CONTINUED_AFTER_PLUGIN_RESULT true");
    println!("FINAL_ASSISTANT_TEXT {final_text:?}");
    println!("TERMINAL_RAH_EVENT Completed");
    println!("PROHIBITED_ACTIONS none observed; restricted bridge configuration remained active");
    println!("PLUGIN_RESTART_OR_REPLAY false");
    Ok(())
}

async fn wait_for_plugin_audit(adapter: &PluginAdapter) -> Result<Option<Value>, String> {
    match tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let diagnostics = adapter.diagnostics();
            for line in diagnostics.stderr.lines() {
                if let Some(payload) = line.strip_prefix("RAH_PLUGIN_AUDIT ") {
                    return serde_json::from_str(payload)
                        .map_err(|error| format!("invalid plugin audit JSON: {error}"));
                }
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    {
        Ok(result) => result.map(Some),
        Err(_) => Ok(None),
    }
}

fn verify_plugin_audit(audit: &Value, adapter: &PluginAdapter) -> Result<(), String> {
    if audit["execution_count"] != 1
        || audit["received_arguments"] != json!([{"request": CERTIFICATION_REQUEST}])
        || audit["tools_call"]["method"] != "tools/call"
        || audit["tools_call"]["params"]["name"] != "echo"
        || audit["tools_call"]["params"]["arguments"] != json!({"request": CERTIFICATION_REQUEST})
    {
        return Err(format!("unexpected plugin audit result: {audit}"));
    }
    let cwd = PathBuf::from(
        audit["cwd"]
            .as_str()
            .ok_or_else(|| "plugin audit cwd was not a string".to_owned())?,
    );
    let repository = std::fs::canonicalize(env::current_dir().map_err(|error| error.to_string())?)
        .map_err(|error| error.to_string())?;
    let canonical_cwd = std::fs::canonicalize(&cwd).map_err(|error| error.to_string())?;
    if canonical_cwd == repository || canonical_cwd != adapter.diagnostics().cwd {
        return Err(format!("plugin cwd was not isolated: {}", cwd.display()));
    }
    let names = environment_name_set(&audit["environment"])?;
    let expected = if cfg!(windows) {
        BTreeSet::from(["RAH_PLUGIN_PROTOCOL".to_owned(), "SYSTEMROOT".to_owned()])
    } else {
        BTreeSet::from(["RAH_PLUGIN_PROTOCOL".to_owned()])
    };
    if names != expected || audit["environment"]["RAH_PLUGIN_PROTOCOL"] != PLUGIN_PROTOCOL_VERSION {
        return Err(format!("plugin environment was not minimized: {audit}"));
    }
    for prohibited in [
        "PATH",
        "HTTP_PROXY",
        "HTTPS_PROXY",
        "ALL_PROXY",
        "NO_PROXY",
        "OPENAI_API_KEY",
        "CODEX_API_KEY",
        "GITHUB_TOKEN",
        "AWS_ACCESS_KEY_ID",
        "AZURE_TOKEN",
    ] {
        if names.contains(prohibited) {
            return Err(format!(
                "prohibited environment variable was visible: {prohibited}"
            ));
        }
    }
    Ok(())
}

fn environment_names(environment: &Value) -> Result<String, String> {
    Ok(environment_name_set(environment)?
        .into_iter()
        .collect::<Vec<_>>()
        .join(","))
}

fn environment_name_set(environment: &Value) -> Result<BTreeSet<String>, String> {
    let object = environment
        .as_object()
        .ok_or_else(|| "plugin audit environment was not an object".to_owned())?;
    Ok(object
        .keys()
        .map(|name| name.to_ascii_uppercase())
        .collect())
}

fn default_plugin_executable() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target")
        .join("debug")
        .join(format!("rah-plugin-echo{}", env::consts::EXE_SUFFIX))
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
