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
        match create_new(&parent, &request.name, request.content.as_bytes(), None) {
            Ok(()) => {}
            Err(NativeCreateError::AlreadyExists) => {
                return Ok(output("create_failed_known", None, None, None));
            }
            Err(NativeCreateError::WriteFailed(_)) => {
                return Ok(output("write_failed_known", None, None, None));
            }
            Err(_) => return Ok(output("create_failed_known", None, None, None)),
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
