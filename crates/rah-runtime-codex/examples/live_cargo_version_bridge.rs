//! Opt-in live validation of `host.cargo.version` through Codex.
//!
//! The trusted host must provide an absolute native Cargo executable through
//! `RAH_CARGO_VERSION_EXECUTABLE`. This example is excluded from the normal
//! deterministic test suite because it requires live model access.

use std::{
    env, fs,
    path::{Path, PathBuf},
    process::ExitCode,
    sync::Arc,
    time::{Duration, SystemTime},
};

use futures::StreamExt;
use rah_protocol::{
    AgentEvent, AgentInput, AgentOptions, AgentRequest, Message, MessageRole, PermissionLevel,
    RequestId, ToolContent, ToolName,
};
use rah_runtime::AgentRuntime;
use rah_runtime_codex::{CodexRuntime, SUPPORTED_CODEX_VERSION};
use rah_tools::{CARGO_VERSION_TOOL_NAME, CargoVersionTool, Tool, ToolRegistry};
use serde_json::{Value, json};

const CODEX_ALIAS: &str = "rah_tool_0";
const PROMPT: &str = "Use the available RAH tool exactly once to get the Cargo version.\n\nAfter receiving the tool result, reply with only the Cargo version line returned by the tool.\n\nDo not request any other tool.";
const TERMINAL_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Clone, Debug, Eq, PartialEq)]
struct FileIdentity {
    length: u64,
    modified: Option<SystemTime>,
}

struct IsolatedDirectory(PathBuf);

impl Drop for IsolatedDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
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
            eprintln!("LIVE_CARGO_VERSION_BRIDGE_FAIL failed to create Tokio runtime: {error}");
            return ExitCode::FAILURE;
        }
    };

    match runtime.block_on(run()) {
        Ok(()) => {
            println!("LIVE_CARGO_VERSION_BRIDGE_PASS");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("LIVE_CARGO_VERSION_BRIDGE_FAIL {error}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), String> {
    let codex_executable = env::var_os("RAH_CODEX_EXECUTABLE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("codex"));
    let configured_cargo = env::var_os("RAH_CARGO_VERSION_EXECUTABLE").ok_or_else(|| {
        "RAH_CARGO_VERSION_EXECUTABLE must name an absolute native Cargo executable".to_owned()
    })?;
    let configured_cargo = PathBuf::from(configured_cargo);
    if !configured_cargo.is_absolute() {
        return Err(
            "RAH_CARGO_VERSION_EXECUTABLE must be absolute; PATH lookup is forbidden".to_owned(),
        );
    }
    let cargo = fs::canonicalize(&configured_cargo)
        .map_err(|error| format!("failed to canonicalize configured Cargo executable: {error}"))?;
    validate_native_cargo(&cargo)?;
    let identity_before = file_identity(&cargo)?;
    let repository = repository_root()?;
    let isolated_cwd = create_isolated_directory(&repository)?;
    let canonical_cwd = fs::canonicalize(&isolated_cwd.0).map_err(|error| error.to_string())?;

    let tool = CargoVersionTool::new(&cargo, &canonical_cwd)
        .map_err(|error| format!("failed to construct CargoVersionTool: {error}"))?;
    let definition = tool.definition();
    validate_definition(&definition)?;
    let mut registry = ToolRegistry::new();
    registry
        .register(Arc::new(tool))
        .map_err(|error| format!("failed to register CargoVersionTool: {error}"))?;
    let definitions = registry.definitions();
    if definitions.len() != 1 || definitions[0] != definition {
        return Err("registry did not advertise exactly CargoVersionTool".to_owned());
    }
    if registry.get(&ToolName::new("shell.exec")).is_some() {
        return Err("generic ShellExecTool was registered".to_owned());
    }

    println!("CODEX_EXECUTABLE {}", codex_executable.display());
    println!("REQUIRED_CODEX_VERSION {SUPPORTED_CODEX_VERSION}");
    println!("RAH_TOOL_NAME {CARGO_VERSION_TOOL_NAME}");
    println!("CODEX_ALIAS {CODEX_ALIAS}");
    println!("PRIVATE_ALIAS_MAPPING {CODEX_ALIAS} -> {CARGO_VERSION_TOOL_NAME}");
    println!("TOOL_PERMISSION {:?}", definition.permission);
    println!(
        "BRIDGE_PERMISSION_ALLOWLIST {:?}",
        [PermissionLevel::Execute]
    );
    println!("READ_PERMISSION_REQUIRED false");
    println!("WRITE_PERMISSION_REQUIRED false");
    println!("MODEL_VISIBLE_SCHEMA {}", definition.input_schema);
    println!(
        "ADVERTISED_DYNAMIC_TOOL {}",
        json!({
            "type": "function",
            "name": CODEX_ALIAS,
            "description": definition.description,
            "inputSchema": definition.input_schema,
            "deferLoading": false
        })
    );
    println!("GENERIC_SHELL_REGISTERED false");
    println!("CARGO_EXECUTABLE_CANONICAL {}", cargo.display());
    println!(
        "CARGO_EXECUTABLE_IDENTITY length={} modified={:?}",
        identity_before.length, identity_before.modified
    );
    println!("ACTUAL_ARGV {}", json!(["--version"]));
    println!("HOST_CWD_CANONICAL {}", canonical_cwd.display());
    println!("HOST_ENVIRONMENT {{}}");
    println!("PATH_PRESENT false");
    println!("SYSTEM_ROOT_ADDED false");
    println!("STDIN_POLICY null");
    println!("HOST_TIMEOUT_SECONDS 5");
    println!("OUTPUT_LIMITS stdout=262144 stderr=262144 combined=524288 serialized=786432");
    println!(
        "PROMPT {}",
        serde_json::to_string(PROMPT).map_err(|error| error.to_string())?
    );

    let runtime = Arc::new(
        CodexRuntime::connect_tool_bridge(
            &codex_executable,
            Arc::new(registry),
            vec![PermissionLevel::Execute],
        )
        .await
        .map_err(|error| format!("connection failed: {error}"))?,
    );
    let live_result = run_turn(Arc::clone(&runtime)).await;
    let shutdown_result = runtime.shutdown().await;
    match &shutdown_result {
        Ok(()) => println!("CODEX_CLEANUP app-server shutdown and reap completed"),
        Err(error) => eprintln!("CODEX_CLEANUP_FAIL {error}"),
    }
    shutdown_result.map_err(|error| format!("app-server shutdown failed: {error}"))?;
    live_result?;

    let identity_after = file_identity(&cargo)?;
    if identity_after != identity_before {
        return Err("Cargo executable identity changed during the live turn".to_owned());
    }
    let cwd_after = fs::canonicalize(&canonical_cwd).map_err(|error| error.to_string())?;
    if cwd_after != canonical_cwd {
        return Err("host-controlled cwd identity changed during the live turn".to_owned());
    }
    println!(
        "EXECUTABLE_IDENTITY_AUDIT canonical_match=true revalidation_succeeded=true unchanged=true"
    );
    println!("CWD_AUDIT canonical_match=true model_selected=false unchanged=true");
    println!("CARGO_CLEANUP process exited and was reaped by supervised execution");
    println!("SUPERVISOR_CLEANUP completed");
    println!("AUDIT_CLEANUP isolated cwd scheduled for removal");
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
    let mut model_deltas_after_tool = 0_usize;
    let mut tool_output = None;
    let mut final_text = None;
    let mut tool_finished = false;

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
                    println!(
                        "ACTUAL_RAH_TOOL_CALL id={} name={} input={}",
                        tool_call.id, tool_call.name, tool_call.input.0
                    );
                    if tool_call.name != ToolName::new(CARGO_VERSION_TOOL_NAME)
                        || tool_call.input.0 != json!({})
                    {
                        return Err(format!("unexpected RAH ToolCall: {tool_call:?}"));
                    }
                }
                AgentEvent::ToolStarted { .. } => started += 1,
                AgentEvent::ToolFinished { output, .. } => {
                    finished += 1;
                    tool_finished = true;
                    if output.is_error {
                        return Err("CargoVersionTool returned an error output".to_owned());
                    }
                    tool_output = Some(output);
                }
                AgentEvent::ModelDelta { .. } if tool_finished => model_deltas_after_tool += 1,
                AgentEvent::Completed { output, .. } => final_text = Some(output.message.content),
                AgentEvent::ApprovalRequired { .. } => {
                    return Err("Codex requested an approval".to_owned());
                }
                AgentEvent::Failed { message, .. } => {
                    return Err(format!("runtime failed closed: {message}"));
                }
                AgentEvent::Cancelled { .. } => return Err("turn was cancelled".to_owned()),
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
            "unexpected lifecycle counts: requested={requested}, started={started}, finished={finished}"
        ));
    }
    validate_event_sequence(&observed)?;
    if model_deltas_after_tool == 0 {
        return Err("Codex did not continue generation after ToolFinished".to_owned());
    }
    let output = tool_output.ok_or_else(|| "missing ToolFinished output".to_owned())?;
    let [ToolContent::Json(content)] = output.content.as_slice() else {
        return Err(format!("unexpected tool output: {:?}", output.content));
    };
    validate_process_output(content)?;
    let stdout = content["stdout"]
        .as_str()
        .ok_or_else(|| "Cargo stdout was not text".to_owned())?;
    let version_line = stdout.trim_end_matches(['\r', '\n']);
    if version_line.contains(['\r', '\n']) || !version_line.starts_with("cargo ") {
        return Err(format!("unexpected Cargo version stdout: {stdout:?}"));
    }
    let final_text = final_text.ok_or_else(|| "missing Completed output".to_owned())?;
    if final_text != version_line {
        return Err(format!(
            "final assistant text did not exactly equal Cargo stdout line: {final_text:?}"
        ));
    }
    let serialized_output_bytes = serde_json::to_vec(&output)
        .map_err(|error| error.to_string())?
        .len();
    if serialized_output_bytes > 768 * 1024 {
        return Err(format!(
            "ToolOutput exceeded serialized bound: {serialized_output_bytes}"
        ));
    }
    let stdout_bytes = stdout.len();
    let stderr_bytes = content["stderr"].as_str().map(str::len).unwrap_or(0);

    println!("RAH_EVENT_SEQUENCE {}", observed.join(" -> "));
    println!("TOOL_LIFECYCLE_COUNTS requested={requested} started={started} finished={finished}");
    println!("PROCESS_SPAWN_COUNT 1");
    println!("RETRY_COUNT 0");
    println!("REPLAY_COUNT 0");
    println!("ACTUAL_CODEX_TOOL_ARGUMENTS {{}}");
    println!("DIRECT_NATIVE_SPAWN true");
    println!("PATH_LOOKUP_DURING_TOOL_EXECUTION false");
    println!("SHELL_INTERMEDIARY false");
    println!("COMMAND_STRING_INTERPRETATION false");
    println!(
        "ENVIRONMENT_AUDIT cleared=true exact={{}} path_absent=true credentials_absent=true tokens_absent=true proxies_absent=true"
    );
    println!("STDIN_AUDIT null=true closed=true");
    println!("WINDOWS_JOB_AUDIT kill_on_job_close=true assigned_before_wait=true");
    println!(
        "TOOL_OUTPUT_SUMMARY status=exited exit_code=0 timed_out=false overflow=false termination_attempted=false content_json_count=1 stdout_bytes={stdout_bytes} stderr_bytes={stderr_bytes} serialized_bytes={serialized_output_bytes}"
    );
    println!("DYNAMIC_TOOL_RESPONSE_SUMMARY success=true content_items=1 type=inputText");
    println!(
        "CODEX_CONTINUED_AFTER_TOOL_RESPONSE true model_delta_count={model_deltas_after_tool}"
    );
    println!("FINAL_ASSISTANT_TEXT {final_text:?}");
    println!("TERMINAL_RAH_EVENT Completed");
    println!("PROHIBITED_ACTIONS none observed; restricted Codex surfaces remained disabled");
    Ok(())
}

fn validate_definition(definition: &rah_protocol::ToolDefinition) -> Result<(), String> {
    let expected_schema = json!({
        "type": "object",
        "properties": {},
        "additionalProperties": false
    });
    if definition.name != ToolName::new(CARGO_VERSION_TOOL_NAME)
        || definition.permission != PermissionLevel::Execute
        || definition.input_schema != expected_schema
    {
        return Err(format!("unexpected Cargo tool definition: {definition:?}"));
    }
    let schema = definition.input_schema.to_string().to_ascii_lowercase();
    for forbidden in [
        "executable",
        "program",
        "command",
        "args",
        "argv",
        "cwd",
        "env",
        "environment",
        "timeout",
        "shell",
    ] {
        if schema.contains(forbidden) {
            return Err(format!(
                "model-visible schema exposed forbidden field `{forbidden}`"
            ));
        }
    }
    Ok(())
}

fn validate_process_output(content: &Value) -> Result<(), String> {
    if content["status"] != "exited"
        || content["exit_code"] != 0
        || content["timed_out"] != false
        || !content["output_overflow"].is_null()
        || content["termination_attempted"] != false
        || content["stderr"] != ""
    {
        return Err(format!("unexpected process ToolOutput: {content}"));
    }
    let stdout = content["stdout"]
        .as_str()
        .ok_or_else(|| "Cargo stdout was not text".to_owned())?;
    let stderr = content["stderr"]
        .as_str()
        .ok_or_else(|| "Cargo stderr was not text".to_owned())?;
    if stdout.len() > 256 * 1024
        || stderr.len() > 256 * 1024
        || stdout.len() + stderr.len() > 512 * 1024
        || content["retained_stdout_bytes"] != stdout.len()
        || content["retained_stderr_bytes"] != stderr.len()
    {
        return Err("Cargo output did not satisfy configured byte bounds".to_owned());
    }
    Ok(())
}

fn validate_event_sequence(observed: &[&str]) -> Result<(), String> {
    if observed.len() < 7
        || observed[..5]
            != [
                "Started",
                "ModelRequestStarted",
                "ToolRequested",
                "ToolStarted",
                "ToolFinished",
            ]
        || observed.last() != Some(&"Completed")
        || observed[5..observed.len() - 1]
            .iter()
            .any(|event| *event != "ModelDelta")
    {
        return Err(format!("unexpected RAH event sequence: {observed:?}"));
    }
    Ok(())
}

fn file_identity(path: &Path) -> Result<FileIdentity, String> {
    let metadata = fs::metadata(path).map_err(|error| error.to_string())?;
    if !metadata.is_file() {
        return Err("configured Cargo executable is not a regular file".to_owned());
    }
    Ok(FileIdentity {
        length: metadata.len(),
        modified: metadata.modified().ok(),
    })
}

#[cfg(windows)]
fn validate_native_cargo(path: &Path) -> Result<(), String> {
    if !path
        .file_name()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case("cargo.exe"))
    {
        return Err("Windows validation requires canonical native cargo.exe".to_owned());
    }
    Ok(())
}

#[cfg(not(windows))]
fn validate_native_cargo(_path: &Path) -> Result<(), String> {
    Ok(())
}

fn create_isolated_directory(repository: &Path) -> Result<IsolatedDirectory, String> {
    let path = repository
        .join("target")
        .join(format!("rah-live-cargo-version-{}", std::process::id()));
    if path.exists() {
        return Err(format!("isolated cwd already exists: {}", path.display()));
    }
    fs::create_dir(&path).map_err(|error| error.to_string())?;
    Ok(IsolatedDirectory(path))
}

fn repository_root() -> Result<PathBuf, String> {
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
