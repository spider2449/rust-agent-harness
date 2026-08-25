//! Opt-in certified live validation for trusted-profile-composed `repo.create-file`.
//!
//! Host observations, rather than the model's final prose, determine success.

use std::{
    collections::{BTreeMap, HashMap},
    env, fs,
    path::{Path, PathBuf},
    process::{Command, ExitCode},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[cfg(windows)]
use std::os::windows::fs::MetadataExt;

#[path = "support/live_gate_contract.rs"]
mod live_gate_contract;

use futures::StreamExt;
use rah_cli::profile_composition::{EffectiveProfileComposition, compose};
use rah_protocol::{
    AgentEvent, AgentInput, AgentOptions, AgentRequest, Message, MessageRole, PermissionLevel,
    RequestId, ToolCallId, ToolContent, ToolOutput,
};
use rah_runtime::AgentRuntime;
use rah_runtime_codex::{CodexRuntime, SUPPORTED_CODEX_VERSION};
use rah_tools::{
    REPOSITORY_CREATE_FILE_TOOL_NAME, REPOSITORY_FILE_INFO_TOOL_NAME, REPOSITORY_STATUS_TOOL_NAME,
    TrustedStaticProfile,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const PROFILE_ID: &str = "live-create-file";
const GIT_RESOURCE_ID: &str = "live-git";
const REPOSITORY_RESOURCE_ID: &str = "live-repository";
const TARGET: &str = "src/live_marker.rs";
const SENTINEL: &str = "sentinel.txt";
const CONTENT: &str =
    "// RAH certified live fixture: UTF-8 \u{03bb}\npub const LIVE_CREATE_FILE: &str = \"ok\";\n";
const FINAL_MARKER: &str = "RAH_CREATE_FILE_LIVE_OK";
const TIMEOUT: Duration = Duration::from_secs(120);
const TOOLS: [&str; 3] = [
    REPOSITORY_CREATE_FILE_TOOL_NAME,
    REPOSITORY_FILE_INFO_TOOL_NAME,
    REPOSITORY_STATUS_TOOL_NAME,
];
static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter("rah=warn")
        .without_time()
        .init();
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("LIVE_CREATE_FILE_FAIL Tokio runtime: {error}");
            return ExitCode::FAILURE;
        }
    };
    match runtime.block_on(run()) {
        Ok(()) => {
            println!("{FINAL_MARKER}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("LIVE_CREATE_FILE_FAIL {error}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), String> {
    let git = native_git()?;
    let mut fixture = Fixture::create(&git)?;
    let profile = TrustedStaticProfile::load(fixture.write_profile(&git)?)
        .map_err(|error| format!("trusted profile source validation failed: {error}"))?;
    let composition = compose(profile)
        .await
        .map_err(|error| format!("effective composition failed: {error}"))?;
    let aliases = verify_composition(&composition)?;
    let registry = composition.registry_handle();
    let codex = env::var_os("RAH_CODEX_EXECUTABLE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("codex"));
    let runtime =
        CodexRuntime::connect_tool_bridge(&codex, registry.clone(), vec![PermissionLevel::Execute])
            .await
            .map_err(|error| {
                format!("native Codex discovery, schema, or bridge connection failed: {error}")
            })?;
    let turn = run_turn(&runtime, &fixture.request()).await;
    let shutdown = runtime
        .shutdown()
        .await
        .map_err(|error| format!("Codex app-server shutdown/reap failed: {error}"));
    drop(runtime);
    drop(registry);
    composition.shutdown().await;
    shutdown?;
    let outcome = turn?;
    fixture.assert_after(&git)?;
    audit_observers(&outcome, &fixture)?;
    live_gate_contract::require_exactly_once(
        REPOSITORY_CREATE_FILE_TOOL_NAME,
        outcome.count(REPOSITORY_CREATE_FILE_TOOL_NAME, Count::Requested),
        outcome.count(REPOSITORY_CREATE_FILE_TOOL_NAME, Count::Started),
        outcome.count(REPOSITORY_CREATE_FILE_TOOL_NAME, Count::Finished),
    )?;

    println!("CODEX_VERSION {SUPPORTED_CODEX_VERSION}");
    println!("CODEX_EXECUTABLE_IDENTITY native_discovery_and_exact_version_verified=true");
    println!("PROFILE_SOURCE_VALIDATION succeeded");
    println!(
        "EFFECTIVE_COMPOSITION succeeded trusted_source=true real_composer=true fresh_registry=true"
    );
    for (tool, alias) in aliases {
        println!("PRIVATE_ALIAS_MAPPING {tool} -> {alias}");
    }
    println!("BRIDGE_PERMISSION_ALLOWLIST [Execute]");
    println!(
        "RESTRICTED_CODEX_CAPABILITIES shell=false file=false mcp=false process=false network_tools=false web=false image=false apps=false approvals=false"
    );
    println!(
        "TOOL_COUNTS {} native_successful_create=1 bridge_replay=false",
        outcome.summary()
    );
    println!(
        "TARGET path={TARGET} length={} sha256={}",
        CONTENT.len(),
        fixture.hash
    );
    println!("OBSERVER_ASSERTIONS file_info=passed status=passed");
    println!(
        "REPOSITORY_INVARIANTS target_regular=true target_not_reparse=true index_bytes=true head=true refs=true sentinel=true untracked_only=true staging=false"
    );
    println!("RAH_EVENT_SEQUENCE {}", outcome.sequence.join(" -> "));
    println!("TERMINAL_STATE Completed");
    println!("FINAL_ASSISTANT_TEXT_DIAGNOSTIC {:?}", outcome.final_text);
    println!("FINAL_ASSISTANT_TEXT_AUTHORITY diagnostic_only");
    println!("CLEANUP_STATE codex_app_server=reaped git_children=absent provider_children=absent");
    fixture.remove()?;
    println!("TEMP_REPOSITORY_CLEANUP removed=true");
    Ok(())
}

fn verify_composition(
    composition: &EffectiveProfileComposition,
) -> Result<BTreeMap<String, String>, String> {
    let effective = composition.effective_profile();
    let definitions = composition.registry().definitions();
    if effective.profile_id != PROFILE_ID
        || !effective.providers.is_empty()
        || definitions.len() != TOOLS.len()
        || effective.capabilities.len() != TOOLS.len()
    {
        return Err("effective profile was not the intended closed repository toolkit".to_owned());
    }
    let mut aliases = BTreeMap::new();
    for (index, definition) in definitions.iter().enumerate() {
        let name = definition.name.as_str();
        if !TOOLS.contains(&name)
            || definition.permission != PermissionLevel::Execute
            || aliases
                .insert(name.to_owned(), format!("rah_tool_{index}"))
                .is_some()
        {
            return Err("fresh registry included unexpected or duplicate authority".to_owned());
        }
    }
    if !aliases.contains_key(REPOSITORY_CREATE_FILE_TOOL_NAME)
        || effective.capabilities.iter().any(|capability| {
            !TOOLS.contains(&capability.capability_id.as_str())
                || !capability.enabled
                || !capability.registered
                || capability.permission != PermissionLevel::Execute
                || capability.resources != [GIT_RESOURCE_ID, REPOSITORY_RESOURCE_ID]
                || capability.validation != "validated"
        })
    {
        return Err("effective inventory did not preserve closed Execute authority".to_owned());
    }
    Ok(aliases)
}

async fn run_turn(runtime: &CodexRuntime, expected_create: &Value) -> Result<Outcome, String> {
    let handle = runtime
        .start(AgentRequest {
            request_id: RequestId::new(),
            input: AgentInput {
                messages: vec![Message {
                    role: MessageRole::User,
                    content: prompt(expected_create)?,
                }],
            },
            options: AgentOptions::default(),
        })
        .await
        .map_err(|error| format!("failed to start Codex turn: {error}"))?;
    let mut events = handle.into_events();
    let mut outcome = Outcome::default();
    tokio::time::timeout(TIMEOUT, async {
        while let Some(event) = events.next().await {
            outcome.sequence.push(event_name(&event).to_owned());
            match event {
                AgentEvent::ToolRequested { tool_call, .. } => {
                    let name = tool_call.name.as_str().to_owned();
                    if !TOOLS.contains(&name.as_str())
                        || outcome
                            .calls
                            .insert(tool_call.id.clone(), name.clone())
                            .is_some()
                    {
                        return Err(
                            "unexpected tool, duplicate call ID, or alternate mutation path"
                                .to_owned(),
                        );
                    }
                    if name == REPOSITORY_CREATE_FILE_TOOL_NAME
                        && tool_call.input.0 != *expected_create
                    {
                        return Err("repo.create-file arguments were not exact".to_owned());
                    }
                    *outcome.requested.entry(name).or_default() += 1;
                }
                AgentEvent::ToolStarted { tool_call_id, .. } => {
                    outcome.bump(&tool_call_id, Count::Started)?
                }
                AgentEvent::ToolFinished {
                    tool_call_id,
                    output,
                    ..
                } => {
                    outcome.bump(&tool_call_id, Count::Finished)?;
                    let name = outcome
                        .calls
                        .get(&tool_call_id)
                        .ok_or_else(|| "finished output lacked requested call".to_owned())?
                        .clone();
                    if output.is_error {
                        return Err(format!("{name} returned a public error: {output:?}"));
                    }
                    outcome.outputs.insert(name, output);
                }
                AgentEvent::Completed { output, .. } => {
                    outcome.final_text = Some(output.message.content)
                }
                AgentEvent::ApprovalRequired { .. } => {
                    return Err("restricted runtime requested approval".to_owned());
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
    outcome
        .final_text
        .as_ref()
        .ok_or_else(|| "missing Completed output".to_owned())?;
    live_gate_contract::require_completed(&outcome.sequence)?;
    Ok(outcome)
}

fn prompt(request: &Value) -> Result<String, String> {
    Ok(format!(
        "Use only the available RAH repository tools. Create exactly one new UTF-8 file by calling repo.create-file exactly once with exactly this JSON input: {}. Then call repo.file-info for {TARGET} and repo.status to verify the created file is regular, exact, and untracked. Do not call any other tool, do not create directories, and respond compactly after the observers.",
        serde_json::to_string(request).map_err(display_error)?
    ))
}

fn audit_observers(outcome: &Outcome, fixture: &Fixture) -> Result<(), String> {
    for name in [REPOSITORY_FILE_INFO_TOOL_NAME, REPOSITORY_STATUS_TOOL_NAME] {
        if outcome.count(name, Count::Requested) == 0
            || outcome.count(name, Count::Started) == 0
            || outcome.count(name, Count::Finished) == 0
        {
            return Err(format!(
                "required observer {name} lacked a complete lifecycle"
            ));
        }
    }
    let values = outcome.values()?;
    let file_info = values
        .get(REPOSITORY_FILE_INFO_TOOL_NAME)
        .ok_or_else(|| "missing repo.file-info output".to_owned())?;
    if file_info["path"]["value"] != TARGET
        || file_info["index"]["tracked"] != false
        || file_info["worktree"]["present"] != true
        || file_info["worktree"]["kind"] != "regular_file"
        || file_info["worktree"]["size_bytes"] != CONTENT.len()
        || file_info["content"]["sha256"] != fixture.hash
        || file_info["content"]["byte_length"] != CONTENT.len()
    {
        return Err(format!(
            "repo.file-info did not prove the created target: {file_info}"
        ));
    }
    let entries = values
        .get(REPOSITORY_STATUS_TOOL_NAME)
        .ok_or_else(|| "missing repo.status output".to_owned())?["entries"]
        .as_array()
        .ok_or_else(|| "repo.status entries missing".to_owned())?;
    if entries.len() != 1
        || !entries.iter().any(|entry| {
            entry["path"]["value"] == TARGET
                && entry["index_state"] == "untracked"
                && entry["worktree_state"] == "untracked"
        })
    {
        return Err("repo.status did not prove target-only untracked state".to_owned());
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum Count {
    Requested,
    Started,
    Finished,
}
#[derive(Default)]
struct Outcome {
    requested: HashMap<String, usize>,
    started: HashMap<String, usize>,
    finished: HashMap<String, usize>,
    calls: HashMap<ToolCallId, String>,
    outputs: HashMap<String, ToolOutput>,
    sequence: Vec<String>,
    final_text: Option<String>,
}
impl Outcome {
    fn bump(&mut self, id: &ToolCallId, count: Count) -> Result<(), String> {
        let name = self
            .calls
            .get(id)
            .ok_or_else(|| "lifecycle event lacked requested call".to_owned())?
            .clone();
        *match count {
            Count::Requested => &mut self.requested,
            Count::Started => &mut self.started,
            Count::Finished => &mut self.finished,
        }
        .entry(name)
        .or_default() += 1;
        Ok(())
    }
    fn count(&self, name: &str, count: Count) -> usize {
        *match count {
            Count::Requested => &self.requested,
            Count::Started => &self.started,
            Count::Finished => &self.finished,
        }
        .get(name)
        .unwrap_or(&0)
    }
    fn summary(&self) -> String {
        TOOLS
            .into_iter()
            .map(|name| {
                format!(
                    "{name}:requested={},started={},finished={}",
                    self.count(name, Count::Requested),
                    self.count(name, Count::Started),
                    self.count(name, Count::Finished)
                )
            })
            .collect::<Vec<_>>()
            .join(";")
    }
    fn values(&self) -> Result<HashMap<&str, &Value>, String> {
        let mut values = HashMap::new();
        for (name, output) in &self.outputs {
            let [ToolContent::Json(value)] = output.content.as_slice() else {
                return Err("tool output was not one JSON value".to_owned());
            };
            values.insert(name.as_str(), value);
        }
        Ok(values)
    }
}

struct Fixture {
    directory: PathBuf,
    root: PathBuf,
    target: PathBuf,
    before_head: Vec<u8>,
    before_refs: Vec<u8>,
    before_index: Vec<u8>,
    before_sentinel: Vec<u8>,
    hash: String,
}
impl Fixture {
    fn create(git: &Path) -> Result<Self, String> {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(display_error)?
            .as_nanos();
        let directory = env::temp_dir().join(format!(
            ".rah-live-create-file-{}-{stamp}-{}",
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
        fs::create_dir_all(root.join("src/generated")).map_err(display_error)?;
        fs::write(root.join(SENTINEL), "RAH_CREATE_FILE_SENTINEL\n").map_err(display_error)?;
        fs::write(root.join("src/.gitkeep"), "\n").map_err(display_error)?;
        git_ok(git, &root, &["add", "--", SENTINEL, "src/.gitkeep"])?;
        git_ok(
            git,
            &root,
            &["commit", "--quiet", "-m", "live create-file fixture"],
        )?;
        let root = fs::canonicalize(root).map_err(display_error)?;
        let target = root.join(TARGET);
        if target.exists()
            || git_output(git, &root, &["ls-files", "--error-unmatch", "--", TARGET]).is_ok()
            || git_status(git, &root, &["check-ignore", "--quiet", "--", TARGET])? == 0
        {
            return Err("target was not absent and non-ignored before the live turn".to_owned());
        }
        let fixture = Self {
            before_head: git_output(git, &root, &["rev-parse", "HEAD"])?,
            before_refs: git_output(git, &root, &["show-ref", "--head"])?,
            before_index: fs::read(root.join(".git/index")).map_err(display_error)?,
            before_sentinel: fs::read(root.join(SENTINEL)).map_err(display_error)?,
            hash: format!("{:x}", Sha256::digest(CONTENT.as_bytes())),
            directory,
            root,
            target,
        };
        if !git_output(git, &fixture.root, &["status", "--porcelain=v1"])?.is_empty() {
            return Err("fixture baseline must be clean".to_owned());
        }
        Ok(fixture)
    }
    fn write_profile(&self, git: &Path) -> Result<PathBuf, String> {
        let path = self.directory.join("trusted-profile.json");
        let capabilities = TOOLS.into_iter().map(|name| json!({"name":name,"enabled":true,"permission":"execute","executable":GIT_RESOURCE_ID,"repository":REPOSITORY_RESOURCE_ID})).collect::<Vec<_>>();
        let document = json!({"profile_version":1,"profile_id":PROFILE_ID,"resources":{"executables":{GIT_RESOURCE_ID:{"path":git,"kind":"native"}},"repositories":{REPOSITORY_RESOURCE_ID:{"path":&self.root}}},"capabilities":capabilities});
        fs::write(&path, serde_json::to_vec(&document).map_err(display_error)?)
            .map_err(display_error)?;
        Ok(path)
    }
    fn request(&self) -> Value {
        json!({"path":TARGET,"content":CONTENT})
    }
    fn assert_after(&self, git: &Path) -> Result<(), String> {
        let metadata = fs::symlink_metadata(&self.target).map_err(display_error)?;
        if !metadata.is_file() || metadata.file_type().is_symlink() || is_reparse(&metadata) {
            return Err("target was not a regular non-reparse file".to_owned());
        }
        if fs::read(&self.target).map_err(display_error)? != CONTENT.as_bytes() {
            return Err("target content changed unexpectedly".to_owned());
        }
        if format!(
            "{:x}",
            Sha256::digest(fs::read(&self.target).map_err(display_error)?)
        ) != self.hash
        {
            return Err("target SHA-256 changed unexpectedly".to_owned());
        }
        if fs::read(self.root.join(SENTINEL)).map_err(display_error)? != self.before_sentinel {
            return Err("sentinel changed unexpectedly".to_owned());
        }
        if fs::read(self.root.join(".git/index")).map_err(display_error)? != self.before_index {
            return Err("raw Git index changed unexpectedly".to_owned());
        }
        if git_output(git, &self.root, &["rev-parse", "HEAD"])? != self.before_head {
            return Err("HEAD changed unexpectedly".to_owned());
        }
        if git_output(git, &self.root, &["show-ref", "--head"])? != self.before_refs {
            return Err("refs changed unexpectedly".to_owned());
        }
        let status = git_output(
            git,
            &self.root,
            &["status", "--porcelain=v1", "--untracked-files=all"],
        )?;
        if status != format!("?? {TARGET}\n").as_bytes() {
            return Err(format!(
                "worktree status was not target-only untracked: {:?}",
                String::from_utf8_lossy(&status)
            ));
        }
        Ok(())
    }
    fn remove(&mut self) -> Result<(), String> {
        fs::remove_dir_all(&self.directory).map_err(display_error)?;
        (!self.directory.exists())
            .then_some(())
            .ok_or_else(|| "temporary fixture remained after cleanup".to_owned())
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.directory);
    }
}
#[cfg(windows)]
fn is_reparse(metadata: &fs::Metadata) -> bool {
    metadata.file_attributes() & 0x400 != 0
}
#[cfg(not(windows))]
fn is_reparse(_: &fs::Metadata) -> bool {
    false
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
        return Err("native Git executable is required".to_owned());
    }
    fs::canonicalize(
        String::from_utf8(output.stdout)
            .map_err(display_error)?
            .lines()
            .next()
            .ok_or_else(|| "native Git executable was not reported".to_owned())?,
    )
    .map_err(display_error)
}
fn git_ok(git: &Path, root: &Path, args: &[&str]) -> Result<(), String> {
    if git_status(git, root, args)? == 0 {
        Ok(())
    } else {
        Err(format!("fixture Git command {args:?} failed"))
    }
}
fn git_status(git: &Path, root: &Path, args: &[&str]) -> Result<i32, String> {
    Command::new(git)
        .args(args)
        .current_dir(root)
        .status()
        .map(|status| status.code().unwrap_or(-1))
        .map_err(display_error)
}
fn git_output(git: &Path, root: &Path, args: &[&str]) -> Result<Vec<u8>, String> {
    let output = Command::new(git)
        .args(args)
        .current_dir(root)
        .output()
        .map_err(display_error)?;
    output
        .status
        .success()
        .then_some(output.stdout)
        .ok_or_else(|| {
            format!(
                "fixture Git command {args:?} failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            )
        })
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
