//! Opt-in live echo validation for the Codex dynamic Tool Bridge.
//!
//! This example requires the exactly supported Codex CLI version and live model
//! access. It is intentionally excluded from the normal deterministic test suite.

use std::{
    env,
    path::PathBuf,
    process::ExitCode,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use async_trait::async_trait;
use futures::StreamExt;
use rah_protocol::{
    AgentEvent, AgentInput, AgentOptions, AgentRequest, Message, MessageRole, PermissionLevel,
    RequestId, ToolContent, ToolInput, ToolOutput,
};
use rah_runtime::AgentRuntime;
use rah_runtime_codex::{CodexRuntime, SUPPORTED_CODEX_VERSION};
use rah_tools::{EchoTool, Tool, ToolContext, ToolError, ToolRegistry};
use serde_json::json;

const EXPECTED_TEXT: &str = "RAH_TOOL_BRIDGE_OK";
const PROMPT: &str = "You have an echo tool.\nCall the echo tool exactly once with:\n{\"text\":\"RAH_TOOL_BRIDGE_OK\"}\nAfter receiving the tool result, reply with exactly the returned text.";
const TERMINAL_TIMEOUT: Duration = Duration::from_secs(120);

struct CountingEchoTool {
    executions: Arc<AtomicUsize>,
}

#[async_trait]
impl Tool for CountingEchoTool {
    fn definition(&self) -> rah_protocol::ToolDefinition {
        EchoTool::new().definition()
    }

    async fn execute(
        &self,
        input: ToolInput,
        context: ToolContext,
    ) -> Result<ToolOutput, ToolError> {
        self.executions.fetch_add(1, Ordering::SeqCst);
        EchoTool::new().execute(input, context).await
    }
}

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
            eprintln!("LIVE_ECHO_BRIDGE_FAIL failed to create Tokio runtime: {error}");
            return ExitCode::FAILURE;
        }
    };

    match runtime.block_on(run()) {
        Ok(()) => {
            println!("LIVE_ECHO_BRIDGE_PASS");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("LIVE_ECHO_BRIDGE_FAIL {error}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), String> {
    let executable = env::var_os("RAH_CODEX_EXECUTABLE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("codex"));
    let executions = Arc::new(AtomicUsize::new(0));
    let mut registry = ToolRegistry::new();
    registry
        .register(Arc::new(CountingEchoTool {
            executions: Arc::clone(&executions),
        }))
        .map_err(|error| format!("failed to register echo: {error}"))?;

    println!("CODEX_EXECUTABLE {}", executable.display());
    println!("REQUIRED_CODEX_VERSION {SUPPORTED_CODEX_VERSION}");
    println!("EXPERIMENTAL_API true");
    println!("ALLOWED_PERMISSIONS {:?}", [PermissionLevel::None]);
    println!(
        "ADVERTISED_DYNAMIC_TOOL {}",
        json!({
            "type": "function",
            "name": "echo",
            "description": "Returns the supplied text unchanged.",
            "inputSchema": {
                "type": "object",
                "properties": { "text": { "type": "string" } },
                "required": ["text"],
                "additionalProperties": false
            },
            "deferLoading": false
        })
    );
    println!(
        "PROMPT {}",
        serde_json::to_string(PROMPT).map_err(|error| error.to_string())?
    );

    let runtime = Arc::new(
        CodexRuntime::connect_echo_bridge(&executable, Arc::new(registry))
            .await
            .map_err(|error| format!("connection failed: {error}"))?,
    );
    let live_result = run_turn(Arc::clone(&runtime), &executions).await;
    let shutdown_result = runtime.shutdown().await;

    match &shutdown_result {
        Ok(()) => println!("PROCESS_CLEANUP app-server shutdown completed"),
        Err(error) => eprintln!("PROCESS_CLEANUP_FAIL {error}"),
    }
    shutdown_result.map_err(|error| format!("app-server shutdown failed: {error}"))?;
    live_result
}

async fn run_turn(runtime: Arc<CodexRuntime>, executions: &AtomicUsize) -> Result<(), String> {
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
    let mut tool_text = None;
    let mut final_text = None;
    let mut finished_index = None;
    let mut continuation_index = None;

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
                    if tool_call.name.as_str() != "echo"
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
                        return Err("EchoTool returned an error output".to_owned());
                    }
                    tool_text = match output.content.as_slice() {
                        [ToolContent::Text(text)] => Some(text.clone()),
                        content => return Err(format!("unexpected EchoTool output: {content:?}")),
                    };
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

    let execution_count = executions.load(Ordering::SeqCst);
    let final_text = final_text.ok_or_else(|| "missing Completed output".to_owned())?;
    if requested != 1 || started != 1 || finished != 1 {
        return Err(format!(
            "unexpected tool lifecycle counts: requested={requested}, started={started}, finished={finished}"
        ));
    }
    if execution_count != 1 {
        return Err(format!("EchoTool execution count was {execution_count}"));
    }
    if tool_text.as_deref() != Some(EXPECTED_TEXT) {
        return Err(format!("unexpected returned tool text: {tool_text:?}"));
    }
    if continuation_index.is_none() {
        return Err("no Codex model continuation followed ToolFinished".to_owned());
    }
    if final_text != EXPECTED_TEXT {
        return Err(format!("unexpected final assistant text: {final_text:?}"));
    }
    if observed.last() != Some(&"Completed") {
        return Err(format!("Completed was not terminal: {observed:?}"));
    }

    println!("RAH_EVENT_SEQUENCE {}", observed.join(" -> "));
    println!("TOOL_LIFECYCLE_COUNTS requested={requested} started={started} finished={finished}");
    println!("ECHO_EXECUTION_COUNT {execution_count}");
    println!("TOOL_RETURNED_TEXT {tool_text:?}");
    println!("CODEX_CONTINUED_AFTER_TOOL_RESPONSE true");
    println!("FINAL_ASSISTANT_TEXT {final_text:?}");
    println!("TERMINAL_RAH_EVENT Completed");
    println!("PROHIBITED_ACTIONS none observed; restricted runtime remained active");
    Ok(())
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
