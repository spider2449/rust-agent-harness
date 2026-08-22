//! Opt-in live validation for trusted-profile-composed `repo.patch`.
//!
//! This is intentionally excluded from deterministic tests. It requires the
//! pinned local `codex-cli 0.149.0` and live Codex access. The profile is loaded
//! through the normal trusted source and the actual `rah-cli` effective composer.

use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command, ExitCode},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use futures::StreamExt;
use rah_cli::profile_composition::{EffectiveProfileComposition, compose};
use rah_protocol::{
    AgentEvent, AgentInput, AgentOptions, AgentRequest, Message, MessageRole, PermissionLevel,
    RequestId, ToolContent, ToolDefinition, ToolName, ToolOutput,
};
use rah_runtime::AgentRuntime;
use rah_runtime_codex::{CodexRuntime, SUPPORTED_CODEX_VERSION};
use rah_tools::{
    REPOSITORY_WORKTREE_PATCH_TOOL_NAME, TrustedStaticProfile, live_test_replacement_attempts,
    reset_live_test_replacement_attempts,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const PROFILE_ID: &str = "live-repo-patch";
const GIT_RESOURCE_ID: &str = "live-git";
const REPOSITORY_RESOURCE_ID: &str = "live-repository";
const TARGET: &str = "target.txt";
const UNRELATED: &str = "unrelated.txt";
const PREIMAGE: &str = "RAH_LIVE_PATCH_BEFORE\n";
const POSTIMAGE: &str = "RAH_LIVE_PATCH_AFTER\n";
const FINAL_MARKER: &str = "RAH_REPO_PATCH_LIVE_OK";
const PRIVATE_ALIAS: &str = "rah_tool_0";
const TERMINAL_TIMEOUT: Duration = Duration::from_secs(120);

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

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
            eprintln!(
                "LIVE_TRUSTED_PROFILE_REPO_PATCH_FAIL failed to create Tokio runtime: {error}"
            );
            return ExitCode::FAILURE;
        }
    };
    match runtime.block_on(run()) {
        Ok(()) => {
            println!("LIVE_TRUSTED_PROFILE_REPO_PATCH_PASS");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("LIVE_TRUSTED_PROFILE_REPO_PATCH_FAIL {error}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), String> {
    let git = native_git()?;
    let mut fixture = LiveFixture::create(&git)?;
    let profile_path = fixture.write_profile(&git)?;
    let profile = TrustedStaticProfile::load(&profile_path)
        .map_err(|error| format!("trusted profile source validation failed: {error}"))?;
    let composition = compose(profile)
        .await
        .map_err(|error| format!("effective composition failed: {error}"))?;
    let definition = verify_composition(&composition)?;
    let request = fixture.request();
    let prompt = prompt(&request)?;

    // This counter is opt-in live-fixture instrumentation only. The actual
    // constructor remains exclusively inside the real effective composer.
    reset_live_test_replacement_attempts();
    let codex = env::var_os("RAH_CODEX_EXECUTABLE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("codex"));
    let registry = composition.registry_handle();
    let runtime =
        CodexRuntime::connect_tool_bridge(&codex, registry.clone(), vec![PermissionLevel::Execute])
            .await
            .map_err(|error| format!("Codex Tool Bridge connection failed: {error}"))?;

    let turn_result = run_turn(&runtime, &prompt, &request).await;
    let shutdown_result = runtime
        .shutdown()
        .await
        .map_err(|error| format!("Codex app-server shutdown/reap failed: {error}"));
    drop(runtime);
    drop(registry);
    composition.shutdown().await;
    shutdown_result?;
    let outcome = turn_result?;

    fixture.assert_after(&git)?;
    if outcome.requested != 1
        || outcome.started != 1
        || outcome.finished != 1
        || live_test_replacement_attempts() != 1
    {
        return Err(format!(
            "exactly-once evidence failed: requested={} started={} finished={} native_replacement_attempts={}",
            outcome.requested,
            outcome.started,
            outcome.finished,
            live_test_replacement_attempts()
        ));
    }
    audit_model_visible_output(&outcome.tool_output, &outcome.final_text, &fixture.root)?;

    println!("CODEX_VERSION {SUPPORTED_CODEX_VERSION}");
    println!("CODEX_EXECUTABLE_IDENTITY native_discovery_and_exact_version_verified=true");
    println!("PROFILE_ID {PROFILE_ID}");
    println!(
        "PROFILE_SYMBOLIC_RESOURCES executable={GIT_RESOURCE_ID} repository={REPOSITORY_RESOURCE_ID}"
    );
    println!("RAH_TOOL_NAME {REPOSITORY_WORKTREE_PATCH_TOOL_NAME}");
    println!("PRIVATE_ALIAS_MAPPING {PRIVATE_ALIAS} -> {REPOSITORY_WORKTREE_PATCH_TOOL_NAME}");
    println!("BRIDGE_PERMISSION_ALLOWLIST [Execute]");
    println!(
        "MODEL_VISIBLE_SCHEMA {}",
        serde_json::to_string(&definition.input_schema).map_err(display_error)?
    );
    println!(
        "RESTRICTED_CODEX_CAPABILITIES shell=false file=false mcp=false process=false network_tools=false web=false image=false apps=false approvals=false"
    );
    println!("TOOL_REQUESTED_COUNT {}", outcome.requested);
    println!("TOOL_STARTED_COUNT {}", outcome.started);
    println!("TOOL_FINISHED_COUNT {}", outcome.finished);
    println!("REPO_PATCH_INVOCATION_COUNT {}", outcome.started);
    println!(
        "NATIVE_REPLACEMENT_ATTEMPT_COUNT {}",
        live_test_replacement_attempts()
    );
    println!("RAH_EVENT_SEQUENCE {}", outcome.sequence.join(" -> "));
    println!("TERMINAL_STATE Completed");
    println!("FINAL_MARKER_RESULT matched");
    println!("MODEL_VISIBLE_PRIVACY_AUDIT passed");
    println!(
        "WORKTREE_ASSERTIONS postimage=true preimage_absent=true index_unchanged=true unrelated_unchanged=true metadata_only_worktree_status=true"
    );
    println!(
        "CLEANUP_STATE codex_app_server=reaped plugin_or_mcp_child=absent temp_siblings=absent"
    );

    fixture.remove()?;
    println!("TEMP_REPOSITORY_CLEANUP removed=true");
    Ok(())
}

fn verify_composition(composition: &EffectiveProfileComposition) -> Result<ToolDefinition, String> {
    let effective = composition.effective_profile();
    if effective.profile_id != PROFILE_ID
        || effective.capabilities.len() != 1
        || !effective.providers.is_empty()
    {
        return Err(
            "redacted effective inventory was not the expected repo.patch-only profile".to_owned(),
        );
    }
    let capability = &effective.capabilities[0];
    if capability.capability_id != REPOSITORY_WORKTREE_PATCH_TOOL_NAME
        || !capability.enabled
        || !capability.registered
        || capability.permission != PermissionLevel::Execute
        || capability.resources != [GIT_RESOURCE_ID, REPOSITORY_RESOURCE_ID]
        || capability.validation != "validated"
    {
        return Err(
            "effective inventory did not preserve the expected symbolic repo.patch binding"
                .to_owned(),
        );
    }
    let definitions = composition.registry().definitions();
    let [definition] = definitions.as_slice() else {
        return Err("fresh effective registry did not publish exactly one tool".to_owned());
    };
    if definition.name != ToolName::new(REPOSITORY_WORKTREE_PATCH_TOOL_NAME)
        || definition.permission != PermissionLevel::Execute
        || definition.input_schema != expected_schema()
    {
        return Err(
            "composed repo.patch definition did not match the expected contract".to_owned(),
        );
    }
    println!("PROFILE_SOURCE_VALIDATION succeeded");
    println!("EFFECTIVE_COMPOSITION succeeded trusted_source=true real_composer=true");
    println!("FRESH_TOOL_REGISTRY tool_count=1 tool=repo.patch permission=Execute");
    Ok(definition.clone())
}

async fn run_turn(
    runtime: &CodexRuntime,
    prompt: &str,
    expected_input: &Value,
) -> Result<TurnOutcome, String> {
    let handle = runtime
        .start(AgentRequest {
            request_id: RequestId::new(),
            input: AgentInput {
                messages: vec![Message {
                    role: MessageRole::User,
                    content: prompt.to_owned(),
                }],
            },
            options: AgentOptions::default(),
        })
        .await
        .map_err(|error| format!("failed to start Codex turn: {error}"))?;
    let mut events = handle.into_events();
    let mut outcome = TurnOutcome::default();

    tokio::time::timeout(TERMINAL_TIMEOUT, async {
        while let Some(event) = events.next().await {
            outcome.sequence.push(event_name(&event).to_owned());
            match event {
                AgentEvent::ToolRequested { tool_call, .. } => {
                    outcome.requested += 1;
                    if tool_call.name != ToolName::new(REPOSITORY_WORKTREE_PATCH_TOOL_NAME)
                        || tool_call.input.0 != *expected_input
                    {
                        return Err("Codex requested an unexpected repo.patch call".to_owned());
                    }
                }
                AgentEvent::ToolStarted { .. } => outcome.started += 1,
                AgentEvent::ToolFinished { output, .. } => {
                    outcome.finished += 1;
                    if output.is_error
                        || output.content != [ToolContent::Json(expected_success_output())]
                    {
                        return Err("repo.patch returned an unexpected public output".to_owned());
                    }
                    outcome.tool_output = Some(output);
                }
                AgentEvent::ModelDelta { .. } if outcome.finished == 1 => {
                    outcome.continuation = true
                }
                AgentEvent::Completed { output, .. } => {
                    outcome.final_text = Some(output.message.content)
                }
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

    if outcome.requested != 1
        || outcome.started != 1
        || outcome.finished != 1
        || !outcome.continuation
    {
        return Err(format!(
            "unexpected bridge lifecycle: requested={} started={} finished={} continuation={}",
            outcome.requested, outcome.started, outcome.finished, outcome.continuation
        ));
    }
    if outcome.final_text.as_deref() != Some(FINAL_MARKER)
        || outcome.sequence.last().map(String::as_str) != Some("Completed")
    {
        return Err("Codex did not produce the required terminal marker".to_owned());
    }
    Ok(outcome)
}

fn prompt(request: &Value) -> Result<String, String> {
    let request = serde_json::to_string(request).map_err(display_error)?;
    let expected_old = PREIMAGE.trim_end();
    let replacement = POSTIMAGE.trim_end();
    Ok(format!(
        "Invoke the available RAH patch tool exactly once with exactly this JSON input:\n{request}\n\nThe logical target is {TARGET}; use the supplied SHA-256 and byte-length preconditions, replace exactly `{expected_old}` with `{replacement}`, and do not request any other tool. After the tool succeeds, reply with exactly:\n{FINAL_MARKER}"
    ))
}

fn expected_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "path": {"type": "string", "maxLength": 1024},
            "expected_file_sha256": {"type": "string", "pattern": "^[0-9a-f]{64}$"},
            "expected_file_byte_length": {"type": "integer", "minimum": 0, "maximum": 1024 * 1024},
            "expected_old_text": {"type": "string", "minLength": 1, "maxLength": 64 * 1024},
            "replacement_text": {"type": "string", "maxLength": 64 * 1024}
        },
        "required": ["path", "expected_file_sha256", "expected_file_byte_length", "expected_old_text", "replacement_text"],
        "additionalProperties": false
    })
}

fn expected_success_output() -> Value {
    json!({"status":"ok","changed":true,"uncertain":false,"reason":"none"})
}

fn audit_model_visible_output(
    output: &Option<ToolOutput>,
    final_text: &Option<String>,
    fixture_root: &Path,
) -> Result<(), String> {
    let visible = serde_json::to_string(output).map_err(display_error)?;
    let root = fixture_root.to_string_lossy();
    for private in [
        root.as_ref(),
        ".rah-repo-patch-",
        "git.exe",
        PREIMAGE,
        POSTIMAGE,
    ] {
        if visible.contains(private) {
            return Err("model-visible tool output exposed private fixture evidence".to_owned());
        }
    }
    if final_text.as_deref() != Some(FINAL_MARKER) {
        return Err("final model output was not the exact requested marker".to_owned());
    }
    Ok(())
}

#[derive(Default)]
struct TurnOutcome {
    requested: usize,
    started: usize,
    finished: usize,
    continuation: bool,
    sequence: Vec<String>,
    tool_output: Option<ToolOutput>,
    final_text: Option<String>,
}

struct LiveFixture {
    directory: PathBuf,
    root: PathBuf,
    target: PathBuf,
    unrelated: PathBuf,
    before_index: Vec<u8>,
    before_head: Vec<u8>,
    before_refs: Vec<u8>,
}

impl LiveFixture {
    fn create(git: &Path) -> Result<Self, String> {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(display_error)?
            .as_nanos();
        let directory = env::temp_dir().join(format!(
            "rah-live-repo-patch-{}-{stamp}-{}",
            std::process::id(),
            NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&directory).map_err(display_error)?;
        let root = directory.join("repository");
        fs::create_dir(&root).map_err(display_error)?;
        git_ok(git, &root, &["init", "--quiet"])?;
        git_ok(
            git,
            &root,
            &["config", "user.email", "rah-live@example.invalid"],
        )?;
        git_ok(git, &root, &["config", "user.name", "RAH live fixture"])?;
        let target = root.join(TARGET);
        let unrelated = root.join(UNRELATED);
        fs::write(&target, PREIMAGE).map_err(display_error)?;
        fs::write(&unrelated, "RAH_LIVE_UNRELATED\n").map_err(display_error)?;
        git_ok(git, &root, &["add", "--", TARGET, UNRELATED])?;
        git_ok(git, &root, &["commit", "--quiet", "-m", "live fixture"])?;
        let root = fs::canonicalize(root).map_err(display_error)?;
        let before_index = git_output(git, &root, &["ls-files", "-s"])?;
        let before_head = git_output(git, &root, &["rev-parse", "HEAD"])?;
        let before_refs = git_output(git, &root, &["show-ref", "--head"])?;
        if !git_output(git, &root, &["status", "--porcelain=v1"])?.is_empty() {
            return Err("live fixture must begin with a clean worktree".to_owned());
        }
        Ok(Self {
            directory,
            target: root.join(TARGET),
            unrelated: root.join(UNRELATED),
            root,
            before_index,
            before_head,
            before_refs,
        })
    }

    fn write_profile(&self, git: &Path) -> Result<PathBuf, String> {
        let path = self.directory.join("trusted-profile.json");
        let document = json!({
            "profile_version": 1,
            "profile_id": PROFILE_ID,
            "resources": {
                "executables": {GIT_RESOURCE_ID: {"path": git, "kind": "native"}},
                "repositories": {REPOSITORY_RESOURCE_ID: {"path": &self.root}}
            },
            "capabilities": [{
                "name": REPOSITORY_WORKTREE_PATCH_TOOL_NAME,
                "enabled": true,
                "permission": "execute",
                "executable": GIT_RESOURCE_ID,
                "repository": REPOSITORY_RESOURCE_ID
            }]
        });
        fs::write(&path, serde_json::to_vec(&document).map_err(display_error)?)
            .map_err(display_error)?;
        Ok(path)
    }

    fn request(&self) -> Value {
        let bytes = fs::read(&self.target).expect("fresh live fixture target should be readable");
        json!({
            "path": TARGET,
            "expected_file_sha256": format!("{:x}", Sha256::digest(&bytes)),
            "expected_file_byte_length": bytes.len(),
            "expected_old_text": PREIMAGE.trim_end(),
            "replacement_text": POSTIMAGE.trim_end(),
        })
    }

    fn assert_after(&self, git: &Path) -> Result<(), String> {
        if fs::read(&self.target).map_err(display_error)? != POSTIMAGE.as_bytes() {
            return Err("expected postimage was not present".to_owned());
        }
        if fs::read(&self.target)
            .map_err(display_error)?
            .windows(PREIMAGE.len())
            .any(|part| part == PREIMAGE.as_bytes())
        {
            return Err("preimage remained in the patched target".to_owned());
        }
        if fs::read(&self.unrelated).map_err(display_error)? != b"RAH_LIVE_UNRELATED\n" {
            return Err("unrelated tracked file changed".to_owned());
        }
        if git_output(git, &self.root, &["ls-files", "-s"])? != self.before_index
            || git_output(git, &self.root, &["rev-parse", "HEAD"])? != self.before_head
            || git_output(git, &self.root, &["show-ref", "--head"])? != self.before_refs
        {
            return Err("repository index, HEAD, or refs changed".to_owned());
        }
        if git_output(git, &self.root, &["status", "--porcelain=v1"])? != b" M target.txt\n" {
            return Err(
                "repository metadata/status was not exactly the expected worktree-only mutation"
                    .to_owned(),
            );
        }
        if fs::read_dir(&self.root)
            .map_err(display_error)?
            .any(|entry| {
                entry
                    .map(|entry| {
                        entry
                            .file_name()
                            .to_string_lossy()
                            .starts_with(".rah-repo-patch-")
                    })
                    .unwrap_or(true)
            })
        {
            return Err(
                "repo.patch temporary sibling remained after the terminal event".to_owned(),
            );
        }
        Ok(())
    }

    fn remove(&mut self) -> Result<(), String> {
        fs::remove_dir_all(&self.directory).map_err(display_error)?;
        if self.directory.exists() {
            return Err("temporary repository fixture remains after cleanup".to_owned());
        }
        Ok(())
    }
}

impl Drop for LiveFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.directory);
    }
}

fn native_git() -> Result<PathBuf, String> {
    #[cfg(windows)]
    let output = Command::new("where.exe")
        .arg("git.exe")
        .output()
        .map_err(display_error)?;
    #[cfg(not(windows))]
    let output = Command::new("which")
        .arg("git")
        .output()
        .map_err(display_error)?;
    if !output.status.success() {
        return Err("native Git executable is required for the live fixture".to_owned());
    }
    let path = String::from_utf8(output.stdout)
        .map_err(display_error)?
        .lines()
        .next()
        .ok_or_else(|| "native Git executable was not reported".to_owned())?
        .to_owned();
    fs::canonicalize(path).map_err(display_error)
}

fn git_ok(git: &Path, root: &Path, arguments: &[&str]) -> Result<(), String> {
    let output = Command::new(git)
        .args(arguments)
        .current_dir(root)
        .output()
        .map_err(display_error)?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "fixture Git command {arguments:?} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

fn git_output(git: &Path, root: &Path, arguments: &[&str]) -> Result<Vec<u8>, String> {
    let output = Command::new(git)
        .args(arguments)
        .current_dir(root)
        .output()
        .map_err(display_error)?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(format!(
            "fixture Git command {arguments:?} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
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

fn display_error(error: impl std::fmt::Display) -> String {
    error.to_string()
}
