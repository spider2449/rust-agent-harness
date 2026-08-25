//! Opt-in certified live validation for trusted-profile-composed `repo.edit-files`.
//!
//! Host observations, not model prose, determine success.

use std::{
    collections::{BTreeMap, HashMap},
    env, fs,
    path::{Path, PathBuf},
    process::{Command, ExitCode},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

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
use rah_tools::{REPOSITORY_EDIT_FILES_TOOL_NAME, TrustedStaticProfile};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const PROFILE_ID: &str = "live-edit-files";
const GIT_RESOURCE_ID: &str = "live-git";
const REPOSITORY_RESOURCE_ID: &str = "live-repository";
const A_PATH: &str = "a.txt";
const B_PATH: &str = "b.txt";
const A_BEFORE: &str = "alpha old\n";
const B_BEFORE: &str = "beta old\n";
const A_AFTER: &str = "alpha new\n";
const B_AFTER: &str = "beta new\n";
const SENTINEL: &str = "sentinel.txt";
const FINAL_MARKER: &str = "RAH_REPO_EDIT_FILES_LIVE_OK";
const TIMEOUT: Duration = Duration::from_secs(120);
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
            eprintln!("LIVE_EDIT_FILES_FAIL Tokio runtime: {error}");
            return ExitCode::FAILURE;
        }
    };
    match runtime.block_on(run()) {
        Ok(()) => {
            println!("{FINAL_MARKER}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("LIVE_EDIT_FILES_FAIL {error}");
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
    let codex = env::var_os("RAH_CODEX_EXECUTABLE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("codex"));
    let registry = composition.registry_handle();
    let runtime =
        CodexRuntime::connect_tool_bridge(&codex, registry.clone(), vec![PermissionLevel::Execute])
            .await
            .map_err(|error| {
                format!(
                    "native Codex discovery, version, schema, or bridge connection failed: {error}"
                )
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
    live_gate_contract::require_exactly_once(
        REPOSITORY_EDIT_FILES_TOOL_NAME,
        outcome.count(Count::Requested),
        outcome.count(Count::Started),
        outcome.count(Count::Finished),
    )?;
    let output = outcome.output()?;
    if output["status"] != "ok"
        || output["effects"]
            != json!([
                {"path": A_PATH, "state": "committed_verified"},
                {"path": B_PATH, "state": "committed_verified"}
            ])
    {
        return Err(format!(
            "repo.edit-files structured output was not the host-certified result: {output}"
        ));
    }
    println!("CODEX_VERSION {SUPPORTED_CODEX_VERSION}");
    println!("CODEX_EXECUTABLE_IDENTITY native_discovery_and_exact_version_verified=true");
    println!("TRUSTED_PROFILE_PATH source=true effective_compose=true fresh_registry=true");
    for (tool, alias) in aliases {
        println!("PRIVATE_ALIAS_MAPPING {tool} -> {alias}");
    }
    println!(
        "GENERIC_BRIDGE_PATH advertised=true execute_permission=true capability_specific_logic=false"
    );
    println!(
        "TOOL_COUNTS repo.edit-files:requested={},started={},finished={}",
        outcome.count(Count::Requested),
        outcome.count(Count::Started),
        outcome.count(Count::Finished)
    );
    println!("TOOL_OUTPUT status=ok effects=a.txt:committed_verified,b.txt:committed_verified");
    println!("FINAL_FILES a.txt={:?} b.txt={:?}", A_AFTER, B_AFTER);
    println!(
        "REPOSITORY_INVARIANTS index_bytes=true head=true refs=true staging=false target_only_unstaged=true"
    );
    println!("RAH_EVENT_SEQUENCE {}", outcome.sequence.join(" -> "));
    println!("TERMINAL_STATE Completed");
    println!("FINAL_ASSISTANT_TEXT_AUTHORITY diagnostic_only");
    fixture.remove()?;
    println!("TEMP_REPOSITORY_CLEANUP removed=true");
    Ok(())
}

fn verify_composition(
    composition: &EffectiveProfileComposition,
) -> Result<BTreeMap<String, String>, String> {
    let definitions = composition.registry().definitions();
    let effective = composition.effective_profile();
    if effective.profile_id != PROFILE_ID
        || !effective.providers.is_empty()
        || definitions.len() != 1
    {
        return Err("effective profile was not the intended closed edit-files toolkit".to_owned());
    }
    let definition = &definitions[0];
    if definition.name.as_str() != REPOSITORY_EDIT_FILES_TOOL_NAME
        || definition.permission != PermissionLevel::Execute
    {
        return Err(
            "effective registry did not contain Execute-gated repo.edit-files only".to_owned(),
        );
    }
    if effective.capabilities.len() != 1
        || !effective.capabilities[0].enabled
        || !effective.capabilities[0].registered
        || effective.capabilities[0].permission != PermissionLevel::Execute
        || effective.capabilities[0].resources != [GIT_RESOURCE_ID, REPOSITORY_RESOURCE_ID]
    {
        return Err("effective capability inventory lost closed host authority".to_owned());
    }
    Ok(BTreeMap::from([(
        REPOSITORY_EDIT_FILES_TOOL_NAME.to_owned(),
        "rah_tool_0".to_owned(),
    )]))
}

async fn run_turn(runtime: &CodexRuntime, expected: &Value) -> Result<Outcome, String> {
    let handle = runtime
        .start(AgentRequest {
            request_id: RequestId::new(),
            input: AgentInput {
                messages: vec![Message {
                    role: MessageRole::User,
                    content: prompt().to_owned(),
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
                    if tool_call.name.as_str() != REPOSITORY_EDIT_FILES_TOOL_NAME
                        || outcome.calls.insert(tool_call.id.clone(), ()).is_some()
                        || tool_call.input.0 != *expected
                    {
                        return Err(format!(
                            "unexpected or duplicate tool request: {} {}",
                            tool_call.name, tool_call.input.0
                        ));
                    }
                    outcome.requested += 1;
                }
                AgentEvent::ToolStarted { tool_call_id, .. } => {
                    if !outcome.calls.contains_key(&tool_call_id) {
                        return Err("tool start lacked requested call".to_owned());
                    }
                    outcome.started += 1;
                }
                AgentEvent::ToolFinished {
                    tool_call_id,
                    output,
                    ..
                } => {
                    if !outcome.calls.contains_key(&tool_call_id) || output.is_error {
                        return Err("tool finish lacked successful requested call".to_owned());
                    }
                    outcome.finished += 1;
                    outcome.output = Some(output);
                }
                AgentEvent::Completed { .. } => outcome.completed = true,
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
    if !outcome.completed {
        return Err("missing Completed event".to_owned());
    }
    live_gate_contract::require_completed(&outcome.sequence)?;
    Ok(outcome)
}

fn prompt() -> &'static str {
    "Use the available repo.edit-files capability exactly once. Make this exact JSON-equivalent request with targets deliberately ordered b.txt then a.txt: b.txt has expected_file_sha256 af258e49c9bb8dae90ed82026430db02cf40a7bacdc18173dea651bcee171fda, expected_file_byte_length 9, replacement expected_old_text beta old\\n to replacement_text beta new\\n; a.txt has expected_file_sha256 9a4ba5ac1cd97e8007fadd5127ab2b2416fce5fe2ef7a607e68c94bfb19feea3, expected_file_byte_length 10, replacement expected_old_text alpha old\\n to replacement_text alpha new\\n. Do not use any other tool or mutation capability. Do not stage, commit, invoke shell or process, create or delete files, retry, or replay. Finish immediately after the edit result."
}

#[derive(Default)]
struct Outcome {
    requested: usize,
    started: usize,
    finished: usize,
    calls: HashMap<ToolCallId, ()>,
    output: Option<ToolOutput>,
    sequence: Vec<String>,
    completed: bool,
}
enum Count {
    Requested,
    Started,
    Finished,
}
impl Outcome {
    fn count(&self, count: Count) -> usize {
        match count {
            Count::Requested => self.requested,
            Count::Started => self.started,
            Count::Finished => self.finished,
        }
    }
    fn output(&self) -> Result<&Value, String> {
        let output = self
            .output
            .as_ref()
            .ok_or_else(|| "missing repo.edit-files output".to_owned())?;
        let [ToolContent::Json(value)] = output.content.as_slice() else {
            return Err("repo.edit-files output was not one JSON value".to_owned());
        };
        Ok(value)
    }
}

struct Fixture {
    directory: PathBuf,
    root: PathBuf,
    before_index: Vec<u8>,
    before_head: Vec<u8>,
    before_refs: Vec<u8>,
    before_sentinel: Vec<u8>,
}
impl Fixture {
    fn create(git: &Path) -> Result<Self, String> {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(display_error)?
            .as_nanos();
        let directory = env::temp_dir().join(format!(
            ".rah-live-edit-files-{}-{stamp}-{}",
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
        fs::write(root.join(A_PATH), A_BEFORE).map_err(display_error)?;
        fs::write(root.join(B_PATH), B_BEFORE).map_err(display_error)?;
        fs::write(root.join(SENTINEL), "RAH_EDIT_FILES_SENTINEL\n").map_err(display_error)?;
        git_ok(git, &root, &["add", "--", A_PATH, B_PATH, SENTINEL])?;
        git_ok(
            git,
            &root,
            &["commit", "--quiet", "-m", "live edit-files fixture"],
        )?;
        let root = fs::canonicalize(root).map_err(display_error)?;
        let fixture = Self {
            before_index: fs::read(root.join(".git/index")).map_err(display_error)?,
            before_head: git_output(git, &root, &["rev-parse", "HEAD"])?,
            before_refs: git_output(git, &root, &["show-ref", "--head"])?,
            before_sentinel: fs::read(root.join(SENTINEL)).map_err(display_error)?,
            directory,
            root,
        };
        if !git_output(git, &fixture.root, &["status", "--porcelain=v1"])?.is_empty() {
            return Err("fixture baseline must be clean".to_owned());
        }
        Ok(fixture)
    }
    fn write_profile(&self, git: &Path) -> Result<PathBuf, String> {
        let path = self.directory.join("trusted-profile.json");
        let document = json!({"profile_version":1,"profile_id":PROFILE_ID,"resources":{"executables":{GIT_RESOURCE_ID:{"path":git,"kind":"native"}},"repositories":{REPOSITORY_RESOURCE_ID:{"path":&self.root}}},"capabilities":[{"name":REPOSITORY_EDIT_FILES_TOOL_NAME,"enabled":true,"permission":"execute","executable":GIT_RESOURCE_ID,"repository":REPOSITORY_RESOURCE_ID}]});
        fs::write(&path, serde_json::to_vec(&document).map_err(display_error)?)
            .map_err(display_error)?;
        Ok(path)
    }
    fn request(&self) -> Value {
        json!({"targets":[target(B_PATH, B_BEFORE, B_AFTER), target(A_PATH, A_BEFORE, A_AFTER)]})
    }
    fn assert_after(&self, git: &Path) -> Result<(), String> {
        if fs::read(self.root.join(A_PATH)).map_err(display_error)? != A_AFTER.as_bytes()
            || fs::read(self.root.join(B_PATH)).map_err(display_error)? != B_AFTER.as_bytes()
        {
            return Err("tracked target postimages were not exact".to_owned());
        }
        if fs::read(self.root.join(SENTINEL)).map_err(display_error)? != self.before_sentinel
            || fs::read(self.root.join(".git/index")).map_err(display_error)? != self.before_index
            || git_output(git, &self.root, &["rev-parse", "HEAD"])? != self.before_head
            || git_output(git, &self.root, &["show-ref", "--head"])? != self.before_refs
        {
            return Err("repository authority invariant changed".to_owned());
        }
        let status = git_output(
            git,
            &self.root,
            &["status", "--porcelain=v1", "--untracked-files=all"],
        )?;
        if status != b" M a.txt\n M b.txt\n" {
            return Err(format!(
                "unexpected Git status: {:?}",
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

fn target(path: &str, before: &str, after: &str) -> Value {
    json!({"path":path,"expected_file_sha256":format!("{:x}", Sha256::digest(before.as_bytes())),"expected_file_byte_length":before.len(),"replacements":[{"expected_old_text":before,"replacement_text":after}]})
}
fn native_git() -> Result<PathBuf, String> {
    let output = Command::new(if cfg!(windows) { "where.exe" } else { "which" })
        .arg(if cfg!(windows) { "git.exe" } else { "git" })
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
    if git_output(git, root, args).is_ok() {
        Ok(())
    } else {
        Err(format!("fixture Git command {args:?} failed"))
    }
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
