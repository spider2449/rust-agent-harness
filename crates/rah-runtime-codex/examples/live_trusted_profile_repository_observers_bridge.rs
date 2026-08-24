//! Opt-in Windows live validation for the trusted repository observer toolkit.
//!
//! This example is deliberately excluded from deterministic tests. It uses the
//! pinned native Codex executable, the real trusted-profile composer, and a
//! disposable mixed-state Git repository. It never targets the RAH checkout.

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
use rah_tools::{
    REPOSITORY_DIFF_STAGED_TOOL_NAME, REPOSITORY_DIFF_TOOL_NAME, REPOSITORY_FILE_INFO_TOOL_NAME,
    REPOSITORY_STATUS_TOOL_NAME, TrustedStaticProfile,
};
use serde_json::{Value, json};

const PROFILE_ID: &str = "live-repository-observers";
const GIT_RESOURCE_ID: &str = "live-git";
const REPOSITORY_RESOURCE_ID: &str = "live-repository";
const TRACKED: &str = "tracked.txt";
const STAGED: &str = "staged.txt";
const UNTRACKED: &str = "untracked.txt";
const FINAL_MARKER: &str = "RAH_REPOSITORY_OBSERVERS_LIVE_OK";
const TERMINAL_TIMEOUT: Duration = Duration::from_secs(120);
const OBSERVERS: [&str; 4] = [
    REPOSITORY_FILE_INFO_TOOL_NAME,
    REPOSITORY_STATUS_TOOL_NAME,
    REPOSITORY_DIFF_TOOL_NAME,
    REPOSITORY_DIFF_STAGED_TOOL_NAME,
];

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
            eprintln!("LIVE_REPOSITORY_OBSERVERS_FAIL Tokio runtime: {error}");
            return ExitCode::FAILURE;
        }
    };
    match runtime.block_on(run()) {
        Ok(()) => {
            println!("{FINAL_MARKER}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("LIVE_REPOSITORY_OBSERVERS_FAIL {error}");
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

    let turn = run_turn(&runtime).await;
    let shutdown = runtime
        .shutdown()
        .await
        .map_err(|error| format!("Codex app-server shutdown/reap failed: {error}"));
    drop(runtime);
    drop(registry);
    composition.shutdown().await;
    shutdown?;
    let outcome = turn?;
    fixture.assert_unchanged(&git)?;
    audit_outputs(&outcome, &fixture.root)?;

    println!("CODEX_VERSION {SUPPORTED_CODEX_VERSION}");
    println!("CODEX_EXECUTABLE_IDENTITY native_discovery_and_exact_version_verified=true");
    println!("PROFILE_SOURCE_VALIDATION succeeded");
    println!("EFFECTIVE_COMPOSITION succeeded trusted_source=true real_composer=true");
    for (name, alias) in &aliases {
        println!("PRIVATE_ALIAS_MAPPING {name} -> {alias}");
    }
    println!(
        "RESTRICTED_CODEX_CAPABILITIES shell=false file=false mcp=false process=false network_tools=false web=false image=false apps=false approvals=false"
    );
    println!("TOOL_COUNTS {}", outcome.count_summary());
    println!("FINAL_ASSISTANT_TEXT_DIAGNOSTIC {:?}", outcome.final_text);
    println!("FINAL_ASSISTANT_TEXT_AUTHORITY diagnostic_only");
    println!("RAH_EVENT_SEQUENCE {}", outcome.sequence.join(" -> "));
    println!("SEMANTIC_ASSERTIONS status=passed file_info=passed diff=passed diff_staged=passed");
    println!(
        "READ_ONLY_ASSERTIONS head=true refs=true index=true tracked=true staged=true untracked=true diffs=true"
    );
    println!("MODEL_VISIBLE_PRIVACY_AUDIT passed");
    println!(
        "CLEANUP_STATE codex_app_server=reaped git_children=absent mcp_child=absent plugin_child=absent"
    );
    fixture.remove()?;
    println!("TEMP_REPOSITORY_CLEANUP removed=true");
    Ok(())
}

fn verify_composition(
    composition: &EffectiveProfileComposition,
) -> Result<BTreeMap<String, String>, String> {
    let effective = composition.effective_profile();
    if effective.profile_id != PROFILE_ID
        || effective.capabilities.len() != OBSERVERS.len()
        || !effective.providers.is_empty()
    {
        return Err(
            "effective profile was not exactly the four-observer trusted profile".to_owned(),
        );
    }
    let definitions = composition.registry().definitions();
    if definitions.len() != OBSERVERS.len() {
        return Err("fresh effective registry did not contain exactly four observers".to_owned());
    }
    let mut aliases = BTreeMap::new();
    for (index, definition) in definitions.iter().enumerate() {
        let name = definition.name.as_str();
        if !OBSERVERS.contains(&name) || definition.permission != PermissionLevel::Execute {
            return Err(
                "effective registry contained an unexpected observer definition".to_owned(),
            );
        }
        let alias = format!("rah_tool_{index}");
        if aliases.insert(name.to_owned(), alias).is_some() {
            return Err("observer alias collision detected".to_owned());
        }
    }
    if aliases.len() != OBSERVERS.len()
        || effective.capabilities.iter().any(|capability| {
            !OBSERVERS.contains(&capability.capability_id.as_str())
                || !capability.enabled
                || !capability.registered
                || capability.permission != PermissionLevel::Execute
                || capability.resources != [GIT_RESOURCE_ID, REPOSITORY_RESOURCE_ID]
                || capability.validation != "validated"
        })
    {
        return Err("trusted effective inventory lost observer authority constraints".to_owned());
    }
    Ok(aliases)
}

async fn run_turn(runtime: &CodexRuntime) -> Result<TurnOutcome, String> {
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
    let mut outcome = TurnOutcome::default();
    tokio::time::timeout(TERMINAL_TIMEOUT, async {
        while let Some(event) = events.next().await {
            outcome.sequence.push(event_name(&event).to_owned());
            match event {
                AgentEvent::ToolRequested { tool_call, .. } => {
                    let name = tool_call.name.as_str().to_owned();
                    if !OBSERVERS.contains(&name.as_str())
                        || outcome
                            .call_names
                            .insert(tool_call.id.clone(), name.clone())
                            .is_some()
                    {
                        return Err(
                            "unexpected observer, repo.patch, or duplicate call ID requested"
                                .to_owned(),
                        );
                    }
                    let expected = if name == REPOSITORY_FILE_INFO_TOOL_NAME {
                        json!({"path": TRACKED})
                    } else {
                        json!({})
                    };
                    if tool_call.input.0 != expected {
                        return Err(format!("{name} received unexpected model input"));
                    }
                    *outcome.requested.entry(name).or_default() += 1;
                }
                AgentEvent::ToolStarted { tool_call_id, .. } => {
                    outcome.bump(&tool_call_id, CountKind::Started)?
                }
                AgentEvent::ToolFinished {
                    tool_call_id,
                    output,
                    ..
                } => {
                    outcome.bump(&tool_call_id, CountKind::Finished)?;
                    if output.is_error {
                        return Err("observer returned a public error".to_owned());
                    }
                    outcome.outputs.insert(tool_call_id, output);
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
    for name in OBSERVERS {
        live_gate_contract::require_exactly_once(
            name,
            *outcome.requested.get(name).unwrap_or(&0),
            *outcome.started.get(name).unwrap_or(&0),
            *outcome.finished.get(name).unwrap_or(&0),
        )?;
    }
    Ok(outcome)
}

fn prompt() -> &'static str {
    "Use only the available RAH observer tools. Invoke each exactly once, in this order: repo.status with {}, repo.file-info with {\"path\":\"tracked.txt\"}, repo.diff with {}, then repo.diff-staged with {}. Do not modify anything and do not invoke any other tool. After all four successful observations, respond compactly."
}

enum CountKind {
    Started,
    Finished,
}

#[derive(Default)]
struct TurnOutcome {
    requested: HashMap<String, usize>,
    started: HashMap<String, usize>,
    finished: HashMap<String, usize>,
    call_names: HashMap<ToolCallId, String>,
    outputs: HashMap<ToolCallId, ToolOutput>,
    sequence: Vec<String>,
    final_text: Option<String>,
}

impl TurnOutcome {
    fn bump(&mut self, id: &ToolCallId, kind: CountKind) -> Result<(), String> {
        let name = self
            .call_names
            .get(id)
            .ok_or_else(|| "lifecycle event lacked a requested call".to_owned())?
            .clone();
        let counts = match kind {
            CountKind::Started => &mut self.started,
            CountKind::Finished => &mut self.finished,
        };
        *counts.entry(name).or_default() += 1;
        Ok(())
    }
    fn count_summary(&self) -> String {
        OBSERVERS
            .into_iter()
            .map(|name| {
                format!(
                    "{name}:requested={},started={},finished={},invocations={}",
                    self.requested.get(name).unwrap_or(&0),
                    self.started.get(name).unwrap_or(&0),
                    self.finished.get(name).unwrap_or(&0),
                    self.started.get(name).unwrap_or(&0)
                )
            })
            .collect::<Vec<_>>()
            .join(";")
    }
}

fn audit_outputs(outcome: &TurnOutcome, root: &Path) -> Result<(), String> {
    let mut values = HashMap::new();
    for (id, output) in &outcome.outputs {
        let name = outcome
            .call_names
            .get(id)
            .ok_or_else(|| "finished output lacked requested call".to_owned())?;
        let [ToolContent::Json(value)] = output.content.as_slice() else {
            return Err("observer output was not one JSON value".to_owned());
        };
        values.insert(name.as_str(), value);
    }
    let status = values
        .get(REPOSITORY_STATUS_TOOL_NAME)
        .ok_or_else(|| "missing status output".to_owned())?;
    let entries = status["entries"]
        .as_array()
        .ok_or_else(|| "status entries missing".to_owned())?;
    for (path, index, worktree) in [
        (TRACKED, "unmodified", "modified"),
        (STAGED, "modified", "unmodified"),
        (UNTRACKED, "untracked", "untracked"),
    ] {
        if !entries.iter().any(|entry| {
            entry["path"]["value"] == path
                && entry["index_state"] == index
                && entry["worktree_state"] == worktree
        }) {
            return Err(format!("repo.status did not report expected {path} state"));
        }
    }
    let file_info = values
        .get(REPOSITORY_FILE_INFO_TOOL_NAME)
        .ok_or_else(|| "missing file-info output".to_owned())?;
    if file_info["path"]["value"] != TRACKED
        || file_info["index"]["tracked"] != true
        || file_info["worktree_modified_vs_index"] != true
    {
        return Err("repo.file-info did not report tracked worktree modification".to_owned());
    }
    assert_diff(
        values
            .get(REPOSITORY_DIFF_TOOL_NAME)
            .ok_or_else(|| "missing diff output".to_owned())?,
        "worktree_to_index",
        "index",
        TRACKED,
        STAGED,
    )?;
    assert_diff(
        values
            .get(REPOSITORY_DIFF_STAGED_TOOL_NAME)
            .ok_or_else(|| "missing staged diff output".to_owned())?,
        "index_to_head",
        "head",
        STAGED,
        TRACKED,
    )?;
    let visible = serde_json::to_string(&outcome.outputs).map_err(display_error)?;
    let root = root.to_string_lossy();
    for private in [
        root.as_ref(),
        ".rah-live-repository-observers-",
        "git.exe",
        "HostExecutionPolicy",
        "RepositoryObserver",
        "trusted-profile",
    ] {
        if visible.contains(private) {
            return Err(
                "model-visible observer output exposed private host information".to_owned(),
            );
        }
    }
    Ok(())
}

fn assert_diff(
    value: &Value,
    comparison: &str,
    base: &str,
    included: &str,
    excluded: &str,
) -> Result<(), String> {
    let files = value["files"]
        .as_array()
        .ok_or_else(|| "diff files missing".to_owned())?;
    if value["comparison"] != comparison
        || value["base"] != base
        || !files
            .iter()
            .any(|file| file["new_path"]["value"] == included)
        || files
            .iter()
            .any(|file| file["new_path"]["value"] == excluded)
    {
        return Err(format!(
            "{comparison} did not retain its fixed semantic boundary"
        ));
    }
    Ok(())
}

struct LiveFixture {
    directory: PathBuf,
    root: PathBuf,
    before_head: Vec<u8>,
    before_refs: Vec<u8>,
    before_index: Vec<u8>,
    before_tracked: Vec<u8>,
    before_staged: Vec<u8>,
    before_untracked: Vec<u8>,
    before_staged_diff: Vec<u8>,
    before_unstaged_diff: Vec<u8>,
}

impl LiveFixture {
    fn create(git: &Path) -> Result<Self, String> {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(display_error)?
            .as_nanos();
        let directory = env::temp_dir().join(format!(
            ".rah-live-repository-observers-{}-{stamp}-{}",
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
        fs::write(root.join(TRACKED), "TRACKED_BASELINE\n").map_err(display_error)?;
        fs::write(root.join(STAGED), "STAGED_BASELINE\n").map_err(display_error)?;
        git_ok(git, &root, &["add", "--", TRACKED, STAGED])?;
        git_ok(
            git,
            &root,
            &["commit", "--quiet", "-m", "live observer fixture"],
        )?;
        fs::write(root.join(TRACKED), "TRACKED_UNSTAGED_CHANGE\n").map_err(display_error)?;
        fs::write(root.join(STAGED), "STAGED_INDEX_CHANGE\n").map_err(display_error)?;
        git_ok(git, &root, &["add", "--", STAGED])?;
        fs::write(root.join(UNTRACKED), "UNTRACKED_FILE\n").map_err(display_error)?;
        let root = fs::canonicalize(root).map_err(display_error)?;
        Ok(Self {
            before_head: git_output(git, &root, &["rev-parse", "HEAD"])?,
            before_refs: git_output(git, &root, &["show-ref", "--head"])?,
            before_index: fs::read(root.join(".git/index")).map_err(display_error)?,
            before_tracked: fs::read(root.join(TRACKED)).map_err(display_error)?,
            before_staged: fs::read(root.join(STAGED)).map_err(display_error)?,
            before_untracked: fs::read(root.join(UNTRACKED)).map_err(display_error)?,
            before_staged_diff: git_output(git, &root, &["diff", "--cached", "--no-ext-diff"])?,
            before_unstaged_diff: git_output(git, &root, &["diff", "--no-ext-diff"])?,
            directory,
            root,
        })
    }
    fn write_profile(&self, git: &Path) -> Result<PathBuf, String> {
        let path = self.directory.join("trusted-profile.json");
        let capabilities = OBSERVERS.into_iter().map(|name| json!({"name":name,"enabled":true,"permission":"execute","executable":GIT_RESOURCE_ID,"repository":REPOSITORY_RESOURCE_ID})).collect::<Vec<_>>();
        fs::write(&path, serde_json::to_vec(&json!({"profile_version":1,"profile_id":PROFILE_ID,"resources":{"executables":{GIT_RESOURCE_ID:{"path":git,"kind":"native"}},"repositories":{REPOSITORY_RESOURCE_ID:{"path":&self.root}}},"capabilities":capabilities})).map_err(display_error)?).map_err(display_error)?;
        Ok(path)
    }
    fn assert_unchanged(&self, git: &Path) -> Result<(), String> {
        let same = git_output(git, &self.root, &["rev-parse", "HEAD"])? == self.before_head
            && git_output(git, &self.root, &["show-ref", "--head"])? == self.before_refs
            && fs::read(self.root.join(".git/index")).map_err(display_error)? == self.before_index
            && fs::read(self.root.join(TRACKED)).map_err(display_error)? == self.before_tracked
            && fs::read(self.root.join(STAGED)).map_err(display_error)? == self.before_staged
            && fs::read(self.root.join(UNTRACKED)).map_err(display_error)? == self.before_untracked
            && git_output(git, &self.root, &["diff", "--cached", "--no-ext-diff"])?
                == self.before_staged_diff
            && git_output(git, &self.root, &["diff", "--no-ext-diff"])?
                == self.before_unstaged_diff;
        same.then_some(())
            .ok_or_else(|| "observer execution mutated the fixture repository".to_owned())
    }
    fn remove(&mut self) -> Result<(), String> {
        fs::remove_dir_all(&self.directory).map_err(display_error)?;
        (!self.directory.exists())
            .then_some(())
            .ok_or_else(|| "temporary fixture remained after cleanup".to_owned())
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
    output
        .status
        .success()
        .then_some(())
        .ok_or_else(|| "native Git executable is required for the live fixture".to_owned())?;
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
    let output = Command::new(git)
        .args(args)
        .current_dir(root)
        .output()
        .map_err(display_error)?;
    output.status.success().then_some(()).ok_or_else(|| {
        format!(
            "fixture Git command {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )
    })
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
