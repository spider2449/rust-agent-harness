//! Host-authorized creation of one ordinary repository directory entry.

use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use async_trait::async_trait;
use futures::lock::{Mutex as AsyncMutex, MutexGuard};
use rah_protocol::{PermissionLevel, ToolContent, ToolDefinition, ToolInput, ToolName, ToolOutput};
use serde_json::{Value, json};

use crate::{
    Tool, ToolContext, ToolError,
    git_stage::repository_lease,
    native_repository_create::{NativeCreateError, NativeParent, create_directory},
    repository_worktree_patch::{
        FileIdentity, parse_logical_path, reject_link_or_reparse, reject_reparse_ancestry,
        validate_directory_path,
    },
};

/// Stable name for the bounded repository directory-creation capability.
pub const REPOSITORY_CREATE_DIRECTORY_TOOL_NAME: &str = "repo.create-directory";
const MAX_PATH_BYTES: usize = 1024;

/// Host-constructed tool carrying a separate directory-creation authority.
pub struct RepositoryDirectoryCreationTool {
    authority: RepositoryDirectoryCreationAuthority,
    #[cfg(test)]
    test_hook: Arc<TestHook>,
}

/// Opaque host-created authority for one bounded repository directory creation.
#[derive(Clone)]
pub struct RepositoryDirectoryCreationAuthority {
    policy: Arc<RepositoryDirectoryCreationPolicy>,
}

impl RepositoryDirectoryCreationAuthority {
    /// Binds the authority to a host-selected canonical repository.
    pub fn new(repository_root: impl AsRef<Path>) -> Result<Self, ToolError> {
        Ok(Self {
            policy: Arc::new(RepositoryDirectoryCreationPolicy::new(
                repository_root.as_ref(),
            )?),
        })
    }
}

impl RepositoryDirectoryCreationTool {
    /// Constructs the tool from a host-selected repository authority.
    pub fn new(repository_root: impl AsRef<Path>) -> Result<Self, ToolError> {
        Ok(Self::from_authority(
            RepositoryDirectoryCreationAuthority::new(repository_root)?,
        ))
    }

    /// Constructs a tool from authority already created by the trusted host.
    #[must_use]
    pub fn from_authority(authority: RepositoryDirectoryCreationAuthority) -> Self {
        Self {
            authority,
            #[cfg(test)]
            test_hook: Arc::new(TestHook::default()),
        }
    }
}

#[async_trait]
impl Tool for RepositoryDirectoryCreationTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: ToolName::new(REPOSITORY_CREATE_DIRECTORY_TOOL_NAME),
            description:
                "Creates one new ordinary directory at a validated repository-relative path."
                    .to_owned(),
            input_schema: json!({"type":"object","additionalProperties":false,"required":["path"],"properties":{"path":{"type":"string","minLength":1,"maxLength":MAX_PATH_BYTES}}}),
            permission: PermissionLevel::Execute,
        }
    }

    async fn execute(
        &self,
        input: ToolInput,
        _context: ToolContext,
    ) -> Result<ToolOutput, ToolError> {
        let request = match CreateDirectoryRequest::parse(&input) {
            Ok(request) => request,
            Err(()) => return Ok(result("invalid_input", None, false)),
        };
        let _lease = self.authority.policy.acquire_lease().await;
        let pre = match self.authority.policy.capture(&request) {
            Ok(pre) => pre,
            Err(()) => return Ok(result("precondition_failed", Some(&request.path), false)),
        };
        if self.authority.policy.revalidate(&request, &pre).is_err() {
            return Ok(result("precondition_failed", Some(&request.path), false));
        }
        #[cfg(test)]
        if self
            .test_hook
            .create_target_before_native
            .swap(false, std::sync::atomic::Ordering::SeqCst)
        {
            fs::create_dir(&pre.path).expect("race fixture should create target");
        }
        #[cfg(test)]
        self.test_hook
            .native_attempts
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        match create_directory(&pre.parent, &pre.name) {
            Ok(()) => {
                #[cfg(test)]
                if self
                    .test_hook
                    .force_uncertain
                    .swap(false, std::sync::atomic::Ordering::SeqCst)
                {
                    return Ok(result("uncertain", None, true));
                }
                if self.authority.policy.verify_post(&pre).is_ok() {
                    Ok(result(
                        "directory_created_verified",
                        Some(&request.path),
                        false,
                    ))
                } else {
                    Ok(result("uncertain", None, true))
                }
            }
            Err(NativeCreateError::AlreadyExists) => {
                if self.authority.policy.known_no_effect(&pre).is_ok() {
                    Ok(result("known_no_effect", Some(&request.path), false))
                } else {
                    Ok(result("uncertain", None, true))
                }
            }
            Err(_) => {
                if self.authority.policy.known_no_effect(&pre).is_ok() {
                    Ok(result("known_no_effect", Some(&request.path), false))
                } else {
                    Ok(result("uncertain", None, true))
                }
            }
        }
    }
}

struct RepositoryDirectoryCreationPolicy {
    root: PathBuf,
    root_identity: FileIdentity,
    dot_git_identity: FileIdentity,
    lease: Arc<AsyncMutex<()>>,
}

struct PreState {
    path: PathBuf,
    name: String,
    parent: NativeParent,
    parent_identity: FileIdentity,
    git: GitSnapshot,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct GitSnapshot {
    index: Option<Vec<u8>>,
    head: Vec<u8>,
    packed_refs: Option<Vec<u8>>,
    refs: Vec<(PathBuf, Vec<u8>)>,
}

impl RepositoryDirectoryCreationPolicy {
    fn new(root: &Path) -> Result<Self, ToolError> {
        if !root.is_absolute() {
            return Err(policy_error("repository root must be absolute"));
        }
        reject_reparse_ancestry(root, "repository root")?;
        let root = fs::canonicalize(root).map_err(fs_error)?;
        validate_directory_path(&root, &root, "repository root")?;
        let dot_git = root.join(".git");
        reject_link_or_reparse(&dot_git, "repository metadata")?;
        if !fs::metadata(&dot_git).map_err(fs_error)?.is_dir() {
            return Err(policy_error("bare or unsupported repository metadata"));
        }
        Ok(Self {
            root_identity: FileIdentity::capture(&root)?,
            dot_git_identity: FileIdentity::capture(&dot_git)?,
            lease: repository_lease(&root),
            root,
        })
    }

    async fn acquire_lease(&self) -> MutexGuard<'_, ()> {
        self.lease.lock().await
    }

    fn capture(&self, request: &CreateDirectoryRequest) -> Result<PreState, ()> {
        self.repository_ok().map_err(|_| ())?;
        let path = self.root.join(&request.path);
        let parent = path.parent().ok_or(())?;
        validate_directory_path(&self.root, parent, "directory parent").map_err(|_| ())?;
        reject_reparse_ancestry(parent, "directory parent").map_err(|_| ())?;
        if has_nested_metadata(&self.root, parent) || fs::symlink_metadata(&path).is_ok() {
            return Err(());
        }
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or(())?
            .to_owned();
        let parent_identity = FileIdentity::capture(parent).map_err(|_| ())?;
        let native_parent =
            NativeParent::open(&self.root, parent.strip_prefix(&self.root).map_err(|_| ())?)
                .map_err(|_| ())?;
        Ok(PreState {
            path,
            name,
            parent: native_parent,
            parent_identity,
            git: git_snapshot(&self.root).map_err(|_| ())?,
        })
    }

    fn revalidate(&self, request: &CreateDirectoryRequest, pre: &PreState) -> Result<(), ()> {
        let current = self.capture(request)?;
        if current.path != pre.path
            || current.parent_identity != pre.parent_identity
            || current.git != pre.git
        {
            return Err(());
        }
        Ok(())
    }

    fn verify_post(&self, pre: &PreState) -> Result<(), ()> {
        self.repository_ok().map_err(|_| ())?;
        let metadata = fs::symlink_metadata(&pre.path).map_err(|_| ())?;
        if metadata.file_type().is_symlink()
            || !metadata.is_dir()
            || FileIdentity::capture(&pre.path).map_err(|_| ())?.link_count != 1
            || reject_link_or_reparse(&pre.path, "created directory").is_err()
            || reject_reparse_ancestry(pre.path.parent().ok_or(())?, "directory parent").is_err()
        {
            return Err(());
        }
        if FileIdentity::capture(pre.path.parent().ok_or(())?).map_err(|_| ())?
            != pre.parent_identity
        {
            return Err(());
        }
        if fs::read_dir(&pre.path).map_err(|_| ())?.next().is_some()
            || git_snapshot(&self.root).map_err(|_| ())? != pre.git
        {
            return Err(());
        }
        Ok(())
    }

    fn known_no_effect(&self, pre: &PreState) -> Result<(), ()> {
        self.repository_ok().map_err(|_| ())?;
        if fs::symlink_metadata(&pre.path).is_ok()
            || FileIdentity::capture(pre.path.parent().ok_or(())?).map_err(|_| ())?
                != pre.parent_identity
            || git_snapshot(&self.root).map_err(|_| ())? != pre.git
        {
            return Err(());
        }
        Ok(())
    }

    fn repository_ok(&self) -> Result<(), ToolError> {
        reject_reparse_ancestry(&self.root, "repository root")?;
        let root = fs::canonicalize(&self.root).map_err(fs_error)?;
        let dot_git = root.join(".git");
        if root != self.root
            || FileIdentity::capture(&root)? != self.root_identity
            || FileIdentity::capture(&dot_git)? != self.dot_git_identity
        {
            return Err(policy_error("repository identity changed"));
        }
        Ok(())
    }
}

struct CreateDirectoryRequest {
    path: PathBuf,
}

impl CreateDirectoryRequest {
    fn parse(input: &ToolInput) -> Result<Self, ()> {
        let object = input.0.as_object().ok_or(())?;
        if object.len() != 1 {
            return Err(());
        }
        let value = object.get("path").and_then(Value::as_str).ok_or(())?;
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
        Ok(Self { path })
    }
}

fn has_nested_metadata(root: &Path, parent: &Path) -> bool {
    let mut current = root.to_path_buf();
    let Ok(relative) = parent.strip_prefix(root) else {
        return true;
    };
    for component in relative.components() {
        current.push(component.as_os_str());
        if current.join(".git").exists() {
            return true;
        }
    }
    false
}

fn git_snapshot(root: &Path) -> Result<GitSnapshot, std::io::Error> {
    let dot_git = root.join(".git");
    let index = optional_file(&dot_git.join("index"))?;
    let head = fs::read(dot_git.join("HEAD"))?;
    let packed_refs = optional_file(&dot_git.join("packed-refs"))?;
    let mut refs = Vec::new();
    collect_files(&dot_git.join("refs"), &dot_git, &mut refs)?;
    refs.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(GitSnapshot {
        index,
        head,
        packed_refs,
        refs,
    })
}

fn optional_file(path: &Path) -> Result<Option<Vec<u8>>, std::io::Error> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

fn collect_files(
    path: &Path,
    base: &Path,
    files: &mut Vec<(PathBuf, Vec<u8>)>,
) -> Result<(), std::io::Error> {
    if !path.exists() {
        return Ok(());
    }
    reject_link_or_reparse(path, "repository refs")
        .map_err(|_| std::io::Error::other("unsafe repository refs"))?;
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let child = entry.path();
        reject_link_or_reparse(&child, "repository refs")
            .map_err(|_| std::io::Error::other("unsafe repository refs"))?;
        if entry.file_type()?.is_dir() {
            collect_files(&child, base, files)?;
        } else {
            files.push((
                child.strip_prefix(base).unwrap_or(&child).to_path_buf(),
                fs::read(child)?,
            ));
        }
    }
    Ok(())
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

fn result(status: &str, path: Option<&Path>, uncertain: bool) -> ToolOutput {
    let mut value = json!({"status": status, "uncertain": uncertain});
    if let Some(path) = path {
        value["path"] = Value::String(path.to_string_lossy().replace('\\', "/"));
    }
    if status == "directory_created_verified" {
        value["git_metadata_changed"] = Value::Bool(false);
    }
    ToolOutput {
        content: vec![ToolContent::Json(value)],
        is_error: status != "directory_created_verified",
    }
}

fn policy_error(message: impl Into<String>) -> ToolError {
    ToolError::Execution {
        message: message.into(),
    }
}
fn fs_error(error: impl std::fmt::Display) -> ToolError {
    policy_error(format!(
        "repository directory authority rejected state: {error}"
    ))
}

#[cfg(test)]
#[derive(Default)]
struct TestHook {
    create_target_before_native: std::sync::atomic::AtomicBool,
    force_uncertain: std::sync::atomic::AtomicBool,
    native_attempts: std::sync::atomic::AtomicUsize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::executor::block_on;
    use std::{
        sync::atomic::Ordering,
        time::{SystemTime, UNIX_EPOCH},
    };

    struct Fixture {
        root: PathBuf,
    }
    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "rah-create-directory-{}",
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir_all(root.join(".git/refs/heads")).unwrap();
            fs::write(root.join(".git/HEAD"), b"ref: refs/heads/main\n").unwrap();
            fs::create_dir(root.join("existing")).unwrap();
            Self { root }
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
    fn execute(tool: &RepositoryDirectoryCreationTool, value: Value) -> Value {
        block_on(tool.execute(ToolInput(value), ToolContext::default()))
            .unwrap()
            .content
            .into_iter()
            .next()
            .and_then(|content| match content {
                ToolContent::Json(value) => Some(value),
                _ => None,
            })
            .unwrap()
    }

    #[test]
    fn creates_one_leaf_under_root_and_nested_parent() {
        let fixture = Fixture::new();
        let tool = RepositoryDirectoryCreationTool::new(&fixture.root).unwrap();
        assert_eq!(
            execute(&tool, json!({"path":"new-dir"}))["status"],
            "directory_created_verified"
        );
        assert_eq!(
            execute(&tool, json!({"path":"existing/nested"}))["status"],
            "directory_created_verified"
        );
        assert!(fixture.root.join("new-dir").is_dir());
        assert!(fixture.root.join("existing/nested").is_dir());
        assert!(
            fs::read_dir(fixture.root.join("new-dir"))
                .unwrap()
                .next()
                .is_none()
        );
    }

    #[test]
    fn rejects_authority_input_and_path_contract_failures() {
        let fixture = Fixture::new();
        let tool = RepositoryDirectoryCreationTool::new(&fixture.root).unwrap();
        for value in [
            json!({}),
            json!({"path":""}),
            json!({"path":"../escape"}),
            json!({"path":"a\\b"}),
            json!({"path":"new","extra":true}),
        ] {
            assert_eq!(execute(&tool, value)["status"], "invalid_input");
        }
        assert_eq!(
            execute(&tool, json!({"path":"missing/leaf"}))["status"],
            "precondition_failed"
        );
        assert!(!fixture.root.join("missing").exists());
    }

    #[test]
    fn rejects_existing_objects_and_file_parent() {
        let fixture = Fixture::new();
        fs::write(fixture.root.join("file"), b"x").unwrap();
        let tool = RepositoryDirectoryCreationTool::new(&fixture.root).unwrap();
        assert_eq!(
            execute(&tool, json!({"path":"existing"}))["status"],
            "precondition_failed"
        );
        assert_eq!(
            execute(&tool, json!({"path":"file/child"}))["status"],
            "precondition_failed"
        );
    }

    #[test]
    fn target_race_is_known_no_effect_without_retry() {
        let fixture = Fixture::new();
        let tool = RepositoryDirectoryCreationTool::new(&fixture.root).unwrap();
        tool.test_hook
            .create_target_before_native
            .store(true, Ordering::SeqCst);
        assert_eq!(
            execute(&tool, json!({"path":"raced"}))["status"],
            "uncertain"
        );
        assert_eq!(tool.test_hook.native_attempts.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn uncertain_postcondition_is_not_replayed_or_deleted() {
        let fixture = Fixture::new();
        let tool = RepositoryDirectoryCreationTool::new(&fixture.root).unwrap();
        tool.test_hook.force_uncertain.store(true, Ordering::SeqCst);
        assert_eq!(
            execute(&tool, json!({"path":"uncertain"}))["status"],
            "uncertain"
        );
        assert!(fixture.root.join("uncertain").is_dir());
        assert_eq!(tool.test_hook.native_attempts.load(Ordering::SeqCst), 1);
    }
}
