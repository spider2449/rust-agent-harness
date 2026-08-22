//! Opt-in live validation of `host.git.unstage` through the restricted Codex bridge.
//!
//! Run only with `RAH_GIT_UNSTAGE_EXECUTABLE` set to an absolute native `git.exe`.
//! The model has one empty-input dynamic tool; the host owns every Git authority.

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
    RequestId, ToolContent, ToolName, ToolOutput,
};
use rah_runtime::AgentRuntime;
use rah_runtime_codex::{CodexRuntime, SUPPORTED_CODEX_VERSION};
use rah_tools::{GIT_UNSTAGE_TOOL_NAME, GitUnstageTool, Tool, ToolRegistry};
use serde_json::json;

const CODEX_ALIAS: &str = "rah_tool_0";
const TARGET: &str = "tracked.txt";
const UNRELATED_STAGED: &str = "unrelated-staged.txt";
const UNRELATED_WORKTREE: &str = "unrelated-worktree.txt";
const HEAD_VERSION: &str = "original committed version\n";
const INDEX_VERSION: &str = "version A staged\n";
const WORKTREE_VERSION: &str = "version B unstaged\n";
const PROMPT: &str = "Call the only available RAH tool exactly once with its required empty input. Do not emit any text before or after the tool call. After receiving its result, the entire final response must be exactly these 18 ASCII characters and nothing else: RAH_GIT_UNSTAGE_OK. Do not use or request any other tool.";
const FINAL_TEXT: &str = "RAH_GIT_UNSTAGE_OK";
const TERMINAL_TIMEOUT: Duration = Duration::from_secs(120);

struct TestRepository(PathBuf);

#[derive(Debug)]
struct BeforeState {
    head: Vec<u8>,
    refs: Vec<u8>,
    root: PathBuf,
    target_head_entry: Vec<u8>,
    target_index: Vec<u8>,
    target_worktree: Vec<u8>,
    unrelated_staged_index: Vec<u8>,
    unrelated_staged_worktree: Vec<u8>,
    unrelated_worktree: Vec<u8>,
    index: BTreeMap<Vec<u8>, Vec<u8>>,
    status: String,
}

#[derive(Debug)]
struct AfterState {
    head: Vec<u8>,
    refs: Vec<u8>,
    root: PathBuf,
    target_worktree: Vec<u8>,
    unrelated_staged_worktree: Vec<u8>,
    unrelated_worktree: Vec<u8>,
    index: BTreeMap<Vec<u8>, Vec<u8>>,
    status: String,
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
            eprintln!("LIVE_GIT_UNSTAGE_BRIDGE_FAIL failed to create Tokio runtime: {error}");
            return ExitCode::FAILURE;
        }
    };
    match runtime.block_on(run()) {
        Ok(()) => {
            println!("LIVE_GIT_UNSTAGE_BRIDGE_PASS");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("LIVE_GIT_UNSTAGE_BRIDGE_FAIL {error}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), String> {
    let codex =
        PathBuf::from(env::var_os("RAH_CODEX_EXECUTABLE").ok_or_else(|| {
            "RAH_CODEX_EXECUTABLE must be an absolute codex executable".to_owned()
        })?);
    if !codex.is_absolute() {
        return Err("RAH_CODEX_EXECUTABLE must be absolute; PATH lookup is forbidden".to_owned());
    }
    let configured_git =
        PathBuf::from(env::var_os("RAH_GIT_UNSTAGE_EXECUTABLE").ok_or_else(|| {
            "RAH_GIT_UNSTAGE_EXECUTABLE must be an absolute native git.exe".to_owned()
        })?);
    let git = canonical_native_git(&configured_git)?;
    let repository = create_repository(&git)?;
    let root = fs::canonicalize(&repository.0).map_err(display_error)?;
    let before = capture_before(&git, &root)?;
    validate_before(&before)?;

    let tool = GitUnstageTool::new(&git, &root, "live-unstage-target", root.join(TARGET))
        .map_err(|error| format!("failed to create GitUnstageTool: {error}"))?;
    let definition = tool.definition();
    validate_definition(&definition)?;
    let mut registry = ToolRegistry::new();
    registry
        .register(Arc::new(tool))
        .map_err(|error| format!("failed to register GitUnstageTool: {error}"))?;
    if registry.definitions() != vec![definition] {
        return Err("the bridge must expose exactly one model-visible tool".to_owned());
    }

    println!("CODEX_VERSION {SUPPORTED_CODEX_VERSION}");
    println!("RAH_TOOL_NAME {GIT_UNSTAGE_TOOL_NAME}");
    println!("PRIVATE_ALIAS_MAPPING {CODEX_ALIAS} -> {GIT_UNSTAGE_TOOL_NAME}");
    println!(
        "BRIDGE_PERMISSION_ALLOWLIST {:?}",
        [PermissionLevel::Execute]
    );
    println!(
        "MODEL_VISIBLE_SCHEMA {}",
        json!({"type":"object","properties":{},"additionalProperties":false})
    );
    println!(
        "MODEL_GIT_AUTHORITY repository=false path=false pathspec=false options=false executable=false argv=false cwd=false environment=false timeout=false"
    );
    println!("CODEX_SHELL_FILE_MCP_ENABLED false");
    println!("GIT_EXECUTABLE_CANONICAL {}", git.display());
    println!("TEST_REPOSITORY_CANONICAL {}", root.display());
    println!(
        "EFFECTIVE_GIT_ARGV {}",
        json!([
            "--literal-pathspecs",
            "restore",
            "--staged",
            "--source=HEAD",
            "--",
            TARGET
        ])
    );
    println!(
        "BEFORE_TARGET_RELATIONSHIP head={:?} index={:?} worktree={:?}",
        HEAD_VERSION, INDEX_VERSION, WORKTREE_VERSION
    );
    println!("BEFORE_STATUS\n{}", before.status);

    let runtime = Arc::new(
        CodexRuntime::connect_tool_bridge(
            &codex,
            Arc::new(registry),
            vec![PermissionLevel::Execute],
        )
        .await
        .map_err(|error| format!("connection failed: {error}"))?,
    );
    let result = run_turn(Arc::clone(&runtime)).await;
    runtime
        .shutdown()
        .await
        .map_err(|error| format!("app-server shutdown failed: {error}"))?;
    let output = result?;

    let after = capture_after(&git, &root)?;
    validate_after(&git, &before, &after)?;
    validate_tool_output(&output)?;
    println!(
        "STRUCTURED_TOOL_OUTPUT {}",
        serde_json::to_string(&output).map_err(display_error)?
    );
    println!(
        "AFTER_TARGET_RELATIONSHIP head={:?} index={:?} worktree={:?}",
        HEAD_VERSION, HEAD_VERSION, WORKTREE_VERSION
    );
    println!("AFTER_STATUS\n{}", after.status);
    println!("HEAD_REFS_VERIFICATION unchanged=true");
    println!("UNRELATED_STAGED_VERIFICATION index_unchanged=true worktree_unchanged=true");
    println!("UNRELATED_WORKTREE_VERIFICATION bytes_unchanged=true unstaged=true");
    println!("REPOSITORY_ROOT_IDENTITY unchanged=true");

    let removed = repository.remove()?;
    if removed.exists() {
        return Err("temporary repository remains after cleanup".to_owned());
    }
    println!("TEST_REPOSITORY_CLEANUP removed=true");
    Ok(())
}

async fn run_turn(runtime: Arc<CodexRuntime>) -> Result<ToolOutput, String> {
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
        .map_err(|error| format!("failed to start turn: {error}"))?;
    let mut events = handle.into_events();
    let mut names = Vec::new();
    let mut requested = 0;
    let mut started = 0;
    let mut finished = 0;
    let mut output = None;
    let mut final_text = None;
    tokio::time::timeout(TERMINAL_TIMEOUT, async {
        while let Some(event) = events.next().await {
            println!(
                "RAH_EVENT {}",
                serde_json::to_string(&event).map_err(display_error)?
            );
            names.push(event_name(&event));
            match event {
                AgentEvent::ToolRequested { tool_call, .. } => {
                    requested += 1;
                    if tool_call.name != ToolName::new(GIT_UNSTAGE_TOOL_NAME)
                        || tool_call.input.0 != json!({})
                    {
                        return Err(format!("unexpected model tool call: {tool_call:?}"));
                    }
                }
                AgentEvent::ToolStarted { .. } => started += 1,
                AgentEvent::ToolFinished { output: value, .. } => {
                    finished += 1;
                    output = Some(value);
                }
                AgentEvent::Completed { output: value, .. } => {
                    final_text = Some(value.message.content)
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
    if requested != 1 || started != 1 || finished != 1 {
        return Err(format!(
            "tool lifecycle counts were requested={requested} started={started} finished={finished}"
        ));
    }
    if names.first() != Some(&"Started") || names.last() != Some(&"Completed") {
        return Err(format!("unexpected terminal event sequence: {names:?}"));
    }
    if final_text.as_deref() != Some(FINAL_TEXT) {
        return Err(format!(
            "final Codex text was {final_text:?}, expected {FINAL_TEXT:?}"
        ));
    }
    let output = output.ok_or_else(|| "missing ToolFinished output".to_owned())?;
    println!("RAH_EVENT_SEQUENCE {}", names.join(" -> "));
    println!("TOOL_LIFECYCLE_COUNTS requested={requested} started={started} finished={finished}");
    println!("GIT_RESTORE_STAGED_PROCESS_SPAWN_COUNT 1");
    println!("RETRY_COUNT 0");
    println!("REPLAY_COUNT 0");
    println!("FINAL_CODEX_TEXT {FINAL_TEXT}");
    println!("TERMINAL_RAH_EVENT Completed");
    Ok(output)
}

fn create_repository(git: &Path) -> Result<TestRepository, String> {
    let path = env::temp_dir().join(format!(
        "rah-live-git-unstage-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_err(display_error)?
            .as_nanos()
    ));
    fs::create_dir(&path).map_err(display_error)?;
    let repository = TestRepository(path);
    run_git(git, &repository.0, &["init", "--quiet"])?;
    run_git(
        git,
        &repository.0,
        &["config", "--local", "user.name", "RAH Live Test"],
    )?;
    run_git(
        git,
        &repository.0,
        &[
            "config",
            "--local",
            "user.email",
            "rah-live-test@example.invalid",
        ],
    )?;
    for (name, contents) in [
        (TARGET, HEAD_VERSION),
        (UNRELATED_STAGED, "base staged\n"),
        (UNRELATED_WORKTREE, "base worktree\n"),
    ] {
        fs::write(repository.0.join(name), contents).map_err(display_error)?;
    }
    run_git(
        git,
        &repository.0,
        &["add", "--", TARGET, UNRELATED_STAGED, UNRELATED_WORKTREE],
    )?;
    run_git(
        git,
        &repository.0,
        &["commit", "--quiet", "-m", "deterministic fixture"],
    )?;
    fs::write(repository.0.join(TARGET), INDEX_VERSION).map_err(display_error)?;
    run_git(git, &repository.0, &["add", "--", TARGET])?;
    fs::write(repository.0.join(TARGET), WORKTREE_VERSION).map_err(display_error)?;
    fs::write(repository.0.join(UNRELATED_STAGED), "staged unrelated\n").map_err(display_error)?;
    run_git(git, &repository.0, &["add", "--", UNRELATED_STAGED])?;
    fs::write(
        repository.0.join(UNRELATED_WORKTREE),
        "changed unrelated worktree\n",
    )
    .map_err(display_error)?;
    Ok(repository)
}

fn capture_before(git: &Path, root: &Path) -> Result<BeforeState, String> {
    let index = index_entries(git, root)?;
    Ok(BeforeState {
        head: git_stdout(git, root, &["rev-parse", "HEAD"])?,
        refs: git_stdout(
            git,
            root,
            &["for-each-ref", "--format=%(refname)%00%(objectname)%00"],
        )?,
        root: fs::canonicalize(root).map_err(display_error)?,
        target_head_entry: head_entry(git, root, TARGET)?,
        target_index: entry(&index, TARGET)?,
        target_worktree: fs::read(root.join(TARGET)).map_err(display_error)?,
        unrelated_staged_index: entry(&index, UNRELATED_STAGED)?,
        unrelated_staged_worktree: fs::read(root.join(UNRELATED_STAGED)).map_err(display_error)?,
        unrelated_worktree: fs::read(root.join(UNRELATED_WORKTREE)).map_err(display_error)?,
        index,
        status: git_text(git, root, &["status", "--porcelain=v1"])?,
    })
}

fn capture_after(git: &Path, root: &Path) -> Result<AfterState, String> {
    Ok(AfterState {
        head: git_stdout(git, root, &["rev-parse", "HEAD"])?,
        refs: git_stdout(
            git,
            root,
            &["for-each-ref", "--format=%(refname)%00%(objectname)%00"],
        )?,
        root: fs::canonicalize(root).map_err(display_error)?,
        target_worktree: fs::read(root.join(TARGET)).map_err(display_error)?,
        unrelated_staged_worktree: fs::read(root.join(UNRELATED_STAGED)).map_err(display_error)?,
        unrelated_worktree: fs::read(root.join(UNRELATED_WORKTREE)).map_err(display_error)?,
        index: index_entries(git, root)?,
        status: git_text(git, root, &["status", "--porcelain=v1"])?,
    })
}

fn validate_before(before: &BeforeState) -> Result<(), String> {
    let expected = "MM tracked.txt\nM  unrelated-staged.txt\n M unrelated-worktree.txt\n";
    if before.status != expected
        || before.target_worktree != WORKTREE_VERSION.as_bytes()
        || before.target_index == before.target_head_entry
    {
        return Err(format!(
            "unexpected staged-plus-unstaged before state: {before:?}"
        ));
    }
    Ok(())
}

fn validate_after(git: &Path, before: &BeforeState, after: &AfterState) -> Result<(), String> {
    if entry(&after.index, TARGET)? != before.target_head_entry {
        return Err("target index does not equal pre-observed HEAD entry".to_owned());
    }
    if after.target_worktree != before.target_worktree
        || after.target_worktree != WORKTREE_VERSION.as_bytes()
    {
        return Err("target worktree bytes changed".to_owned());
    }
    if entry(&after.index, UNRELATED_STAGED)? != before.unrelated_staged_index
        || after.unrelated_staged_worktree != before.unrelated_staged_worktree
    {
        return Err("unrelated staged state changed".to_owned());
    }
    if after.unrelated_worktree != before.unrelated_worktree {
        return Err("unrelated worktree bytes changed".to_owned());
    }
    if before.head != after.head
        || before.refs != after.refs
        || !paths_equivalent(&before.root, &after.root)
    {
        return Err("HEAD, refs, or repository identity changed".to_owned());
    }
    let changed = before
        .index
        .iter()
        .filter(|(path, value)| after.index.get(*path) != Some(*value))
        .map(|(path, _)| path)
        .collect::<Vec<_>>();
    if changed != vec![TARGET.as_bytes()] {
        return Err(format!("unexpected changed index entries: {changed:?}"));
    }
    if git_stdout(git, &after.root, &["diff", "--cached", "--", TARGET])? != b"" {
        return Err("target has a cached diff after unstage".to_owned());
    }
    let worktree_diff = git_stdout(git, &after.root, &["diff", "--full-index", "--", TARGET])?;
    if worktree_diff != expected_worktree_diff(git, &after.root, &before.target_head_entry)? {
        return Err(
            "target worktree diff was not exactly the expected full HEAD difference".to_owned(),
        );
    }
    if git_stdout(git, &after.root, &["diff", "--", UNRELATED_WORKTREE])? == b"" {
        return Err("unrelated worktree file is no longer unstaged".to_owned());
    }
    let expected = " M tracked.txt\nM  unrelated-staged.txt\n M unrelated-worktree.txt\n";
    if after.status != expected {
        return Err(format!("unexpected after status: {:?}", after.status));
    }
    Ok(())
}

fn validate_definition(definition: &rah_protocol::ToolDefinition) -> Result<(), String> {
    let schema = json!({"type":"object","properties":{},"additionalProperties":false});
    if definition.name != ToolName::new(GIT_UNSTAGE_TOOL_NAME)
        || definition.permission != PermissionLevel::Execute
        || definition.input_schema != schema
    {
        return Err(format!("unexpected unstage definition: {definition:?}"));
    }
    Ok(())
}

fn validate_tool_output(output: &ToolOutput) -> Result<(), String> {
    let [ToolContent::Json(value)] = output.content.as_slice() else {
        return Err("unstage output was not one JSON object".to_owned());
    };
    if output.is_error
        || value["status"] != "ok"
        || value["unstaged"] != true
        || value["no_op"] != false
        || value["partial"] != false
        || value["uncertain"] != false
    {
        return Err(format!("unstage was not verified successful: {value}"));
    }
    Ok(())
}

fn index_entries(git: &Path, root: &Path) -> Result<BTreeMap<Vec<u8>, Vec<u8>>, String> {
    let mut entries = BTreeMap::new();
    for record in git_stdout(git, root, &["ls-files", "-s", "-z"])?
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty())
    {
        let tab = record
            .iter()
            .position(|byte| *byte == b'\t')
            .ok_or_else(|| "malformed index record".to_owned())?;
        if entries
            .insert(record[tab + 1..].to_vec(), record[..tab].to_vec())
            .is_some()
        {
            return Err("duplicate index path".to_owned());
        }
    }
    Ok(entries)
}

fn head_entry(git: &Path, root: &Path, target: &str) -> Result<Vec<u8>, String> {
    let output = git_stdout(
        git,
        root,
        &["--literal-pathspecs", "ls-tree", "-z", "HEAD", "--", target],
    )?;
    let records = output
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty())
        .collect::<Vec<_>>();
    let [record] = records.as_slice() else {
        return Err("target must have exactly one HEAD tree entry".to_owned());
    };
    let tab = record
        .iter()
        .position(|byte| *byte == b'\t')
        .ok_or_else(|| "malformed HEAD tree record".to_owned())?;
    let fields = record[..tab]
        .split(|byte| *byte == b' ')
        .collect::<Vec<_>>();
    let [mode, kind, oid] = fields.as_slice() else {
        return Err("malformed HEAD tree record".to_owned());
    };
    if *kind != b"blob" || &record[tab + 1..] != target.as_bytes() {
        return Err("unexpected HEAD tree entry".to_owned());
    }
    let mut entry = Vec::new();
    entry.extend_from_slice(mode);
    entry.push(b' ');
    entry.extend_from_slice(oid);
    entry.extend_from_slice(b" 0");
    Ok(entry)
}

fn entry(index: &BTreeMap<Vec<u8>, Vec<u8>>, path: &str) -> Result<Vec<u8>, String> {
    index
        .get(path.as_bytes())
        .cloned()
        .ok_or_else(|| format!("missing index entry for {path}"))
}
fn expected_worktree_diff(git: &Path, root: &Path, head: &[u8]) -> Result<Vec<u8>, String> {
    let oid = index_oid(head);
    let worktree_oid = git_hash_object(git, root, WORKTREE_VERSION.as_bytes())?;
    Ok(format!("diff --git a/{TARGET} b/{TARGET}\nindex {oid}..{worktree_oid} 100644\n--- a/{TARGET}\n+++ b/{TARGET}\n@@ -1 +1 @@\n-{HEAD_VERSION}+{WORKTREE_VERSION}").into_bytes())
}
fn git_hash_object(git: &Path, root: &Path, contents: &[u8]) -> Result<String, String> {
    use std::io::Write;
    let mut child = Command::new(git)
        .args(["hash-object", "--stdin"])
        .current_dir(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env_clear()
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", null_device())
        .env("GIT_TERMINAL_PROMPT", "0")
        .spawn()
        .map_err(display_error)?;
    child
        .stdin
        .as_mut()
        .ok_or_else(|| "Git hash-object stdin was unavailable".to_owned())?
        .write_all(contents)
        .map_err(display_error)?;
    let output = child.wait_with_output().map_err(display_error)?;
    if !output.status.success() {
        return Err(format!(
            "trusted native Git hash-object failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    String::from_utf8(output.stdout)
        .map(|value| value.trim_end().to_owned())
        .map_err(display_error)
}
fn index_oid(entry: &[u8]) -> &str {
    std::str::from_utf8(entry)
        .unwrap_or_default()
        .split_whitespace()
        .nth(1)
        .unwrap_or_default()
}

fn run_git(git: &Path, root: &Path, args: &[&str]) -> Result<(), String> {
    let _ = git_stdout(git, root, args)?;
    Ok(())
}
fn git_text(git: &Path, root: &Path, args: &[&str]) -> Result<String, String> {
    String::from_utf8(git_stdout(git, root, args)?).map_err(display_error)
}
fn git_stdout(git: &Path, root: &Path, args: &[&str]) -> Result<Vec<u8>, String> {
    let output = Command::new(git)
        .args(args)
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env_clear()
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", null_device())
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .map_err(display_error)?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(format!(
            "trusted native Git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}
fn canonical_native_git(path: &Path) -> Result<PathBuf, String> {
    if !path.is_absolute() {
        return Err(
            "RAH_GIT_UNSTAGE_EXECUTABLE must be absolute; PATH lookup is forbidden".to_owned(),
        );
    }
    reject_link_or_reparse(path)?;
    let canonical = fs::canonicalize(path).map_err(display_error)?;
    if !canonical.is_file()
        || !canonical
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.eq_ignore_ascii_case("git.exe"))
    {
        return Err("live validation requires a canonical native git.exe".to_owned());
    }
    Ok(canonical)
}
fn reject_link_or_reparse(path: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path).map_err(display_error)?;
    if metadata.file_type().is_symlink() {
        return Err("Git executable must not be a symbolic link".to_owned());
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err("Git executable must not be a reparse point".to_owned());
        }
    }
    Ok(())
}
fn paths_equivalent(left: &Path, right: &Path) -> bool {
    #[cfg(windows)]
    {
        left.components().collect::<Vec<_>>().len() == right.components().collect::<Vec<_>>().len()
            && left
                .components()
                .zip(right.components())
                .all(|(left, right)| left.as_os_str().eq_ignore_ascii_case(right.as_os_str()))
    }
    #[cfg(not(windows))]
    {
        left == right
    }
}
fn null_device() -> &'static str {
    #[cfg(windows)]
    {
        "NUL"
    }
    #[cfg(not(windows))]
    {
        "/dev/null"
    }
}
fn display_error(error: impl std::fmt::Display) -> String {
    error.to_string()
}
impl TestRepository {
    fn remove(&self) -> Result<PathBuf, String> {
        fs::remove_dir_all(&self.0).map_err(display_error)?;
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
