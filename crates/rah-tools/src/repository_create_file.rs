//! Host-authorized, exclusive creation of one new repository worktree file.

use std::{
    fs,
    path::{Component, Path, PathBuf},
    sync::Arc,
};

use async_trait::async_trait;
use futures::lock::{Mutex as AsyncMutex, MutexGuard};
use rah_protocol::{PermissionLevel, ToolContent, ToolDefinition, ToolInput, ToolName, ToolOutput};
use serde_json::{Map, Value, json};
use sha2::{Digest as _, Sha256};

use crate::{
    HostArgumentPolicy, HostExecutionPolicy, Tool, ToolContext, ToolError,
    git_stage::repository_lease,
    git_support::git_environment,
    native_repository_create::{NativeCreateError, NativeParent, create_new},
};

/// Stable name for the bounded repository file-creation capability.
pub const REPOSITORY_CREATE_FILE_TOOL_NAME: &str = "repo.create-file";
const MAX_CONTENT_BYTES: usize = 256 * 1024;
const MAX_REQUEST_BYTES: usize = 320 * 1024;
const MAX_PATH_BYTES: usize = 4096;

/// Host-configured capability for one exclusive create-new operation.
pub struct RepositoryFileCreationTool {
    policy: RepositoryFileCreationPolicy,
    #[cfg(test)]
    test_hook: Arc<TestHook>,
}

impl RepositoryFileCreationTool {
    /// Constructs the tool from host-owned Git executable and repository root.
    pub fn new(
        git_executable: impl AsRef<Path>,
        repository_root: impl AsRef<Path>,
    ) -> Result<Self, ToolError> {
        Ok(Self {
            policy: RepositoryFileCreationPolicy::new(
                git_executable.as_ref(),
                repository_root.as_ref(),
            )?,
            #[cfg(test)]
            test_hook: Arc::new(TestHook::default()),
        })
    }
}

#[async_trait]
impl Tool for RepositoryFileCreationTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: ToolName::new(REPOSITORY_CREATE_FILE_TOOL_NAME),
            description: "Creates one new UTF-8 file at a validated repository-relative path."
                .to_owned(),
            input_schema: json!({"type":"object","additionalProperties":false,"required":["path","content"],"properties":{"path":{"type":"string"},"content":{"type":"string"}}}),
            permission: PermissionLevel::Execute,
        }
    }

    async fn execute(
        &self,
        input: ToolInput,
        _context: ToolContext,
    ) -> Result<ToolOutput, ToolError> {
        let request = match CreateRequest::parse(&input) {
            Ok(value) => value,
            Err(()) => return Ok(output("invalid_target", None, None, None)),
        };
        let _lease = self.policy.acquire_lease().await;
        let pre = match self.policy.validate(&request).await {
            Ok(value) => value,
            Err(()) => return Ok(output("precondition_failed", None, None, None)),
        };
        let parent = match self.policy.revalidate(&request).await {
            Ok(value) => value,
            Err(()) => return Ok(output("precondition_failed", None, None, None)),
        };
        #[cfg(test)]
        self.test_hook.before_native_create(&pre.path);
        #[cfg(test)]
        self.test_hook.record_native_attempt();
        let fail_after = {
            #[cfg(test)]
            {
                self.test_hook.write_fail_after()
            }
            #[cfg(not(test))]
            {
                None
            }
        };
        match create_new(
            &parent,
            &request.name,
            request.content.as_bytes(),
            fail_after,
        ) {
            Ok(()) => {}
            Err(NativeCreateError::AlreadyExists) => {
                return Ok(output("create_failed_known", None, None, None));
            }
            Err(NativeCreateError::WriteFailed(_)) => {
                return Ok(output("write_failed_known", None, None, None));
            }
            Err(_) => return Ok(output("create_failed_known", None, None, None)),
        }
        #[cfg(test)]
        if self.test_hook.force_post_verification_failure() {
            return Ok(output("uncertain", None, None, None));
        }
        if self.policy.verify_post(&request, &pre).await.is_err() {
            return Ok(output("uncertain", None, None, None));
        }
        Ok(output(
            "ok",
            Some(&request.path),
            Some(request.content.len()),
            Some(sha256_hex(request.content.as_bytes())),
        ))
    }
}

#[cfg(test)]
#[derive(Default)]
struct TestHook {
    create_target_before_native: std::sync::atomic::AtomicBool,
    write_fail_after: std::sync::atomic::AtomicUsize,
    force_post_verification_failure: std::sync::atomic::AtomicBool,
    native_attempts: std::sync::atomic::AtomicUsize,
}

#[cfg(test)]
impl TestHook {
    fn before_native_create(&self, path: &Path) {
        if self
            .create_target_before_native
            .swap(false, std::sync::atomic::Ordering::SeqCst)
        {
            fs::write(path, b"external target").expect("race fixture should create target");
        }
    }

    fn write_fail_after(&self) -> Option<usize> {
        let value = self
            .write_fail_after
            .load(std::sync::atomic::Ordering::SeqCst);
        if value == 0 { None } else { Some(value - 1) }
    }

    fn record_native_attempt(&self) {
        self.native_attempts
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }

    fn force_post_verification_failure(&self) -> bool {
        self.force_post_verification_failure
            .swap(false, std::sync::atomic::Ordering::SeqCst)
    }
}

/// Private, host-owned policy. Model input cannot supply any authority here.
struct RepositoryFileCreationPolicy {
    root: PathBuf,
    git: PathBuf,
    lease: Arc<AsyncMutex<()>>,
}

impl RepositoryFileCreationPolicy {
    fn new(git: &Path, root: &Path) -> Result<Self, ToolError> {
        if !root.is_absolute() || !git.is_absolute() {
            return Err(config_error("host paths must be absolute"));
        }
        let root = fs::canonicalize(root).map_err(config_error)?;
        let git = fs::canonicalize(git).map_err(config_error)?;
        if !root.is_dir()
            || !git.is_file()
            || is_link_or_reparse(&root)
            || is_link_or_reparse(&git)
            || !root.join(".git").exists()
        {
            return Err(config_error("repository or Git identity is invalid"));
        }
        Ok(Self {
            lease: repository_lease(&root),
            root,
            git,
        })
    }
    async fn acquire_lease(&self) -> MutexGuard<'_, ()> {
        self.lease.lock().await
    }
    async fn validate(&self, request: &CreateRequest) -> Result<PreState, ()> {
        self.revalidate_root()?;
        let path = self.root.join(&request.path);
        let parent = path.parent().ok_or(())?;
        if fs::symlink_metadata(&path).is_ok() || !parent_beneath_real(&self.root, parent) {
            return Err(());
        }
        self.require_git_absent(request).await?;
        Ok(PreState {
            path,
            head: self
                .git_output(&["rev-parse", "--verify", "HEAD"])
                .await
                .map_err(|_| ())?,
            refs: self
                .git_output(&["for-each-ref", "--format=%(refname)%00%(objectname)%00"])
                .await
                .map_err(|_| ())?,
            index: self
                .git_output(&["ls-files", "-s", "-z"])
                .await
                .map_err(|_| ())?,
        })
    }
    async fn revalidate(&self, request: &CreateRequest) -> Result<NativeParent, ()> {
        self.validate(request).await?;
        let parent = request.path.parent().unwrap_or_else(|| Path::new(""));
        NativeParent::open(&self.root, parent).map_err(|_| ())
    }
    async fn require_git_absent(&self, request: &CreateRequest) -> Result<(), ()> {
        let target = request.path.to_str().ok_or(())?;
        if !self
            .git_output(&["--literal-pathspecs", "ls-tree", "-z", "HEAD", "--", target])
            .await
            .map_err(|_| ())?
            .is_empty()
        {
            return Err(());
        }
        if !self
            .git_output(&["--literal-pathspecs", "ls-files", "-s", "-z", "--", target])
            .await
            .map_err(|_| ())?
            .is_empty()
        {
            return Err(());
        }
        match self
            .git_status(&["check-ignore", "-q", "--", target])
            .await?
        {
            1 => {}
            _ => return Err(()),
        }
        // Task 084's conservative policy: the capability fails closed whenever
        // sparse checkout is active, rather than guessing materialization scope.
        if self
            .git_status(&["config", "--bool", "core.sparseCheckout"])
            .await?
            == 0
        {
            return Err(());
        }
        let mut ancestor = PathBuf::new();
        let components = request.path.components().collect::<Vec<_>>();
        for component in &components[..components.len().saturating_sub(1)] {
            ancestor.push(component.as_os_str());
            let name = ancestor.to_str().ok_or(())?;
            let entry = self
                .git_output(&["--literal-pathspecs", "ls-files", "-s", "-z", "--", name])
                .await
                .map_err(|_| ())?;
            if entry.windows(6).any(|part| part == b"160000") {
                return Err(());
            }
        }
        Ok(())
    }
    async fn verify_post(&self, request: &CreateRequest, pre: &PreState) -> Result<(), ()> {
        self.revalidate_root()?;
        let metadata = fs::symlink_metadata(&pre.path).map_err(|_| ())?;
        if !metadata.is_file()
            || is_link_or_reparse(&pre.path)
            || fs::read(&pre.path).map_err(|_| ())? != request.content.as_bytes()
        {
            return Err(());
        }
        if self
            .git_output(&["rev-parse", "--verify", "HEAD"])
            .await
            .map_err(|_| ())?
            != pre.head
            || self
                .git_output(&["for-each-ref", "--format=%(refname)%00%(objectname)%00"])
                .await
                .map_err(|_| ())?
                != pre.refs
            || self
                .git_output(&["ls-files", "-s", "-z"])
                .await
                .map_err(|_| ())?
                != pre.index
        {
            return Err(());
        }
        Ok(())
    }
    fn revalidate_root(&self) -> Result<(), ()> {
        if fs::canonicalize(&self.root).map_err(|_| ())? != self.root
            || is_link_or_reparse(&self.root)
            || !self.root.join(".git").is_dir()
        {
            Err(())
        } else {
            Ok(())
        }
    }
    async fn git_output(&self, args: &[&str]) -> Result<Vec<u8>, ToolError> {
        let process = self
            .git_process(args)?
            .execute_process(&ToolInput(json!({})))
            .await?;
        if process.exit_code == Some(0) && !process.timed_out && process.overflow.is_none() {
            Ok(process.stdout)
        } else {
            Err(ToolError::Execution {
                message: "bounded Git observation failed".to_owned(),
            })
        }
    }
    async fn git_status(&self, args: &[&str]) -> Result<i32, ()> {
        let process = self
            .git_process(args)
            .map_err(|_| ())?
            .execute_process(&ToolInput(json!({})))
            .await
            .map_err(|_| ())?;
        process.exit_code.ok_or(())
    }
    fn git_process(&self, args: &[&str]) -> Result<HostExecutionPolicy, ToolError> {
        HostExecutionPolicy::new(
            &self.git,
            HostArgumentPolicy::Exact(args.iter().map(|value| (*value).to_owned()).collect()),
            &self.root,
            ".",
        )?
        .with_environment(git_environment())
    }
}

struct PreState {
    path: PathBuf,
    head: Vec<u8>,
    refs: Vec<u8>,
    index: Vec<u8>,
}
struct CreateRequest {
    path: PathBuf,
    name: String,
    content: String,
}
impl CreateRequest {
    fn parse(input: &ToolInput) -> Result<Self, ()> {
        if serde_json::to_vec(&input.0).map_err(|_| ())?.len() > MAX_REQUEST_BYTES {
            return Err(());
        }
        let object = input.0.as_object().ok_or(())?;
        if object.len() != 2 || !object.contains_key("path") || !object.contains_key("content") {
            return Err(());
        }
        let path = object.get("path").and_then(Value::as_str).ok_or(())?;
        let content = object.get("content").and_then(Value::as_str).ok_or(())?;
        if content.len() > MAX_CONTENT_BYTES || content.contains('\0') {
            return Err(());
        }
        let path = parse_path(path)?;
        Ok(Self {
            name: path
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or(())?
                .to_owned(),
            path,
            content: content.to_owned(),
        })
    }
}
fn parse_path(value: &str) -> Result<PathBuf, ()> {
    if value.is_empty()
        || value.len() > MAX_PATH_BYTES
        || value.contains(['\0', '\\', ':'])
        || value.starts_with('/')
        || value.starts_with("//")
        || value.ends_with('/')
        || Path::new(value).is_absolute()
    {
        return Err(());
    }
    for component in value.split('/') {
        if component.is_empty()
            || matches!(component, "." | "..")
            || component.eq_ignore_ascii_case(".git")
            || reserved_windows_name(component)
        {
            return Err(());
        }
    }
    let path = PathBuf::from(value);
    if path
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        Err(())
    } else {
        Ok(path)
    }
}
fn reserved_windows_name(component: &str) -> bool {
    let stem = component
        .trim_end_matches(['.', ' '])
        .split('.')
        .next()
        .unwrap_or("");
    matches!(
        stem.to_ascii_uppercase().as_str(),
        "CON" | "PRN" | "AUX" | "NUL"
    ) || stem.len() == 4
        && matches!(&stem[..3].to_ascii_uppercase()[..], "COM" | "LPT")
        && matches!(stem.as_bytes()[3], b'1'..=b'9')
}
fn parent_beneath_real(root: &Path, parent: &Path) -> bool {
    let Ok(relative) = parent.strip_prefix(root) else {
        return false;
    };
    let mut current = root.to_path_buf();
    for component in relative.components() {
        current.push(component.as_os_str());
        if !current.is_dir() || is_link_or_reparse(&current) {
            return false;
        }
    }
    true
}
fn is_link_or_reparse(path: &Path) -> bool {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return true;
    };
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        false
    }
}
fn config_error(error: impl std::fmt::Display) -> ToolError {
    ToolError::Execution {
        message: format!("host creation authority rejected configuration: {error}"),
    }
}
fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn output(
    status: &str,
    path: Option<&Path>,
    length: Option<usize>,
    sha256: Option<String>,
) -> ToolOutput {
    let mut object = Map::new();
    object.insert("status".to_owned(), Value::String(status.to_owned()));
    if let (Some(path), Some(length), Some(sha256)) = (path, length, sha256) {
        object.insert(
            "path".to_owned(),
            Value::String(path.to_string_lossy().replace('\\', "/")),
        );
        object.insert("length".to_owned(), json!(length));
        object.insert("sha256".to_owned(), Value::String(sha256));
    }
    ToolOutput {
        content: vec![ToolContent::Json(Value::Object(object))],
        is_error: status != "ok",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        process::Command,
        sync::atomic::Ordering,
        time::{SystemTime, UNIX_EPOCH},
    };

    struct Fixture {
        root: PathBuf,
        git: PathBuf,
    }
    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "rah-create-file-fault-{}",
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir_all(root.join("src")).unwrap();
            let git = native_git();
            for args in [
                ["init", "--quiet"].as_slice(),
                ["config", "user.name", "RAH Test"].as_slice(),
                ["config", "user.email", "rah@example.invalid"].as_slice(),
            ] {
                run(&git, &root, args);
            }
            fs::write(root.join("sentinel"), b"unchanged").unwrap();
            run(&git, &root, &["add", "sentinel"]);
            run(&git, &root, &["commit", "--quiet", "-m", "base"]);
            Self { root, git }
        }
        fn snapshot(&self) -> (Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>) {
            (
                fs::read(self.root.join(".git/HEAD")).unwrap(),
                fs::read(self.root.join(".git/index")).unwrap(),
                git_stdout(
                    &self.git,
                    &self.root,
                    &["for-each-ref", "--format=%(refname)%00%(objectname)%00"],
                ),
                fs::read(self.root.join("sentinel")).unwrap(),
            )
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
    fn native_git() -> PathBuf {
        #[cfg(windows)]
        let command = ("where.exe", "git.exe");
        #[cfg(not(windows))]
        let command = ("which", "git");
        let output = Command::new(command.0).arg(command.1).output().unwrap();
        fs::canonicalize(
            String::from_utf8(output.stdout)
                .unwrap()
                .lines()
                .next()
                .unwrap(),
        )
        .unwrap()
    }
    fn run(git: &Path, root: &Path, args: &[&str]) {
        assert!(
            Command::new(git)
                .args(args)
                .current_dir(root)
                .status()
                .unwrap()
                .success(),
            "{args:?}"
        );
    }
    fn git_stdout(git: &Path, root: &Path, args: &[&str]) -> Vec<u8> {
        let output = Command::new(git)
            .args(args)
            .current_dir(root)
            .output()
            .unwrap();
        assert!(output.status.success(), "{args:?}");
        output.stdout
    }
    fn execute(tool: &RepositoryFileCreationTool, input: Value) -> Value {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let output = runtime
            .block_on(tool.execute(ToolInput(input), ToolContext::default()))
            .unwrap();
        let [ToolContent::Json(value)] = output.content.as_slice() else {
            panic!("JSON result required")
        };
        value.clone()
    }
    fn assert_snapshot(fixture: &Fixture, snapshot: &(Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>)) {
        assert_eq!(
            fs::read(fixture.root.join(".git/HEAD")).unwrap(),
            snapshot.0
        );
        assert_eq!(
            fs::read(fixture.root.join(".git/index")).unwrap(),
            snapshot.1
        );
        assert_eq!(
            git_stdout(
                &fixture.git,
                &fixture.root,
                &["for-each-ref", "--format=%(refname)%00%(objectname)%00"]
            ),
            snapshot.2
        );
        assert_eq!(fs::read(fixture.root.join("sentinel")).unwrap(), snapshot.3);
    }

    #[test]
    fn target_race_uses_one_exclusive_create_without_overwrite_or_retry() {
        let fixture = Fixture::new();
        let tool = RepositoryFileCreationTool::new(&fixture.git, &fixture.root).unwrap();
        let snapshot = fixture.snapshot();
        tool.test_hook
            .create_target_before_native
            .store(true, Ordering::SeqCst);
        let value = execute(&tool, json!({"path":"src/race.rs","content":"wanted"}));
        assert_eq!(value["status"], "create_failed_known");
        assert_eq!(
            fs::read(fixture.root.join("src/race.rs")).unwrap(),
            b"external target"
        );
        assert_eq!(tool.test_hook.native_attempts.load(Ordering::SeqCst), 1);
        assert_snapshot(&fixture, &snapshot);
    }

    #[test]
    fn partial_write_is_retained_and_reported_once() {
        let fixture = Fixture::new();
        let tool = RepositoryFileCreationTool::new(&fixture.git, &fixture.root).unwrap();
        let snapshot = fixture.snapshot();
        tool.test_hook.write_fail_after.store(3, Ordering::SeqCst);
        let value = execute(&tool, json!({"path":"src/partial.rs","content":"abcdef"}));
        assert_eq!(value["status"], "write_failed_known");
        assert_eq!(
            fs::read(fixture.root.join("src/partial.rs")).unwrap(),
            b"ab"
        );
        assert_eq!(tool.test_hook.native_attempts.load(Ordering::SeqCst), 1);
        assert_snapshot(&fixture, &snapshot);
    }

    #[test]
    fn lost_post_create_certification_is_uncertain_without_replay_or_cleanup() {
        let fixture = Fixture::new();
        let tool = RepositoryFileCreationTool::new(&fixture.git, &fixture.root).unwrap();
        let snapshot = fixture.snapshot();
        tool.test_hook
            .force_post_verification_failure
            .store(true, Ordering::SeqCst);
        let value = execute(
            &tool,
            json!({"path":"src/uncertain.rs","content":"committed"}),
        );
        assert_eq!(value["status"], "uncertain");
        assert_eq!(
            fs::read(fixture.root.join("src/uncertain.rs")).unwrap(),
            b"committed"
        );
        assert_eq!(tool.test_hook.native_attempts.load(Ordering::SeqCst), 1);
        assert_snapshot(&fixture, &snapshot);
    }
}
