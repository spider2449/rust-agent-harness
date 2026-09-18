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
    repository_boundary::RepositoryNestedBoundaryPolicy,
    repository_git_layout::RepositoryGitLayout,
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
    /// Binds the authority to a host-selected Git executable and repository.
    pub fn new(
        git_executable: impl AsRef<Path>,
        repository_root: impl AsRef<Path>,
    ) -> Result<Self, ToolError> {
        Ok(Self {
            policy: Arc::new(RepositoryDirectoryCreationPolicy::new(
                git_executable.as_ref(),
                repository_root.as_ref(),
            )?),
        })
    }

    /// Confirms that this opaque authority is bound to the selected root.
    #[must_use]
    pub fn matches_repository_root(&self, repository_root: &Path) -> bool {
        self.policy.root == repository_root
    }
}

impl RepositoryDirectoryCreationTool {
    /// Constructs the tool from a host-selected Git executable and repository.
    pub fn new(
        git_executable: impl AsRef<Path>,
        repository_root: impl AsRef<Path>,
    ) -> Result<Self, ToolError> {
        Ok(Self::from_authority(
            RepositoryDirectoryCreationAuthority::new(git_executable, repository_root)?,
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
        let pre = match self.authority.policy.capture(&request).await {
            Ok(pre) => pre,
            Err(()) => return Ok(result("precondition_failed", Some(&request.path), false)),
        };
        if self
            .authority
            .policy
            .revalidate(&request, &pre)
            .await
            .is_err()
        {
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
                if self.authority.policy.verify_post(&pre).await.is_ok() {
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
                if self.authority.policy.known_no_effect(&pre).await.is_ok() {
                    Ok(result("known_no_effect", Some(&request.path), false))
                } else {
                    Ok(result("uncertain", None, true))
                }
            }
            Err(_) => {
                if self.authority.policy.known_no_effect(&pre).await.is_ok() {
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
    git: PathBuf,
    root_identity: FileIdentity,
    git_identity: FileIdentity,
    layout: RepositoryGitLayout,
    boundary: RepositoryNestedBoundaryPolicy,
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
}

impl RepositoryDirectoryCreationPolicy {
    fn new(git: &Path, root: &Path) -> Result<Self, ToolError> {
        if !root.is_absolute() || !git.is_absolute() {
            return Err(policy_error("host identities must be absolute"));
        }
        reject_reparse_ancestry(root, "repository root")?;
        let layout = RepositoryGitLayout::capture(git, root)?;
        let root = layout.root().to_path_buf();
        validate_directory_path(&root, &root, "repository root")?;
        reject_reparse_ancestry(git, "Git executable")?;
        let git = fs::canonicalize(git).map_err(fs_error)?;
        if !fs::metadata(&git).map_err(fs_error)?.is_file() {
            return Err(policy_error("Git executable is invalid"));
        }
        Ok(Self {
            root_identity: FileIdentity::capture(&root)?,
            git_identity: FileIdentity::capture(&git)?,
            layout,
            boundary: RepositoryNestedBoundaryPolicy::new(&root),
            lease: repository_lease(&root),
            git,
            root,
        })
    }

    async fn acquire_lease(&self) -> MutexGuard<'_, ()> {
        self.lease.lock().await
    }

    async fn capture(&self, request: &CreateDirectoryRequest) -> Result<PreState, ()> {
        self.repository_ok().await.map_err(|_| ())?;
        let path = self.root.join(&request.path);
        let parent = path.parent().ok_or(())?;
        validate_directory_path(&self.root, parent, "directory parent").map_err(|_| ())?;
        reject_reparse_ancestry(parent, "directory parent").map_err(|_| ())?;
        if self.boundary.validate_existing(parent).is_err() || fs::symlink_metadata(&path).is_ok() {
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
            git: git_snapshot(&self.layout).map_err(|_| ())?,
        })
    }

    async fn revalidate(&self, request: &CreateDirectoryRequest, pre: &PreState) -> Result<(), ()> {
        let current = self.capture(request).await?;
        if current.path != pre.path
            || !current.parent_identity.same_object(&pre.parent_identity)
            || current.git != pre.git
        {
            return Err(());
        }
        Ok(())
    }

    async fn verify_post(&self, pre: &PreState) -> Result<(), ()> {
        self.repository_ok().await.map_err(|_| ())?;
        self.boundary
            .validate_existing(pre.path.parent().ok_or(())?)
            .map_err(|_| ())?;
        let metadata = fs::symlink_metadata(&pre.path).map_err(|_| ())?;
        if metadata.file_type().is_symlink()
            || !metadata.is_dir()
            || reject_link_or_reparse(&pre.path, "created directory").is_err()
            || reject_reparse_ancestry(pre.path.parent().ok_or(())?, "directory parent").is_err()
        {
            return Err(());
        }
        if !FileIdentity::capture(pre.path.parent().ok_or(())?)
            .map_err(|_| ())?
            .same_object(&pre.parent_identity)
        {
            return Err(());
        }
        if fs::read_dir(&pre.path).map_err(|_| ())?.next().is_some()
            || git_snapshot(&self.layout).map_err(|_| ())? != pre.git
        {
            return Err(());
        }
        Ok(())
    }

    async fn known_no_effect(&self, pre: &PreState) -> Result<(), ()> {
        self.repository_ok().await.map_err(|_| ())?;
        self.boundary
            .validate_existing(pre.path.parent().ok_or(())?)
            .map_err(|_| ())?;
        if fs::symlink_metadata(&pre.path).is_ok()
            || !FileIdentity::capture(pre.path.parent().ok_or(())?)
                .map_err(|_| ())?
                .same_object(&pre.parent_identity)
            || git_snapshot(&self.layout).map_err(|_| ())? != pre.git
        {
            return Err(());
        }
        Ok(())
    }

    async fn repository_ok(&self) -> Result<(), ToolError> {
        reject_reparse_ancestry(&self.root, "repository root")?;
        let root = fs::canonicalize(&self.root).map_err(fs_error)?;
        if root != self.root
            || !FileIdentity::capture(&root)?.same_object(&self.root_identity)
            || !FileIdentity::capture(&self.git)?.same_object(&self.git_identity)
        {
            return Err(policy_error("repository identity changed"));
        }
        self.layout.validate_git(&self.git).await?;
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

fn git_snapshot(layout: &RepositoryGitLayout) -> Result<GitSnapshot, std::io::Error> {
    let index = optional_file(&layout.index_path())?;
    let head = fs::read(layout.head_path())?;
    Ok(GitSnapshot { index, head })
}

fn optional_file(path: &Path) -> Result<Option<Vec<u8>>, std::io::Error> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
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
    use std::sync::atomic::Ordering;

    struct Fixture {
        _worktrees: crate::repository_git_layout::test_fixture::WorktreeFixture,
        git: PathBuf,
        root: PathBuf,
    }
    impl Fixture {
        fn new() -> Self {
            let worktrees = crate::repository_git_layout::test_fixture::WorktreeFixture::new();
            let root = worktrees.main.clone();
            fs::create_dir(root.join("existing")).unwrap();
            Self {
                git: worktrees.git.clone(),
                root,
                _worktrees: worktrees,
            }
        }
    }
    fn execute(tool: &RepositoryDirectoryCreationTool, value: Value) -> Value {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime
            .block_on(tool.execute(ToolInput(value), ToolContext::default()))
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
        let tool = RepositoryDirectoryCreationTool::new(&fixture.git, &fixture.root).unwrap();
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
    fn linked_creation_is_confined_to_the_selected_worktree() {
        let fixture = crate::repository_git_layout::test_fixture::WorktreeFixture::new();
        let tool = RepositoryDirectoryCreationTool::new(&fixture.git, &fixture.linked_a).unwrap();
        assert_eq!(
            execute(&tool, json!({"path":"new-dir"}))["status"],
            "directory_created_verified"
        );
        assert!(fixture.linked_a.join("new-dir").is_dir());
        assert!(!fixture.main.join("new-dir").exists());
        assert!(!fixture.linked_b.join("new-dir").exists());
    }

    #[test]
    fn rejects_nested_repository_parent_before_native_creation() {
        let fixture = Fixture::new();
        fs::create_dir(fixture.root.join("nested")).unwrap();
        fs::create_dir(fixture.root.join("nested/.git")).unwrap();
        let tool = RepositoryDirectoryCreationTool::new(&fixture.git, &fixture.root).unwrap();

        assert_eq!(
            execute(&tool, json!({"path":"nested/new-dir"}))["status"],
            "precondition_failed"
        );
        assert!(!fixture.root.join("nested/new-dir").exists());
        assert_eq!(tool.test_hook.native_attempts.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn rejects_authority_input_and_path_contract_failures() {
        let fixture = Fixture::new();
        let tool = RepositoryDirectoryCreationTool::new(&fixture.git, &fixture.root).unwrap();
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
        let tool = RepositoryDirectoryCreationTool::new(&fixture.git, &fixture.root).unwrap();
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
        let tool = RepositoryDirectoryCreationTool::new(&fixture.git, &fixture.root).unwrap();
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
        let tool = RepositoryDirectoryCreationTool::new(&fixture.git, &fixture.root).unwrap();
        tool.test_hook.force_uncertain.store(true, Ordering::SeqCst);
        assert_eq!(
            execute(&tool, json!({"path":"uncertain"}))["status"],
            "uncertain"
        );
        assert!(fixture.root.join("uncertain").is_dir());
        assert_eq!(tool.test_hook.native_attempts.load(Ordering::SeqCst), 1);
    }
}
