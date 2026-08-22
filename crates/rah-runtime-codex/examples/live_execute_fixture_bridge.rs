//! Opt-in live validation of the hardened Execute fixture through Codex.
//!
//! This example requires the exactly supported Codex CLI version, the prebuilt
//! repository fixture binary, and live model access. It is excluded from the
//! normal deterministic test suite.

use std::{
    collections::BTreeMap,
    env,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    process::ExitCode,
    sync::Arc,
    time::Duration,
};

use futures::StreamExt;
use rah_protocol::{
    AgentEvent, AgentInput, AgentOptions, AgentRequest, Message, MessageRole, PermissionLevel,
    RequestId, ToolContent, ToolName,
};
use rah_runtime::AgentRuntime;
use rah_runtime_codex::{CodexRuntime, SUPPORTED_CODEX_VERSION};
use rah_tools::{HostArgumentPolicy, HostExecutionPolicy, HostExecutionTool, Tool, ToolRegistry};
use serde_json::json;

const TOOL_NAME: &str = "process.test.echo";
const CODEX_ALIAS: &str = "rah_tool_0";
const EXPECTED_TEXT: &str = "RAH_EXECUTE_BRIDGE_OK";
const FIXED_ENVIRONMENT_NAME: &str = "RAH_EXECUTE_FIXED";
const FIXED_ENVIRONMENT_VALUE: &str = "visible";
const PROMPT: &str = "Use the available RAH tool exactly once with the text:\nRAH_EXECUTE_BRIDGE_OK\n\nAfter receiving the tool result, reply with exactly:\nRAH_EXECUTE_BRIDGE_OK\n\nDo not request any other tool.";
const TERMINAL_TIMEOUT: Duration = Duration::from_secs(120);

struct AuditDirectory(PathBuf);

impl Drop for AuditDirectory {
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
            eprintln!("LIVE_EXECUTE_FIXTURE_BRIDGE_FAIL failed to create Tokio runtime: {error}");
            return ExitCode::FAILURE;
        }
    };

    match runtime.block_on(run()) {
        Ok(()) => {
            println!("LIVE_EXECUTE_FIXTURE_BRIDGE_PASS");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("LIVE_EXECUTE_FIXTURE_BRIDGE_FAIL {error}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), String> {
    let codex_executable = env::var_os("RAH_CODEX_EXECUTABLE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("codex"));
    let repository = repository_root()?;
    let fixture = fixture_path(&repository)?;
    let audit = create_audit_directory(&repository)?;
    let audit_file = audit.0.join("fixture-audit.txt");
    let count_file = audit.0.join("fixture-count.txt");
    let cwd = fs::canonicalize(&repository).map_err(|error| error.to_string())?;
    let mut environment = BTreeMap::new();
    environment.insert(
        OsString::from(FIXED_ENVIRONMENT_NAME),
        OsString::from(FIXED_ENVIRONMENT_VALUE),
    );
    let policy = HostExecutionPolicy::new(
        &fixture,
        HostArgumentPolicy::Text {
            prefix: vec![
                "audited-echo".to_owned(),
                audit_file.to_string_lossy().into_owned(),
                count_file.to_string_lossy().into_owned(),
            ],
            max_bytes: 64 * 1024,
        },
        &repository,
        ".",
    )
    .map_err(|error| format!("failed to construct HostExecutionPolicy: {error}"))?
    .with_environment(environment)
    .map_err(|error| format!("failed to configure environment: {error}"))?;
    let tool = HostExecutionTool::new(
        TOOL_NAME,
        "Echoes one literal text value through the repository-owned validation fixture.",
        policy,
    );
    let definition = tool.definition();
    validate_definition(&definition)?;
    let mut registry = ToolRegistry::new();
    registry
        .register(Arc::new(tool))
        .map_err(|error| format!("failed to register Execute fixture tool: {error}"))?;
    if registry.definitions().len() != 1 {
        return Err("registry did not contain exactly one advertised tool".to_owned());
    }

    println!("CODEX_EXECUTABLE {}", codex_executable.display());
    println!("REQUIRED_CODEX_VERSION {SUPPORTED_CODEX_VERSION}");
    println!("EXECUTE_TOOL_NAME {TOOL_NAME}");
    println!("CODEX_SAFE_ALIAS {CODEX_ALIAS}");
    println!("PRIVATE_ALIAS_MAPPING {CODEX_ALIAS} -> {TOOL_NAME}");
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
    println!("FIXTURE_EXECUTABLE_CANONICAL {}", fixture.display());
    println!(
        "HOST_ARGUMENT_POLICY_PREFIX {}",
        json!(["audited-echo", "<host-audit-file>", "<host-count-file>"])
    );
    println!("HOST_CWD_CANONICAL {}", cwd.display());
    println!(
        "HOST_ENVIRONMENT {}",
        json!({FIXED_ENVIRONMENT_NAME: FIXED_ENVIRONMENT_VALUE})
    );
    println!("HOST_TIMEOUT_SECONDS 5");
    println!("STDIN_POLICY null");
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
    let live_result = run_turn(
        Arc::clone(&runtime),
        &audit_file,
        &count_file,
        &fixture,
        &cwd,
    )
    .await;
    let shutdown_result = runtime.shutdown().await;
    match &shutdown_result {
        Ok(()) => println!("CODEX_CLEANUP app-server shutdown and reap completed"),
        Err(error) => eprintln!("CODEX_CLEANUP_FAIL {error}"),
    }
    shutdown_result.map_err(|error| format!("app-server shutdown failed: {error}"))?;
    live_result?;
    println!("FIXTURE_CLEANUP child exited normally and was reaped");
    println!("AUDIT_CLEANUP host audit directory scheduled for removal");
    Ok(())
}

async fn run_turn(
    runtime: Arc<CodexRuntime>,
    audit_file: &Path,
    count_file: &Path,
    fixture: &Path,
    expected_cwd: &Path,
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
                        "ACTUAL_RAH_TOOL_CALL name={} input={}",
                        tool_call.name, tool_call.input.0
                    );
                    if tool_call.name != ToolName::new(TOOL_NAME)
                        || tool_call.input.0 != json!({"text": EXPECTED_TEXT})
                    {
                        return Err(format!("unexpected RAH ToolCall: {tool_call:?}"));
                    }
                }
                AgentEvent::ToolStarted { .. } => started += 1,
                AgentEvent::ToolFinished { output, .. } => {
                    finished += 1;
                    tool_finished = true;
                    if output.is_error {
                        return Err("HostExecutionTool returned an error output".to_owned());
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
    if content["status"] != "exited"
        || content["stdout"] != EXPECTED_TEXT
        || content["stderr"] != ""
        || content["exit_code"] != 0
    {
        return Err(format!("unexpected process ToolOutput: {content}"));
    }
    let serialized_output_bytes = serde_json::to_vec(&output)
        .map_err(|error| error.to_string())?
        .len();
    if serialized_output_bytes > 768 * 1024 {
        return Err(format!(
            "ToolOutput exceeded serialized bound: {serialized_output_bytes}"
        ));
    }
    let final_text = final_text.ok_or_else(|| "missing Completed output".to_owned())?;
    if final_text != EXPECTED_TEXT {
        return Err(format!("unexpected final assistant text: {final_text:?}"));
    }
    let audit = parse_audit(audit_file)?;
    validate_audit(&audit, count_file, fixture, expected_cwd)?;

    println!("RAH_EVENT_SEQUENCE {}", observed.join(" -> "));
    println!("TOOL_LIFECYCLE_COUNTS requested={requested} started={started} finished={finished}");
    println!("PROCESS_EXECUTION_COUNT {}", audit["execution_count"]);
    println!(
        "ACTUAL_FIXTURE_ARGV {}",
        json!([
            "audited-echo",
            "<host-audit-file>",
            "<host-count-file>",
            audit["input"]
        ])
    );
    println!("CWD_AUDIT canonical_match=true model_selected=false");
    println!(
        "ENVIRONMENT_AUDIT cleared=true exact={}",
        audit["environment"]
    );
    println!("STDIN_AUDIT bytes_read={}", audit["stdin_bytes"]);
    println!(
        "WINDOWS_JOB_AUDIT process_in_job={}",
        audit["process_in_job"]
    );
    println!(
        "TOOL_OUTPUT_SUMMARY status=exited stdout_bytes={} stderr_bytes={} serialized_bytes={serialized_output_bytes}",
        EXPECTED_TEXT.len(),
        0
    );
    println!("DYNAMIC_TOOL_RESPONSE_SUMMARY success=true content_items=1 type=inputText");
    println!(
        "CODEX_CONTINUED_AFTER_TOOL_RESPONSE true model_delta_count={model_deltas_after_tool}"
    );
    println!("FINAL_ASSISTANT_TEXT {final_text:?}");
    println!("TERMINAL_RAH_EVENT Completed");
    println!("REPLAY_RETRY_COUNT 0");
    println!("PROHIBITED_ACTIONS none observed; restricted Codex surfaces remained disabled");
    Ok(())
}

fn validate_definition(definition: &rah_protocol::ToolDefinition) -> Result<(), String> {
    if definition.name != ToolName::new(TOOL_NAME)
        || definition.permission != PermissionLevel::Execute
        || definition.input_schema
            != json!({
                "type": "object",
                "properties": {"text": {"type": "string", "maxLength": 64 * 1024}},
                "required": ["text"],
                "additionalProperties": false
            })
    {
        return Err(format!(
            "unexpected Execute tool definition: {definition:?}"
        ));
    }
    let schema = definition.input_schema.to_string().to_ascii_lowercase();
    for forbidden in [
        "executable",
        "program",
        "args",
        "argv",
        "cwd",
        "env",
        "environment",
        "timeout",
        "shell",
        "command",
    ] {
        if schema.contains(forbidden) {
            return Err(format!(
                "model-visible schema exposed forbidden field `{forbidden}`"
            ));
        }
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

fn parse_audit(path: &Path) -> Result<BTreeMap<String, String>, String> {
    let text =
        fs::read_to_string(path).map_err(|error| format!("failed to read host audit: {error}"))?;
    text.lines()
        .map(|line| {
            line.split_once('=')
                .map(|(name, value)| (name.to_owned(), value.to_owned()))
                .ok_or_else(|| format!("malformed host audit line: {line:?}"))
        })
        .collect()
}

fn validate_audit(
    audit: &BTreeMap<String, String>,
    count_file: &Path,
    fixture: &Path,
    expected_cwd: &Path,
) -> Result<(), String> {
    let expected_environment = format!("{FIXED_ENVIRONMENT_NAME}={FIXED_ENVIRONMENT_VALUE}");
    let expected = [
        ("execution_count", "1"),
        ("input", EXPECTED_TEXT),
        ("stdin_bytes", "0"),
        ("environment", expected_environment.as_str()),
    ];
    for (name, value) in expected {
        if audit.get(name).map(String::as_str) != Some(value) {
            return Err(format!(
                "host audit field `{name}` was not {value:?}: {audit:?}"
            ));
        }
    }
    if fs::read_to_string(count_file).map_err(|error| error.to_string())? != "1" {
        return Err("fixture count file did not prove exactly one process execution".to_owned());
    }
    let actual_executable =
        fs::canonicalize(&audit["executable"]).map_err(|error| error.to_string())?;
    if actual_executable != fixture {
        return Err(format!(
            "fixture executable mismatch: {}",
            actual_executable.display()
        ));
    }
    let actual_cwd = fs::canonicalize(&audit["cwd"]).map_err(|error| error.to_string())?;
    if actual_cwd != expected_cwd {
        return Err(format!("fixture cwd mismatch: {}", actual_cwd.display()));
    }
    #[cfg(windows)]
    if audit.get("process_in_job").map(String::as_str) != Some("true") {
        return Err("fixture did not observe Windows Job Object membership".to_owned());
    }
    Ok(())
}

fn create_audit_directory(repository: &Path) -> Result<AuditDirectory, String> {
    let path = repository
        .join("target")
        .join(format!("rah-live-execute-audit-{}", std::process::id()));
    if path.exists() {
        return Err(format!(
            "audit directory already exists: {}",
            path.display()
        ));
    }
    fs::create_dir(&path).map_err(|error| error.to_string())?;
    Ok(AuditDirectory(path))
}

fn fixture_path(repository: &Path) -> Result<PathBuf, String> {
    let executable = if cfg!(windows) {
        repository.join("target/debug/rah_execute_fixture.exe")
    } else {
        repository.join("target/debug/rah_execute_fixture")
    };
    fs::canonicalize(&executable).map_err(|error| {
        format!(
            "failed to resolve prebuilt repository fixture {}: {error}; run `cargo build -p rah-tools --bin rah_execute_fixture` first",
            executable.display()
        )
    })
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
