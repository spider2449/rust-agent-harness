//! Host-authorized movement of one clean HEAD-tracked repository file.

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

/// Stable name for the bounded repository file rename/move capability.
pub const REPOSITORY_RENAME_FILE_TOOL_NAME: &str = "repo.rename-file";
const MAX_PATH_BYTES: usize = 1024;
const MAX_FILE_BYTES: usize = 1024 * 1024;

/// Host-created authority for one selected repository.
pub struct RepositoryFileRenameTool {
    policy: Arc<RepositoryFileRenamePolicy>,
}

/// Opaque host-created rename authority.
#[derive(Clone)]
pub struct RepositoryFileRenameAuthority {
    policy: Arc<RepositoryFileRenamePolicy>,
}

impl RepositoryFileRenameAuthority {
    /// Binds the authority to host-selected repository resources.
    pub fn new(git: impl AsRef<Path>, root: impl AsRef<Path>) -> Result<Self, ToolError> {
        Ok(Self {
            policy: Arc::new(RepositoryFileRenamePolicy::new(
                git.as_ref(),
                root.as_ref(),
            )?),
        })
    }
    /// Checks whether the authority still names the supplied host resources.
    pub fn matches_resources(&self, git: &Path, root: &Path) -> bool {
        self.policy.matches_resources(git, root)
    }
}

impl RepositoryFileRenameTool {
    /// Creates a tool from host-selected resources.
    pub fn new(git: impl AsRef<Path>, root: impl AsRef<Path>) -> Result<Self, ToolError> {
        Ok(Self::from_authority(RepositoryFileRenameAuthority::new(
            git, root,
        )?))
    }
    /// Creates a tool from an authority constructed by the host.
    #[must_use]
    pub fn from_authority(authority: RepositoryFileRenameAuthority) -> Self {
        Self {
            policy: authority.policy,
        }
    }
}

#[async_trait]
impl Tool for RepositoryFileRenameTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: ToolName::new(REPOSITORY_RENAME_FILE_TOOL_NAME),
            description: "Renames one clean HEAD-tracked file to an absent path within the same bounded repository.".to_owned(),
            input_schema: json!({"type":"object","properties":{"source_path":{"type":"string","minLength":1,"maxLength":MAX_PATH_BYTES},"destination_path":{"type":"string","minLength":1,"maxLength":MAX_PATH_BYTES},"expected_source_file_sha256":{"type":"string","pattern":"^[0-9a-f]{64}$"},"expected_source_file_byte_length":{"type":"integer","minimum":0,"maximum":MAX_FILE_BYTES}},"required":["source_path","destination_path","expected_source_file_sha256","expected_source_file_byte_length"],"additionalProperties":false}),
            permission: PermissionLevel::Execute,
        }
    }

    async fn execute(
        &self,
        input: ToolInput,
        _context: ToolContext,
    ) -> Result<ToolOutput, ToolError> {
        let request = match RenameRequest::parse(&input) {
            Ok(request) => request,
            Err(()) => return Ok(result("invalid_input", None, false)),
        };
        let _lease = self.policy.lease.lock().await;
        let pre = match self.policy.capture(&request).await {
            Ok(pre) => pre,
            Err(()) => return Ok(result("precondition_failed", None, false)),
        };
        #[cfg(test)]
        self.policy
            .test_hook
            .apply(&pre, &self.policy.root, &request.destination_path);
        if self.policy.revalidate(&request, &pre).await.is_err() {
            return Ok(result("precondition_failed", None, false));
        }
        #[cfg(test)]
        self.policy
            .rename_attempts
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let native = rename_once(&pre.source, &pre.destination);
        if native.is_err() {
            return Ok(if self.policy.intact(&request, &pre).await {
                result("known_no_effect", None, false)
            } else {
                result("uncertain", None, true)
            });
        }
        #[cfg(test)]
        if self
            .policy
            .force_uncertain
            .swap(false, std::sync::atomic::Ordering::SeqCst)
        {
            return Ok(result("uncertain", None, true));
        }
        if self.policy.verify_post(&pre).await.is_ok() {
            Ok(result(
                "renamed_verified",
                Some(&request.destination_path),
                false,
            ))
        } else {
            Ok(result("uncertain", None, true))
        }
    }
}

/// Separate private authority; it does not compose creation, deletion, patch, index, or commit authority.
struct RepositoryFileRenamePolicy {
    git: PathBuf,
    root: PathBuf,
    root_identity: FileIdentity,
    git_identity: FileIdentity,
    dot_git_identity: FileIdentity,
    lease: Arc<AsyncMutex<()>>,
    #[cfg(test)]
    test_hook: TestHook,
    #[cfg(test)]
    rename_attempts: std::sync::atomic::AtomicUsize,
    #[cfg(test)]
    force_uncertain: std::sync::atomic::AtomicBool,
}

impl RepositoryFileRenamePolicy {
    fn new(git: &Path, root: &Path) -> Result<Self, ToolError> {
        if !git.is_absolute() || !root.is_absolute() {
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
        reject_reparse_ancestry(git, "Git executable")?;
        let git = fs::canonicalize(git).map_err(fs_error)?;
        if !fs::metadata(&git).map_err(fs_error)?.is_file() {
            return Err(policy_error("Git identity is invalid"));
        }
        Ok(Self {
            root_identity: FileIdentity::capture(&root)?,
            git_identity: FileIdentity::capture(&git)?,
            dot_git_identity: FileIdentity::capture(&dot_git)?,
            lease: crate::git_stage::repository_lease(&root),
            git,
            root,
            #[cfg(test)]
            test_hook: TestHook::default(),
            #[cfg(test)]
            rename_attempts: Default::default(),
            #[cfg(test)]
            force_uncertain: Default::default(),
        })
    }
    fn matches_resources(&self, git: &Path, root: &Path) -> bool {
        fs::canonicalize(git).ok().as_deref() == Some(&self.git)
            && fs::canonicalize(root).ok().as_deref() == Some(&self.root)
    }
    async fn capture(&self, request: &RenameRequest) -> Result<Preimage, ()> {
        self.repository_ok().map_err(|_| ())?;
        if paths_equivalent(
            &self.root.join(&request.source_path),
            &self.root.join(&request.destination_path),
        ) {
            return Err(());
        }
        let source = validate_existing_target(&self.root, &request.source_path).map_err(|_| ())?;
        let metadata = fs::metadata(&source).map_err(|_| ())?;
        reject_unsupported_file_attributes(&metadata).map_err(|_| ())?;
        let identity = FileIdentity::capture(&source).map_err(|_| ())?;
        if identity.link_count != 1 {
            return Err(());
        }
        let bytes = fs::read(&source).map_err(|_| ())?;
        if bytes.len() > MAX_FILE_BYTES
            || bytes.len() != request.length
            || sha256(&bytes) != request.sha256
        {
            return Err(());
        }
        let git = self.git_state(&request.source_path).await?;
        if git.blob != bytes {
            return Err(());
        }
        let destination = self.destination(&request.destination_path)?;
        let parent = destination.parent().ok_or(())?;
        let parent_identity = FileIdentity::capture(parent).map_err(|_| ())?;
        Ok(Preimage {
            source,
            destination,
            source_path: request.source_path.clone(),
            identity,
            parent_identity,
            bytes,
            git,
            index: fs::read(self.root.join(".git/index")).map_err(|_| ())?,
        })
    }
    fn destination(&self, relative: &Path) -> Result<PathBuf, ()> {
        let destination = self.root.join(relative);
        let parent = destination.parent().ok_or(())?;
        validate_directory_path(&self.root, parent, "destination parent").map_err(|_| ())?;
        if fs::symlink_metadata(&destination).is_ok() {
            reject_link_or_reparse(&destination, "destination").map_err(|_| ())?;
            return Err(());
        }
        Ok(destination)
    }
    async fn revalidate(&self, request: &RenameRequest, pre: &Preimage) -> Result<(), ()> {
        let current = self.capture(request).await?;
        if !paths_equivalent(&current.source, &pre.source)
            || current.identity != pre.identity
            || current.parent_identity != pre.parent_identity
            || current.bytes != pre.bytes
            || current.git != pre.git
            || current.index != pre.index
        {
            return Err(());
        }
        Ok(())
    }
    async fn intact(&self, request: &RenameRequest, pre: &Preimage) -> bool {
        self.capture(request).await.is_ok() && fs::symlink_metadata(&pre.destination).is_err()
    }
    async fn verify_post(&self, pre: &Preimage) -> Result<(), ()> {
        self.repository_ok().map_err(|_| ())?;
        if fs::symlink_metadata(&pre.source).is_ok()
            || reject_link_or_reparse(&pre.destination, "destination").is_err()
            || fs::read(&pre.destination).map_err(|_| ())? != pre.bytes
        {
            return Err(());
        }
        let destination_identity = FileIdentity::capture(&pre.destination).map_err(|_| ())?;
        if destination_identity.link_count != 1 {
            return Err(());
        }
        let git = self.git_state(&pre.source_path).await?;
        if git != pre.git || fs::read(self.root.join(".git/index")).map_err(|_| ())? != pre.index {
            return Err(());
        }
        Ok(())
    }
    fn repository_ok(&self) -> Result<(), ToolError> {
        reject_reparse_ancestry(&self.root, "repository root")?;
        let root = fs::canonicalize(&self.root).map_err(fs_error)?;
        if !paths_equivalent(&root, &self.root)
            || FileIdentity::capture(&root)? != self.root_identity
            || FileIdentity::capture(&root.join(".git"))? != self.dot_git_identity
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
            || !valid_index(&index, target.as_bytes())
        {
            return Err(());
        }
        let tag = self
            .git_output(vec!["ls-files", "-v", "-z", "--", &target])
            .await?;
        if tag != [b"H ", target.as_bytes(), &[0]].concat() {
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
        let output = HostExecutionPolicy::new(
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
        if output.exit_code == Some(0) && !output.timed_out && output.overflow.is_none() {
            Ok(output.stdout)
        } else {
            Err(())
        }
    }
}

struct Preimage {
    source: PathBuf,
    destination: PathBuf,
    source_path: PathBuf,
    identity: FileIdentity,
    parent_identity: FileIdentity,
    bytes: Vec<u8>,
    git: GitState,
    index: Vec<u8>,
}
#[derive(Clone, PartialEq, Eq)]
struct GitState {
    blob: Vec<u8>,
    fingerprint: Vec<u8>,
}
struct RenameRequest {
    source_path: PathBuf,
    destination_path: PathBuf,
    sha256: String,
    length: usize,
}
impl RenameRequest {
    fn parse(input: &ToolInput) -> Result<Self, ()> {
        let object = input.0.as_object().ok_or(())?;
        if object.len() != 4 {
            return Err(());
        }
        let source_path = parse_rename_path(
            object
                .get("source_path")
                .and_then(Value::as_str)
                .ok_or(())?,
        )
        .map_err(|_| ())?;
        let destination_path = parse_rename_path(
            object
                .get("destination_path")
                .and_then(Value::as_str)
                .ok_or(())?,
        )
        .map_err(|_| ())?;
        let sha256 = object
            .get("expected_source_file_sha256")
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
            .get("expected_source_file_byte_length")
            .and_then(Value::as_u64)
            .and_then(|n| usize::try_from(n).ok())
            .filter(|n| *n <= MAX_FILE_BYTES)
            .ok_or(())?;
        Ok(Self {
            source_path,
            destination_path,
            sha256: sha256.to_owned(),
            length,
        })
    }
}
fn parse_rename_path(value: &str) -> Result<PathBuf, ()> {
    let path = parse_logical_path(value, MAX_PATH_BYTES).map_err(|_| ())?;
    for component in path.components() {
        let name = component.as_os_str().to_str().ok_or(())?;
        if name.contains(['*', '?', '[', ']'])
            || name.ends_with(['.', ' '])
            || reserved_windows_name(name)
        {
            return Err(());
        }
    }
    Ok(path)
}
fn reserved_windows_name(component: &str) -> bool {
    let stem = component
        .trim_end_matches(['.', ' '])
        .split('.')
        .next()
        .unwrap_or("")
        .to_ascii_uppercase();
    matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (stem.len() == 4
            && matches!(&stem[..3], "COM" | "LPT")
            && matches!(stem.as_bytes()[3], b'1'..=b'9'))
}
fn valid_index(bytes: &[u8], path: &[u8]) -> bool {
    let Some(record) = bytes.strip_suffix(&[0]) else {
        return false;
    };
    let Some(tab) = record.iter().position(|b| *b == b'\t') else {
        return false;
    };
    &record[tab + 1..] == path
        && record[..tab]
            .split(|b| *b == b' ')
            .collect::<Vec<_>>()
            .as_slice()
            .get(2)
            == Some(&&b"0"[..])
}
fn rename_once(source: &Path, destination: &Path) -> Result<(), std::io::Error> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Storage::FileSystem::MoveFileExW;
        let source = source
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect::<Vec<_>>();
        let destination = destination
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect::<Vec<_>>();
        let ok = unsafe { MoveFileExW(source.as_ptr(), destination.as_ptr(), 0) };
        if ok == 0 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::ffi::OsStrExt;
        let source = std::ffi::CString::new(source.as_os_str().as_bytes())
            .map_err(|_| std::io::ErrorKind::InvalidInput)?;
        let destination = std::ffi::CString::new(destination.as_os_str().as_bytes())
            .map_err(|_| std::io::ErrorKind::InvalidInput)?;
        let result = unsafe {
            libc::renameat2(
                libc::AT_FDCWD,
                source.as_ptr(),
                libc::AT_FDCWD,
                destination.as_ptr(),
                libc::RENAME_NOREPLACE,
            )
        };
        if result == 0 {
            Ok(())
        } else {
            Err(std::io::Error::last_os_error())
        }
    }
    #[cfg(all(unix, not(target_os = "linux")))]
    {
        let _ = (source, destination);
        Err(std::io::Error::other(
            "no supported no-replace rename primitive",
        ))
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
        is_error: status != "renamed_verified",
    }
}
fn policy_error(message: impl Into<String>) -> ToolError {
    ToolError::Execution {
        message: format!(
            "repository file rename policy rejected capability: {}",
            message.into()
        ),
    }
}
fn fs_error(error: impl std::fmt::Display) -> ToolError {
    policy_error(error.to_string())
}

#[cfg(test)]
#[derive(Default)]
struct TestHook {
    modify_source: std::sync::atomic::AtomicBool,
    create_destination: std::sync::atomic::AtomicBool,
}
#[cfg(test)]
impl TestHook {
    fn apply(&self, pre: &Preimage, root: &Path, destination: &Path) {
        use std::sync::atomic::Ordering;
        if self.modify_source.swap(false, Ordering::SeqCst) {
            fs::write(&pre.source, b"changed").unwrap();
        }
        if self.create_destination.swap(false, Ordering::SeqCst) {
            fs::write(root.join(destination), b"external").unwrap();
        }
    }
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
                "rah-rename-{}",
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir_all(root.join("existing")).unwrap();
            let git = git_path();
            for args in [
                ["init", "--quiet"].as_slice(),
                ["config", "user.name", "RAH Test"].as_slice(),
                ["config", "user.email", "rah@example.invalid"].as_slice(),
            ] {
                run(&git, &root, args);
            }
            fs::write(root.join("old.txt"), b"rename bytes").unwrap();
            run(&git, &root, &["add", "old.txt"]);
            run(&git, &root, &["commit", "--quiet", "-m", "base"]);
            Self { root, git }
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
        let out = Command::new(command.0).arg(command.1).output().unwrap();
        fs::canonicalize(
            String::from_utf8(out.stdout)
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
    fn execute(tool: &RepositoryFileRenameTool, input: Value) -> Value {
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
    fn request(source: &str, destination: &str) -> Value {
        json!({"source_path":source,"destination_path":destination,"expected_source_file_sha256":sha256(b"rename bytes"),"expected_source_file_byte_length":12})
    }
    #[test]
    fn same_directory_rename_preserves_bytes_and_attempt_count() {
        let f = Fixture::new();
        let t = RepositoryFileRenameTool::new(&f.git, &f.root).unwrap();
        assert_eq!(
            execute(&t, request("old.txt", "renamed.txt"))["status"],
            "renamed_verified"
        );
        assert_eq!(
            fs::read(f.root.join("renamed.txt")).unwrap(),
            b"rename bytes"
        );
        assert!(!f.root.join("old.txt").exists());
        assert_eq!(
            t.policy
                .rename_attempts
                .load(std::sync::atomic::Ordering::SeqCst),
            1
        );
    }
    #[test]
    fn cross_directory_move_is_supported() {
        let f = Fixture::new();
        let t = RepositoryFileRenameTool::new(&f.git, &f.root).unwrap();
        assert_eq!(
            execute(&t, request("old.txt", "existing/moved.txt"))["status"],
            "renamed_verified"
        );
        assert_eq!(
            fs::read(f.root.join("existing/moved.txt")).unwrap(),
            b"rename bytes"
        );
    }
    #[test]
    fn malformed_and_colliding_requests_make_no_attempt() {
        let f = Fixture::new();
        let t = RepositoryFileRenameTool::new(&f.git, &f.root).unwrap();
        assert_eq!(
            execute(&t, json!({"source_path":"old.txt"}))["status"],
            "invalid_input"
        );
        fs::write(f.root.join("existing.txt"), b"x").unwrap();
        assert_eq!(
            execute(&t, request("old.txt", "existing.txt"))["status"],
            "precondition_failed"
        );
        assert_eq!(
            t.policy
                .rename_attempts
                .load(std::sync::atomic::Ordering::SeqCst),
            0
        );
    }
    #[test]
    fn missing_parent_directory_and_directory_collision_are_refused() {
        let f = Fixture::new();
        let t = RepositoryFileRenameTool::new(&f.git, &f.root).unwrap();
        assert_eq!(
            execute(&t, request("old.txt", "missing/moved.txt"))["status"],
            "precondition_failed"
        );
        assert_eq!(
            execute(&t, request("old.txt", "existing"))["status"],
            "precondition_failed"
        );
        assert_eq!(
            t.policy
                .rename_attempts
                .load(std::sync::atomic::Ordering::SeqCst),
            0
        );
    }
    #[test]
    fn stale_preconditions_and_dirty_source_are_effect_free() {
        let f = Fixture::new();
        let t = RepositoryFileRenameTool::new(&f.git, &f.root).unwrap();
        let mut stale = request("old.txt", "renamed.txt");
        stale["expected_source_file_sha256"] = json!(sha256(b"stale"));
        assert_eq!(execute(&t, stale)["status"], "precondition_failed");
        fs::write(f.root.join("old.txt"), b"dirty").unwrap();
        assert_eq!(
            execute(&t, request("old.txt", "renamed.txt"))["status"],
            "precondition_failed"
        );
        assert_eq!(
            t.policy
                .rename_attempts
                .load(std::sync::atomic::Ordering::SeqCst),
            0
        );
    }
    #[test]
    fn destination_race_is_refused_after_capture_without_attempt() {
        let f = Fixture::new();
        let t = RepositoryFileRenameTool::new(&f.git, &f.root).unwrap();
        t.policy
            .test_hook
            .create_destination
            .store(true, std::sync::atomic::Ordering::SeqCst);
        assert_eq!(
            execute(&t, request("old.txt", "race.txt"))["status"],
            "precondition_failed"
        );
        assert_eq!(fs::read(f.root.join("race.txt")).unwrap(), b"external");
        assert_eq!(
            t.policy
                .rename_attempts
                .load(std::sync::atomic::Ordering::SeqCst),
            0
        );
    }
    #[test]
    fn path_aliases_are_invalid_input() {
        let f = Fixture::new();
        let t = RepositoryFileRenameTool::new(&f.git, &f.root).unwrap();
        for (source, destination) in [
            ("../old.txt", "new.txt"),
            ("old.txt", ".git/x"),
            ("old.txt", "a/*"),
            ("old.txt", "CON"),
        ] {
            assert_eq!(
                execute(&t, request(source, destination))["status"],
                "invalid_input"
            );
        }
        assert_eq!(
            t.policy
                .rename_attempts
                .load(std::sync::atomic::Ordering::SeqCst),
            0
        );
    }
    #[test]
    fn changed_source_after_capture_is_refused_without_attempt() {
        let f = Fixture::new();
        let t = RepositoryFileRenameTool::new(&f.git, &f.root).unwrap();
        t.policy
            .test_hook
            .modify_source
            .store(true, std::sync::atomic::Ordering::SeqCst);
        assert_eq!(
            execute(&t, request("old.txt", "renamed.txt"))["status"],
            "precondition_failed"
        );
        assert_eq!(
            t.policy
                .rename_attempts
                .load(std::sync::atomic::Ordering::SeqCst),
            0
        );
    }
    #[test]
    fn uncertain_post_observation_is_not_retried() {
        let f = Fixture::new();
        let t = RepositoryFileRenameTool::new(&f.git, &f.root).unwrap();
        t.policy
            .force_uncertain
            .store(true, std::sync::atomic::Ordering::SeqCst);
        assert_eq!(
            execute(&t, request("old.txt", "renamed.txt"))["status"],
            "uncertain"
        );
        assert_eq!(
            t.policy
                .rename_attempts
                .load(std::sync::atomic::Ordering::SeqCst),
            1
        );
    }
}
