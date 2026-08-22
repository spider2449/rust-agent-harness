use std::{
    collections::{BTreeMap, HashMap},
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock, Weak},
};

use async_trait::async_trait;
use futures::lock::{Mutex as AsyncMutex, MutexGuard};
use rah_protocol::{PermissionLevel, ToolContent, ToolDefinition, ToolInput, ToolName, ToolOutput};
use serde_json::json;

use crate::{
    HostArgumentPolicy, HostExecutionPolicy, Tool, ToolContext, ToolError,
    git_support::{git_environment, git_error},
    host_execute::{is_beneath, paths_equivalent},
};

/// Stable name for the single-target host-authorized Git staging capability.
pub const GIT_STAGE_TOOL_NAME: &str = "host.git.stage";
const MAX_WORKTREE_SNAPSHOT_BYTES: usize = 1024 * 1024;

/// Stages exactly one host-bound tracked regular file in one trusted repository.
///
/// `symbolic_target` and `target_path` are trusted host configuration, never
/// model input. The only model-visible input accepted by this tool is `{}`.
pub struct GitStageTool {
    policy: GitIndexMutationPolicy,
}

impl GitStageTool {
    /// Creates a staging capability with one canonical repository, native Git
    /// executable, symbolic target, and existing regular target file.
    pub fn new(
        git_executable: impl AsRef<Path>,
        repository_root: impl AsRef<Path>,
        symbolic_target: impl Into<String>,
        target_path: impl AsRef<Path>,
    ) -> Result<Self, ToolError> {
        Ok(Self {
            policy: GitIndexMutationPolicy::new(
                git_executable.as_ref(),
                repository_root.as_ref(),
                symbolic_target.into(),
                target_path.as_ref(),
                GitIndexMutation::Stage,
            )?,
        })
    }
}

#[async_trait]
impl Tool for GitStageTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: ToolName::new(GIT_STAGE_TOOL_NAME),
            description: "Stages one host-authorized tracked file.".to_owned(),
            input_schema: json!({"type":"object","properties":{},"additionalProperties":false}),
            permission: PermissionLevel::Execute,
        }
    }

    async fn execute(
        &self,
        input: ToolInput,
        _context: ToolContext,
    ) -> Result<ToolOutput, ToolError> {
        reject_input(&input)?;
        let _lease = self.policy.acquire_lease().await;
        self.policy.execute_once(GitIndexMutation::Stage).await
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum GitIndexMutation {
    Stage,
    Unstage,
}

pub(crate) struct GitIndexMutationPolicy {
    root: PathBuf,
    root_identity: FileIdentity,
    dot_git_identity: FileIdentity,
    target: Target,
    track: HostExecutionPolicy,
    mutation: HostExecutionPolicy,
    head: HostExecutionPolicy,
    head_tree: HostExecutionPolicy,
    refs: HostExecutionPolicy,
    index: HostExecutionPolicy,
    lease: Arc<AsyncMutex<()>>,
}

impl GitIndexMutationPolicy {
    pub(crate) fn new(
        git: &Path,
        root: &Path,
        symbolic_target: String,
        target_path: &Path,
        mutation_kind: GitIndexMutation,
    ) -> Result<Self, ToolError> {
        if symbolic_target.is_empty() {
            return Err(git_error("symbolic target must not be empty"));
        }
        if !root.is_absolute() {
            return Err(git_error("repository root must be an absolute path"));
        }
        if !target_path.is_absolute() {
            return Err(git_error("authorized target must be an absolute path"));
        }
        reject_link(root, "repository root")?;
        let root = canonical_directory(root, "repository root")?;
        let dot_git = root.join(".git");
        reject_link(&dot_git, "repository metadata")?;
        if !dot_git.exists() {
            return Err(git_error("repository metadata is missing"));
        }
        let target = Target::capture(&root, symbolic_target, target_path)?;
        let relative = target.git_relative.clone();
        if relative.contains('\0') {
            return Err(git_error("authorized target contains NUL"));
        }
        let exact = |arguments: Vec<String>| {
            HostExecutionPolicy::new(git, HostArgumentPolicy::Exact(arguments), &root, ".")?
                .with_environment(git_environment())
        };
        Ok(Self {
            root_identity: FileIdentity::capture(&root)?,
            dot_git_identity: FileIdentity::capture(&dot_git)?,
            lease: repository_lease(&root),
            track: exact(vec![
                "--literal-pathspecs".into(),
                "ls-files".into(),
                "--error-unmatch".into(),
                "--".into(),
                relative.clone(),
            ])?,
            mutation: exact(mutation_arguments(mutation_kind, &relative))?,
            head: exact(vec!["rev-parse".into(), "HEAD".into()])?,
            head_tree: exact(vec![
                "--literal-pathspecs".into(),
                "ls-tree".into(),
                "-z".into(),
                "HEAD".into(),
                "--".into(),
                relative.clone(),
            ])?,
            refs: exact(vec![
                "for-each-ref".into(),
                "--format=%(refname)%00%(objectname)%00".into(),
            ])?,
            index: exact(vec!["ls-files".into(), "-s".into(), "-z".into()])?,
            root,
            target,
        })
    }

    pub(crate) async fn acquire_lease(&self) -> MutexGuard<'_, ()> {
        self.lease.lock().await
    }

    pub(crate) async fn execute_once(
        &self,
        mutation_kind: GitIndexMutation,
    ) -> Result<ToolOutput, ToolError> {
        let pre = self.capture_state(mutation_kind).await?;
        self.require_tracked().await?;
        self.revalidate()?;
        let process = self.mutation.execute_process(&ToolInput(json!({}))).await;
        #[cfg(test)]
        let process = test_after_stage::run(process).await;
        let post = self.capture_state(mutation_kind).await;
        Ok(self.result(pre, process, post, mutation_kind))
    }

    async fn require_tracked(&self) -> Result<(), ToolError> {
        let output = self.track.execute_process(&ToolInput(json!({}))).await?;
        if output.exit_code != Some(0) || output.timed_out || output.overflow.is_some() {
            return Err(git_error("authorized target is not tracked by Git"));
        }
        Ok(())
    }

    async fn capture_state(&self, mutation_kind: GitIndexMutation) -> Result<State, ToolError> {
        self.revalidate()?;
        let head = successful_output(&self.head).await?;
        let head_entry = match mutation_kind {
            GitIndexMutation::Stage => None,
            GitIndexMutation::Unstage => Some(parse_head_entry(
                &successful_output(&self.head_tree).await?,
                self.target.git_relative.as_bytes(),
            )?),
        };
        let refs = successful_output(&self.refs).await?;
        let index = successful_output(&self.index).await?;
        Ok(State {
            head,
            head_entry,
            refs,
            index: parse_index(&index)?,
            worktree: WorktreeSnapshot::capture(&self.root)?,
        })
    }

    fn revalidate(&self) -> Result<(), ToolError> {
        reject_link(&self.root, "repository root")?;
        let root = canonical_directory(&self.root, "repository root")?;
        let dot_git = root.join(".git");
        reject_link(&dot_git, "repository metadata")?;
        if !paths_equivalent(&root, &self.root)
            || FileIdentity::capture(&root)? != self.root_identity
        {
            return Err(git_error("repository root identity changed"));
        }
        if FileIdentity::capture(&dot_git)? != self.dot_git_identity {
            return Err(git_error("repository metadata identity changed"));
        }
        self.target.revalidate(&self.root)
    }

    fn result(
        &self,
        pre: State,
        process: Result<rah_sandbox::HostProcessOutput, ToolError>,
        post: Result<State, ToolError>,
        mutation_kind: GitIndexMutation,
    ) -> ToolOutput {
        let verification = match post {
            Ok(post) => verify(
                &pre,
                &post,
                self.target.git_relative.as_bytes(),
                mutation_kind,
            ),
            Err(error) => Verification::uncertain(error.to_string()),
        };
        let process_failed = process.as_ref().map_or(true, |output| {
            output.exit_code != Some(0) || output.timed_out || output.overflow.is_some()
        });
        let status = if verification.violation {
            "policy_violation"
        } else if verification.uncertain || (process_failed && verification.changed) {
            "uncertain"
        } else if !process_failed {
            "ok"
        } else {
            "failed_known"
        };
        ToolOutput {
            content: vec![ToolContent::Json(json!({
                "status": status,
                "target": self.target.symbolic,
                "changed": verification.changed,
                "staged": matches!(mutation_kind, GitIndexMutation::Stage) && verification.target_changed,
                "unstaged": matches!(mutation_kind, GitIndexMutation::Unstage) && verification.target_changed,
                "no_op": !verification.target_changed && status == "ok",
                "partial": status == "policy_violation" || status == "uncertain",
                "uncertain": status == "uncertain"
            }))],
            is_error: status != "ok",
        }
    }
}

async fn successful_output(policy: &HostExecutionPolicy) -> Result<Vec<u8>, ToolError> {
    let output = policy.execute_process(&ToolInput(json!({}))).await?;
    if output.exit_code != Some(0) || output.timed_out || output.overflow.is_some() {
        return Err(git_error(
            "state observation command did not complete successfully",
        ));
    }
    Ok(output.stdout)
}

struct Target {
    symbolic: String,
    path: PathBuf,
    git_relative: String,
    parent: FileIdentity,
    identity: FileIdentity,
}
impl Target {
    fn capture(root: &Path, symbolic: String, path: &Path) -> Result<Self, ToolError> {
        reject_link(path, "authorized target")?;
        let path = fs::canonicalize(path).map_err(fs_error)?;
        let metadata = fs::metadata(&path).map_err(fs_error)?;
        if !metadata.is_file() || !path.starts_with(root) {
            return Err(git_error(
                "authorized target must be an existing regular file inside repository",
            ));
        }
        let parent = path
            .parent()
            .ok_or_else(|| git_error("authorized target has no parent"))?;
        Ok(Self {
            git_relative: path
                .strip_prefix(root)
                .map_err(|error| git_error(error.to_string()))?
                .to_string_lossy()
                .replace('\\', "/"),
            parent: FileIdentity::capture(parent)?,
            identity: FileIdentity::capture(&path)?,
            symbolic,
            path,
        })
    }
    fn revalidate(&self, root: &Path) -> Result<(), ToolError> {
        reject_link(&self.path, "authorized target")?;
        let current = fs::canonicalize(&self.path).map_err(fs_error)?;
        let parent = current
            .parent()
            .ok_or_else(|| git_error("authorized target has no parent"))?;
        if !paths_equivalent(&current, &self.path)
            || !is_beneath(&current, root)
            || !fs::metadata(&current).map_err(fs_error)?.is_file()
            || FileIdentity::capture(parent)? != self.parent
            || FileIdentity::capture(&current)? != self.identity
        {
            return Err(git_error("authorized target identity changed"));
        }
        Ok(())
    }
}

struct State {
    head: Vec<u8>,
    head_entry: Option<Vec<u8>>,
    refs: Vec<u8>,
    index: BTreeMap<Vec<u8>, Vec<u8>>,
    worktree: WorktreeSnapshot,
}
struct WorktreeSnapshot(BTreeMap<PathBuf, Vec<u8>>);
impl WorktreeSnapshot {
    fn capture(root: &Path) -> Result<Self, ToolError> {
        let mut files = BTreeMap::new();
        let mut total = 0;
        capture_tree(root, root, &mut files, &mut total)?;
        Ok(Self(files))
    }
}
fn capture_tree(
    root: &Path,
    directory: &Path,
    files: &mut BTreeMap<PathBuf, Vec<u8>>,
    total: &mut usize,
) -> Result<(), ToolError> {
    for entry in fs::read_dir(directory).map_err(fs_error)? {
        let entry = entry.map_err(fs_error)?;
        let path = entry.path();
        if path.file_name().is_some_and(|name| name == ".git") {
            continue;
        }
        reject_link(&path, "worktree entry")?;
        let metadata = fs::metadata(&path).map_err(fs_error)?;
        if metadata.is_dir() {
            capture_tree(root, &path, files, total)?;
        } else if metadata.is_file() {
            let bytes = fs::read(&path).map_err(fs_error)?;
            *total = total.saturating_add(bytes.len());
            if *total > MAX_WORKTREE_SNAPSHOT_BYTES {
                return Err(git_error("worktree snapshot exceeds bounded size"));
            }
            files.insert(
                path.strip_prefix(root)
                    .map_err(|error| git_error(error.to_string()))?
                    .to_path_buf(),
                bytes,
            );
        } else {
            return Err(git_error("worktree contains unsupported entry type"));
        }
    }
    Ok(())
}
struct Verification {
    changed: bool,
    target_changed: bool,
    violation: bool,
    uncertain: bool,
}
impl Verification {
    fn uncertain(_message: String) -> Self {
        Self {
            changed: false,
            target_changed: false,
            violation: false,
            uncertain: true,
        }
    }
}
fn verify(
    pre: &State,
    post: &State,
    target: &[u8],
    mutation_kind: GitIndexMutation,
) -> Verification {
    let target = target.to_vec();
    let target_changed = pre.index.get(&target) != post.index.get(&target);
    let unrelated_equal = pre
        .index
        .iter()
        .filter(|(path, _)| *path != &target)
        .eq(post.index.iter().filter(|(path, _)| *path != &target));
    let mutation_matches_authority = match mutation_kind {
        GitIndexMutation::Stage => true,
        GitIndexMutation::Unstage => pre
            .head_entry
            .as_ref()
            .is_some_and(|head_entry| post.index.get(&target) == Some(head_entry)),
    };
    let violation = pre.head != post.head
        || pre.refs != post.refs
        || pre.worktree.0 != post.worktree.0
        || !unrelated_equal
        || !mutation_matches_authority;
    Verification {
        changed: target_changed,
        target_changed,
        violation,
        uncertain: false,
    }
}

fn mutation_arguments(mutation_kind: GitIndexMutation, relative: &str) -> Vec<String> {
    match mutation_kind {
        GitIndexMutation::Stage => vec![
            "--literal-pathspecs".into(),
            "add".into(),
            "--".into(),
            relative.into(),
        ],
        GitIndexMutation::Unstage => vec![
            "--literal-pathspecs".into(),
            "restore".into(),
            "--staged".into(),
            "--source=HEAD".into(),
            "--".into(),
            relative.into(),
        ],
    }
}

fn parse_head_entry(bytes: &[u8], target: &[u8]) -> Result<Vec<u8>, ToolError> {
    let records = bytes
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty())
        .collect::<Vec<_>>();
    let [record] = records.as_slice() else {
        return Err(git_error(
            "authorized target must have exactly one HEAD tree entry",
        ));
    };
    let Some(separator) = record.iter().position(|byte| *byte == b'\t') else {
        return Err(git_error("Git HEAD tree observation was malformed"));
    };
    if &record[separator + 1..] != target {
        return Err(git_error(
            "Git HEAD tree observation selected an unexpected target",
        ));
    }
    let fields = record[..separator]
        .split(|byte| *byte == b' ')
        .collect::<Vec<_>>();
    let [mode, kind, object] = fields.as_slice() else {
        return Err(git_error("Git HEAD tree observation was malformed"));
    };
    if *kind != b"blob" || !matches!(*mode, b"100644" | b"100755") {
        return Err(git_error(
            "authorized target must be a regular HEAD tree entry",
        ));
    }
    let mut entry = Vec::with_capacity(mode.len() + object.len() + 3);
    entry.extend_from_slice(mode);
    entry.push(b' ');
    entry.extend_from_slice(object);
    entry.extend_from_slice(b" 0");
    Ok(entry)
}
fn parse_index(bytes: &[u8]) -> Result<BTreeMap<Vec<u8>, Vec<u8>>, ToolError> {
    let mut entries = BTreeMap::new();
    for record in bytes
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty())
    {
        let Some(separator) = record.iter().position(|byte| *byte == b'\t') else {
            return Err(git_error("Git index observation was malformed"));
        };
        let (entry, path) = record.split_at(separator);
        let path = &path[1..];
        if entries.insert(path.to_vec(), entry.to_vec()).is_some() {
            return Err(git_error("Git index observation contains duplicate path"));
        }
    }
    Ok(entries)
}
pub(crate) fn reject_input(input: &ToolInput) -> Result<(), ToolError> {
    let Some(object) = input.0.as_object() else {
        return Err(ToolError::InvalidInput {
            message: "input must be an object".into(),
        });
    };
    if let Some(field) = object.keys().next() {
        return Err(ToolError::InvalidInput {
            message: format!("unknown field `{field}`"),
        });
    }
    Ok(())
}
fn canonical_directory(path: &Path, label: &str) -> Result<PathBuf, ToolError> {
    let path = fs::canonicalize(path).map_err(fs_error)?;
    if !path.is_dir() {
        return Err(git_error(format!("{label} must be an existing directory")));
    }
    Ok(path)
}
fn reject_link(path: &Path, label: &str) -> Result<(), ToolError> {
    let metadata = fs::symlink_metadata(path).map_err(fs_error)?;
    if metadata.file_type().is_symlink() {
        return Err(git_error(format!("{label} must not be a symbolic link")));
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err(git_error(format!("{label} must not be a reparse point")));
        }
    }
    Ok(())
}
fn fs_error(error: std::io::Error) -> ToolError {
    git_error(error.to_string())
}
#[derive(Clone, Debug, Eq, PartialEq)]
struct FileIdentity {
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
    #[cfg(windows)]
    creation_time: u64,
    #[cfg(windows)]
    size: u64,
    #[cfg(not(any(unix, windows)))]
    length: u64,
}
impl FileIdentity {
    fn capture(path: &Path) -> Result<Self, ToolError> {
        let metadata = fs::metadata(path).map_err(fs_error)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            Ok(Self {
                device: metadata.dev(),
                inode: metadata.ino(),
            })
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            Ok(Self {
                creation_time: metadata.creation_time(),
                size: metadata.file_size(),
            })
        }
        #[cfg(not(any(unix, windows)))]
        {
            Ok(Self {
                length: metadata.len(),
            })
        }
    }
}
fn repository_lease(root: &Path) -> Arc<AsyncMutex<()>> {
    static LEASES: OnceLock<Mutex<HashMap<String, Weak<AsyncMutex<()>>>>> = OnceLock::new();
    let key = repository_key(root);
    let mut leases = LEASES
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .expect("repository lease registry mutex poisoned");
    if let Some(lease) = leases.get(&key).and_then(Weak::upgrade) {
        return lease;
    }
    let lease = Arc::new(AsyncMutex::new(()));
    leases.insert(key, Arc::downgrade(&lease));
    lease
}
fn repository_key(root: &Path) -> String {
    #[cfg(windows)]
    {
        root.to_string_lossy().to_ascii_lowercase()
    }
    #[cfg(not(windows))]
    {
        root.to_string_lossy().into_owned()
    }
}

// This seam exists only in unit-test builds. It runs after the real, fixed
// native Git invocation and before post-state observation, so it cannot alter
// executable selection, argv, cwd, or repository authority.
#[cfg(test)]
mod test_after_stage {
    use std::{
        path::PathBuf,
        process::Command,
        sync::{Arc, Mutex, OnceLock},
    };

    use rah_sandbox::HostProcessOutput;
    use tokio::sync::{Notify, OwnedSemaphorePermit, Semaphore};

    use crate::ToolError;

    pub(super) enum Hook {
        MutateUnrelatedIndex {
            git: PathBuf,
            root: PathBuf,
        },
        LoseProcessResult,
        TimeoutAfterMutation,
        BlockAfterMutation {
            entered: Arc<Notify>,
            release: Arc<Notify>,
        },
    }

    struct InstalledHook {
        hook: Hook,
        _permit: OwnedSemaphorePermit,
    }

    static HOOK: OnceLock<Mutex<Option<InstalledHook>>> = OnceLock::new();
    static SERIAL: OnceLock<Arc<Semaphore>> = OnceLock::new();

    pub(super) async fn install(hook: Hook) {
        let permit = SERIAL
            .get_or_init(|| Arc::new(Semaphore::new(1)))
            .clone()
            .acquire_owned()
            .await
            .expect("Git stage test hook semaphore must stay open");
        let mut slot = HOOK
            .get_or_init(|| Mutex::new(None))
            .lock()
            .expect("Git stage test hook mutex poisoned");
        assert!(
            slot.replace(InstalledHook {
                hook,
                _permit: permit,
            })
            .is_none(),
            "Git stage test hook already installed"
        );
    }

    pub(super) async fn run(
        mut process: Result<HostProcessOutput, ToolError>,
    ) -> Result<HostProcessOutput, ToolError> {
        let installed = HOOK
            .get_or_init(|| Mutex::new(None))
            .lock()
            .expect("Git stage test hook mutex poisoned")
            .take();
        match installed {
            Some(InstalledHook {
                hook: Hook::MutateUnrelatedIndex { git, root },
                _permit,
            }) => {
                let status = Command::new(git)
                    .args(["add", "--", "other.txt"])
                    .current_dir(root)
                    .status()
                    .expect("test hook should invoke native Git");
                assert!(
                    status.success(),
                    "test hook should mutate unrelated index entry"
                );
            }
            Some(InstalledHook {
                hook: Hook::LoseProcessResult,
                _permit,
            }) => {
                process = Err(ToolError::Execution {
                    message: "deterministic lost process result after Git mutation".to_owned(),
                });
            }
            Some(InstalledHook {
                hook: Hook::TimeoutAfterMutation,
                _permit,
            }) => {
                process = Ok(HostProcessOutput {
                    stdout: Vec::new(),
                    stderr: Vec::new(),
                    exit_code: None,
                    timed_out: true,
                    overflow: None,
                    termination_attempted: true,
                });
            }
            Some(InstalledHook {
                hook: Hook::BlockAfterMutation { entered, release },
                _permit,
            }) => {
                entered.notify_one();
                release.notified().await;
            }
            None => {}
        }
        process
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        fs,
        path::{Path, PathBuf},
        process::Command,
        sync::{
            Arc,
            atomic::{AtomicU64, Ordering},
        },
    };

    use rah_protocol::{ToolContent, ToolInput};
    use serde_json::json;
    use tokio::sync::Notify;

    use crate::GitUnstageTool;

    use super::{
        GitIndexMutation, GitStageTool, State, Tool, ToolContext, WorktreeSnapshot,
        test_after_stage, verify,
    };

    static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn repository() -> (Self, PathBuf, PathBuf) {
            let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
            let base = std::env::temp_dir()
                .join(format!("rah-git-stage-unit-{}-{id}", std::process::id()));
            let _ = fs::remove_dir_all(&base);
            fs::create_dir(&base).unwrap();
            let git = native_git();
            let root = base.join("repository");
            fs::create_dir(&root).unwrap();
            run_git(&git, &root, &["init", "--quiet"]);
            fs::write(root.join("target.txt"), "initial\n").unwrap();
            fs::write(root.join("other.txt"), "other\n").unwrap();
            run_git(&git, &root, &["add", "--", "target.txt", "other.txt"]);
            run_git(
                &git,
                &root,
                &[
                    "-c",
                    "user.name=RAH",
                    "-c",
                    "user.email=rah@example.invalid",
                    "commit",
                    "--quiet",
                    "-m",
                    "initial",
                ],
            );
            (Self(base), git, root)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[cfg(windows)]
    fn native_git() -> PathBuf {
        let output = Command::new("where.exe").arg("git.exe").output().unwrap();
        fs::canonicalize(
            String::from_utf8(output.stdout)
                .unwrap()
                .lines()
                .next()
                .unwrap(),
        )
        .unwrap()
    }

    #[cfg(not(windows))]
    fn native_git() -> PathBuf {
        let output = Command::new("which").arg("git").output().unwrap();
        fs::canonicalize(String::from_utf8(output.stdout).unwrap().trim()).unwrap()
    }

    fn run_git(git: &Path, root: &Path, args: &[&str]) {
        assert!(
            Command::new(git)
                .args(args)
                .current_dir(root)
                .status()
                .unwrap()
                .success()
        );
    }

    fn content(output: &rah_protocol::ToolOutput) -> &serde_json::Value {
        let [ToolContent::Json(content)] = output.content.as_slice() else {
            panic!("expected JSON result")
        };
        content
    }

    #[test]
    fn verifier_detects_an_unrelated_index_mutation() {
        let mut pre_index = BTreeMap::new();
        pre_index.insert(b"target.txt".to_vec(), b"target-before".to_vec());
        pre_index.insert(b"other.txt".to_vec(), b"other-before".to_vec());
        let mut post_index = pre_index.clone();
        post_index.insert(b"target.txt".to_vec(), b"target-after".to_vec());
        post_index.insert(b"other.txt".to_vec(), b"other-after".to_vec());
        let pre = State {
            head: b"head".to_vec(),
            head_entry: Some(b"100644 object 0".to_vec()),
            refs: b"refs".to_vec(),
            index: pre_index,
            worktree: WorktreeSnapshot(BTreeMap::new()),
        };
        let post = State {
            head: b"head".to_vec(),
            head_entry: Some(b"100644 object 0".to_vec()),
            refs: b"refs".to_vec(),
            index: post_index,
            worktree: WorktreeSnapshot(BTreeMap::new()),
        };
        assert!(verify(&pre, &post, b"target.txt", GitIndexMutation::Stage).violation);
    }

    #[tokio::test]
    async fn full_tool_path_detects_unrelated_index_mutation_and_lost_result_conservatively() {
        let (_base, git, root) = TestDirectory::repository();
        fs::write(root.join("target.txt"), "target changed\n").unwrap();
        fs::write(root.join("other.txt"), "other changed\n").unwrap();
        let tool =
            GitStageTool::new(&git, &root, "release-artifact", root.join("target.txt")).unwrap();
        test_after_stage::install(test_after_stage::Hook::MutateUnrelatedIndex {
            git: git.clone(),
            root: root.clone(),
        })
        .await;
        let output = tool
            .execute(ToolInput(json!({})), ToolContext::default())
            .await
            .unwrap();
        assert!(output.is_error);
        assert_eq!(content(&output)["status"], "policy_violation");
        assert_eq!(content(&output)["partial"], true);

        let (_base, git, root) = TestDirectory::repository();
        fs::write(root.join("target.txt"), "target changed\n").unwrap();
        let tool =
            GitStageTool::new(&git, &root, "release-artifact", root.join("target.txt")).unwrap();
        test_after_stage::install(test_after_stage::Hook::LoseProcessResult).await;
        let output = tool
            .execute(ToolInput(json!({})), ToolContext::default())
            .await
            .unwrap();
        assert!(output.is_error);
        assert_eq!(content(&output)["status"], "uncertain");
        assert_eq!(content(&output)["staged"], true);
        assert_eq!(content(&output)["partial"], true);
        assert_eq!(content(&output)["uncertain"], true);

        let (_base, git, root) = TestDirectory::repository();
        fs::write(root.join("target.txt"), "target changed\n").unwrap();
        let tool =
            GitStageTool::new(&git, &root, "release-artifact", root.join("target.txt")).unwrap();
        test_after_stage::install(test_after_stage::Hook::TimeoutAfterMutation).await;
        let output = tool
            .execute(ToolInput(json!({})), ToolContext::default())
            .await
            .unwrap();
        assert!(output.is_error);
        assert_eq!(content(&output)["status"], "uncertain");
        assert_eq!(content(&output)["staged"], true);
        assert_eq!(content(&output)["partial"], true);
        assert_eq!(content(&output)["uncertain"], true);
    }

    #[tokio::test]
    async fn abort_after_real_git_mutation_returns_no_normal_result_and_never_replays() {
        let (_base, git, root) = TestDirectory::repository();
        fs::write(root.join("target.txt"), "target changed\n").unwrap();
        let tool = Arc::new(
            GitStageTool::new(&git, &root, "release-artifact", root.join("target.txt")).unwrap(),
        );
        let entered = Arc::new(Notify::new());
        let release = Arc::new(Notify::new());
        test_after_stage::install(test_after_stage::Hook::BlockAfterMutation {
            entered: entered.clone(),
            release,
        })
        .await;
        let task = tokio::spawn({
            let tool = tool.clone();
            async move {
                tool.execute(ToolInput(json!({})), ToolContext::default())
                    .await
            }
        });
        entered.notified().await;
        task.abort();
        assert!(
            task.await.is_err(),
            "aborted mutation must not emit a normal result"
        );
        assert!(
            Command::new(&git)
                .args(["diff-files", "--quiet", "--", "target.txt"])
                .current_dir(&root)
                .status()
                .unwrap()
                .success(),
            "the real Git mutation happened before cancellation"
        );
    }

    #[tokio::test]
    async fn unstage_detects_extra_index_changes_and_treats_lost_results_or_abort_as_no_replay() {
        let (_base, git, root) = TestDirectory::repository();
        fs::write(root.join("target.txt"), "staged target\n").unwrap();
        fs::write(root.join("other.txt"), "staged other\n").unwrap();
        run_git(&git, &root, &["add", "--", "target.txt"]);
        let tool =
            GitUnstageTool::new(&git, &root, "release-artifact", root.join("target.txt")).unwrap();
        test_after_stage::install(test_after_stage::Hook::MutateUnrelatedIndex {
            git: git.clone(),
            root: root.clone(),
        })
        .await;
        let output = tool
            .execute(ToolInput(json!({})), ToolContext::default())
            .await
            .unwrap();
        assert!(output.is_error);
        assert_eq!(content(&output)["status"], "policy_violation");

        let (_base, git, root) = TestDirectory::repository();
        fs::write(root.join("target.txt"), "staged target\n").unwrap();
        run_git(&git, &root, &["add", "--", "target.txt"]);
        let tool =
            GitUnstageTool::new(&git, &root, "release-artifact", root.join("target.txt")).unwrap();
        test_after_stage::install(test_after_stage::Hook::LoseProcessResult).await;
        let output = tool
            .execute(ToolInput(json!({})), ToolContext::default())
            .await
            .unwrap();
        assert!(output.is_error);
        assert_eq!(content(&output)["status"], "uncertain");
        assert_eq!(content(&output)["unstaged"], true);

        let (_base, git, root) = TestDirectory::repository();
        fs::write(root.join("target.txt"), "staged target\n").unwrap();
        run_git(&git, &root, &["add", "--", "target.txt"]);
        let tool =
            GitUnstageTool::new(&git, &root, "release-artifact", root.join("target.txt")).unwrap();
        test_after_stage::install(test_after_stage::Hook::TimeoutAfterMutation).await;
        let output = tool
            .execute(ToolInput(json!({})), ToolContext::default())
            .await
            .unwrap();
        assert!(output.is_error);
        assert_eq!(content(&output)["status"], "uncertain");
        assert_eq!(content(&output)["unstaged"], true);

        let (_base, git, root) = TestDirectory::repository();
        fs::write(root.join("target.txt"), "staged target\n").unwrap();
        run_git(&git, &root, &["add", "--", "target.txt"]);
        let tool = Arc::new(
            GitUnstageTool::new(&git, &root, "release-artifact", root.join("target.txt")).unwrap(),
        );
        let entered = Arc::new(Notify::new());
        let release = Arc::new(Notify::new());
        test_after_stage::install(test_after_stage::Hook::BlockAfterMutation {
            entered: entered.clone(),
            release,
        })
        .await;
        let task = tokio::spawn({
            let tool = tool.clone();
            async move {
                tool.execute(ToolInput(json!({})), ToolContext::default())
                    .await
            }
        });
        entered.notified().await;
        task.abort();
        assert!(task.await.is_err());
        assert!(
            Command::new(&git)
                .args(["diff", "--cached", "--quiet", "--", "target.txt"])
                .current_dir(&root)
                .status()
                .unwrap()
                .success(),
            "the sole fixed unstage happened before abort"
        );
    }
}
