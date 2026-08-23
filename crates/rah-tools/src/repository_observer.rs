//! Private fixed-command foundation for repository observers.
//!
//! This deliberately does not expose a general Git invocation API.  Each
//! observer chooses from the small command enum below, while the host owns the
//! executable, repository identity, cwd, environment, output limits, timeout,
//! and exclusive RAH repository lease.

use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

use futures::lock::{Mutex as AsyncMutex, MutexGuard};
use rah_protocol::ToolInput;
use rah_sandbox::{HostProcessOutput, OutputLimits};
use serde_json::json;

use crate::{
    HostArgumentPolicy, HostExecutionPolicy, ToolError,
    git_support::{git_environment, git_error},
    host_execute::paths_equivalent,
};

const OBSERVER_TIMEOUT: Duration = Duration::from_secs(5);
const OBSERVER_STDOUT_LIMIT: usize = 96 * 1024;
const OBSERVER_STDERR_LIMIT: usize = 8 * 1024;

/// The only command shapes currently authorized for repository observation.
#[derive(Clone, Copy)]
pub(crate) enum FileInfoCommand {
    Index,
    Head,
    HeadTree,
    Status,
}

/// One private, host-configured repository observer envelope.
pub(crate) struct RepositoryObserver {
    repository: RepositoryIdentity,
    index: HostExecutionPolicy,
    head: HostExecutionPolicy,
    head_tree: HostExecutionPolicy,
    status: HostExecutionPolicy,
    lease: Arc<AsyncMutex<()>>,
}

impl RepositoryObserver {
    pub(crate) fn new(git: &Path, root: &Path) -> Result<Self, ToolError> {
        let repository = RepositoryIdentity::capture(root)?;
        let exact = |arguments: Vec<String>| {
            HostExecutionPolicy::new(
                git,
                HostArgumentPolicy::Exact(arguments),
                &repository.root,
                ".",
            )?
            .with_environment(git_environment())?
            .with_timeout(OBSERVER_TIMEOUT)?
            .with_output_limits(OutputLimits {
                stdout_bytes: OBSERVER_STDOUT_LIMIT,
                stderr_bytes: OBSERVER_STDERR_LIMIT,
                combined_bytes: OBSERVER_STDOUT_LIMIT + OBSERVER_STDERR_LIMIT,
            })
        };
        let path = |arguments: Vec<String>| {
            HostExecutionPolicy::new(
                git,
                HostArgumentPolicy::Text {
                    prefix: arguments,
                    max_bytes: 1024,
                },
                &repository.root,
                ".",
            )?
            .with_environment(git_environment())?
            .with_timeout(OBSERVER_TIMEOUT)?
            .with_output_limits(OutputLimits {
                stdout_bytes: OBSERVER_STDOUT_LIMIT,
                stderr_bytes: OBSERVER_STDERR_LIMIT,
                combined_bytes: OBSERVER_STDOUT_LIMIT + OBSERVER_STDERR_LIMIT,
            })
        };

        Ok(Self {
            index: path(vec![
                "--no-pager".into(),
                "--literal-pathspecs".into(),
                "ls-files".into(),
                "--stage".into(),
                "-v".into(),
                "-z".into(),
                "--full-name".into(),
                "--no-abbrev".into(),
                "--".into(),
            ])?,
            head: exact(vec![
                "--no-pager".into(),
                "rev-parse".into(),
                "--verify".into(),
                "-q".into(),
                "HEAD".into(),
            ])?,
            head_tree: path(vec![
                "--no-pager".into(),
                "--literal-pathspecs".into(),
                "ls-tree".into(),
                "-z".into(),
                "-l".into(),
                "HEAD".into(),
                "--".into(),
            ])?,
            status: path(vec![
                "--no-pager".into(),
                "--literal-pathspecs".into(),
                "status".into(),
                "--porcelain=v2".into(),
                "-z".into(),
                "--untracked-files=all".into(),
                "--ignored=no".into(),
                "--no-renames".into(),
                "--ignore-submodules=all".into(),
                "--".into(),
            ])?,
            lease: crate::git_stage::repository_lease(&repository.root),
            repository,
        })
    }

    pub(crate) fn root(&self) -> &Path {
        &self.repository.root
    }

    pub(crate) fn revalidate(&self) -> Result<(), ToolError> {
        self.repository.revalidate()
    }

    pub(crate) async fn acquire_lease(&self) -> MutexGuard<'_, ()> {
        self.lease.lock().await
    }

    /// Executes one of the fixed observation commands within a common timeout.
    pub(crate) async fn run(
        &self,
        command: FileInfoCommand,
        path: Option<&str>,
        started: Instant,
    ) -> Result<HostProcessOutput, ToolError> {
        self.revalidate()?;
        let remaining = OBSERVER_TIMEOUT
            .checked_sub(started.elapsed())
            .ok_or_else(|| git_error("repository observation exceeded its total timeout"))?;
        if remaining.is_zero() {
            return Err(git_error(
                "repository observation exceeded its total timeout",
            ));
        }
        let policy = match command {
            FileInfoCommand::Index => &self.index,
            FileInfoCommand::Head => &self.head,
            FileInfoCommand::HeadTree => &self.head_tree,
            FileInfoCommand::Status => &self.status,
        }
        .clone()
        .with_timeout(remaining)?;
        let input = match path {
            Some(path) => ToolInput(json!({"text": path})),
            None => ToolInput(json!({})),
        };
        policy.execute_process(&input).await
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RepositoryIdentity {
    root: PathBuf,
    root_identity: FileIdentity,
    dot_git_identity: FileIdentity,
}

impl RepositoryIdentity {
    fn capture(root: &Path) -> Result<Self, ToolError> {
        if !root.is_absolute() {
            return Err(git_error("repository root must be an absolute path"));
        }
        reject_reparse_ancestry(root, "repository root")?;
        let root = canonical_directory(root, "repository root")?;
        let dot_git = root.join(".git");
        reject_link_or_reparse(&dot_git, "repository metadata")?;
        let metadata = fs::metadata(&dot_git).map_err(fs_error)?;
        if !metadata.is_dir() && !metadata.is_file() {
            return Err(git_error("repository metadata must be a directory or file"));
        }
        Ok(Self {
            root_identity: FileIdentity::capture(&root)?,
            dot_git_identity: FileIdentity::capture(&dot_git)?,
            root,
        })
    }

    fn revalidate(&self) -> Result<(), ToolError> {
        reject_reparse_ancestry(&self.root, "repository root")?;
        let root = canonical_directory(&self.root, "repository root")?;
        let dot_git = root.join(".git");
        reject_link_or_reparse(&dot_git, "repository metadata")?;
        if !paths_equivalent(&root, &self.root)
            || FileIdentity::capture(&root)? != self.root_identity
            || FileIdentity::capture(&dot_git)? != self.dot_git_identity
        {
            return Err(git_error("repository identity changed"));
        }
        Ok(())
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
    size: u64,
    #[cfg(windows)]
    attributes: u32,
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
                attributes: metadata.file_attributes(),
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

fn canonical_directory(path: &Path, label: &str) -> Result<PathBuf, ToolError> {
    let path = fs::canonicalize(path).map_err(fs_error)?;
    if !path.is_dir() {
        return Err(git_error(format!("{label} must be an existing directory")));
    }
    Ok(path)
}

pub(crate) fn reject_reparse_ancestry(path: &Path, label: &str) -> Result<(), ToolError> {
    for ancestor in path.ancestors() {
        if ancestor.exists() {
            reject_link_or_reparse(ancestor, label)?;
        }
    }
    Ok(())
}

pub(crate) fn reject_link_or_reparse(path: &Path, label: &str) -> Result<(), ToolError> {
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
