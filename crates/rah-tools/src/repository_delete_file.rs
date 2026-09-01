//! Host-authorized deletion of one clean HEAD-tracked repository file.

use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use async_trait::async_trait;
use futures::lock::Mutex as AsyncMutex;
use rah_protocol::{PermissionLevel, ToolContent, ToolDefinition, ToolInput, ToolName, ToolOutput};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};

use crate::{
    HostArgumentPolicy, HostExecutionPolicy, Tool, ToolContext, ToolError,
    git_support::git_environment,
    host_execute::paths_equivalent,
    repository_worktree_patch::{
        FileIdentity, parse_logical_path, reject_link_or_reparse, reject_reparse_ancestry,
        reject_unsupported_file_attributes, validate_directory_path, validate_existing_target,
    },
};

/// Stable name for the bounded repository file-deletion capability.
pub const REPOSITORY_DELETE_FILE_TOOL_NAME: &str = "repo.delete-file";
const MAX_PATH_BYTES: usize = 1024;
const MAX_FILE_BYTES: usize = 1024 * 1024;

/// Host-constructed authority for exactly one protected repository file.
pub struct RepositoryFileDeletionTool {
    policy: Arc<RepositoryFileDeletionPolicy>,
}

/// Opaque host-created authority for one bounded repository file deletion.
#[derive(Clone)]
pub struct RepositoryFileDeletionAuthority {
    policy: Arc<RepositoryFileDeletionPolicy>,
}

impl RepositoryFileDeletionAuthority {
    /// Constructs deletion authority from host-selected repository identities.
    pub fn new(
        git_executable: impl AsRef<Path>,
        repository_root: impl AsRef<Path>,
    ) -> Result<Self, ToolError> {
        Ok(Self {
            policy: Arc::new(RepositoryFileDeletionPolicy::new(
                git_executable.as_ref(),
                repository_root.as_ref(),
            )?),
        })
    }

    /// Tests whether this host authority is bound to the selected resources.
    pub fn matches_resources(&self, git_executable: &Path, repository_root: &Path) -> bool {
        self.policy
            .matches_resources(git_executable, repository_root)
    }
}

impl RepositoryFileDeletionTool {
    /// Constructs deletion authority from host-selected identities.
    pub fn new(
        git_executable: impl AsRef<Path>,
        repository_root: impl AsRef<Path>,
    ) -> Result<Self, ToolError> {
        Ok(Self::from_authority(RepositoryFileDeletionAuthority::new(
            git_executable,
            repository_root,
        )?))
    }

    /// Constructs a tool from an authority already created by the trusted host.
    #[must_use]
    pub fn from_authority(authority: RepositoryFileDeletionAuthority) -> Self {
        Self {
            policy: authority.policy,
        }
    }
}

#[async_trait]
impl Tool for RepositoryFileDeletionTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: ToolName::new(REPOSITORY_DELETE_FILE_TOOL_NAME),
            description: "Deletes one clean HEAD-tracked repository-relative file.".to_owned(),
            input_schema: json!({"type":"object","properties":{"path":{"type":"string","minLength":1,"maxLength":MAX_PATH_BYTES},"expected_file_sha256":{"type":"string","pattern":"^[0-9a-f]{64}$"},"expected_file_byte_length":{"type":"integer","minimum":0,"maximum":MAX_FILE_BYTES}},"required":["path","expected_file_sha256","expected_file_byte_length"],"additionalProperties":false}),
            permission: PermissionLevel::Execute,
        }
    }

    async fn execute(
        &self,
        input: ToolInput,
        _context: ToolContext,
    ) -> Result<ToolOutput, ToolError> {
        let request = match DeleteRequest::parse(&input) {
            Ok(request) => request,
            Err(()) => return Ok(result("invalid_input", None, false)),
        };
        let _lease = self.policy.lease.lock().await;
        let pre = match self.policy.capture(&request).await {
            Ok(pre) => pre,
            Err(()) => return Ok(result("precondition_failed", Some(&request.path), false)),
        };
        #[cfg(test)]
        if self
            .policy
            .test_modify_before_delete
            .swap(false, std::sync::atomic::Ordering::SeqCst)
        {
            fs::write(&pre.path, b"changed externally").expect("test mutation should work");
        }
        if self.policy.revalidate(&request, &pre).await.is_err() {
            return Ok(result("precondition_failed", Some(&request.path), false));
        }
        #[cfg(test)]
        self.policy
            .delete_attempts
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let native = delete_once(&pre.path);
        if native.is_err() {
            let intact = self.policy.intact(&request, &pre).await;
            return Ok(result(
                if intact {
                    "known_no_effect"
                } else {
                    "uncertain"
                },
                Some(&request.path),
                !intact,
            ));
        }
        #[cfg(test)]
        if self
            .policy
            .force_uncertain
            .swap(false, std::sync::atomic::Ordering::SeqCst)
        {
            return Ok(result("uncertain", None, true));
        }
        if self.policy.deleted_verified(&pre).await.is_ok() {
            Ok(result("deleted_verified", Some(&request.path), false))
        } else {
            Ok(result("uncertain", None, true))
        }
    }
}

/// Separate host-owned deletion policy; no other mutation policy grants it.
struct RepositoryFileDeletionPolicy {
    git: PathBuf,
    root: PathBuf,
    root_identity: FileIdentity,
    git_identity: FileIdentity,
    dot_git_identity: FileIdentity,
    lease: Arc<AsyncMutex<()>>,
    #[cfg(test)]
    test_modify_before_delete: std::sync::atomic::AtomicBool,
    #[cfg(test)]
    force_uncertain: std::sync::atomic::AtomicBool,
    #[cfg(test)]
    delete_attempts: std::sync::atomic::AtomicUsize,
}

impl RepositoryFileDeletionPolicy {
    fn matches_resources(&self, git: &Path, root: &Path) -> bool {
        fs::canonicalize(git).ok().as_deref() == Some(self.git.as_path())
            && fs::canonicalize(root).ok().as_deref() == Some(self.root.as_path())
    }

    fn new(git: &Path, root: &Path) -> Result<Self, ToolError> {
        if !root.is_absolute() || !git.is_absolute() {
            return Err(policy_error("host identities must be absolute"));
        }
        reject_reparse_ancestry(root, "repository root")?;
        let root = fs::canonicalize(root).map_err(fs_error)?;
        validate_directory_path(&root, &root, "repository root")?;
        let dot_git = root.join(".git");
        reject_link_or_reparse(&dot_git, "repository metadata")?;
        if !fs::metadata(&dot_git).map_err(fs_error)?.is_dir() {
            return Err(policy_error("linked worktrees are unsupported"));
        }
        let git = fs::canonicalize(git).map_err(fs_error)?;
        if !fs::metadata(&git).map_err(fs_error)?.is_file() {
            return Err(policy_error("Git identity is invalid"));
        }
        reject_link_or_reparse(&git, "Git executable")?;
        let lease = crate::git_stage::repository_lease(&root);
        Ok(Self {
            root_identity: FileIdentity::capture(&root)?,
            git_identity: FileIdentity::capture(&git)?,
            dot_git_identity: FileIdentity::capture(&dot_git)?,
            git,
            root,
            lease,
            #[cfg(test)]
            test_modify_before_delete: Default::default(),
            #[cfg(test)]
            force_uncertain: Default::default(),
            #[cfg(test)]
            delete_attempts: Default::default(),
        })
    }

    async fn capture(&self, request: &DeleteRequest) -> Result<Preimage, ()> {
        self.repository_ok().map_err(|_| ())?;
        let path = validate_existing_target(&self.root, &request.path).map_err(|_| ())?;
        let metadata = fs::metadata(&path).map_err(|_| ())?;
        reject_unsupported_file_attributes(&metadata).map_err(|_| ())?;
        let identity = FileIdentity::capture(&path).map_err(|_| ())?;
        if identity.link_count != 1 {
            return Err(());
        }
        let bytes = fs::read(&path).map_err(|_| ())?;
        if bytes.len() > MAX_FILE_BYTES
            || bytes.len() != request.length
            || sha256(&bytes) != request.sha256
        {
            return Err(());
        }
        let git = self.git_state(&request.path).await?;
        if git.blob != bytes {
            return Err(());
        }
        Ok(Preimage {
            path,
            git_path: request.path.clone(),
            identity,
            bytes,
            git,
            index: fs::read(self.root.join(".git/index")).map_err(|_| ())?,
        })
    }

    async fn revalidate(&self, request: &DeleteRequest, pre: &Preimage) -> Result<(), ()> {
        let current = self.capture(request).await?;
        if !paths_equivalent(&current.path, &pre.path)
            || current.identity != pre.identity
            || current.git != pre.git
            || current.index != pre.index
            || current.bytes != pre.bytes
        {
            return Err(());
        }
        Ok(())
    }

    async fn intact(&self, request: &DeleteRequest, pre: &Preimage) -> bool {
        self.capture(request).await.is_ok() && fs::metadata(&pre.path).is_ok()
    }

    async fn deleted_verified(&self, pre: &Preimage) -> Result<(), ()> {
        self.repository_ok().map_err(|_| ())?;
        if fs::symlink_metadata(&pre.path).is_ok()
            || self.git_state(Path::new(&pre.git_path)).await?.blob != pre.bytes
        {
            return Err(());
        }
        if fs::read(self.root.join(".git/index")).map_err(|_| ())? != pre.index
            || self.git_state(Path::new(&pre.git_path)).await?.fingerprint != pre.git.fingerprint
        {
            return Err(());
        }
        Ok(())
    }

    fn repository_ok(&self) -> Result<(), ToolError> {
        reject_reparse_ancestry(&self.root, "repository root")?;
        let current = fs::canonicalize(&self.root).map_err(fs_error)?;
        let dot_git = current.join(".git");
        if !paths_equivalent(&current, &self.root)
            || FileIdentity::capture(&current)? != self.root_identity
            || FileIdentity::capture(&dot_git)? != self.dot_git_identity
            || reject_link_or_reparse(&self.git, "Git executable").is_err()
            || fs::metadata(&self.git).map_err(fs_error)?.is_dir()
            || FileIdentity::capture(&self.git)? != self.git_identity
        {
            return Err(policy_error("repository identity changed"));
        }
        Ok(())
    }

    async fn git_state(&self, path: &Path) -> Result<GitState, ()> {
        let target = path.to_string_lossy().replace('\\', "/");
        let head = self
            .git_output(vec!["rev-parse", "--verify", "HEAD"])
            .await?;
        let branch = self
            .git_output(vec!["symbolic-ref", "--quiet", "--short", "HEAD"])
            .await?;
        let tree = self
            .git_output(vec![
                "--literal-pathspecs",
                "ls-tree",
                "-z",
                "HEAD",
                "--",
                &target,
            ])
            .await?;
        let index = self
            .git_output(vec![
                "--literal-pathspecs",
                "ls-files",
                "-s",
                "-z",
                "--",
                &target,
            ])
            .await?;
        if !tree.starts_with(b"100644 blob ") && !tree.starts_with(b"100755 blob ")
            || tree != index_to_tree(&index, &target).ok_or(())?
        {
            return Err(());
        }
        let tags = self
            .git_output(vec![
                "--literal-pathspecs",
                "ls-files",
                "-v",
                "-z",
                "--",
                &target,
            ])
            .await?;
        if !tags.starts_with(b"H ") {
            return Err(());
        }
        let blob = self
            .git_output(vec!["show", &format!("HEAD:{target}")])
            .await?;
        let refs = self
            .git_output(vec![
                "for-each-ref",
                "--format=%(refname)%00%(objectname)%00",
            ])
            .await?;
        Ok(GitState {
            blob,
            fingerprint: [head, branch, tree, index, refs].concat(),
        })
    }
    async fn git_output(&self, args: Vec<&str>) -> Result<Vec<u8>, ()> {
        let process = HostExecutionPolicy::new(
            &self.git,
            HostArgumentPolicy::Exact(args.into_iter().map(str::to_owned).collect()),
            &self.root,
            ".",
        )
        .map_err(|_| ())?
        .with_environment(git_environment())
        .map_err(|_| ())?
        .execute_process(&ToolInput(json!({})))
        .await
        .map_err(|_| ())?;
        if process.exit_code != Some(0) || process.timed_out || process.overflow.is_some() {
            Err(())
        } else {
            Ok(process.stdout)
        }
    }
}

struct Preimage {
    path: PathBuf,
    git_path: PathBuf,
    identity: FileIdentity,
    bytes: Vec<u8>,
    git: GitState,
    index: Vec<u8>,
}
#[derive(Clone, PartialEq, Eq)]
struct GitState {
    blob: Vec<u8>,
    fingerprint: Vec<u8>,
}
struct DeleteRequest {
    path: PathBuf,
    sha256: String,
    length: usize,
}
impl DeleteRequest {
    fn parse(input: &ToolInput) -> Result<Self, ()> {
        let object = input.0.as_object().ok_or(())?;
        if object.len() != 3 {
            return Err(());
        }
        let path = parse_logical_path(
            object.get("path").and_then(Value::as_str).ok_or(())?,
            MAX_PATH_BYTES,
        )
        .map_err(|_| ())?;
        let sha256 = object
            .get("expected_file_sha256")
            .and_then(Value::as_str)
            .ok_or(())?;
        if sha256.len() != 64
            || !sha256
                .bytes()
                .all(|b| b.is_ascii_digit() || matches!(b, b'a'..=b'f'))
        {
            return Err(());
        }
        let length = object
            .get("expected_file_byte_length")
            .and_then(Value::as_u64)
            .and_then(|v| usize::try_from(v).ok())
            .ok_or(())?;
        if length > MAX_FILE_BYTES {
            return Err(());
        }
        Ok(Self {
            path,
            sha256: sha256.to_owned(),
            length,
        })
    }
}

fn index_to_tree(index: &[u8], target: &str) -> Option<Vec<u8>> {
    let record = index.strip_suffix(&[0])?;
    let tab = record.iter().position(|b| *b == b'\t')?;
    if &record[tab + 1..] != target.as_bytes() {
        return None;
    }
    let fields = record[..tab].split(|b| *b == b' ').collect::<Vec<_>>();
    if fields.len() != 3 || fields[2] != b"0" {
        return None;
    }
    Some(
        [
            fields[0],
            b" blob ",
            fields[1],
            b"\t",
            target.as_bytes(),
            &[0],
        ]
        .concat(),
    )
}
fn delete_once(path: &Path) -> Result<(), std::io::Error> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        let wide = path
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect::<Vec<_>>();
        let ok = unsafe { windows_sys::Win32::Storage::FileSystem::DeleteFileW(wide.as_ptr()) };
        if ok == 0 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
    #[cfg(not(windows))]
    {
        fs::remove_file(path)
    }
}
fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn result(status: &str, path: Option<&Path>, uncertain: bool) -> ToolOutput {
    let mut value = json!({"status":status,"uncertain":uncertain});
    if let Some(path) = path {
        value["path"] = Value::String(path.to_string_lossy().replace('\\', "/"));
    }
    ToolOutput {
        content: vec![ToolContent::Json(value)],
        is_error: status != "deleted_verified",
    }
}
fn policy_error(message: impl Into<String>) -> ToolError {
    ToolError::Execution {
        message: format!(
            "repository file deletion policy rejected capability: {}",
            message.into()
        ),
    }
}
fn fs_error(error: impl std::fmt::Display) -> ToolError {
    policy_error(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        process::Command,
        time::{SystemTime, UNIX_EPOCH},
    };

    struct Fixture {
        root: PathBuf,
        git: PathBuf,
    }
    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "rah-delete-{}",
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir_all(&root).unwrap();
            let git = git_path();
            for args in [
                vec!["init", "--quiet"],
                vec!["config", "user.name", "RAH Test"],
                vec!["config", "user.email", "rah@example.invalid"],
            ] {
                run(&git, &root, &args);
            }
            fs::write(root.join("target.txt"), b"protected\n").unwrap();
            fs::write(root.join("other.txt"), b"untouched\n").unwrap();
            run(&git, &root, &["add", "."]);
            run(&git, &root, &["commit", "--quiet", "-m", "base"]);
            Self { root, git }
        }
        fn request(&self) -> Value {
            json!({"path":"target.txt","expected_file_sha256":sha256(b"protected\n"),"expected_file_byte_length":10})
        }
        fn index(&self) -> Vec<u8> {
            fs::read(self.root.join(".git/index")).unwrap()
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
    fn git_path() -> PathBuf {
        #[cfg(windows)]
        let command = ("where.exe", "git.exe");
        #[cfg(not(windows))]
        let command = ("which", "git");
        fs::canonicalize(
            String::from_utf8(
                Command::new(command.0)
                    .arg(command.1)
                    .output()
                    .unwrap()
                    .stdout,
            )
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
    fn execute(tool: &RepositoryFileDeletionTool, value: Value) -> Value {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let output = runtime
            .block_on(tool.execute(ToolInput(value), ToolContext::default()))
            .unwrap();
        let [ToolContent::Json(value)] = output.content.as_slice() else {
            panic!("JSON result required")
        };
        value.clone()
    }

    #[test]
    fn deletes_one_clean_tracked_file_without_staging_or_collateral_effect() {
        let fixture = Fixture::new();
        let tool = RepositoryFileDeletionTool::new(&fixture.git, &fixture.root).unwrap();
        let index = fixture.index();
        assert_eq!(
            execute(&tool, fixture.request())["status"],
            "deleted_verified"
        );
        assert!(!fixture.root.join("target.txt").exists());
        assert_eq!(fixture.index(), index);
        assert_eq!(
            fs::read(fixture.root.join("other.txt")).unwrap(),
            b"untouched\n"
        );
        assert_eq!(
            tool.policy
                .delete_attempts
                .load(std::sync::atomic::Ordering::SeqCst),
            1
        );
    }

    #[test]
    fn execute_permission_and_other_states_do_not_imply_deletion() {
        let fixture = Fixture::new();
        let tool = RepositoryFileDeletionTool::new(&fixture.git, &fixture.root).unwrap();
        assert_eq!(tool.definition().permission, PermissionLevel::Execute);
        let mut request = fixture.request();
        request["expected_file_sha256"] = Value::String("0".repeat(64));
        assert_eq!(execute(&tool, request)["status"], "precondition_failed");
        assert!(fixture.root.join("target.txt").exists());
        for path in ["missing.txt", "other.txt", ".git"] {
            let value = json!({"path":path,"expected_file_sha256":sha256(b"protected\n"),"expected_file_byte_length":10});
            assert!(matches!(
                execute(&tool, value)["status"].as_str(),
                Some("precondition_failed") | Some("invalid_input")
            ));
        }
    }

    #[test]
    fn stale_preimage_is_refused_before_native_attempt() {
        let fixture = Fixture::new();
        let tool = RepositoryFileDeletionTool::new(&fixture.git, &fixture.root).unwrap();
        tool.policy
            .test_modify_before_delete
            .store(true, std::sync::atomic::Ordering::SeqCst);
        assert_eq!(
            execute(&tool, fixture.request())["status"],
            "precondition_failed"
        );
        assert_eq!(
            tool.policy
                .delete_attempts
                .load(std::sync::atomic::Ordering::SeqCst),
            0
        );
        assert_eq!(
            fs::read(fixture.root.join("target.txt")).unwrap(),
            b"changed externally"
        );
    }

    #[test]
    fn dirty_and_staged_targets_fail_closed() {
        let fixture = Fixture::new();
        let tool = RepositoryFileDeletionTool::new(&fixture.git, &fixture.root).unwrap();
        fs::write(fixture.root.join("target.txt"), b"new work\n").unwrap();
        assert_eq!(
            execute(&tool, fixture.request())["status"],
            "precondition_failed"
        );

        let fixture = Fixture::new();
        let tool = RepositoryFileDeletionTool::new(&fixture.git, &fixture.root).unwrap();
        fs::write(fixture.root.join("target.txt"), b"staged work\n").unwrap();
        run(&fixture.git, &fixture.root, &["add", "target.txt"]);
        assert_eq!(
            execute(&tool, fixture.request())["status"],
            "precondition_failed"
        );
        assert!(fixture.root.join("target.txt").exists());
    }

    #[cfg(unix)]
    #[test]
    fn symlink_target_is_rejected_without_following_it() {
        let fixture = Fixture::new();
        fs::remove_file(fixture.root.join("target.txt")).unwrap();
        std::os::unix::fs::symlink("other.txt", fixture.root.join("target.txt")).unwrap();
        let tool = RepositoryFileDeletionTool::new(&fixture.git, &fixture.root).unwrap();
        assert_eq!(
            execute(&tool, fixture.request())["status"],
            "precondition_failed"
        );
        assert_eq!(
            fs::read(fixture.root.join("other.txt")).unwrap(),
            b"untouched\n"
        );
    }

    #[test]
    fn post_attempt_uncertainty_is_not_replayed() {
        let fixture = Fixture::new();
        let tool = RepositoryFileDeletionTool::new(&fixture.git, &fixture.root).unwrap();
        tool.policy
            .force_uncertain
            .store(true, std::sync::atomic::Ordering::SeqCst);
        assert_eq!(execute(&tool, fixture.request())["status"], "uncertain");
        assert!(!fixture.root.join("target.txt").exists());
        assert_eq!(
            tool.policy
                .delete_attempts
                .load(std::sync::atomic::Ordering::SeqCst),
            1
        );
    }

    #[test]
    fn closed_path_schema_rejects_escape_aliases_and_extra_targets() {
        for value in [
            json!({"path":"../target.txt","expected_file_sha256":"0","expected_file_byte_length":0}),
            json!({"path":"C:/target.txt","expected_file_sha256":"0","expected_file_byte_length":0}),
            json!({"path":"target.txt","expected_file_sha256":"0","expected_file_byte_length":0,"force":true}),
        ] {
            assert!(DeleteRequest::parse(&ToolInput(value)).is_err());
        }
    }
}
