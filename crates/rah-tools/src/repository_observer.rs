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
    repository_boundary::{RepositoryNestedBoundaryPolicy, is_reparse_point},
    repository_diff::DiffBaseline,
    repository_git_layout::RepositoryGitLayout,
};

const FILE_INFO_TIMEOUT: Duration = Duration::from_secs(5);
const STATUS_TIMEOUT: Duration = Duration::from_secs(10);
const DIFF_TIMEOUT: Duration = Duration::from_secs(15);
pub(crate) const SEARCH_TIMEOUT: Duration = Duration::from_secs(15);
const OBSERVER_STDOUT_LIMIT: usize = 96 * 1024;
const OBSERVER_STDERR_LIMIT: usize = 8 * 1024;
pub(crate) const STATUS_OUTPUT_LIMIT: usize = 4 * 1024 * 1024;
pub(crate) const DIFF_OUTPUT_LIMIT: usize = 1024 * 1024;
pub(crate) const SEARCH_INVENTORY_OUTPUT_LIMIT: usize = 4 * 1024 * 1024;
pub(crate) const OBSERVER_MAX_RECORDS: usize = 100_000;
pub(crate) const OBSERVER_MAX_PATH_BYTES: usize = 1024;

/// The only command shapes currently authorized for repository observation.
#[derive(Clone, Copy)]
pub(crate) enum ObserverCommand {
    Index,
    TrackedInventory,
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
    git: PathBuf,
    repository: RepositoryIdentity,
    index: HostExecutionPolicy,
    tracked_inventory: HostExecutionPolicy,
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
    boundary: RepositoryNestedBoundaryPolicy,
    lease: Arc<AsyncMutex<()>>,
}

impl RepositoryObserver {
    pub(crate) fn new(git: &Path, root: &Path) -> Result<Self, ToolError> {
        let repository = RepositoryIdentity::capture(git, root)?;
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
            git: git.to_path_buf(),
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
            tracked_inventory: exact(vec![
                "--no-pager".into(),
                "ls-files".into(),
                "--cached".into(),
                "--deduplicate".into(),
                "-z".into(),
                "--full-name".into(),
                "--".into(),
            ])?
            .with_timeout(SEARCH_TIMEOUT)?
            .with_output_limits(OutputLimits {
                stdout_bytes: SEARCH_INVENTORY_OUTPUT_LIMIT,
                stderr_bytes: OBSERVER_STDERR_LIMIT,
                combined_bytes: SEARCH_INVENTORY_OUTPUT_LIMIT + OBSERVER_STDERR_LIMIT,
            })?,
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
            boundary: RepositoryNestedBoundaryPolicy::new(&repository.root),
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

    pub(crate) fn validate_target(&self, path: &Path) -> Result<(), ToolError> {
        self.boundary.validate_existing(path)
    }

    pub(crate) fn validate_observation(&self) -> Result<(), ToolError> {
        self.boundary.validate_observation()
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
        self.repository.validate_git(&self.git).await?;
        if matches!(
            command,
            ObserverCommand::Status
                | ObserverCommand::TrackedInventory
                | ObserverCommand::DiffRaw(_)
                | ObserverCommand::DiffNumstat(_)
                | ObserverCommand::DiffPatch(_)
        ) {
            self.validate_observation()?;
        }
        let timeout = match command {
            ObserverCommand::Status => STATUS_TIMEOUT,
            ObserverCommand::TrackedInventory => SEARCH_TIMEOUT,
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
            ObserverCommand::TrackedInventory => &self.tracked_inventory,
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
    layout: RepositoryGitLayout,
}

impl RepositoryIdentity {
    pub(crate) fn capture(git: &Path, root: &Path) -> Result<Self, ToolError> {
        if !root.is_absolute() {
            return Err(git_error("repository root must be an absolute path"));
        }
        reject_reparse_ancestry(root, "repository root")?;
        let root = canonical_directory(root, "repository root")?;
        let layout = RepositoryGitLayout::capture(git, &root)?;
        Ok(Self {
            root_identity: FileIdentity::capture(&root)?,
            layout,
            root,
        })
    }

    pub(crate) fn revalidate(&self) -> Result<(), ToolError> {
        reject_reparse_ancestry(&self.root, "repository root")?;
        let root = canonical_directory(&self.root, "repository root")?;
        if !paths_equivalent(&root, &self.root)
            || FileIdentity::capture(&root)? != self.root_identity
        {
            return Err(git_error("repository identity changed"));
        }
        self.layout.revalidate()?;
        Ok(())
    }

    pub(crate) async fn validate_git(&self, git: &Path) -> Result<(), ToolError> {
        self.revalidate()?;
        self.layout.validate_git(git).await?;
        self.revalidate()
    }

    pub(crate) fn layout(&self) -> &RepositoryGitLayout {
        &self.layout
    }

    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    pub(crate) fn git_dir(&self) -> &Path {
        self.layout.git_dir()
    }

    pub(crate) fn index_path(&self) -> PathBuf {
        self.layout.index_path()
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

#[derive(Default)]
pub(crate) struct TrackedInventory {
    pub(crate) paths: Vec<String>,
    pub(crate) non_addressable_path: u64,
}

pub(crate) enum TrackedCandidate {
    Eligible(fs::Metadata),
    Missing,
    NonRegular,
}

pub(crate) fn successful_tracked_inventory(
    output: rah_sandbox::HostProcessOutput,
) -> Result<Vec<u8>, ToolError> {
    if output.exit_code == Some(0)
        && !output.timed_out
        && output.overflow.is_none()
        && output.stdout.len() <= SEARCH_INVENTORY_OUTPUT_LIMIT
    {
        Ok(output.stdout)
    } else {
        Err(git_error(
            "tracked repository inventory did not complete successfully",
        ))
    }
}

pub(crate) fn parse_tracked_inventory(bytes: &[u8]) -> Result<TrackedInventory, ToolError> {
    if bytes.len() > SEARCH_INVENTORY_OUTPUT_LIMIT {
        return Err(git_error("tracked repository inventory exceeded its limit"));
    }
    if bytes.is_empty() {
        return Ok(TrackedInventory::default());
    }
    if !bytes.ends_with(&[0]) {
        return Err(git_error(
            "tracked repository inventory had a malformed NUL record",
        ));
    }
    let mut inventory = TrackedInventory::default();
    let mut start = 0;
    let mut records = 0;
    for (index, byte) in bytes.iter().enumerate() {
        if *byte != 0 {
            continue;
        }
        if index == start {
            return Err(git_error(
                "tracked repository inventory had an empty record",
            ));
        }
        records += 1;
        if records > OBSERVER_MAX_RECORDS {
            return Err(git_error(
                "tracked repository inventory record limit exceeded",
            ));
        }
        let record = &bytes[start..index];
        match std::str::from_utf8(record) {
            Ok(path) if is_safe_repository_path(path, OBSERVER_MAX_PATH_BYTES) => {
                inventory.paths.push(path.to_owned())
            }
            _ => inventory.non_addressable_path += 1,
        }
        start = index + 1;
    }
    inventory.paths.sort();
    inventory.paths.dedup();
    Ok(inventory)
}

pub(crate) fn is_safe_repository_path(path: &str, limit: usize) -> bool {
    if path.is_empty()
        || path.len() > limit
        || path.starts_with('/')
        || path.starts_with('\\')
        || path.contains('\\')
        || path.contains(':')
        || path.contains('\0')
    {
        return false;
    }
    path.split('/').all(|component| {
        !component.is_empty()
            && component != "."
            && component != ".."
            && !component.eq_ignore_ascii_case(".git")
    })
}

pub(crate) fn repository_target_path(root: &Path, path: &str) -> PathBuf {
    root.join(path.replace('/', std::path::MAIN_SEPARATOR_STR))
}

pub(crate) fn inspect_tracked_candidate(path: &Path) -> Result<TrackedCandidate, ToolError> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(TrackedCandidate::Missing);
        }
        Err(_) => return Err(git_error("repository candidate could not be inspected")),
    };
    if metadata.file_type().is_symlink() || is_reparse_point(&metadata) || !metadata.is_file() {
        return Ok(TrackedCandidate::NonRegular);
    }
    Ok(TrackedCandidate::Eligible(metadata))
}

/// Opens one already-validated worktree file without following a final link.
///
/// The caller still performs ordinary-file and boundary checks before and
/// after opening; this only closes the final-component link-following gap
/// during a bounded read.
pub(crate) fn open_regular_no_follow(path: &Path) -> Result<fs::File, std::io::Error> {
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
    options.open(path)
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

    use std::time::Instant;

    use super::{ObserverCommand, RepositoryObserver};

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

    #[tokio::test]
    async fn linked_observers_read_the_selected_worktree_head() {
        let fixture = crate::repository_git_layout::test_fixture::WorktreeFixture::new();
        let observer_a = RepositoryObserver::new(&fixture.git, &fixture.linked_a).unwrap();
        let observer_b = RepositoryObserver::new(&fixture.git, &fixture.linked_b).unwrap();
        let head_a_before = observer_a
            .run(ObserverCommand::Head, None, Instant::now())
            .await
            .unwrap()
            .stdout;
        crate::repository_git_layout::test_fixture::run(
            &fixture.git,
            &fixture.linked_b,
            &[
                "-c",
                "user.name=RAH Test",
                "-c",
                "user.email=rah@example.invalid",
                "commit",
                "--allow-empty",
                "--quiet",
                "-m",
                "only B",
            ],
        );
        let head_a_after = observer_a
            .run(ObserverCommand::Head, None, Instant::now())
            .await
            .unwrap()
            .stdout;
        let head_b_after = observer_b
            .run(ObserverCommand::Head, None, Instant::now())
            .await
            .unwrap()
            .stdout;
        assert_eq!(head_a_before, head_a_after);
        assert_ne!(head_a_after, head_b_after);
    }
}
