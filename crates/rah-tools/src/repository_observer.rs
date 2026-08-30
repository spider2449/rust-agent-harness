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
    git_support::{git_error, repository_observer_environment},
    host_execute::paths_equivalent,
    repository_diff::DiffBaseline,
};

const FILE_INFO_TIMEOUT: Duration = Duration::from_secs(5);
const STATUS_TIMEOUT: Duration = Duration::from_secs(10);
const DIFF_TIMEOUT: Duration = Duration::from_secs(15);
const OBSERVER_STDOUT_LIMIT: usize = 96 * 1024;
const OBSERVER_STDERR_LIMIT: usize = 8 * 1024;
pub(crate) const STATUS_OUTPUT_LIMIT: usize = 4 * 1024 * 1024;
pub(crate) const DIFF_OUTPUT_LIMIT: usize = 1024 * 1024;

/// The only command shapes currently authorized for repository observation.
#[derive(Clone, Copy)]
pub(crate) enum ObserverCommand {
    Index,
    Head,
    HeadTree,
    FileInfoStatus,
    Status,
    DiffRaw(DiffBaseline),
    DiffNumstat(DiffBaseline),
    DiffPatch(DiffBaseline),
}

/// One private, host-configured repository observer envelope.
pub(crate) struct RepositoryObserver {
    repository: RepositoryIdentity,
    index: HostExecutionPolicy,
    head: HostExecutionPolicy,
    head_tree: HostExecutionPolicy,
    file_info_status: HostExecutionPolicy,
    status: HostExecutionPolicy,
    diff_raw: HostExecutionPolicy,
    diff_numstat: HostExecutionPolicy,
    diff_patch: HostExecutionPolicy,
    staged_diff_raw: HostExecutionPolicy,
    staged_diff_numstat: HostExecutionPolicy,
    staged_diff_patch: HostExecutionPolicy,
    lease: Arc<AsyncMutex<()>>,
}

impl RepositoryObserver {
    pub(crate) fn new(git: &Path, root: &Path) -> Result<Self, ToolError> {
        let repository = RepositoryIdentity::capture(root)?;
        let environment = repository_observer_environment(&repository.root)?;
        let exact = |arguments: Vec<String>| {
            HostExecutionPolicy::new(
                git,
                HostArgumentPolicy::Exact(arguments),
                &repository.root,
                ".",
            )?
            .with_environment(environment.clone())?
            .with_timeout(FILE_INFO_TIMEOUT)?
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
            .with_environment(environment.clone())?
            .with_timeout(FILE_INFO_TIMEOUT)?
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
            file_info_status: path(vec![
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
            status: exact(vec![
                "--no-pager".into(),
                "status".into(),
                "--porcelain=v2".into(),
                "-z".into(),
                "--untracked-files=normal".into(),
                "--ignored=no".into(),
                "--no-renames".into(),
                "--ignore-submodules=all".into(),
            ])?
            .with_timeout(STATUS_TIMEOUT)?
            .with_output_limits(OutputLimits {
                stdout_bytes: STATUS_OUTPUT_LIMIT,
                stderr_bytes: OBSERVER_STDERR_LIMIT,
                combined_bytes: STATUS_OUTPUT_LIMIT + OBSERVER_STDERR_LIMIT,
            })?,
            diff_raw: exact(vec![
                "--no-pager".into(),
                "diff".into(),
                "--raw".into(),
                "-z".into(),
                "--no-abbrev".into(),
                "--no-renames".into(),
                "--no-ext-diff".into(),
                "--no-textconv".into(),
                "--ignore-submodules=all".into(),
                "--submodule=short".into(),
            ])?
            .with_timeout(DIFF_TIMEOUT)?
            .with_output_limits(OutputLimits {
                stdout_bytes: DIFF_OUTPUT_LIMIT,
                stderr_bytes: OBSERVER_STDERR_LIMIT,
                combined_bytes: DIFF_OUTPUT_LIMIT + OBSERVER_STDERR_LIMIT,
            })?,
            diff_numstat: exact(vec![
                "--no-pager".into(),
                "diff".into(),
                "--numstat".into(),
                "-z".into(),
                "--no-renames".into(),
                "--no-ext-diff".into(),
                "--no-textconv".into(),
                "--ignore-submodules=all".into(),
                "--submodule=short".into(),
            ])?
            .with_timeout(DIFF_TIMEOUT)?
            .with_output_limits(OutputLimits {
                stdout_bytes: DIFF_OUTPUT_LIMIT,
                stderr_bytes: OBSERVER_STDERR_LIMIT,
                combined_bytes: DIFF_OUTPUT_LIMIT + OBSERVER_STDERR_LIMIT,
            })?,
            diff_patch: exact(vec![
                "--no-pager".into(),
                "diff".into(),
                "--patch".into(),
                "--no-color".into(),
                "--no-prefix".into(),
                "--full-index".into(),
                "--no-renames".into(),
                "--no-relative".into(),
                "--no-ext-diff".into(),
                "--no-textconv".into(),
                "--diff-algorithm=myers".into(),
                "--no-indent-heuristic".into(),
                "--inter-hunk-context=0".into(),
                "--unified=3".into(),
                "--ignore-submodules=all".into(),
                "--submodule=short".into(),
            ])?
            .with_timeout(DIFF_TIMEOUT)?
            .with_output_limits(OutputLimits {
                stdout_bytes: DIFF_OUTPUT_LIMIT,
                stderr_bytes: OBSERVER_STDERR_LIMIT,
                combined_bytes: DIFF_OUTPUT_LIMIT + OBSERVER_STDERR_LIMIT,
            })?,
            staged_diff_raw: exact(vec![
                "--no-pager".into(),
                "diff".into(),
                "--cached".into(),
                "--raw".into(),
                "-z".into(),
                "--no-abbrev".into(),
                "--no-renames".into(),
                "--no-ext-diff".into(),
                "--no-textconv".into(),
                "--ignore-submodules=all".into(),
                "--submodule=short".into(),
            ])?
            .with_timeout(DIFF_TIMEOUT)?
            .with_output_limits(OutputLimits {
                stdout_bytes: DIFF_OUTPUT_LIMIT,
                stderr_bytes: OBSERVER_STDERR_LIMIT,
                combined_bytes: DIFF_OUTPUT_LIMIT + OBSERVER_STDERR_LIMIT,
            })?,
            staged_diff_numstat: exact(vec![
                "--no-pager".into(),
                "diff".into(),
                "--cached".into(),
                "--numstat".into(),
                "-z".into(),
                "--no-renames".into(),
                "--no-ext-diff".into(),
                "--no-textconv".into(),
                "--ignore-submodules=all".into(),
                "--submodule=short".into(),
            ])?
            .with_timeout(DIFF_TIMEOUT)?
            .with_output_limits(OutputLimits {
                stdout_bytes: DIFF_OUTPUT_LIMIT,
                stderr_bytes: OBSERVER_STDERR_LIMIT,
                combined_bytes: DIFF_OUTPUT_LIMIT + OBSERVER_STDERR_LIMIT,
            })?,
            staged_diff_patch: exact(vec![
                "--no-pager".into(),
                "diff".into(),
                "--cached".into(),
                "--patch".into(),
                "--no-color".into(),
                "--no-prefix".into(),
                "--full-index".into(),
                "--no-renames".into(),
                "--no-relative".into(),
                "--no-ext-diff".into(),
                "--no-textconv".into(),
                "--diff-algorithm=myers".into(),
                "--no-indent-heuristic".into(),
                "--inter-hunk-context=0".into(),
                "--unified=3".into(),
                "--ignore-submodules=all".into(),
                "--submodule=short".into(),
            ])?
            .with_timeout(DIFF_TIMEOUT)?
            .with_output_limits(OutputLimits {
                stdout_bytes: DIFF_OUTPUT_LIMIT,
                stderr_bytes: OBSERVER_STDERR_LIMIT,
                combined_bytes: DIFF_OUTPUT_LIMIT + OBSERVER_STDERR_LIMIT,
            })?,
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
        command: ObserverCommand,
        path: Option<&str>,
        started: Instant,
    ) -> Result<HostProcessOutput, ToolError> {
        self.revalidate()?;
        let timeout = match command {
            ObserverCommand::Status => STATUS_TIMEOUT,
            ObserverCommand::DiffRaw(_)
            | ObserverCommand::DiffNumstat(_)
            | ObserverCommand::DiffPatch(_) => DIFF_TIMEOUT,
            ObserverCommand::Index
            | ObserverCommand::Head
            | ObserverCommand::HeadTree
            | ObserverCommand::FileInfoStatus => FILE_INFO_TIMEOUT,
        };
        let remaining = timeout
            .checked_sub(started.elapsed())
            .ok_or_else(|| git_error("repository observation exceeded its total timeout"))?;
        if remaining.is_zero() {
            return Err(git_error(
                "repository observation exceeded its total timeout",
            ));
        }
        let policy = match command {
            ObserverCommand::Index => &self.index,
            ObserverCommand::Head => &self.head,
            ObserverCommand::HeadTree => &self.head_tree,
            ObserverCommand::FileInfoStatus => &self.file_info_status,
            ObserverCommand::Status => &self.status,
            ObserverCommand::DiffRaw(DiffBaseline::WorktreeVsIndex) => &self.diff_raw,
            ObserverCommand::DiffNumstat(DiffBaseline::WorktreeVsIndex) => &self.diff_numstat,
            ObserverCommand::DiffPatch(DiffBaseline::WorktreeVsIndex) => &self.diff_patch,
            ObserverCommand::DiffRaw(DiffBaseline::IndexVsHead) => &self.staged_diff_raw,
            ObserverCommand::DiffNumstat(DiffBaseline::IndexVsHead) => &self.staged_diff_numstat,
            ObserverCommand::DiffPatch(DiffBaseline::IndexVsHead) => &self.staged_diff_patch,
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
pub(crate) struct RepositoryIdentity {
    root: PathBuf,
    root_identity: FileIdentity,
    dot_git_identity: FileIdentity,
}

impl RepositoryIdentity {
    pub(crate) fn capture(root: &Path) -> Result<Self, ToolError> {
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

    pub(crate) fn revalidate(&self) -> Result<(), ToolError> {
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

    pub(crate) fn root(&self) -> &Path {
        &self.root
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FileIdentity {
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
    #[cfg(windows)]
    volume_serial_number: u32,
    #[cfg(windows)]
    file_index: u64,
    #[cfg(not(any(unix, windows)))]
    length: u64,
}

impl FileIdentity {
    pub(crate) fn capture(path: &Path) -> Result<Self, ToolError> {
        #[cfg(not(windows))]
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
            let (volume_serial_number, file_index) = windows_file_identity(path)?;
            Ok(Self {
                volume_serial_number,
                file_index,
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

#[cfg(windows)]
fn windows_file_identity(path: &Path) -> Result<(u32, u64), ToolError> {
    use std::os::windows::{fs::OpenOptionsExt, io::AsRawHandle};
    use windows_sys::Win32::Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, FILE_FLAG_BACKUP_SEMANTICS, GetFileInformationByHandle,
    };

    let mut options = fs::OpenOptions::new();
    options.read(true).custom_flags(FILE_FLAG_BACKUP_SEMANTICS);
    let file = options.open(path).map_err(fs_error)?;
    // The handle remains owned by `file` throughout the Windows API call, and
    // the API initializes the out structure when it reports success.
    let information = unsafe {
        let mut information = std::mem::zeroed::<BY_HANDLE_FILE_INFORMATION>();
        if GetFileInformationByHandle(file.as_raw_handle(), &mut information) == 0 {
            return Err(fs_error(std::io::Error::last_os_error()));
        }
        information
    };
    Ok((
        information.dwVolumeSerialNumber,
        u64::from(information.nFileIndexHigh) << 32 | u64::from(information.nFileIndexLow),
    ))
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

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        process::Command,
        sync::atomic::{AtomicU64, Ordering},
    };

    use tokio::{
        sync::oneshot,
        time::{Duration, timeout},
    };

    use super::RepositoryObserver;

    static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

    fn native_git() -> PathBuf {
        #[cfg(windows)]
        let output = Command::new("where.exe").arg("git.exe").output().unwrap();
        #[cfg(not(windows))]
        let output = Command::new("which").arg("git").output().unwrap();
        assert!(output.status.success());
        let path = String::from_utf8(output.stdout).unwrap();
        fs::canonicalize(path.lines().next().unwrap()).unwrap()
    }

    fn repository() -> PathBuf {
        let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "rah-repository-observer-lease-{}-{id}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join(".git")).unwrap();
        root
    }

    #[tokio::test]
    async fn observers_for_one_root_share_and_release_the_existing_exclusive_lease() {
        let root = repository();
        let git = native_git();
        let first = RepositoryObserver::new(&git, &root).unwrap();
        let second = RepositoryObserver::new(&git, &root).unwrap();
        let held = first.acquire_lease().await;
        let (entered, mut observed) = oneshot::channel();
        let waiter = tokio::spawn(async move {
            let _guard = second.acquire_lease().await;
            let _ = entered.send(());
        });
        assert!(
            timeout(Duration::from_millis(50), &mut observed)
                .await
                .is_err(),
            "a second observer must wait for the existing repository lease"
        );
        drop(held);
        timeout(Duration::from_secs(1), &mut observed)
            .await
            .unwrap()
            .unwrap();
        waiter.await.unwrap();
        let _ = fs::remove_dir_all(root);
    }
}
