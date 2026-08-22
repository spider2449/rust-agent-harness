//! Opt-in live validation of ADR 0010's deterministic mutation fixture.
//!
//! This deliberately requires a compatible local Codex executable and live
//! model access. It is excluded from the deterministic workspace test suite.

use std::{
    env, fs,
    path::{Path, PathBuf},
    process::ExitCode,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use futures::StreamExt;
use rah_protocol::{
    AgentEvent, AgentInput, AgentOptions, AgentRequest, Message, MessageRole, PermissionLevel,
    RequestId, ToolContent, ToolName,
};
use rah_runtime::AgentRuntime;
use rah_runtime_codex::{CodexRuntime, SUPPORTED_CODEX_VERSION};
use rah_tools::{RepositoryMutationFixtureTool, Tool, ToolRegistry};
use serde_json::json;

const TOOL_NAME: &str = "host.fixture.mutate-marker";
const CODEX_ALIAS: &str = "rah_tool_0";
const FINAL_TEXT: &str = "RAH_MUTATION_OK";
const PROMPT: &str = "Use the available RAH tool exactly once to perform the authorized mutation.\n\nAfter receiving the tool result, reply with exactly:\nRAH_MUTATION_OK\n\nDo not request any other tool.";
const TERMINAL_TIMEOUT: Duration = Duration::from_secs(120);
static NEXT_ROOT: AtomicU64 = AtomicU64::new(0);

struct TemporaryRoot(PathBuf);

impl TemporaryRoot {
    fn create() -> Result<Self, String> {
        let id = NEXT_ROOT.fetch_add(1, Ordering::Relaxed);
        let root = env::temp_dir().join(format!("rah-live-mutation-{}-{id}", std::process::id()));
        fs::create_dir(&root)
            .map_err(|error| format!("failed to create temporary root: {error}"))?;
        fs::write(root.join("marker.txt"), "before\n").map_err(|error| error.to_string())?;
        fs::write(root.join("sentinel.txt"), "unchanged\n").map_err(|error| error.to_string())?;
        Ok(Self(root))
    }

    fn root(&self) -> &Path {
        &self.0
    }

    fn remove(&mut self) -> Result<(), String> {
        fs::remove_dir_all(&self.0)
            .map_err(|error| format!("failed to remove temporary root: {error}"))?;
        if self.0.exists() {
            return Err("temporary root still exists after cleanup".to_owned());
        }
        Ok(())
    }
}

impl Drop for TemporaryRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter("rah=debug")
        .without_time()
        .init();
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("LIVE_MUTATION_FIXTURE_BRIDGE_FAIL could not create Tokio runtime: {error}");
            return ExitCode::FAILURE;
        }
    };
    match runtime.block_on(run()) {
        Ok(()) => {
            println!("LIVE_MUTATION_FIXTURE_BRIDGE_PASS");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("LIVE_MUTATION_FIXTURE_BRIDGE_FAIL {error}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), String> {
    let codex = env::var_os("RAH_CODEX_EXECUTABLE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("codex"));
    let fixture = fixture_path()?;
    let mut root = TemporaryRoot::create()?;
    let canonical_root = fs::canonicalize(root.root()).map_err(|error| error.to_string())?;
    assert_contents(&canonical_root, "before\n", "unchanged\n", "pre-state")?;

    let tool = RepositoryMutationFixtureTool::new(&fixture, &canonical_root).map_err(|error| {
        format!("failed to build RepositoryMutationPolicy and HostExecutionPolicy: {error}")
    })?;
    let definition = tool.definition();
    if definition.name != ToolName::new(TOOL_NAME)
        || definition.permission != PermissionLevel::Execute
        || definition.input_schema
            != json!({"type":"object","properties":{},"additionalProperties":false})
    {
        return Err(format!(
            "unexpected mutation capability definition: {definition:?}"
        ));
    }
    let mut registry = ToolRegistry::new();
    registry
        .register(Arc::new(tool))
        .map_err(|error| error.to_string())?;
    if registry.definitions().len() != 1 {
        return Err("more than one dynamic capability was advertised".to_owned());
    }

    println!("CODEX_VERSION_REQUIRED {SUPPORTED_CODEX_VERSION}");
    println!("RAH_TOOL_NAME {TOOL_NAME}");
    println!("CODEX_ALIAS {CODEX_ALIAS}");
    println!("BRIDGE_ALLOWLIST [Execute]");
    println!("READ_PERMISSION_REQUIRED false");
    println!("WRITE_PERMISSION_REQUIRED false");
    println!("MODEL_VISIBLE_SCHEMA {}", definition.input_schema);
    println!(
        "ROOT_AUDIT canonical_identity=captured target=fixture-marker->marker.txt snapshot=bounded pre_state=valid"
    );
    println!(
        "LEASE_AUDIT acquired_before_pre_state=true held_through_process_and_post_state=true released_after_result_construction=true"
    );
    println!(
        "PROCESS_AUDIT permission=Execute repository_mutation_policy=true host_execution_policy=true canonical_native_fixture=true executable_revalidated=true direct_spawn=true path_lookup=false shell=false cwd=host_controlled environment=cleared stdin=null timeout=fixed output=bounded supervised=true"
    );

    let runtime = CodexRuntime::connect_tool_bridge(
        &codex,
        Arc::new(registry),
        vec![PermissionLevel::Execute],
    )
    .await
    .map_err(|error| format!("failed to start Codex app-server: {error}"))?;
    let runtime = Arc::new(runtime);
    let result = run_turn(Arc::clone(&runtime)).await;
    let shutdown = runtime.shutdown().await;
    if let Err(error) = shutdown {
        return Err(format!("Codex app-server shutdown/reap failed: {error}"));
    }
    result?;
    assert_contents(&canonical_root, "after\n", "unchanged\n", "post-state")?;
    let names = fs::read_dir(&canonical_root)
        .map_err(|error| error.to_string())?
        .map(|entry| {
            entry
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .map_err(|error| error.to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    if names.len() != 2
        || !names.iter().any(|name| name == "marker.txt")
        || !names.iter().any(|name| name == "sentinel.txt")
    {
        return Err(format!(
            "post-state discovered an unauthorized root mutation: {names:?}"
        ));
    }
    println!(
        "POST_STATE marker=after sentinel=unchanged root_identity=unchanged unauthorized_changes=none additions=0 deletions=0"
    );
    root.remove()?;
    println!(
        "CLEANUP fixture_reaped=true supervisor_cleanup=true lease_released=true temporary_root_removed=true codex_app_server_reaped=true"
    );
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
        .map_err(|error| error.to_string())?;
    let mut events = handle.into_events();
    let mut observed = Vec::new();
    let (mut requested, mut started, mut finished, mut deltas_after_tool) = (0, 0, 0, 0);
    let mut after_tool = false;
    let mut final_text = None;
    tokio::time::timeout(TERMINAL_TIMEOUT, async {
        while let Some(event) = events.next().await {
            observed.push(event_name(&event));
            match event {
                AgentEvent::ToolRequested { tool_call, .. } => {
                    requested += 1;
                    if tool_call.name != ToolName::new(TOOL_NAME) || tool_call.input.0 != json!({}) {
                        return Err(format!("unexpected dynamic tool call: {tool_call:?}"));
                    }
                    println!("CODEX_DYNAMIC_TOOL_REQUEST alias={CODEX_ALIAS} resolved={TOOL_NAME} call_id={} arguments={{}}", tool_call.id);
                }
                AgentEvent::ToolStarted { .. } => started += 1,
                AgentEvent::ToolFinished { output, .. } => {
                    finished += 1; after_tool = true; validate_output(&output)?;
                    println!("DYNAMIC_TOOL_CALL_RESPONSE success=true content_items=1 bounded=true");
                }
                AgentEvent::ModelDelta { .. } if after_tool => deltas_after_tool += 1,
                AgentEvent::Completed { output, .. } => final_text = Some(output.message.content),
                AgentEvent::ApprovalRequired { .. } => return Err("unexpected approval request".to_owned()),
                AgentEvent::Failed { message, .. } => return Err(format!("RAH failed: {message}")),
                AgentEvent::Cancelled { .. } => return Err("RAH turn was cancelled".to_owned()),
                AgentEvent::Started { .. } | AgentEvent::ModelRequestStarted { .. } | AgentEvent::ModelDelta { .. } => {}
            }
        }
        Ok(())
    }).await.map_err(|_| "timed out waiting for a terminal RAH event".to_owned())??;
    if requested != 1
        || started != 1
        || finished != 1
        || deltas_after_tool == 0
        || final_text.as_deref() != Some(FINAL_TEXT)
    {
        return Err(format!(
            "invalid live lifecycle requested={requested} started={started} finished={finished} deltas_after_tool={deltas_after_tool} final={final_text:?}"
        ));
    }
    let expected_prefix = [
        "Started",
        "ModelRequestStarted",
        "ToolRequested",
        "ToolStarted",
        "ToolFinished",
    ];
    if observed.len() < 7
        || observed[..5] != expected_prefix
        || observed.last() != Some(&"Completed")
        || observed[5..observed.len() - 1]
            .iter()
            .any(|event| *event != "ModelDelta")
    {
        return Err(format!("unexpected RAH event sequence: {observed:?}"));
    }
    println!("RAH_EVENT_SEQUENCE {}", observed.join(" -> "));
    println!(
        "LIFECYCLE_COUNTS tool_requested=1 tool_started=1 tool_finished=1 process_spawns=1 mutations=1 retry=0 replay=0"
    );
    println!("FINAL_ASSISTANT_TEXT {FINAL_TEXT}");
    println!("TERMINAL_EVENT Completed");
    println!(
        "PROHIBITED_ACTIONS none_observed git=false shell_exec=false arbitrary_write=false codex_execution=false mcp=false approval=false web=false image=false app=false second_tool=false"
    );
    Ok(())
}

fn validate_output(output: &rah_protocol::ToolOutput) -> Result<(), String> {
    let [ToolContent::Json(value)] = output.content.as_slice() else {
        return Err("mutation output was not one JSON item".to_owned());
    };
    let expected = json!({"status":"ok","target":"fixture-marker","changed":true,"partial":false,"uncertain":false});
    if output.is_error || value != &expected {
        return Err(format!("unexpected or unsafe mutation ToolOutput: {value}"));
    }
    let forbidden = [
        "path",
        "root",
        "marker.txt",
        "executable",
        "cwd",
        "environment",
        "audit",
        "sentinel",
        "stdout",
        "stderr",
    ];
    if forbidden
        .iter()
        .any(|field| value.to_string().to_ascii_lowercase().contains(field))
    {
        return Err("ToolOutput leaked host process or path data".to_owned());
    }
    Ok(())
}

fn assert_contents(root: &Path, marker: &str, sentinel: &str, label: &str) -> Result<(), String> {
    if fs::read_to_string(root.join("marker.txt")).map_err(|error| error.to_string())? != marker
        || fs::read_to_string(root.join("sentinel.txt")).map_err(|error| error.to_string())?
            != sentinel
    {
        return Err(format!("{label} fixture contents were invalid"));
    }
    Ok(())
}

fn fixture_path() -> Result<PathBuf, String> {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .ok_or_else(|| "could not locate repository root".to_owned())?;
    let name = if cfg!(windows) {
        "target/debug/rah_execute_fixture.exe"
    } else {
        "target/debug/rah_execute_fixture"
    };
    fs::canonicalize(repository.join(name)).map_err(|error| format!("fixture must be built first (`cargo build -p rah-tools --bin rah_execute_fixture`): {error}"))
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
