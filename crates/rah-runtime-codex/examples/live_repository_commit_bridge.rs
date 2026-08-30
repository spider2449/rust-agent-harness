//! Opt-in Windows live validation of the host-owned `repo.commit` Tool Bridge path.
//!
//! This is deliberately not a deterministic or network-required test. It requires
//! the certified `codex-cli 0.149.0`, live model access, and an explicit native
//! Git for Windows executable in `RAH_REPOSITORY_COMMIT_GIT_EXECUTABLE`.

use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command, ExitCode, Stdio},
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
use rah_tools::TrustedStaticProfile;
use serde_json::{Value, json};

const PROFILE_ID: &str = "live-repository-commit";
const GIT_RESOURCE_ID: &str = "live-git";
const REPOSITORY_RESOURCE_ID: &str = "live-repository";
const TOOL: &str = "repo.commit";
const ALIAS: &str = "rah_tool_0";
const MESSAGE: &str = "RAH live reviewed commit";
const FINAL_MARKER: &str = "RAH_REPOSITORY_COMMIT_LIVE_OK";
const TOOL_DESCRIPTION: &str = "Commit the currently host-reviewed staged repository snapshot once using the provided message.";
const IDENTITY_NAME: &str = "RAH Live Commit";
const IDENTITY_EMAIL: &str = "rah-live-commit@example.invalid";
const TERMINAL_TIMEOUT: Duration = Duration::from_secs(120);
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
            eprintln!("LIVE_REPOSITORY_COMMIT_BRIDGE_FAIL Tokio runtime: {error}");
            return ExitCode::FAILURE;
        }
    };
    match runtime.block_on(run()) {
        Ok(()) => {
            println!("LIVE_REPOSITORY_COMMIT_BRIDGE_PASS");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("LIVE_REPOSITORY_COMMIT_BRIDGE_FAIL {error}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), String> {
    let codex = env::var_os("RAH_CODEX_EXECUTABLE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("codex"));
    let git = native_git()?;
    let mut fixture = Fixture::create(&git)?;
    let before = fixture.review(&git)?;
    let profile_path = fixture.write_profile(&git)?;
    let profile = TrustedStaticProfile::load(&profile_path)
        .map_err(|error| format!("trusted profile source validation failed: {error}"))?;
    println!("PROFILE_SOURCE_VALIDATION succeeded");
    let composition = compose(profile)
        .await
        .map_err(|error| format!("effective composition failed: {error}"))?;
    let definition = verify_composition(&composition)?;
    println!("EFFECTIVE_COMPOSITION succeeded");
    println!("FRESH_TOOL_REGISTRY tool_count=1 tool={TOOL} permission=Execute");
    println!(
        "PROFILE_SYMBOLIC_RESOURCES executable={GIT_RESOURCE_ID} repository={REPOSITORY_RESOURCE_ID}"
    );
    let schema = serde_json::to_string(&definition.input_schema).map_err(display)?;
    if definition.description != TOOL_DESCRIPTION {
        return Err("repo.commit definition description changed unexpectedly".to_owned());
    }
    // This records the exact bridge contract established by `snapshot_tools`: this
    // sole ToolDefinition is translated into one immediately visible DynamicToolSpec.
    println!("PUBLIC_RAH_TOOL {TOOL}");
    println!("PRIVATE_ALIAS {ALIAS}");
    println!("DYNAMIC_TOOL_COUNT 1");
    println!("DYNAMIC_TOOL_NAME {ALIAS}");
    println!("DYNAMIC_TOOL_DESCRIPTION {TOOL_DESCRIPTION}");
    println!("DYNAMIC_TOOL_SCHEMA {schema}");
    println!("DYNAMIC_TOOL_DEFER_LOADING false");
    println!("ALLOWED_PERMISSION Execute");
    println!("MODEL_AUTHORITY_FIELDS none");
    println!("PRIVATE_ALIAS_MAPPING {ALIAS} -> {TOOL}");
    println!(
        "RESTRICTED_CODEX_CAPABILITIES shell=false file=false generic_git=false branch_ref=false network_git=false mcp=false web=false image=false apps=false process=false approvals=false"
    );
    println!("CODEX_VERSION {SUPPORTED_CODEX_VERSION}");
    println!("REQUIRED_CODEX_VERSION {SUPPORTED_CODEX_VERSION}");
    println!("CODEX_EXECUTABLE_IDENTITY native_discovery_and_exact_version_verified=true");
    println!("GIT_EXECUTABLE_CANONICAL {}", git.display());
    println!("HOST_REVIEW staged_snapshot_verified=true");
    assert_unchanged(&git, &fixture.root, &before)?;
    composition
        .repository_commit_control()
        .ok_or_else(|| "missing host-only repository commit control".to_owned())?
        .authorize_current_reviewed_snapshot()
        .await
        .map_err(|error| format!("host review authorization failed: {error}"))?;
    println!("HOST_REVIEW_AUTHORIZATION armed=true model_visible=false");

    let registry = composition.registry_handle();
    let runtime_result =
        CodexRuntime::connect_tool_bridge(&codex, registry.clone(), vec![PermissionLevel::Execute])
            .await;
    let outcome = match runtime_result {
        Ok(runtime) => {
            let turn = run_turn(&runtime).await;
            let shutdown = runtime
                .shutdown()
                .await
                .map_err(|error| format!("Codex app-server shutdown/reap failed: {error}"));
            drop(runtime);
            shutdown?;
            println!("CODEX_CLEANUP app-server shutdown and reap completed");
            turn?
        }
        Err(error) => return Err(format!("Codex Tool Bridge connection failed: {error}")),
    };
    drop(registry);
    composition.shutdown().await;

    let actual = fixture.verify_after(&git, &before, &outcome.output)?;
    let expected_final_text = format!("{FINAL_MARKER} {actual}");
    if outcome.final_text.as_deref() != Some(expected_final_text.as_str()) {
        return Err("final assistant commit OID did not match verified fixture HEAD".to_owned());
    }
    println!("TOOL_REQUESTED_COUNT {}", outcome.requested);
    println!("TOOL_STARTED_COUNT {}", outcome.started);
    println!("TOOL_FINISHED_COUNT {}", outcome.finished);
    println!("ACTUAL_RAH_TOOL_CALL name={TOOL} input={{\"message\":\"{MESSAGE}\"}}");
    println!("TOOL_OUTPUT_STATUS committed_verified");
    println!("VERIFIED_COMMIT_OID {actual}");
    println!("RAH_EVENT_SEQUENCE {}", outcome.sequence.join(" -> "));
    println!("CODEX_CONTINUED_AFTER_TOOL_RESPONSE true");
    println!("COMMIT_EXECUTION_COUNT 1");
    println!("BRANCH_ADVANCE_COUNT 1");
    println!("CURRENT_BRANCH_UNCHANGED true");
    println!("OTHER_REFS_UNCHANGED true");
    println!("UNSIGNED_COMMIT true");
    println!("HOST_IDENTITY_VERIFIED true");
    println!("INDEX_POSTCONDITION staged_diff_empty=true unrelated_untracked_preserved=true");
    println!("REFLOG_EFFECT expected_normal_commit_metadata");
    println!("REPLAY_RETRY_COUNT 0");
    println!("FINAL_ASSISTANT_TEXT {:?}", outcome.final_text);
    println!("TERMINAL_RAH_EVENT Completed");
    fixture.remove()?;
    println!("TEST_REPOSITORY_CLEANUP removed=true");
    println!("CLEANUP_STATE codex_app_server=reaped composition=shutdown fixture=removed");
    println!("{FINAL_MARKER}");
    Ok(())
}

fn verify_composition(composition: &EffectiveProfileComposition) -> Result<ToolDefinition, String> {
    let effective = composition.effective_profile();
    if effective.profile_id != PROFILE_ID
        || effective.capabilities.len() != 1
        || !effective.providers.is_empty()
    {
        return Err(
            "effective inventory was not the expected redacted repo.commit-only profile".to_owned(),
        );
    }
    let definitions = composition.registry().definitions();
    let [definition] = definitions.as_slice() else {
        return Err("fresh registry did not contain exactly one tool".to_owned());
    };
    let schema = json!({"type":"object","properties":{"message":{"type":"string","maxLength":16384}},"required":["message"],"additionalProperties":false});
    if definition.name != ToolName::new(TOOL)
        || definition.permission != PermissionLevel::Execute
        || definition.input_schema != schema
    {
        return Err("repo.commit definition did not remain message-only Execute".to_owned());
    }
    let text = definition.input_schema.to_string();
    for forbidden in [
        "repository",
        "path",
        "cwd",
        "git",
        "executable",
        "argv",
        "branch",
        "head",
        "ref",
        "index",
        "tree",
        "authorization",
        "token",
        "identity",
        "author",
        "committer",
        "hooks",
        "signing",
        "remote",
        "credential",
    ] {
        if text.contains(forbidden) {
            return Err(format!("model-visible schema leaked {forbidden}"));
        }
    }
    Ok(definition.clone())
}

async fn run_turn(runtime: &CodexRuntime) -> Result<Outcome, String> {
    let prompt = format!(
        "You must perform the requested repository commit using the one available RAH tool.\n\nCall that tool exactly once with this JSON input:\n{{\"message\":\"{MESSAGE}\"}}\n\nDo not claim success unless the tool returns committed_verified. After it returns committed_verified, reply with exactly one line:\n{FINAL_MARKER} <commit_oid>\n\nReplace <commit_oid> with the exact commit_oid returned by the tool. If the tool does not return committed_verified, do not output the success marker. Do not request any other tool and do not call the tool more than once."
    );
    let handle = runtime
        .start(AgentRequest {
            request_id: RequestId::new(),
            input: AgentInput {
                messages: vec![Message {
                    role: MessageRole::User,
                    content: prompt,
                }],
            },
            options: AgentOptions::default(),
        })
        .await
        .map_err(|error| format!("failed to start Codex turn: {error}"))?;
    let mut events = handle.into_events();
    let mut outcome = Outcome::default();
    tokio::time::timeout(TERMINAL_TIMEOUT, async {
        while let Some(event) = events.next().await {
            outcome.sequence.push(event_name(&event).to_owned());
            match event {
                AgentEvent::ToolRequested { tool_call, .. } => {
                    outcome.requested += 1;
                    if outcome.requested != 1
                        || tool_call.name != ToolName::new(TOOL)
                        || tool_call.input.0 != json!({"message": MESSAGE})
                    {
                        return Err(format!("unexpected model tool call: {tool_call:?}"));
                    }
                }
                AgentEvent::ToolStarted { .. } => {
                    outcome.started += 1;
                    if outcome.started != 1 {
                        return Err("more than one ToolStarted".to_owned());
                    }
                }
                AgentEvent::ToolFinished { output, .. } => {
                    outcome.finished += 1;
                    if outcome.finished != 1 {
                        return Err("more than one ToolFinished".to_owned());
                    }
                    outcome.output = Some(output);
                }
                AgentEvent::ModelDelta { .. } if outcome.finished == 1 => outcome.continued = true,
                AgentEvent::Completed { output, .. } => {
                    outcome.final_text = Some(output.message.content)
                }
                AgentEvent::ApprovalRequired { .. } => {
                    return Err("unexpected approval request".to_owned());
                }
                AgentEvent::Failed { message, .. } => {
                    return Err(format!("runtime failed: {message}"));
                }
                AgentEvent::Cancelled { .. } => return Err("turn cancelled".to_owned()),
                AgentEvent::Started { .. }
                | AgentEvent::ModelRequestStarted { .. }
                | AgentEvent::ModelDelta { .. } => {}
            }
        }
        Ok::<(), String>(())
    })
    .await
    .map_err(|_| "timed out waiting for terminal event".to_owned())??;
    let output_commit_oid = tool_output_commit_oid(&outcome.output)?;
    let expected_final_text = format!("{FINAL_MARKER} {output_commit_oid}");
    if outcome.requested != 1
        || outcome.started != 1
        || outcome.finished != 1
        || !outcome.continued
        || outcome.sequence.first() != Some(&"Started".to_owned())
        || outcome.sequence.last() != Some(&"Completed".to_owned())
        || outcome.final_text.as_deref() != Some(expected_final_text.as_str())
    {
        return Err(format!(
            "live event/lifecycle assertion failed: sequence={:?}, final_assistant_text={:?}",
            outcome.sequence, outcome.final_text
        ));
    }
    Ok(outcome)
}

fn tool_output_commit_oid(output: &Option<ToolOutput>) -> Result<String, String> {
    let output = output
        .as_ref()
        .ok_or_else(|| "missing ToolFinished output".to_owned())?;
    if output.is_error {
        return Err("repo.commit returned an error output".to_owned());
    }
    let [ToolContent::Text(text)] = output.content.as_slice() else {
        return Err("repo.commit output was not bounded text JSON".to_owned());
    };
    let value: Value = serde_json::from_str(text).map_err(display)?;
    if value["status"] != "committed_verified" {
        return Err("repo.commit did not return committed_verified".to_owned());
    }
    value["commit_oid"]
        .as_str()
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| "repo.commit did not return commit_oid".to_owned())
}

#[derive(Default)]
struct Outcome {
    requested: usize,
    started: usize,
    finished: usize,
    continued: bool,
    sequence: Vec<String>,
    output: Option<ToolOutput>,
    final_text: Option<String>,
}
struct Before {
    head: String,
    branch: String,
    tree: String,
    refs: String,
    staged: String,
}
struct Fixture {
    root: PathBuf,
    profile: PathBuf,
}
impl Fixture {
    fn create(git: &Path) -> Result<Self, String> {
        let n = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let root = env::temp_dir().join(format!(
            "rah-live-repository-commit-{}-{}-{n}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(display)?
                .as_nanos()
        ));
        fs::create_dir(&root).map_err(display)?;
        let fixture = Self {
            profile: root.join("trusted-profile.json"),
            root,
        };
        git_run(git, &fixture.root, &["init", "--quiet"])?;
        fs::write(fixture.root.join("tracked.txt"), "base\n").map_err(display)?;
        git_run(git, &fixture.root, &["add", "--", "tracked.txt"])?;
        git_run(
            git,
            &fixture.root,
            &[
                "-c",
                "user.name=fixture",
                "-c",
                "user.email=fixture@example.invalid",
                "commit",
                "--quiet",
                "-m",
                "baseline",
            ],
        )?;
        git_run(
            git,
            &fixture.root,
            &["config", "--local", "user.name", "Ambient Wrong Identity"],
        )?;
        git_run(
            git,
            &fixture.root,
            &[
                "config",
                "--local",
                "user.email",
                "ambient-wrong@example.invalid",
            ],
        )?;
        fs::write(
            fixture.root.join("tracked.txt"),
            "reviewed staged content\n",
        )
        .map_err(display)?;
        git_run(git, &fixture.root, &["add", "--", "tracked.txt"])?;
        fs::write(
            fixture.root.join("unrelated-untracked.txt"),
            "preserve me\n",
        )
        .map_err(display)?;
        Ok(fixture)
    }
    fn review(&self, git: &Path) -> Result<Before, String> {
        let staged = git_text(git, &self.root, &["diff", "--cached", "--", "tracked.txt"])?;
        if !staged.contains("-base")
            || !staged.contains("+reviewed staged content")
            || git_text(git, &self.root, &["diff", "--cached", "--quiet", "HEAD"]).is_ok()
        {
            return Err(
                "host review did not find exactly the intended non-empty staged change".to_owned(),
            );
        }
        let branch = git_text(git, &self.root, &["symbolic-ref", "--quiet", "HEAD"])?;
        if !branch.starts_with("refs/heads/") {
            return Err("fixture is not on an attached branch".to_owned());
        }
        Ok(Before {
            head: git_text(git, &self.root, &["rev-parse", "HEAD"])?,
            branch,
            tree: git_text(git, &self.root, &["write-tree"])?,
            refs: git_text(
                git,
                &self.root,
                &["for-each-ref", "--format=%(refname) %(objectname)"],
            )?,
            staged,
        })
    }
    fn write_profile(&self, git: &Path) -> Result<PathBuf, String> {
        let profile = json!({
            "profile_version": 1,
            "profile_id": PROFILE_ID,
            "resources": {
                "executables": { "git": { "path": git, "kind": "native" } },
                "repositories": { "repo": { "path": &self.root } }
            },
            "capabilities": [{
                "name": TOOL,
                "enabled": true,
                "permission": "execute",
                "executable": "git",
                "repository": "repo",
                "identity_name": IDENTITY_NAME,
                "identity_email": IDENTITY_EMAIL
            }]
        });
        fs::write(
            &self.profile,
            serde_json::to_vec(&profile).map_err(display)?,
        )
        .map_err(display)?;
        Ok(self.profile.clone())
    }
    fn verify_after(
        &self,
        git: &Path,
        before: &Before,
        output: &Option<ToolOutput>,
    ) -> Result<String, String> {
        let output = output
            .as_ref()
            .ok_or_else(|| "missing ToolFinished output".to_owned())?;
        let [ToolContent::Text(text)] = output.content.as_slice() else {
            return Err("repo.commit output was not bounded text JSON".to_owned());
        };
        let value: Value = serde_json::from_str(text).map_err(display)?;
        let head = git_text(git, &self.root, &["rev-parse", "HEAD"])?;
        if output.is_error
            || value["status"] != "committed_verified"
            || value["commit_oid"] != head
            || head == before.head
        {
            return Err("ToolFinished did not report the verified actual commit".to_owned());
        }
        if git_text(git, &self.root, &["rev-parse", "HEAD^"])? != before.head
            || git_text(git, &self.root, &["rev-parse", "HEAD^{tree}"])? != before.tree
            || git_text(git, &self.root, &["symbolic-ref", "HEAD"])? != before.branch
            || git_text(git, &self.root, &["show", "-s", "--format=%B", "HEAD"])? != MESSAGE
            || git_text(
                git,
                &self.root,
                &["show", "-s", "--format=%an <%ae>|%cn <%ce>", "HEAD"],
            )? != format!(
                "{IDENTITY_NAME} <{IDENTITY_EMAIL}>|{IDENTITY_NAME} <{IDENTITY_EMAIL}>"
            )
            || git_text(git, &self.root, &["diff", "--cached", "--quiet"]).is_err()
            || !self.root.join("unrelated-untracked.txt").is_file()
        {
            return Err("postcondition verification failed".to_owned());
        }
        if git_text(git, &self.root, &["cat-file", "-p", "HEAD"])?.contains("gpgsig ")
            || git_text(
                git,
                &self.root,
                &["for-each-ref", "--format=%(refname) %(objectname)"],
            )? != format!("{} {}", before.branch, head)
            || before.refs != format!("{} {}", before.branch, before.head)
            || before.staged.is_empty()
        {
            return Err("signature or ref scope assertion failed".to_owned());
        }
        Ok(head)
    }
    fn remove(&mut self) -> Result<(), String> {
        fs::remove_dir_all(&self.root).map_err(display)?;
        if self.root.exists() {
            return Err("fixture removal failed".to_owned());
        }
        Ok(())
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        if self.root.exists() {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}
fn assert_unchanged(git: &Path, root: &Path, before: &Before) -> Result<(), String> {
    if git_text(git, root, &["rev-parse", "HEAD"])? != before.head
        || git_text(git, root, &["symbolic-ref", "HEAD"])? != before.branch
        || git_text(git, root, &["write-tree"])? != before.tree
    {
        return Err("composition automatically mutated or authorized fixture state".to_owned());
    }
    Ok(())
}
fn native_git() -> Result<PathBuf, String> {
    let configured = PathBuf::from(
        env::var_os("RAH_REPOSITORY_COMMIT_GIT_EXECUTABLE").ok_or_else(|| {
            "RAH_REPOSITORY_COMMIT_GIT_EXECUTABLE must name an absolute native git.exe".to_owned()
        })?,
    );
    if !configured.is_absolute() {
        return Err("RAH_REPOSITORY_COMMIT_GIT_EXECUTABLE must be absolute".to_owned());
    }
    let git = fs::canonicalize(&configured).map_err(display)?;
    if !git.is_file()
        || !git
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.eq_ignore_ascii_case("git.exe"))
    {
        return Err("live validation requires the exact native git.exe".to_owned());
    }
    Ok(git)
}
fn git_run(git: &Path, root: &Path, args: &[&str]) -> Result<(), String> {
    let _ = git_output(git, root, args)?;
    Ok(())
}
fn git_text(git: &Path, root: &Path, args: &[&str]) -> Result<String, String> {
    String::from_utf8(git_output(git, root, args)?)
        .map_err(display)
        .map(|value| value.trim_end_matches(['\r', '\n']).to_owned())
}
fn git_output(git: &Path, root: &Path, args: &[&str]) -> Result<Vec<u8>, String> {
    let output = Command::new(git)
        .args(args)
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env_clear()
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "NUL")
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .map_err(display)?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(format!(
            "fixture Git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}
fn display(error: impl std::fmt::Display) -> String {
    error.to_string()
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
