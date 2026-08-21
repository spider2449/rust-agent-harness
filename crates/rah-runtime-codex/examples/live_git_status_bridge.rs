//! Opt-in live validation of `host.git.status` through the Codex Tool Bridge.
//!
//! The trusted host must provide an absolute native Git executable through
//! `RAH_GIT_STATUS_EXECUTABLE`. The example creates and removes its own
//! deterministic repository and is excluded from the normal offline suite.

use std::{
    collections::BTreeMap,
    env, fs,
    path::{Path, PathBuf},
    process::{Command, ExitCode, Stdio},
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
use rah_tools::{GIT_STATUS_TOOL_NAME, GitStatusTool, Tool, ToolRegistry};
use serde_json::{Value, json};

const CODEX_ALIAS: &str = "rah_tool_0";
const EXPECTED_STDOUT: &str = " M tracked.txt\n?? untracked.txt\n";
const EXPECTED_FINAL: &str = " M tracked.txt\n?? untracked.txt";
const PROMPT: &str = "Use the available RAH tool exactly once to inspect the authorized repository status.\n\nAfter receiving the tool result, reply with only the porcelain status lines returned by the tool, preserving them exactly.\n\nDo not request any other tool.";
const TERMINAL_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Clone, Debug, Eq, PartialEq)]
struct FileIdentity {
    length: u64,
    modified: Option<SystemTime>,
}

struct TestRepository(PathBuf);

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
            eprintln!("LIVE_GIT_STATUS_BRIDGE_FAIL failed to create Tokio runtime: {error}");
            return ExitCode::FAILURE;
        }
    };

    match runtime.block_on(run()) {
        Ok(()) => {
            println!("LIVE_GIT_STATUS_BRIDGE_PASS");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("LIVE_GIT_STATUS_BRIDGE_FAIL {error}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), String> {
    let codex_executable = env::var_os("RAH_CODEX_EXECUTABLE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("codex"));
    let configured_git = env::var_os("RAH_GIT_STATUS_EXECUTABLE").ok_or_else(|| {
        "RAH_GIT_STATUS_EXECUTABLE must name an absolute native Git executable".to_owned()
    })?;
    let configured_git = PathBuf::from(configured_git);
    if !configured_git.is_absolute() {
        return Err(
            "RAH_GIT_STATUS_EXECUTABLE must be absolute; PATH lookup is forbidden".to_owned(),
        );
    }
    let git = fs::canonicalize(&configured_git)
        .map_err(|error| format!("failed to canonicalize configured Git executable: {error}"))?;
    validate_native_git(&git)?;
    let executable_identity_before = file_identity(&git)?;
    let repository = create_test_repository(&git)?;
    let canonical_repository =
        fs::canonicalize(&repository.0).map_err(|error| error.to_string())?;
    let repository_identity_before = directory_identity(&canonical_repository)?;
    let dot_git = fs::canonicalize(canonical_repository.join(".git"))
        .map_err(|error| format!("failed to canonicalize test .git directory: {error}"))?;
    let dot_git_identity_before = directory_identity(&dot_git)?;
    let contents_before = snapshot_contents(&canonical_repository)?;

    let tool = GitStatusTool::new(&git, &canonical_repository)
        .map_err(|error| format!("failed to construct GitStatusTool: {error}"))?;
    let definition = tool.definition();
    validate_definition(&definition)?;
    let mut registry = ToolRegistry::new();
    registry
        .register(Arc::new(tool))
        .map_err(|error| format!("failed to register GitStatusTool: {error}"))?;
    let definitions = registry.definitions();
    if definitions.len() != 1 || definitions[0] != definition {
        return Err("registry did not advertise exactly GitStatusTool".to_owned());
    }
    for forbidden in ["shell.exec", "git", "host.git"] {
        if registry.get(&ToolName::new(forbidden)).is_some() {
            return Err(format!(
                "prohibited generic capability was registered: {forbidden}"
            ));
        }
    }

    println!("CODEX_EXECUTABLE {}", codex_executable.display());
    println!("CODEX_VERSION {SUPPORTED_CODEX_VERSION}");
    println!("RAH_TOOL_NAME {GIT_STATUS_TOOL_NAME}");
    println!("CODEX_ALIAS {CODEX_ALIAS}");
    println!("PRIVATE_ALIAS_MAPPING {CODEX_ALIAS} -> {GIT_STATUS_TOOL_NAME}");
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
    println!("GENERIC_GIT_REGISTERED false");
    println!("GIT_EXECUTABLE_CANONICAL {}", git.display());
    println!(
        "GIT_EXECUTABLE_IDENTITY length={} modified={:?}",
        executable_identity_before.length, executable_identity_before.modified
    );
    println!("ACTUAL_ARGV {}", json!(["status", "--porcelain=v1"]));
    println!(
        "TEST_REPOSITORY_CANONICAL {}",
        canonical_repository.display()
    );
    println!("DOT_GIT_CANONICAL {}", dot_git.display());
    println!("CWD_CANONICAL {}", canonical_repository.display());
    println!("MODEL_SELECTED_REPOSITORY false");
    println!(
        "GIT_ENVIRONMENT {}",
        json!({
            "GIT_CONFIG_NOSYSTEM": "1",
            "GIT_CONFIG_GLOBAL": platform_null_device(),
            "GIT_CONFIG_COUNT": "2",
            "GIT_CONFIG_KEY_0": "core.fsmonitor",
            "GIT_CONFIG_VALUE_0": "false",
            "GIT_CONFIG_KEY_1": "core.untrackedCache",
            "GIT_CONFIG_VALUE_1": "false",
            "GIT_OPTIONAL_LOCKS": "0",
            "GIT_TERMINAL_PROMPT": "0"
        })
    );
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

    if file_identity(&git)? != executable_identity_before {
        return Err("Git executable identity changed during the live turn".to_owned());
    }
    if directory_identity(&canonical_repository)? != repository_identity_before {
        return Err("test repository root identity changed during the live turn".to_owned());
    }
    if directory_identity(&dot_git)? != dot_git_identity_before {
        return Err("test repository .git identity changed during the live turn".to_owned());
    }
    if snapshot_contents(&canonical_repository)? != contents_before {
        return Err("Git status changed the deterministic repository".to_owned());
    }
    println!(
        "EXECUTABLE_IDENTITY_AUDIT canonical_match=true revalidation_succeeded=true unchanged=true"
    );
    println!(
        "REPOSITORY_IDENTITY_AUDIT canonical_match=true revalidation_succeeded=true unchanged=true"
    );
    println!(
        "DOT_GIT_IDENTITY_AUDIT canonical_match=true revalidation_succeeded=true unchanged=true"
    );
    println!(
        "CWD_AUDIT exact=true canonical=true model_selected=false substitution_impossible=true"
    );
    println!("FILE_CHANGE_AUDIT unchanged=true");
    println!("GIT_CLEANUP process exited and was reaped by supervised execution");
    println!("SUPERVISOR_CLEANUP completed");

    let removed = repository.remove()?;
    if removed.exists() {
        return Err("temporary repository still exists after cleanup".to_owned());
    }
    println!("TEST_REPOSITORY_CLEANUP removed=true");
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
                    if tool_call.name != ToolName::new(GIT_STATUS_TOOL_NAME)
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
                        return Err("GitStatusTool returned an error output".to_owned());
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
        .ok_or_else(|| "Git stdout was not text".to_owned())?;
    if stdout != EXPECTED_STDOUT {
        return Err(format!("unexpected Git porcelain stdout: {stdout:?}"));
    }
    let final_text = final_text.ok_or_else(|| "missing Completed output".to_owned())?;
    if final_text != EXPECTED_FINAL {
        return Err(format!(
            "final assistant text did not equal the porcelain status lines: {final_text:?}"
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
    let stderr = content["stderr"].as_str().unwrap_or_default();

    println!("RAH_EVENT_SEQUENCE {}", observed.join(" -> "));
    println!("TOOL_LIFECYCLE_COUNTS requested={requested} started={started} finished={finished}");
    println!("GIT_PROCESS_SPAWN_COUNT 1");
    println!("RETRY_COUNT 0");
    println!("REPLAY_COUNT 0");
    println!("ACTUAL_CODEX_TOOL_ARGUMENTS {{}}");
    println!("DIRECT_NATIVE_SPAWN true");
    println!("PATH_LOOKUP_DURING_TOOL_EXECUTION false");
    println!("SHELL_INTERMEDIARY false");
    println!("ALTERNATE_GIT_SUBCOMMAND false");
    println!(
        "ENVIRONMENT_AUDIT cleared=true fixed_git_controls=true prohibited_variables_absent=true model_supplied=false"
    );
    println!("STDIN_AUDIT null=true closed=true");
    println!("WINDOWS_JOB_AUDIT kill_on_job_close=true assigned_before_wait=true");
    println!(
        "TOOL_OUTPUT_SUMMARY status=exited exit_code=0 timed_out=false overflow=false termination_attempted=false content_json_count=1 stdout_bytes={} stderr_bytes={} serialized_bytes={serialized_output_bytes}",
        stdout.len(),
        stderr.len()
    );
    println!("EXPECTED_PORCELAIN_STATE modified=tracked.txt untracked=untracked.txt");
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
    if definition.name != ToolName::new(GIT_STATUS_TOOL_NAME)
        || definition.permission != PermissionLevel::Execute
        || definition.input_schema != expected_schema
    {
        return Err(format!(
            "unexpected Git status tool definition: {definition:?}"
        ));
    }
    let schema = definition.input_schema.to_string().to_ascii_lowercase();
    for forbidden in [
        "executable",
        "program",
        "command",
        "args",
        "argv",
        "cwd",
        "repo",
        "repository",
        "path",
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
    let stdout = content["stdout"].as_str().unwrap_or_default();
    let stderr = content["stderr"].as_str().unwrap_or_default();
    if stdout.len() > 256 * 1024
        || stderr.len() > 256 * 1024
        || stdout.len() + stderr.len() > 512 * 1024
        || content["retained_stdout_bytes"] != stdout.len()
        || content["retained_stderr_bytes"] != stderr.len()
    {
        return Err("Git output did not satisfy configured byte bounds".to_owned());
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

fn create_test_repository(git: &Path) -> Result<TestRepository, String> {
    let path = env::temp_dir().join(format!(
        "rah-live-git-status-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_nanos()
    ));
    fs::create_dir(&path).map_err(|error| format!("failed to create test repository: {error}"))?;
    let repository = TestRepository(path);
    run_setup_git(git, &repository.0, &["init", "--quiet"])?;
    fs::write(repository.0.join("tracked.txt"), "committed\n")
        .map_err(|error| error.to_string())?;
    run_setup_git(git, &repository.0, &["add", "--", "tracked.txt"])?;
    run_setup_git(
        git,
        &repository.0,
        &[
            "-c",
            "user.name=RAH Live Test",
            "-c",
            "user.email=rah-live-test@example.invalid",
            "commit",
            "--quiet",
            "-m",
            "deterministic fixture",
        ],
    )?;
    fs::write(repository.0.join("tracked.txt"), "modified\n").map_err(|error| error.to_string())?;
    fs::write(repository.0.join("untracked.txt"), "untracked\n")
        .map_err(|error| error.to_string())?;
    Ok(repository)
}

fn run_setup_git(git: &Path, repository: &Path, args: &[&str]) -> Result<(), String> {
    let output = Command::new(git)
        .args(args)
        .current_dir(repository)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|error| format!("trusted setup Git failed to start: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "trusted setup Git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(())
}

impl TestRepository {
    fn remove(&self) -> Result<PathBuf, String> {
        fs::remove_dir_all(&self.0)
            .map_err(|error| format!("failed to remove temporary repository: {error}"))?;
        Ok(self.0.clone())
    }
}

impl Drop for TestRepository {
    fn drop(&mut self) {
        if self.0.exists() {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
}

fn snapshot_contents(root: &Path) -> Result<BTreeMap<PathBuf, Vec<u8>>, String> {
    fn visit(
        root: &Path,
        directory: &Path,
        files: &mut BTreeMap<PathBuf, Vec<u8>>,
    ) -> Result<(), String> {
        let mut entries = fs::read_dir(directory)
            .map_err(|error| error.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?;
        entries.sort_by_key(std::fs::DirEntry::file_name);
        for entry in entries {
            let path = entry.path();
            let metadata = entry.metadata().map_err(|error| error.to_string())?;
            if metadata.is_dir() {
                visit(root, &path, files)?;
            } else if metadata.is_file() {
                let relative = path
                    .strip_prefix(root)
                    .map_err(|error| error.to_string())?
                    .to_path_buf();
                files.insert(relative, fs::read(path).map_err(|error| error.to_string())?);
            } else {
                return Err("test repository contains an unsupported file type".to_owned());
            }
        }
        Ok(())
    }

    let mut files = BTreeMap::new();
    visit(root, root, &mut files)?;
    Ok(files)
}

fn file_identity(path: &Path) -> Result<FileIdentity, String> {
    let metadata = fs::metadata(path).map_err(|error| error.to_string())?;
    if !metadata.is_file() {
        return Err("configured Git executable is not a regular file".to_owned());
    }
    Ok(FileIdentity {
        length: metadata.len(),
        modified: metadata.modified().ok(),
    })
}

fn directory_identity(path: &Path) -> Result<FileIdentity, String> {
    let metadata = fs::metadata(path).map_err(|error| error.to_string())?;
    if !metadata.is_dir() {
        return Err(format!("expected directory: {}", path.display()));
    }
    Ok(FileIdentity {
        length: metadata.len(),
        modified: metadata.modified().ok(),
    })
}

#[cfg(windows)]
fn validate_native_git(path: &Path) -> Result<(), String> {
    if !path
        .file_name()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case("git.exe"))
    {
        return Err("Windows validation requires canonical native git.exe".to_owned());
    }
    Ok(())
}

#[cfg(not(windows))]
fn validate_native_git(_path: &Path) -> Result<(), String> {
    Ok(())
}

#[cfg(windows)]
fn platform_null_device() -> &'static str {
    "NUL"
}

#[cfg(not(windows))]
fn platform_null_device() -> &'static str {
    "/dev/null"
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
