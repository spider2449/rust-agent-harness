use std::{
    fs,
    path::{Path, PathBuf},
};

use async_trait::async_trait;
use rah_protocol::{ToolDefinition, ToolInput, ToolOutput};

use crate::{
    HostArgumentPolicy, HostExecutionPolicy, HostExecutionTool, Tool, ToolContext, ToolError,
    git_support::git_environment,
};

/// Stable tool name for the repository-specific host Git status capability.
pub const GIT_STATUS_TOOL_NAME: &str = "host.git.status";

const MAX_GIT_FILE_BYTES: u64 = 64 * 1024;

/// Runs exactly `git status --porcelain=v1` in one host-authorized repository.
///
/// Construction is trusted host setup. The native Git executable and repository
/// are canonicalized before registration and revalidated before every call.
/// Model input is restricted to an empty object and cannot select process or
/// repository details.
pub struct GitStatusTool {
    repository: RepositoryIdentity,
    inner: HostExecutionTool,
}

impl GitStatusTool {
    /// Creates the capability from an absolute host-selected native Git
    /// executable and an absolute host-selected repository root.
    pub fn new(
        git_executable: impl AsRef<Path>,
        repository_root: impl AsRef<Path>,
    ) -> Result<Self, ToolError> {
        let repository = RepositoryIdentity::capture(repository_root.as_ref())?;
        let policy = HostExecutionPolicy::new(
            git_executable,
            HostArgumentPolicy::Exact(vec!["status".to_owned(), "--porcelain=v1".to_owned()]),
            &repository.root,
            ".",
        )?
        .with_environment(git_environment())?;
        Ok(Self {
            repository,
            inner: HostExecutionTool::new(
                GIT_STATUS_TOOL_NAME,
                "Reports porcelain status for one host-authorized Git repository.",
                policy,
            ),
        })
    }
}

#[async_trait]
impl Tool for GitStatusTool {
    fn definition(&self) -> ToolDefinition {
        self.inner.definition()
    }

    async fn execute(
        &self,
        input: ToolInput,
        context: ToolContext,
    ) -> Result<ToolOutput, ToolError> {
        self.repository.revalidate()?;
        self.inner.execute(input, context).await
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RepositoryIdentity {
    root: PathBuf,
    root_file: FileIdentity,
    dot_git: DotGitIdentity,
}

impl RepositoryIdentity {
    fn capture(path: &Path) -> Result<Self, ToolError> {
        if !path.is_absolute() {
            return Err(repository_error("repository root must be an absolute path"));
        }
        let root = canonical_directory(path, "repository root")?;
        let root_file = FileIdentity::capture(&root)?;
        let dot_git = DotGitIdentity::capture(&root.join(".git"))?;
        Ok(Self {
            root,
            root_file,
            dot_git,
        })
    }

    fn revalidate(&self) -> Result<(), ToolError> {
        let root = canonical_directory(&self.root, "repository root")?;
        if root != self.root || FileIdentity::capture(&root)? != self.root_file {
            return Err(repository_error("repository root identity changed"));
        }
        let dot_git = DotGitIdentity::capture(&root.join(".git"))
            .map_err(|_| repository_error("repository metadata identity changed"))?;
        if dot_git != self.dot_git {
            return Err(repository_error("repository metadata identity changed"));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum DotGitIdentity {
    Directory {
        identity: FileIdentity,
        canonical_path: PathBuf,
    },
    File {
        identity: FileIdentity,
        contents: Vec<u8>,
    },
}

impl DotGitIdentity {
    fn capture(path: &Path) -> Result<Self, ToolError> {
        let link_metadata =
            fs::symlink_metadata(path).map_err(|error| repository_error(error.to_string()))?;
        if link_metadata.file_type().is_symlink() {
            return Err(repository_error(
                "repository .git representation must not be a symbolic link",
            ));
        }
        if link_metadata.is_dir() {
            let canonical_path = canonical_directory(path, "repository .git directory")?;
            let head = canonical_path.join("HEAD");
            if !fs::metadata(&head).is_ok_and(|metadata| metadata.is_file()) {
                return Err(repository_error(
                    "repository .git directory must contain a regular HEAD file",
                ));
            }
            return Ok(Self::Directory {
                identity: FileIdentity::capture(&canonical_path)?,
                canonical_path,
            });
        }
        if link_metadata.is_file() {
            if link_metadata.len() > MAX_GIT_FILE_BYTES {
                return Err(repository_error("repository .git file is too large"));
            }
            let contents = fs::read(path).map_err(|error| repository_error(error.to_string()))?;
            if !contents.starts_with(b"gitdir:") {
                return Err(repository_error(
                    "repository .git file must use the gitdir representation",
                ));
            }
            return Ok(Self::File {
                identity: FileIdentity::capture(path)?,
                contents,
            });
        }
        Err(repository_error(
            "repository .git representation must be a directory or regular file",
        ))
    }
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
    file_size: u64,
    #[cfg(windows)]
    attributes: u32,
    #[cfg(not(any(unix, windows)))]
    length: u64,
    #[cfg(not(any(unix, windows)))]
    modified: Option<std::time::SystemTime>,
}

impl FileIdentity {
    fn capture(path: &Path) -> Result<Self, ToolError> {
        let metadata = fs::metadata(path).map_err(|error| repository_error(error.to_string()))?;
        capture_file_identity(&metadata)
    }
}

#[cfg(unix)]
fn capture_file_identity(metadata: &fs::Metadata) -> Result<FileIdentity, ToolError> {
    use std::os::unix::fs::MetadataExt;

    Ok(FileIdentity {
        device: metadata.dev(),
        inode: metadata.ino(),
    })
}

#[cfg(windows)]
fn capture_file_identity(metadata: &fs::Metadata) -> Result<FileIdentity, ToolError> {
    use std::os::windows::fs::MetadataExt;

    Ok(FileIdentity {
        creation_time: metadata.creation_time(),
        file_size: metadata.file_size(),
        attributes: metadata.file_attributes(),
    })
}

#[cfg(not(any(unix, windows)))]
fn capture_file_identity(metadata: &fs::Metadata) -> Result<FileIdentity, ToolError> {
    Ok(FileIdentity {
        length: metadata.len(),
        modified: metadata.modified().ok(),
    })
}

fn canonical_directory(path: &Path, label: &str) -> Result<PathBuf, ToolError> {
    let canonical = fs::canonicalize(path).map_err(|error| repository_error(error.to_string()))?;
    if !canonical.is_dir() {
        return Err(repository_error(format!(
            "{label} must be an existing directory"
        )));
    }
    Ok(canonical)
}

fn repository_error(message: impl Into<String>) -> ToolError {
    ToolError::Execution {
        message: format!(
            "Git status repository policy rejected capability: {}",
            message.into()
        ),
    }
}
