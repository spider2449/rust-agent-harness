//! Opt-in live validation of trusted-profile composition through the Codex Tool Bridge.
//!
//! This example is intentionally excluded from deterministic tests. It requires
//! `codex-cli 0.149.0`, live model access, and a prebuilt native
//! `rah-plugin-echo` fixture.

use std::{
    env, fs,
    path::PathBuf,
    process::ExitCode,
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use futures::StreamExt;
use rah_cli::profile_composition::compose;
use rah_protocol::{
    AgentEvent, AgentInput, AgentOptions, AgentRequest, Message, MessageRole, PermissionLevel,
    RequestId, ToolContent, ToolName,
};
use rah_runtime::AgentRuntime;
use rah_runtime_codex::{CodexRuntime, SUPPORTED_CODEX_VERSION};
use rah_tools::TrustedStaticProfile;
use serde_json::json;

const PROFILE_ID: &str = "live-trusted-profile-bridge";
const PROVIDER_ID: &str = "test";
const EXPECTED_TOOL: &str = "plugin.test.echo";
const PRIVATE_CODEX_ALIAS: &str = "rah_tool_0";
const EXPECTED_TEXT: &str = "RAH_TRUSTED_PROFILE_LIVE_OK";
const PROMPT: &str = "Use the available echo tool exactly once with this JSON input:\n{\"value\":\"RAH_TRUSTED_PROFILE_LIVE_OK\"}\n\nThen reply with exactly:\nRAH_TRUSTED_PROFILE_LIVE_OK\n\nDo not request any other tool.";
const TERMINAL_TIMEOUT: Duration = Duration::from_secs(120);

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter("rah=warn")
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
            eprintln!("LIVE_TRUSTED_PROFILE_BRIDGE_FAIL failed to create Tokio runtime: {error}");
            return ExitCode::FAILURE;
        }
    };
    match runtime.block_on(run()) {
        Ok(()) => {
            println!("LIVE_TRUSTED_PROFILE_BRIDGE_PASS");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("LIVE_TRUSTED_PROFILE_BRIDGE_FAIL {error}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), String> {
    let fixture = LiveFixture::create()?;
    let profile_path = fixture.write_profile()?;
    let profile = TrustedStaticProfile::load(&profile_path)
        .map_err(|error| format!("trusted profile source validation failed: {error}"))?;
    let composition = compose(profile)
        .await
        .map_err(|error| format!("effective composition failed: {error}"))?;
    verify_composition(&composition)?;

    let codex_executable = env::var_os("RAH_CODEX_EXECUTABLE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("codex"));
    let registry = composition.registry_handle();
    let runtime = CodexRuntime::connect_tool_bridge(
        &codex_executable,
        registry.clone(),
        vec![PermissionLevel::None],
    )
    .await;
    let turn_result = match runtime {
        Ok(runtime) => {
            let turn_result = run_turn(&runtime).await;
            let shutdown_result = runtime
                .shutdown()
                .await
                .map_err(|error| format!("Codex app-server shutdown failed: {error}"));
            drop(runtime);
            shutdown_result.and(turn_result)
        }
        Err(error) => Err(format!("Codex Tool Bridge connection failed: {error}")),
    };
    drop(registry);
    composition.shutdown().await;
    fixture.assert_clean_shutdown().await?;
    turn_result?;

    println!("CODEX_VERSION {SUPPORTED_CODEX_VERSION}");
    println!("PROFILE_ID {PROFILE_ID}");
    println!("PROVIDER kind=process_plugin id={PROVIDER_ID}");
    println!("TOOL name={EXPECTED_TOOL} permission=None");
    println!("PRIVATE_CODEX_ALIAS {PRIVATE_CODEX_ALIAS}");
    println!(
        "RESTRICTED_CODEX_CAPABILITIES shell=false file=false mcp=false web=false image=false apps=false approvals=false"
    );
    println!("CLEANUP_STATE codex_app_server=reaped provider_child=reaped");
    Ok(())
}

fn verify_composition(
    composition: &rah_cli::profile_composition::EffectiveProfileComposition,
) -> Result<(), String> {
    let effective = composition.effective_profile();
    if effective.profile_id != PROFILE_ID
        || effective.providers.len() != 1
        || effective.providers[0].kind != "process_plugin"
        || effective.providers[0].provider_id != PROVIDER_ID
        || effective.providers[0].status != "validated"
    {
        return Err(
            "redacted effective inventory did not describe the expected provider".to_owned(),
        );
    }
    let definitions = composition.registry().definitions();
    if definitions.len() != 1
        || definitions[0].name != ToolName::new(EXPECTED_TOOL)
        || definitions[0].permission != PermissionLevel::None
    {
        return Err("fresh registry did not contain exactly the permitted profile tool".to_owned());
    }
    println!("PROFILE_SOURCE_VALIDATION succeeded");
    println!("EFFECTIVE_COMPOSITION succeeded inventory=redacted owned_provider_lifecycle=true");
    println!("FRESH_TOOL_REGISTRY tool_count=1 tool={EXPECTED_TOOL} permission=None");
    Ok(())
}

async fn run_turn(runtime: &CodexRuntime) -> Result<(), String> {
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
        .map_err(|error| format!("failed to start Codex turn: {error}"))?;
    let mut events = handle.into_events();
    let mut sequence = Vec::new();
    let mut requested = 0_usize;
    let mut started = 0_usize;
    let mut finished = 0_usize;
    let mut continuation = false;
    let mut final_text = None;

    tokio::time::timeout(TERMINAL_TIMEOUT, async {
        while let Some(event) = events.next().await {
            sequence.push(event_name(&event));
            match event {
                AgentEvent::ToolRequested { tool_call, .. } => {
                    requested += 1;
                    if tool_call.name != ToolName::new(EXPECTED_TOOL)
                        || tool_call.input.0 != json!({"value": EXPECTED_TEXT})
                    {
                        return Err("Codex requested an unexpected tool or input".to_owned());
                    }
                }
                AgentEvent::ToolStarted { .. } => started += 1,
                AgentEvent::ToolFinished { output, .. } => {
                    finished += 1;
                    if output.is_error
                        || output.content.as_slice()
                            != [ToolContent::Text(EXPECTED_TEXT.to_owned())]
                    {
                        return Err(
                            "profile-composed tool returned an unexpected output".to_owned()
                        );
                    }
                }
                AgentEvent::ModelDelta { .. } if finished == 1 => continuation = true,
                AgentEvent::Completed { output, .. } => final_text = Some(output.message.content),
                AgentEvent::ApprovalRequired { .. } => {
                    return Err("restricted Codex runtime requested approval".to_owned());
                }
                AgentEvent::Failed { message, .. } => {
                    return Err(format!("Codex failed: {message}"));
                }
                AgentEvent::Cancelled { .. } => return Err("Codex turn was cancelled".to_owned()),
                AgentEvent::Started { .. }
                | AgentEvent::ModelRequestStarted { .. }
                | AgentEvent::ModelDelta { .. } => {}
            }
        }
        Ok::<(), String>(())
    })
    .await
    .map_err(|_| "timed out waiting for Codex completion".to_owned())??;

    if requested != 1 || started != 1 || finished != 1 || !continuation {
        return Err(format!(
            "unexpected bridge lifecycle: requested={requested} started={started} finished={finished} continuation={continuation}"
        ));
    }
    if final_text.as_deref() != Some(EXPECTED_TEXT) || sequence.last() != Some(&"Completed") {
        return Err("Codex did not produce the required terminal marker".to_owned());
    }
    println!("TOOL_REQUESTED_COUNT {requested}");
    println!("TOOL_STARTED_COUNT {started}");
    println!("TOOL_FINISHED_COUNT {finished}");
    println!("RAH_EVENT_SEQUENCE {}", sequence.join(" -> "));
    println!("CODEX_CONTINUATION true");
    println!("TERMINAL_STATE Completed");
    println!("FINAL_MARKER_RESULT matched");
    Ok(())
}

struct LiveFixture {
    directory: PathBuf,
    executable: PathBuf,
    lifecycle: PathBuf,
}

impl LiveFixture {
    fn create() -> Result<Self, String> {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_nanos();
        let directory = env::temp_dir().join(format!(
            "rah-live-trusted-profile-{}-{stamp}-{}",
            std::process::id(),
            NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&directory)
            .map_err(|error| format!("failed to create live fixture: {error}"))?;
        let executable = directory.join(format!("rah-plugin-echo{}", env::consts::EXE_SUFFIX));
        fs::copy(default_plugin_executable(), &executable)
            .map_err(|error| format!("failed to copy native plugin fixture: {error}"))?;
        let request = executable.with_extension("lifecycle-request");
        fs::write(&request, b"observe").map_err(|error| error.to_string())?;
        Ok(Self {
            lifecycle: request.with_extension("lifecycle"),
            directory,
            executable,
        })
    }

    fn write_profile(&self) -> Result<PathBuf, String> {
        let profile = self.directory.join("trusted-profile.json");
        let document = json!({
            "profile_version": 1,
            "profile_id": PROFILE_ID,
            "resources": {"executables": {"plugin-echo": {"path": self.executable, "kind": "native"}}},
            "capabilities": [],
            "process_plugins": [{
                "id": PROVIDER_ID,
                "executable": "plugin-echo",
                "tools": [{
                    "remote_name": "echo",
                    "permission": "None",
                    "input_schema": {"type":"object","properties":{"value":{}},"required":["value"],"additionalProperties":false}
                }]
            }]
        });
        fs::write(
            &profile,
            serde_json::to_vec(&document).map_err(|error| error.to_string())?,
        )
        .map_err(|error| format!("failed to write trusted profile: {error}"))?;
        Ok(profile)
    }

    async fn assert_clean_shutdown(&self) -> Result<(), String> {
        for _ in 0..50 {
            let events = fs::read_to_string(&self.lifecycle).unwrap_or_default();
            if events.lines().collect::<Vec<_>>() == ["spawn", "call", "shutdown", "exit"] {
                println!("PROVIDER_EXECUTION_COUNT 1");
                return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        Err("provider child did not complete exactly one call and clean shutdown".to_owned())
    }
}

impl Drop for LiveFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.directory);
    }
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
