//! Opt-in live `fs.read` validation for the Codex dynamic Tool Bridge.
//!
//! This example requires the exactly supported Codex CLI version and live model
//! access. It is intentionally excluded from the normal deterministic test suite.

use std::{
    env,
    path::{Path, PathBuf},
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
    RequestId, ToolContent, ToolDefinition, ToolInput, ToolOutput,
};
use rah_runtime::AgentRuntime;
use rah_runtime_codex::{CodexRuntime, SUPPORTED_CODEX_VERSION};
use rah_tools::{FsReadTool, Tool, ToolContext, ToolError, ToolRegistry};
use serde_json::json;

const ADVERTISED_ALIAS: &str = "rah_tool_0";
const EXPECTED_RAH_TOOL: &str = "fs.read";
const EXPECTED_PATH: &str = "Cargo.toml";
const EXPECTED_TEXT: &str = "RAH_FS_READ_OK";
const MAX_BYTES: usize = 1024 * 1024;
const PROMPT: &str = "Use the available RAH read-only filesystem tool exactly once.\nRead Cargo.toml from the configured workspace.\nAfter receiving the tool result, reply with exactly:\nRAH_FS_READ_OK\nDo not request any other tool.";
const TERMINAL_TIMEOUT: Duration = Duration::from_secs(120);

struct CountingFsReadTool {
    inner: FsReadTool,
    definition_reads: Arc<AtomicUsize>,
    executions: Arc<AtomicUsize>,
}

#[async_trait]
impl Tool for CountingFsReadTool {
    fn definition(&self) -> ToolDefinition {
        self.definition_reads.fetch_add(1, Ordering::SeqCst);
        self.inner.definition()
    }

    async fn execute(
        &self,
        input: ToolInput,
        context: ToolContext,
    ) -> Result<ToolOutput, ToolError> {
        self.executions.fetch_add(1, Ordering::SeqCst);
        self.inner.execute(input, context).await
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
            eprintln!("LIVE_FS_READ_BRIDGE_FAIL failed to create Tokio runtime: {error}");
            return ExitCode::FAILURE;
        }
    };

    match runtime.block_on(run()) {
        Ok(()) => {
            println!("LIVE_FS_READ_BRIDGE_PASS");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("LIVE_FS_READ_BRIDGE_FAIL {error}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), String> {
    let executable = env::var_os("RAH_CODEX_EXECUTABLE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("codex"));
    let workspace = workspace_root()?;
    let definition_reads = Arc::new(AtomicUsize::new(0));
    let executions = Arc::new(AtomicUsize::new(0));
    let fs_read = FsReadTool::new(&workspace, MAX_BYTES)
        .map_err(|error| format!("failed to configure fs.read workspace: {error}"))?;
    let mut registry = ToolRegistry::new();
    registry
        .register(Arc::new(CountingFsReadTool {
            inner: fs_read,
            definition_reads: Arc::clone(&definition_reads),
            executions: Arc::clone(&executions),
        }))
        .map_err(|error| format!("failed to register fs.read: {error}"))?;

    println!("CODEX_EXECUTABLE {}", executable.display());
    println!("REQUIRED_CODEX_VERSION {SUPPORTED_CODEX_VERSION}");
    println!("WORKSPACE_ROOT {}", workspace.display());
    println!("FS_READ_MAX_BYTES {MAX_BYTES}");
    println!("EXPERIMENTAL_API true");
    println!("ADVERTISED_CODEX_ALIAS {ADVERTISED_ALIAS}");
    println!(
        "ALIAS_IS_NOT_RAH_NAME {}",
        ADVERTISED_ALIAS != EXPECTED_RAH_TOOL
    );
    println!("PRIVATE_ALIAS_MAPPING {ADVERTISED_ALIAS} -> {EXPECTED_RAH_TOOL}");
    println!("ALLOWED_PERMISSIONS {:?}", [PermissionLevel::Read]);
    println!("PROHIBITED_PERMISSIONS Write=false Execute=false");
    println!(
        "ADVERTISED_DYNAMIC_TOOL {}",
        json!({
            "type": "function",
            "name": ADVERTISED_ALIAS,
            "description": "Reads a UTF-8 text file within the configured workspace.",
            "inputSchema": {
                "type": "object",
                "properties": { "path": { "type": "string" } },
                "required": ["path"],
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
        CodexRuntime::connect_tool_bridge(
            &executable,
            Arc::new(registry),
            vec![PermissionLevel::Read],
        )
        .await
        .map_err(|error| format!("connection failed: {error}"))?,
    );
    let live_result = run_turn(
        Arc::clone(&runtime),
        definition_reads.as_ref(),
        executions.as_ref(),
    )
    .await;
    let shutdown_result = runtime.shutdown().await;

    match &shutdown_result {
        Ok(()) => println!("PROCESS_CLEANUP app-server shutdown completed"),
        Err(error) => eprintln!("PROCESS_CLEANUP_FAIL {error}"),
    }
    shutdown_result.map_err(|error| format!("app-server shutdown failed: {error}"))?;
    live_result
}

async fn run_turn(
    runtime: Arc<CodexRuntime>,
    definition_reads: &AtomicUsize,
    executions: &AtomicUsize,
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
    let mut tool_output_bytes = None;
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
                    println!(
                        "RESOLVED_RAH_TOOL_CALL name={} input={}",
                        tool_call.name, tool_call.input.0
                    );
                    if tool_call.name.as_str() != EXPECTED_RAH_TOOL
                        || tool_call.input.0 != json!({"path": EXPECTED_PATH})
                    {
                        return Err(format!("unexpected RAH ToolCall: {tool_call:?}"));
                    }
                }
                AgentEvent::ToolStarted { .. } => started += 1,
                AgentEvent::ToolFinished { output, .. } => {
                    finished += 1;
                    finished_index = Some(index);
                    if output.is_error {
                        return Err("FsReadTool returned an error output".to_owned());
                    }
                    let text = match output.content.as_slice() {
                        [ToolContent::Text(text)] => text,
                        content => {
                            return Err(format!("unexpected FsReadTool output: {content:?}"));
                        }
                    };
                    if !text.contains("[workspace]") || !text.contains("crates/rah-runtime-codex") {
                        return Err("fs.read output did not match the RAH Cargo.toml".to_owned());
                    }
                    tool_output_bytes = Some(text.len());
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
    let definition_read_count = definition_reads.load(Ordering::SeqCst);
    let output_bytes = tool_output_bytes.ok_or_else(|| "missing ToolFinished output".to_owned())?;
    let final_text = final_text.ok_or_else(|| "missing Completed output".to_owned())?;
    if requested != 1 || started != 1 || finished != 1 {
        return Err(format!(
            "unexpected tool lifecycle counts: requested={requested}, started={started}, finished={finished}"
        ));
    }
    if execution_count != 1 {
        return Err(format!("FsReadTool execution count was {execution_count}"));
    }
    if definition_read_count < 3 {
        return Err(format!(
            "registered fs.read definition was read only {definition_read_count} times"
        ));
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
    println!("FS_READ_DEFINITION_READ_COUNT {definition_read_count}");
    println!("FS_READ_EXECUTION_COUNT {execution_count}");
    println!("FILE_READ {EXPECTED_PATH}");
    println!(
        "DYNAMIC_TOOL_RESPONSE success=true content_items=1 type=inputText text_bytes={output_bytes}"
    );
    println!("CODEX_CONTINUED_AFTER_TOOL_RESPONSE true");
    println!("FINAL_ASSISTANT_TEXT {final_text:?}");
    println!("TERMINAL_RAH_EVENT Completed");
    println!("PROHIBITED_ACTIONS none observed; restricted runtime remained active");
    Ok(())
}

fn workspace_root() -> Result<PathBuf, String> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .map(Path::to_path_buf)
        .ok_or_else(|| "failed to resolve the RAH repository root".to_owned())
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
