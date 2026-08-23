use std::{
    collections::{BTreeMap, HashMap},
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock, Weak},
    time::Duration,
};

use async_trait::async_trait;
use futures::lock::{Mutex as AsyncMutex, MutexGuard};
use rah_protocol::{PermissionLevel, ToolContent, ToolDefinition, ToolInput, ToolName, ToolOutput};
use rah_sandbox::HostProcessOutput;
use serde_json::json;

use crate::{
    HostArgumentPolicy, HostExecutionPolicy, HostExecutionTool, Tool, ToolContext, ToolError,
};

const FIXTURE_TOOL_NAME: &str = "host.fixture.mutate-marker";
const FIXTURE_TARGET: &str = "fixture-marker";
const MARKER_RELATIVE_PATH: &str = "marker.txt";
const MARKER_BEFORE: &[u8] = b"before\n";
const MARKER_AFTER: &[u8] = b"after\n";
const MAX_SNAPSHOT_BYTES: usize = 64 * 1024;

/// Host-preauthorized deterministic repository mutation fixture.
///
/// Its model-visible input is an empty object. The host maps the only symbolic
/// target, `fixture-marker`, to `marker.txt` during trusted construction.
pub struct RepositoryMutationFixtureTool {
    policy: RepositoryMutationPolicy,
    process: HostExecutionTool,
}

impl RepositoryMutationFixtureTool {
    /// Creates the normal deterministic fixture capability for a trusted root.
    pub fn new(
        fixture_executable: impl AsRef<Path>,
        repository_root: impl AsRef<Path>,
    ) -> Result<Self, ToolError> {
        Self::new_with_test_mode(
            fixture_executable,
            repository_root,
            RepositoryMutationFixtureTestMode::Normal,
            Duration::from_secs(5),
            None,
        )
    }

    /// Creates a deterministic adversarial fixture configuration for integration
    /// tests. This is host construction support; the mode is never model input.
    #[doc(hidden)]
    pub fn new_with_test_mode(
        fixture_executable: impl AsRef<Path>,
        repository_root: impl AsRef<Path>,
        mode: RepositoryMutationFixtureTestMode,
        timeout: Duration,
        outside_probe: Option<PathBuf>,
    ) -> Result<Self, ToolError> {
        let policy = RepositoryMutationPolicy::new(repository_root.as_ref(), outside_probe)?;
        let arguments = fixture_arguments(&policy, mode);
        let process = HostExecutionPolicy::new(
            fixture_executable,
            HostArgumentPolicy::Exact(arguments),
            policy.root(),
            ".",
        )?
        .with_timeout(timeout)?;
        Ok(Self {
            policy,
            process: HostExecutionTool::new(
                FIXTURE_TOOL_NAME,
                "Mutates one host-authorized deterministic fixture marker.",
                process,
            ),
        })
    }
}

#[async_trait]
impl Tool for RepositoryMutationFixtureTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: ToolName::new(FIXTURE_TOOL_NAME),
            description: "Mutates the host-owned fixture-marker target only.".to_owned(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
            permission: PermissionLevel::Execute,
        }
    }

    async fn execute(
        &self,
        input: ToolInput,
        _context: ToolContext,
    ) -> Result<ToolOutput, ToolError> {
        self.process.validate_input(&input)?;
        let _lease = self.policy.acquire_lease().await;
        let pre = self.policy.capture_pre_state()?;
        let process = self.process.execute_process(&input).await;
        let post = self.policy.capture_post_state();
        Ok(self.policy.build_result(pre, process, post))
    }
}

/// Host-only deterministic behaviors used by integration tests.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RepositoryMutationFixtureTestMode {
    /// Replace the authorized marker content and exit.
    Normal,
    /// Wait before the authorized mutation.
    DelayBeforeMutation(Duration),
    /// Mutate the marker and then wait.
    DelayAfterMutation(Duration),
    /// Mutate the marker and a second file in the root.
    MutateSecondFile,
    /// Mutate the marker and create another file in the root.
    CreateExtraFile,
    /// Mutate the marker and delete the protected file in the root.
    DeleteProtectedFile,
    /// Mutate the marker and the host-selected probe outside the root.
    MutateOutsideRoot,
}

struct RepositoryMutationPolicy {
    root: PathBuf,
    root_identity: FileIdentity,
    target: AuthorizedTarget,
    lease: Arc<AsyncMutex<()>>,
    outside_probe: Option<OutsideProbe>,
}

impl RepositoryMutationPolicy {
    fn new(root: &Path, outside_probe: Option<PathBuf>) -> Result<Self, ToolError> {
        if !root.is_absolute() {
            return Err(policy_error("repository root must be an absolute path"));
        }
        reject_link_or_reparse(root, "repository root")?;
        let root = canonical_directory(root, "repository root")?;
        let root_identity = FileIdentity::capture(&root)?;
        let target_path = root.join(MARKER_RELATIVE_PATH);
        let target = AuthorizedTarget::capture(&root, FIXTURE_TARGET, &target_path)?;
        if fs::read(&target.path).map_err(filesystem_error)? != MARKER_BEFORE {
            return Err(policy_error(
                "fixture marker must initially contain `before\\n`",
            ));
        }
        let outside_probe = outside_probe.map(OutsideProbe::capture).transpose()?;
        Ok(Self {
            lease: repository_lease(&root),
            root,
            root_identity,
            target,
            outside_probe,
        })
    }

    fn root(&self) -> &Path {
        &self.root
    }

    async fn acquire_lease(&self) -> MutexGuard<'_, ()> {
        self.lease.lock().await
    }

    fn capture_pre_state(&self) -> Result<RepositoryState, ToolError> {
        self.revalidate_repository()?;
        self.target.revalidate(&self.root)?;
        let snapshot = DirectorySnapshot::capture(&self.root)?;
        let target = snapshot
            .files
            .get(&self.target.relative_path)
            .ok_or_else(|| policy_error("authorized target disappeared before execution"))?;
        if target.contents != MARKER_BEFORE {
            return Err(policy_error("authorized target precondition changed"));
        }
        let outside_probe = self
            .outside_probe
            .as_ref()
            .map(OutsideProbe::capture_current)
            .transpose()?;
        Ok(RepositoryState {
            snapshot,
            outside_probe,
        })
    }

    fn capture_post_state(&self) -> Result<RepositoryState, ToolError> {
        self.revalidate_repository()?;
        self.target.revalidate(&self.root)?;
        let snapshot = DirectorySnapshot::capture(&self.root)?;
        let outside_probe = self
            .outside_probe
            .as_ref()
            .map(OutsideProbe::capture_current)
            .transpose()?;
        Ok(RepositoryState {
            snapshot,
            outside_probe,
        })
    }

    fn revalidate_repository(&self) -> Result<(), ToolError> {
        reject_link_or_reparse(&self.root, "repository root")?;
        let root = canonical_directory(&self.root, "repository root")?;
        if root != self.root || FileIdentity::capture(&root)? != self.root_identity {
            return Err(policy_error("repository root identity changed"));
        }
        Ok(())
    }

    fn build_result(
        &self,
        pre: RepositoryState,
        process: Result<HostProcessOutput, ToolError>,
        post: Result<RepositoryState, ToolError>,
    ) -> ToolOutput {
        let process_state = ProcessState::from_result(&process);
        let verification = post
            .as_ref()
            .map(|post| verify_postconditions(&pre, post, &self.target))
            .unwrap_or_else(|error| Verification::uncertain(error.to_string()));
        let timed_out = process_state.timed_out;
        let uncertain = verification.uncertain || (timed_out && verification.changed);
        let status = if verification.violation {
            "policy_violation"
        } else if verification.authorized && !process_state.failed && !uncertain {
            "ok"
        } else if uncertain {
            "uncertain"
        } else {
            "failed_known"
        };
        ToolOutput {
            content: vec![ToolContent::Json(json!({
                "status": status,
                "target": FIXTURE_TARGET,
                "changed": verification.changed,
                "partial": verification.partial,
                "uncertain": uncertain
            }))],
            is_error: status != "ok",
        }
    }
}

fn fixture_arguments(
    policy: &RepositoryMutationPolicy,
    mode: RepositoryMutationFixtureTestMode,
) -> Vec<String> {
    let (mode, before_delay, after_delay) = match mode {
        RepositoryMutationFixtureTestMode::Normal => ("normal", 0, 0),
        RepositoryMutationFixtureTestMode::DelayBeforeMutation(delay) => {
            ("normal", delay.as_millis(), 0)
        }
        RepositoryMutationFixtureTestMode::DelayAfterMutation(delay) => {
            ("normal", 0, delay.as_millis())
        }
        RepositoryMutationFixtureTestMode::MutateSecondFile => ("second-file", 0, 0),
        RepositoryMutationFixtureTestMode::CreateExtraFile => ("create-extra", 0, 0),
        RepositoryMutationFixtureTestMode::DeleteProtectedFile => ("delete-protected", 0, 0),
        RepositoryMutationFixtureTestMode::MutateOutsideRoot => ("outside-root", 0, 0),
    };
    let outside = policy
        .outside_probe
        .as_ref()
        .map_or_else(String::new, |probe| probe.path.display().to_string());
    vec![
        "mutate-marker".to_owned(),
        policy.target.path.display().to_string(),
        mode.to_owned(),
        before_delay.to_string(),
        after_delay.to_string(),
        outside,
    ]
}

struct AuthorizedTarget {
    relative_path: PathBuf,
    path: PathBuf,
    parent_identity: FileIdentity,
    identity: FileIdentity,
}

impl AuthorizedTarget {
    fn capture(root: &Path, symbolic_target: &str, path: &Path) -> Result<Self, ToolError> {
        if symbolic_target != FIXTURE_TARGET {
            return Err(policy_error("unknown symbolic target"));
        }
        reject_link_or_reparse(path, "authorized target")?;
        let canonical = fs::canonicalize(path).map_err(filesystem_error)?;
        if !is_beneath(&canonical, root)
            || !fs::metadata(&canonical)
                .map_err(filesystem_error)?
                .is_file()
        {
            return Err(policy_error(
                "authorized target must be an existing regular file beneath root",
            ));
        }
        Ok(Self {
            relative_path: canonical
                .strip_prefix(root)
                .map_err(filesystem_error)?
                .to_path_buf(),
            parent_identity: FileIdentity::capture(
                canonical
                    .parent()
                    .ok_or_else(|| policy_error("authorized target has no parent"))?,
            )?,
            identity: FileIdentity::capture(&canonical)?,
            path: canonical,
        })
    }

    fn revalidate(&self, root: &Path) -> Result<(), ToolError> {
        reject_link_or_reparse(&self.path, "authorized target")?;
        let current = fs::canonicalize(&self.path).map_err(filesystem_error)?;
        let parent = current
            .parent()
            .ok_or_else(|| policy_error("authorized target has no parent"))?;
        if current != self.path
            || !is_beneath(&current, root)
            || !fs::metadata(&current).map_err(filesystem_error)?.is_file()
            || FileIdentity::capture(parent)? != self.parent_identity
            || FileIdentity::capture(&current)? != self.identity
        {
            return Err(policy_error("authorized target identity changed"));
        }
        Ok(())
    }
}

struct RepositoryState {
    snapshot: DirectorySnapshot,
    outside_probe: Option<Vec<u8>>,
}

struct OutsideProbe {
    path: PathBuf,
}

impl OutsideProbe {
    fn capture(path: PathBuf) -> Result<Self, ToolError> {
        let path = fs::canonicalize(path).map_err(filesystem_error)?;
        reject_link_or_reparse(&path, "outside probe")?;
        let _ = fs::read(&path).map_err(filesystem_error)?;
        Ok(Self { path })
    }

    fn capture_current(&self) -> Result<Vec<u8>, ToolError> {
        reject_link_or_reparse(&self.path, "outside probe")?;
        fs::read(&self.path).map_err(filesystem_error)
    }
}

struct DirectorySnapshot {
    files: BTreeMap<PathBuf, FileSnapshot>,
}

impl DirectorySnapshot {
    fn capture(root: &Path) -> Result<Self, ToolError> {
        let mut files = BTreeMap::new();
        capture_directory(root, root, &mut files, &mut 0)?;
        Ok(Self { files })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FileSnapshot {
    identity: FileIdentity,
    contents: Vec<u8>,
}

fn capture_directory(
    root: &Path,
    directory: &Path,
    files: &mut BTreeMap<PathBuf, FileSnapshot>,
    total_bytes: &mut usize,
) -> Result<(), ToolError> {
    reject_link_or_reparse(directory, "fixture directory")?;
    for entry in fs::read_dir(directory).map_err(filesystem_error)? {
        let entry = entry.map_err(filesystem_error)?;
        let path = entry.path();
        reject_link_or_reparse(&path, "fixture entry")?;
        let canonical = fs::canonicalize(&path).map_err(filesystem_error)?;
        if !is_beneath(&canonical, root) {
            return Err(policy_error(
                "fixture entry resolves outside repository root",
            ));
        }
        let metadata = fs::metadata(&canonical).map_err(filesystem_error)?;
        if metadata.is_dir() {
            capture_directory(root, &canonical, files, total_bytes)?;
        } else if metadata.is_file() {
            let contents = fs::read(&canonical).map_err(filesystem_error)?;
            *total_bytes = total_bytes.saturating_add(contents.len());
            if *total_bytes > MAX_SNAPSHOT_BYTES {
                return Err(policy_error("fixture snapshot exceeds its bounded size"));
            }
            let relative = canonical
                .strip_prefix(root)
                .map_err(filesystem_error)?
                .to_path_buf();
            files.insert(
                relative,
                FileSnapshot {
                    identity: FileIdentity::capture(&canonical)?,
                    contents,
                },
            );
        } else {
            return Err(policy_error(
                "fixture scope contains an unsupported entry type",
            ));
        }
    }
    Ok(())
}

struct Verification {
    authorized: bool,
    changed: bool,
    partial: bool,
    uncertain: bool,
    violation: bool,
}

impl Verification {
    fn uncertain(_message: String) -> Self {
        Self {
            authorized: false,
            changed: false,
            partial: false,
            uncertain: true,
            violation: false,
        }
    }
}

fn verify_postconditions(
    pre: &RepositoryState,
    post: &RepositoryState,
    target: &AuthorizedTarget,
) -> Verification {
    let mut changed_paths = pre
        .snapshot
        .files
        .keys()
        .chain(post.snapshot.files.keys())
        .collect::<Vec<_>>();
    changed_paths.sort();
    changed_paths.dedup();
    let changed_paths = changed_paths
        .into_iter()
        .filter(|path| pre.snapshot.files.get(*path) != post.snapshot.files.get(*path))
        .collect::<Vec<_>>();
    let target_changed = changed_paths.as_slice() == [&target.relative_path];
    let target_after = post.snapshot.files.get(&target.relative_path);
    let authorized =
        target_changed && target_after.is_some_and(|file| file.contents == MARKER_AFTER);
    let outside_changed = pre.outside_probe != post.outside_probe;
    let violation = !changed_paths.is_empty() && !authorized || outside_changed;
    Verification {
        authorized,
        changed: !changed_paths.is_empty() || outside_changed,
        partial: !authorized && (!changed_paths.is_empty() || outside_changed),
        uncertain: false,
        violation,
    }
}

struct ProcessState {
    failed: bool,
    timed_out: bool,
}

impl ProcessState {
    fn from_result(result: &Result<HostProcessOutput, ToolError>) -> Self {
        match result {
            Ok(output) => Self {
                failed: output.timed_out
                    || output.overflow.is_some()
                    || output.exit_code != Some(0),
                timed_out: output.timed_out,
            },
            Err(_) => Self {
                failed: true,
                timed_out: false,
            },
        }
    }
}

fn repository_lease(root: &Path) -> Arc<AsyncMutex<()>> {
    static LEASES: OnceLock<Mutex<HashMap<String, Weak<AsyncMutex<()>>>>> = OnceLock::new();
    let key = repository_key(root);
    let leases = LEASES.get_or_init(|| Mutex::new(HashMap::new()));
    let mut leases = leases
        .lock()
        .expect("repository lease registry mutex poisoned");
    if let Some(lease) = leases.get(&key).and_then(Weak::upgrade) {
        return lease;
    }
    let lease = Arc::new(AsyncMutex::new(()));
    leases.insert(key, Arc::downgrade(&lease));
    lease
}

fn repository_key(root: &Path) -> String {
    #[cfg(windows)]
    {
        root.to_string_lossy().to_ascii_lowercase()
    }
    #[cfg(not(windows))]
    {
        root.to_string_lossy().into_owned()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FileIdentity {
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
    #[cfg(windows)]
    volume_serial_number: u32,
    #[cfg(windows)]
    file_index: u64,
}

impl FileIdentity {
    fn capture(path: &Path) -> Result<Self, ToolError> {
        #[cfg(not(windows))]
        let metadata = fs::metadata(path).map_err(filesystem_error)?;
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
            let _ = metadata;
            Ok(Self {})
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
    let file = options.open(path).map_err(filesystem_error)?;
    // The handle remains owned by `file` throughout the Windows API call, and
    // the API initializes the out structure when it reports success.
    let information = unsafe {
        let mut information = std::mem::zeroed::<BY_HANDLE_FILE_INFORMATION>();
        if GetFileInformationByHandle(file.as_raw_handle(), &mut information) == 0 {
            return Err(filesystem_error(std::io::Error::last_os_error()));
        }
        information
    };
    Ok((
        information.dwVolumeSerialNumber,
        u64::from(information.nFileIndexHigh) << 32 | u64::from(information.nFileIndexLow),
    ))
}

fn canonical_directory(path: &Path, label: &str) -> Result<PathBuf, ToolError> {
    let canonical = fs::canonicalize(path).map_err(filesystem_error)?;
    if !canonical.is_dir() {
        return Err(policy_error(format!(
            "{label} must be an existing directory"
        )));
    }
    Ok(canonical)
}

fn reject_link_or_reparse(path: &Path, label: &str) -> Result<(), ToolError> {
    let metadata = fs::symlink_metadata(path).map_err(filesystem_error)?;
    if metadata.file_type().is_symlink() {
        return Err(policy_error(format!("{label} must not be a symbolic link")));
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;

        if metadata.file_attributes() & 0x400 != 0 {
            return Err(policy_error(format!("{label} must not be a reparse point")));
        }
    }
    Ok(())
}

fn is_beneath(path: &Path, root: &Path) -> bool {
    #[cfg(windows)]
    {
        let path = path.components().collect::<Vec<_>>();
        let root = root.components().collect::<Vec<_>>();
        path.len() >= root.len()
            && path
                .iter()
                .zip(root)
                .all(|(path, root)| path.as_os_str().eq_ignore_ascii_case(root.as_os_str()))
    }
    #[cfg(not(windows))]
    {
        path.starts_with(root)
    }
}

fn filesystem_error(error: impl std::fmt::Display) -> ToolError {
    policy_error(error.to_string())
}

fn policy_error(message: impl Into<String>) -> ToolError {
    ToolError::Execution {
        message: format!(
            "repository mutation policy rejected capability: {}",
            message.into()
        ),
    }
}
