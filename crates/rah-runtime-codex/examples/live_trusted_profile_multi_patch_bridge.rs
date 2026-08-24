//! Opt-in Windows live validation of one trusted multi-replacement `repo.patch` call.
//!
//! The example exercises the production trusted-profile composer and Codex tool bridge.
//! It is intentionally excluded from deterministic tests and requires codex-cli 0.149.0.

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
    REPOSITORY_STATUS_TOOL_NAME, REPOSITORY_WORKTREE_PATCH_TOOL_NAME, TrustedStaticProfile,
    live_test_replacement_attempts, reset_live_test_replacement_attempts,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const PROFILE_ID: &str = "live-multi-patch";
const GIT_RESOURCE_ID: &str = "live-git";
const REPOSITORY_RESOURCE_ID: &str = "live-repository";
const TARGET: &str = "target.txt";
const UNRELATED: &str = "unrelated.txt";
const PREIMAGE: &str = "alpha = 1\nbeta = 2\ngamma = 3\nsentinel = unchanged\n";
const POSTIMAGE: &str = "alpha = 10\nbeta = 20\ngamma = 30\nsentinel = unchanged\n";
const FINAL_MARKER: &str = "RAH_MULTI_PATCH_LIVE_OK";
const TERMINAL_TIMEOUT: Duration = Duration::from_secs(120);
const TOOLS: [&str; 5] = [
    REPOSITORY_DIFF_TOOL_NAME,
    REPOSITORY_DIFF_STAGED_TOOL_NAME,
    REPOSITORY_FILE_INFO_TOOL_NAME,
    REPOSITORY_WORKTREE_PATCH_TOOL_NAME,
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
            eprintln!("LIVE_MULTI_PATCH_FAIL Tokio runtime: {error}");
            return ExitCode::FAILURE;
        }
    };
    match runtime.block_on(run()) {
        Ok(()) => {
            println!("{FINAL_MARKER}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("LIVE_MULTI_PATCH_FAIL {error}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), String> {
    let git = native_git()?;
    let mut fixture = LiveFixture::create(&git)?;
    let profile = TrustedStaticProfile::load(fixture.write_profile(&git)?)
        .map_err(|error| format!("trusted profile source validation failed: {error}"))?;
    let composition = compose(profile)
        .await
        .map_err(|error| format!("effective composition failed: {error}"))?;
    let aliases = verify_composition(&composition)?;
    reset_live_test_replacement_attempts();
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
    let turn = run_turn(&runtime, &fixture.expected_request()).await;
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
    if live_test_replacement_attempts() != 1 {
        return Err(format!(
            "repo.patch exactly-once proof failed: {}",
            outcome.count_summary()
        ));
    }
    live_gate_contract::require_exactly_once(
        REPOSITORY_WORKTREE_PATCH_TOOL_NAME,
        outcome.count(REPOSITORY_WORKTREE_PATCH_TOOL_NAME, Count::Requested),
        outcome.count(REPOSITORY_WORKTREE_PATCH_TOOL_NAME, Count::Started),
        outcome.count(REPOSITORY_WORKTREE_PATCH_TOOL_NAME, Count::Finished),
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
        "TOOL_COUNTS {} native_replacement_attempts={}",
        outcome.count_summary(),
        live_test_replacement_attempts()
    );
    println!("FINAL_ASSISTANT_TEXT_DIAGNOSTIC {:?}", outcome.final_text);
    println!("FINAL_ASSISTANT_TEXT_AUTHORITY diagnostic_only");
    println!("RAH_EVENT_SEQUENCE {}", outcome.sequence.join(" -> "));
    println!(
        "MUTATION_ARGUMENTS canonical=repo.patch form=replacements count=3 legacy_fields=false preimage_sha_length=true values=alpha,beta,gamma"
    );
    println!("OBSERVER_ASSERTIONS file_info=passed status=passed diff=passed diff_staged=passed");
    println!(
        "REPOSITORY_INVARIANTS target_only=true index_bytes=true head=true refs=true unrelated_tracked=true staged_diff_empty=true"
    );
    println!("TERMINAL_STATE Completed");
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
    let definitions = composition.registry().definitions();
    if effective.profile_id != PROFILE_ID
        || !effective.providers.is_empty()
        || definitions.len() != TOOLS.len()
    {
        return Err("effective profile was not the intended trusted repository toolkit".to_owned());
    }
    let mut aliases = BTreeMap::new();
    for (index, definition) in definitions.iter().enumerate() {
        let name = definition.name.as_str();
        if !TOOLS.contains(&name) || definition.permission != PermissionLevel::Execute {
            return Err("effective registry contains unexpected authority".to_owned());
        }
        aliases.insert(name.to_owned(), format!("rah_tool_{index}"));
    }
    if effective.capabilities.iter().any(|capability| {
        !TOOLS.contains(&capability.capability_id.as_str())
            || !capability.enabled
            || !capability.registered
            || capability.permission != PermissionLevel::Execute
            || capability.resources != [GIT_RESOURCE_ID, REPOSITORY_RESOURCE_ID]
            || capability.validation != "validated"
    }) {
        return Err(
            "trusted effective inventory lost Execute-only authority constraints".to_owned(),
        );
    }
    Ok(aliases)
}

async fn run_turn(runtime: &CodexRuntime, expected_patch: &Value) -> Result<TurnOutcome, String> {
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
                    if !TOOLS.contains(&name.as_str())
                        || outcome
                            .calls
                            .insert(tool_call.id.clone(), name.clone())
                            .is_some()
                    {
                        return Err("unexpected tool or duplicate tool call ID".to_owned());
                    }
                    if name == REPOSITORY_WORKTREE_PATCH_TOOL_NAME {
                        assert_multi_request(&tool_call.input.0, expected_patch)?;
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
                    if output.is_error {
                        return Err("RAH tool returned a public error".to_owned());
                    }
                    let name = outcome
                        .calls
                        .get(&tool_call_id)
                        .ok_or_else(|| "finished output lacked requested tool call".to_owned())?
                        .clone();
                    outcome.latest_outputs.insert(name, output);
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
    outcome
        .final_text
        .as_ref()
        .ok_or_else(|| "missing Completed output".to_owned())?;
    live_gate_contract::require_completed(&outcome.sequence)?;
    Ok(outcome)
}

fn assert_multi_request(actual: &Value, expected: &Value) -> Result<(), String> {
    if actual != expected
        || actual.get("expected_old_text").is_some()
        || actual.get("replacement_text").is_some()
        || actual["replacements"]
            .as_array()
            .is_none_or(|items| items.len() != 3)
    {
        return Err(format!(
            "repo.patch did not use the exact three-item replacements request: {actual}"
        ));
    }
    Ok(())
}

fn prompt() -> &'static str {
    "Use only the available RAH repository tools. First inspect repository state and the target file with repo.status and repo.file-info. Then change target.txt in exactly ONE repo.patch call using its replacements array form: alpha = 1 to alpha = 10, beta = 2 to beta = 20, and gamma = 3 to gamma = 30. Do not use the legacy old/new form. After the mutation inspect repo.file-info, repo.status, repo.diff, and repo.diff-staged; finish only after they verify the exact requested post-state, no staged mutation, and no unrelated change. Do not invoke any capability outside this RAH repository toolkit. Respond compactly after the observations."
}

fn audit_observers(outcome: &TurnOutcome, fixture: &LiveFixture) -> Result<(), String> {
    for required in [
        REPOSITORY_FILE_INFO_TOOL_NAME,
        REPOSITORY_STATUS_TOOL_NAME,
        REPOSITORY_DIFF_TOOL_NAME,
        REPOSITORY_DIFF_STAGED_TOOL_NAME,
    ] {
        if outcome.count(required, Count::Requested) == 0
            || outcome.count(required, Count::Started) == 0
            || outcome.count(required, Count::Finished) == 0
        {
            return Err(format!(
                "required observer {required} lacked complete lifecycle"
            ));
        }
    }
    let values = outcome.output_values()?;
    let file_info = values
        .get(REPOSITORY_FILE_INFO_TOOL_NAME)
        .ok_or_else(|| "missing repo.file-info output".to_owned())?;
    if file_info["path"]["value"] != TARGET
        || file_info["index"]["tracked"] != true
        || file_info["worktree"]["present"] != true
        || file_info["worktree"]["kind"] != "regular_file"
        || file_info["worktree"]["size_bytes"] != POSTIMAGE.len()
        || file_info["content"]["sha256"] != fixture.post_hash
        || file_info["content"]["byte_length"] != POSTIMAGE.len()
    {
        return Err(format!(
            "repo.file-info did not prove the postimage regular tracked file: {file_info}"
        ));
    }
    let status = values
        .get(REPOSITORY_STATUS_TOOL_NAME)
        .ok_or_else(|| "missing repo.status output".to_owned())?;
    let entries = status["entries"]
        .as_array()
        .ok_or_else(|| "repo.status entries missing".to_owned())?;
    if entries.len() != 1
        || !entries.iter().any(|entry| {
            entry["path"]["value"] == TARGET
                && entry["index_state"] == "unmodified"
                && entry["worktree_state"] == "modified"
        })
    {
        return Err("repo.status did not prove target-only worktree mutation".to_owned());
    }
    let diff = values
        .get(REPOSITORY_DIFF_TOOL_NAME)
        .ok_or_else(|| "missing repo.diff output".to_owned())?;
    let files = diff["files"]
        .as_array()
        .ok_or_else(|| "repo.diff files missing".to_owned())?;
    let rendered = serde_json::to_string(diff).map_err(display_error)?;
    if diff["comparison"] != "worktree_to_index"
        || files.len() != 1
        || files[0]["new_path"]["value"] != TARGET
        || files[0]["binary"] == true
        || !["alpha = 10", "beta = 20", "gamma = 30"]
            .iter()
            .all(|text| rendered.contains(text))
    {
        return Err(
            "repo.diff did not prove the exact textual three-replacement mutation".to_owned(),
        );
    }
    let staged = values
        .get(REPOSITORY_DIFF_STAGED_TOOL_NAME)
        .ok_or_else(|| "missing repo.diff-staged output".to_owned())?;
    if !staged["files"].as_array().is_some_and(Vec::is_empty) {
        return Err("repo.diff-staged reported an unexpected staged mutation".to_owned());
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
struct TurnOutcome {
    requested: HashMap<String, usize>,
    started: HashMap<String, usize>,
    finished: HashMap<String, usize>,
    calls: HashMap<ToolCallId, String>,
    latest_outputs: HashMap<String, ToolOutput>,
    sequence: Vec<String>,
    final_text: Option<String>,
}
impl TurnOutcome {
    fn bump(&mut self, id: &ToolCallId, count: Count) -> Result<(), String> {
        let name = self
            .calls
            .get(id)
            .ok_or_else(|| "lifecycle event lacked requested tool call".to_owned())?
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
    fn count_summary(&self) -> String {
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
    fn output_values(&self) -> Result<HashMap<&str, &Value>, String> {
        let mut values = HashMap::new();
        for (name, output) in &self.latest_outputs {
            let [ToolContent::Json(value)] = output.content.as_slice() else {
                return Err("tool output was not one JSON value".to_owned());
            };
            values.insert(name.as_str(), value);
        }
        Ok(values)
    }
}

struct LiveFixture {
    directory: PathBuf,
    root: PathBuf,
    target: PathBuf,
    before_head: Vec<u8>,
    before_refs: Vec<u8>,
    before_index: Vec<u8>,
    before_target: Vec<u8>,
    before_unrelated: Vec<u8>,
    before_status: Vec<u8>,
    before_staged_diff: Vec<u8>,
    before_unstaged_diff: Vec<u8>,
    post_hash: String,
}
impl LiveFixture {
    fn create(git: &Path) -> Result<Self, String> {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(display_error)?
            .as_nanos();
        let directory = env::temp_dir().join(format!(
            ".rah-live-multi-patch-{}-{stamp}-{}",
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
        fs::write(root.join(TARGET), PREIMAGE).map_err(display_error)?;
        fs::write(root.join(UNRELATED), "tracked sentinel\n").map_err(display_error)?;
        fs::OpenOptions::new()
            .write(true)
            .open(root.join(TARGET))
            .map_err(display_error)?
            .set_times(
                fs::FileTimes::new().set_modified(SystemTime::now() - Duration::from_secs(5)),
            )
            .map_err(display_error)?;
        git_ok(git, &root, &["add", "--", TARGET, UNRELATED])?;
        git_ok(
            git,
            &root,
            &["commit", "--quiet", "-m", "live multi patch fixture"],
        )?;
        let root = fs::canonicalize(root).map_err(display_error)?;
        let target = root.join(TARGET);
        let post_hash = format!("{:x}", Sha256::digest(POSTIMAGE.as_bytes()));
        let fixture = Self {
            before_head: git_output(git, &root, &["rev-parse", "HEAD"])?,
            before_refs: git_output(git, &root, &["show-ref", "--head"])?,
            before_index: fs::read(root.join(".git/index")).map_err(display_error)?,
            before_target: fs::read(&target).map_err(display_error)?,
            before_unrelated: fs::read(root.join(UNRELATED)).map_err(display_error)?,
            before_status: git_output(git, &root, &["status", "--porcelain=v1"])?,
            before_staged_diff: git_output(git, &root, &["diff", "--cached", "--no-ext-diff"])?,
            before_unstaged_diff: git_output(git, &root, &["diff", "--no-ext-diff"])?,
            directory,
            root,
            target,
            post_hash,
        };
        if !fixture.before_status.is_empty()
            || !fixture.before_staged_diff.is_empty()
            || !fixture.before_unstaged_diff.is_empty()
        {
            return Err("fixture baseline must be clean".to_owned());
        }
        Ok(fixture)
    }
    fn write_profile(&self, git: &Path) -> Result<PathBuf, String> {
        let path = self.directory.join("trusted-profile.json");
        let capabilities = TOOLS.into_iter().map(|name| json!({"name":name,"enabled":true,"permission":"execute","executable":GIT_RESOURCE_ID,"repository":REPOSITORY_RESOURCE_ID})).collect::<Vec<_>>();
        fs::write(&path, serde_json::to_vec(&json!({"profile_version":1,"profile_id":PROFILE_ID,"resources":{"executables":{GIT_RESOURCE_ID:{"path":git,"kind":"native"}},"repositories":{REPOSITORY_RESOURCE_ID:{"path":&self.root}}},"capabilities":capabilities})).map_err(display_error)?).map_err(display_error)?;
        Ok(path)
    }
    fn expected_request(&self) -> Value {
        let bytes = fs::read(&self.target).expect("fixture target is readable");
        json!({"path":TARGET,"expected_file_sha256":format!("{:x}", Sha256::digest(&bytes)),"expected_file_byte_length":bytes.len(),"replacements":[{"expected_old_text":"alpha = 1","replacement_text":"alpha = 10"},{"expected_old_text":"beta = 2","replacement_text":"beta = 20"},{"expected_old_text":"gamma = 3","replacement_text":"gamma = 30"}]})
    }
    fn assert_after(&self, git: &Path) -> Result<(), String> {
        if fs::read(&self.target).map_err(display_error)? != POSTIMAGE.as_bytes()
            || self.before_target != PREIMAGE.as_bytes()
        {
            return Err("target bytes did not change to exact postimage".to_owned());
        }
        if fs::read(self.root.join(UNRELATED)).map_err(display_error)? != self.before_unrelated {
            return Err("unrelated fixture bytes changed".to_owned());
        }
        if fs::read(self.root.join(".git/index")).map_err(display_error)? != self.before_index
            || git_output(git, &self.root, &["rev-parse", "HEAD"])? != self.before_head
            || git_output(git, &self.root, &["show-ref", "--head"])? != self.before_refs
            || git_output(git, &self.root, &["diff", "--cached", "--no-ext-diff"])?
                != self.before_staged_diff
        {
            return Err("index, HEAD, refs, or staged diff changed".to_owned());
        }
        if git_output(git, &self.root, &["status", "--porcelain=v1"])? != b" M target.txt\n" {
            return Err("worktree status was not target-only modification".to_owned());
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
        .ok_or_else(|| "native Git executable is required".to_owned())?;
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
