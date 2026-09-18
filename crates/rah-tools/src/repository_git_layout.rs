//! Private, bounded identity evidence for supported Git worktree layouts.

use std::{
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

use rah_sandbox::{HostProcessOutput, OutputLimits};
use serde_json::json;

use crate::{
    HostArgumentPolicy, HostExecutionPolicy, ToolError, ToolInput,
    git_support::{git_error, repository_observer_environment},
};

const GITFILE_LIMIT: usize = 4096;
const RELATION_FILE_LIMIT: usize = 4096;
const REV_PARSE_LIMIT: usize = 16 * 1024;
const WORKTREE_LIST_LIMIT: usize = 1024 * 1024;
const PROBE_STDERR_LIMIT: usize = 16 * 1024;
const PROBE_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone, Debug, Eq, PartialEq)]
struct ObjectIdentity {
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
    #[cfg(windows)]
    volume_serial: u32,
    #[cfg(windows)]
    file_index: u64,
    #[cfg(not(any(unix, windows)))]
    length: u64,
}

impl ObjectIdentity {
    fn capture(path: &Path) -> Result<Self, ToolError> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let metadata = fs::metadata(path).map_err(|_| layout_error())?;
            Ok(Self {
                device: metadata.dev(),
                inode: metadata.ino(),
            })
        }
        #[cfg(windows)]
        {
            use std::os::windows::{fs::OpenOptionsExt, io::AsRawHandle};
            use windows_sys::Win32::Storage::FileSystem::{
                BY_HANDLE_FILE_INFORMATION, FILE_FLAG_BACKUP_SEMANTICS, GetFileInformationByHandle,
            };
            let file = fs::OpenOptions::new()
                .read(true)
                .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
                .open(path)
                .map_err(|_| layout_error())?;
            let mut information = std::mem::MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::zeroed();
            let result = unsafe {
                GetFileInformationByHandle(file.as_raw_handle(), information.as_mut_ptr())
            };
            if result == 0 {
                return Err(layout_error());
            }
            let information = unsafe { information.assume_init() };
            Ok(Self {
                volume_serial: information.dwVolumeSerialNumber,
                file_index: (u64::from(information.nFileIndexHigh) << 32)
                    | u64::from(information.nFileIndexLow),
            })
        }
        #[cfg(not(any(unix, windows)))]
        {
            let metadata = fs::metadata(path).map_err(|_| layout_error())?;
            Ok(Self {
                length: metadata.len(),
            })
        }
    }

    fn same_object(&self, other: &Self) -> bool {
        #[cfg(unix)]
        {
            self.device == other.device && self.inode == other.inode
        }
        #[cfg(windows)]
        {
            self.volume_serial == other.volume_serial && self.file_index == other.file_index
        }
        #[cfg(not(any(unix, windows)))]
        {
            self == other
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FileEvidence {
    path: PathBuf,
    identity: ObjectIdentity,
    bytes: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum LayoutEvidence {
    Main {
        git_dir: PathBuf,
        git_dir_identity: ObjectIdentity,
    },
    Linked(Box<LinkedEvidence>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LinkedEvidence {
    git_file: FileEvidence,
    private_git_dir: PathBuf,
    private_git_dir_identity: ObjectIdentity,
    common_git_dir: PathBuf,
    common_git_dir_identity: ObjectIdentity,
    worktrees_dir: PathBuf,
    worktrees_dir_identity: ObjectIdentity,
    registration_dir: PathBuf,
    registration_dir_identity: ObjectIdentity,
    commondir: FileEvidence,
    backlink: FileEvidence,
}

/// One canonical selected-root Git layout and its non-serialized relationship proof.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RepositoryGitLayout {
    root: PathBuf,
    root_identity: ObjectIdentity,
    git_executable: ExecutableEvidence,
    evidence: LayoutEvidence,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ExecutableEvidence {
    path: PathBuf,
    identity: ObjectIdentity,
    length: u64,
    modified: Option<SystemTime>,
}

impl ExecutableEvidence {
    fn capture(path: &Path) -> Result<Self, ToolError> {
        if !path.is_absolute() {
            return Err(layout_error());
        }
        reject_ancestry(path)?;
        let canonical = fs::canonicalize(path).map_err(|_| layout_error())?;
        reject_object(&canonical)?;
        let metadata = fs::metadata(&canonical).map_err(|_| layout_error())?;
        if !metadata.is_file() {
            return Err(layout_error());
        }
        Ok(Self {
            path: canonical.clone(),
            identity: ObjectIdentity::capture(&canonical)?,
            length: metadata.len(),
            modified: metadata.modified().ok(),
        })
    }

    fn same_current_object(&self, other: &Self) -> bool {
        self.path == other.path
            && self.identity.same_object(&other.identity)
            && self.length == other.length
            && self.modified == other.modified
    }
}

impl RepositoryGitLayout {
    pub(crate) fn capture(git_executable: &Path, root: &Path) -> Result<Self, ToolError> {
        if !root.is_absolute() || !git_executable.is_absolute() {
            return Err(layout_error());
        }
        reject_ancestry(root)?;
        let git_executable = ExecutableEvidence::capture(git_executable)?;
        let root = fs::canonicalize(root).map_err(|_| layout_error())?;
        let metadata = fs::symlink_metadata(&root).map_err(|_| layout_error())?;
        if !metadata.is_dir() || is_reparse(&metadata) {
            return Err(layout_error());
        }
        let git_entry = root.join(".git");
        reject_object(&git_entry)?;
        let git_metadata = fs::symlink_metadata(&git_entry).map_err(|_| layout_error())?;
        let evidence = if git_metadata.is_dir() {
            LayoutEvidence::Main {
                git_dir: fs::canonicalize(&git_entry).map_err(|_| layout_error())?,
                git_dir_identity: ObjectIdentity::capture(&git_entry)?,
            }
        } else if git_metadata.is_file() {
            Self::capture_linked(&git_entry)?
        } else {
            return Err(layout_error());
        };
        Ok(Self {
            root_identity: ObjectIdentity::capture(&root)?,
            root,
            git_executable,
            evidence,
        })
    }

    fn capture_linked(git_file_path: &Path) -> Result<LayoutEvidence, ToolError> {
        let git_file = capture_file(git_file_path, GITFILE_LIMIT)?;
        let git_dir_spelling = parse_single_record(&git_file.bytes, b"gitdir: ")?;
        let git_dir_spelling = resolve_recorded_path(
            git_file_path.parent().ok_or_else(layout_error)?,
            Path::new(git_dir_spelling),
        )?;
        reject_ancestry(&git_dir_spelling)?;
        let private_git_dir = fs::canonicalize(&git_dir_spelling).map_err(|_| layout_error())?;
        require_directory(&private_git_dir)?;

        let backlink_path = private_git_dir.join("gitdir");
        let commondir_path = private_git_dir.join("commondir");
        let backlink = capture_file(&backlink_path, RELATION_FILE_LIMIT)?;
        let commondir = capture_file(&commondir_path, RELATION_FILE_LIMIT)?;
        let backlink_target = parse_path_record(&backlink.bytes)?;
        let canonical_git_file = fs::canonicalize(git_file_path).map_err(|_| layout_error())?;
        let backlink_target = resolve_recorded_path(
            backlink_path.parent().ok_or_else(layout_error)?,
            &backlink_target,
        )?;
        reject_ancestry(&backlink_target)?;
        if fs::canonicalize(&backlink_target).map_err(|_| layout_error())? != canonical_git_file {
            return Err(layout_error());
        }
        let common_spelling = parse_single_record(&commondir.bytes, b"")?;
        if common_spelling != "../.." {
            return Err(layout_error());
        }
        let common_git_dir =
            fs::canonicalize(private_git_dir.join("../..")).map_err(|_| layout_error())?;
        require_directory(&common_git_dir)?;
        let worktrees_dir = private_git_dir
            .parent()
            .ok_or_else(layout_error)?
            .to_path_buf();
        if worktrees_dir
            .file_name()
            .is_none_or(|name| name != "worktrees")
            || fs::canonicalize(&worktrees_dir).map_err(|_| layout_error())?
                != common_git_dir.join("worktrees")
        {
            return Err(layout_error());
        }
        reject_ancestry(&worktrees_dir)?;
        require_directory(&worktrees_dir)?;
        let registration_dir = private_git_dir.clone();
        if registration_dir.parent() != Some(worktrees_dir.as_path())
            || registration_dir.file_name().is_none()
        {
            return Err(layout_error());
        }

        Ok(LayoutEvidence::Linked(Box::new(LinkedEvidence {
            git_file,
            private_git_dir_identity: ObjectIdentity::capture(&private_git_dir)?,
            common_git_dir_identity: ObjectIdentity::capture(&common_git_dir)?,
            worktrees_dir_identity: ObjectIdentity::capture(&worktrees_dir)?,
            registration_dir_identity: ObjectIdentity::capture(&registration_dir)?,
            commondir,
            backlink,
            private_git_dir,
            common_git_dir,
            worktrees_dir,
            registration_dir,
        })))
    }

    pub(crate) fn revalidate(&self) -> Result<(), ToolError> {
        let current_root = fs::canonicalize(&self.root)
            .map_err(|_| git_error("repository root identity changed"))?;
        let root_metadata = fs::symlink_metadata(&current_root)
            .map_err(|_| git_error("repository root identity changed"))?;
        if current_root != self.root
            || !root_metadata.is_dir()
            || is_reparse(&root_metadata)
            || !ObjectIdentity::capture(&current_root)
                .is_ok_and(|identity| identity.same_object(&self.root_identity))
        {
            return Err(git_error("repository root identity changed"));
        }
        let current_executable = ExecutableEvidence::capture(&self.git_executable.path)
            .map_err(|_| git_error("configured executable identity changed"))?;
        if !self.git_executable.same_current_object(&current_executable) {
            return Err(git_error("configured executable identity changed"));
        }
        let current = Self::capture(&self.git_executable.path, &self.root)?;
        if current.root != self.root
            || !current.root_identity.same_object(&self.root_identity)
            || !self
                .git_executable
                .same_current_object(&current.git_executable)
            || !same_evidence(&self.evidence, &current.evidence)
        {
            return Err(git_error("repository metadata identity changed"));
        }
        Ok(())
    }

    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    pub(crate) fn git_dir(&self) -> &Path {
        match &self.evidence {
            LayoutEvidence::Main { git_dir, .. } => git_dir,
            LayoutEvidence::Linked(linked) => &linked.private_git_dir,
        }
    }

    pub(crate) fn common_git_dir(&self) -> &Path {
        match &self.evidence {
            LayoutEvidence::Main { git_dir, .. } => git_dir,
            LayoutEvidence::Linked(linked) => &linked.common_git_dir,
        }
    }

    pub(crate) fn is_linked(&self) -> bool {
        matches!(self.evidence, LayoutEvidence::Linked(_))
    }

    pub(crate) fn index_path(&self) -> PathBuf {
        self.git_dir().join("index")
    }

    pub(crate) fn head_path(&self) -> PathBuf {
        self.git_dir().join("HEAD")
    }

    pub(crate) fn same_private_target(&self, other: &Self) -> bool {
        if self.git_dir() != other.git_dir() {
            return false;
        }
        let left = match &self.evidence {
            LayoutEvidence::Main {
                git_dir_identity, ..
            } => git_dir_identity,
            LayoutEvidence::Linked(linked) => &linked.private_git_dir_identity,
        };
        let right = match &other.evidence {
            LayoutEvidence::Main {
                git_dir_identity, ..
            } => git_dir_identity,
            LayoutEvidence::Linked(linked) => &linked.private_git_dir_identity,
        };
        left.same_object(right)
    }

    /// Runs fixed, bounded Git semantic probes and proves they agree with this layout.
    pub(crate) async fn validate_git(&self, git: &Path) -> Result<(), ToolError> {
        let supplied_git = ExecutableEvidence::capture(git)
            .map_err(|_| git_error("configured executable identity changed"))?;
        if !self.git_executable.same_current_object(&supplied_git) {
            return Err(git_error("configured executable identity changed"));
        }
        self.revalidate()?;
        let top = probe(
            &self.git_executable.path,
            &self.root,
            &["rev-parse", "--show-toplevel"],
            REV_PARSE_LIMIT,
        )
        .await?;
        let private = probe(
            &self.git_executable.path,
            &self.root,
            &["rev-parse", "--path-format=absolute", "--absolute-git-dir"],
            REV_PARSE_LIMIT,
        )
        .await?;
        let common = probe(
            &self.git_executable.path,
            &self.root,
            &["rev-parse", "--path-format=absolute", "--git-common-dir"],
            REV_PARSE_LIMIT,
        )
        .await?;
        let bare = probe(
            &self.git_executable.path,
            &self.root,
            &["rev-parse", "--is-bare-repository"],
            REV_PARSE_LIMIT,
        )
        .await?;
        let superproject = probe(
            &self.git_executable.path,
            &self.root,
            &["rev-parse", "--show-superproject-working-tree"],
            REV_PARSE_LIMIT,
        )
        .await?;
        let index = probe(
            &self.git_executable.path,
            &self.root,
            &["rev-parse", "--path-format=absolute", "--git-path", "index"],
            REV_PARSE_LIMIT,
        )
        .await?;
        let head = probe(
            &self.git_executable.path,
            &self.root,
            &["rev-parse", "--path-format=absolute", "--git-path", "HEAD"],
            REV_PARSE_LIMIT,
        )
        .await?;
        let worktrees = probe(
            &self.git_executable.path,
            &self.root,
            &["worktree", "list", "--porcelain", "-z"],
            WORKTREE_LIST_LIMIT,
        )
        .await?;
        if canonical_probe_path(&top)? != self.root
            || canonical_probe_path(&private)? != self.git_dir()
            || canonical_probe_path(&common)? != self.common_git_dir()
            || parse_line(&bare)? != "false"
            || !superproject.is_empty()
            || canonical_probe_path(&index)? != self.index_path()
            || canonical_probe_path(&head)? != self.head_path()
            || !selected_worktree_registered(&worktrees, &self.root)?
        {
            return Err(layout_error());
        }
        self.revalidate()?;
        Ok(())
    }
}

fn same_evidence(left: &LayoutEvidence, right: &LayoutEvidence) -> bool {
    match (left, right) {
        (
            LayoutEvidence::Main {
                git_dir: left_path,
                git_dir_identity: left_id,
            },
            LayoutEvidence::Main {
                git_dir: right_path,
                git_dir_identity: right_id,
            },
        ) => left_path == right_path && left_id.same_object(right_id),
        (LayoutEvidence::Linked(left), LayoutEvidence::Linked(right)) => {
            left.git_file == right.git_file
                && left.private_git_dir == right.private_git_dir
                && left
                    .private_git_dir_identity
                    .same_object(&right.private_git_dir_identity)
                && left.common_git_dir == right.common_git_dir
                && left
                    .common_git_dir_identity
                    .same_object(&right.common_git_dir_identity)
                && left.worktrees_dir == right.worktrees_dir
                && left
                    .worktrees_dir_identity
                    .same_object(&right.worktrees_dir_identity)
                && left.registration_dir == right.registration_dir
                && left
                    .registration_dir_identity
                    .same_object(&right.registration_dir_identity)
                && left.commondir == right.commondir
                && left.backlink == right.backlink
        }
        _ => false,
    }
}

fn capture_file(path: &Path, limit: usize) -> Result<FileEvidence, ToolError> {
    reject_object(path)?;
    let metadata = fs::symlink_metadata(path).map_err(|_| layout_error())?;
    if !metadata.is_file() || metadata.len() > limit as u64 {
        return Err(layout_error());
    }
    let canonical = fs::canonicalize(path).map_err(|_| layout_error())?;
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    File::open(&canonical)
        .map_err(|_| layout_error())?
        .take((limit + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| layout_error())?;
    if bytes.len() > limit || bytes.contains(&0) {
        return Err(layout_error());
    }
    Ok(FileEvidence {
        path: canonical.clone(),
        identity: ObjectIdentity::capture(&canonical)?,
        bytes,
    })
}

fn parse_single_record<'a>(bytes: &'a [u8], prefix: &[u8]) -> Result<&'a str, ToolError> {
    if bytes.is_empty() || bytes.iter().filter(|byte| **byte == b'\n').count() > 1 {
        return Err(layout_error());
    }
    let content = bytes.strip_suffix(b"\n").unwrap_or(bytes);
    let content = content.strip_prefix(prefix).ok_or_else(layout_error)?;
    let text = std::str::from_utf8(content).map_err(|_| layout_error())?;
    if text.is_empty() || text.contains('\r') || text.contains('\n') || text.contains('\0') {
        return Err(layout_error());
    }
    Ok(text)
}

fn parse_path_record(bytes: &[u8]) -> Result<PathBuf, ToolError> {
    let record = parse_single_record(bytes, b"")?;
    Ok(PathBuf::from(record))
}

fn resolve_recorded_path(base: &Path, spelling: &Path) -> Result<PathBuf, ToolError> {
    if spelling.is_absolute() {
        return Ok(spelling.to_path_buf());
    }

    #[cfg(windows)]
    if spelling.has_root()
        || matches!(
            spelling.components().next(),
            Some(std::path::Component::Prefix(_))
        )
    {
        return Err(layout_error());
    }

    Ok(base.join(spelling))
}

fn require_directory(path: &Path) -> Result<(), ToolError> {
    reject_ancestry(path)?;
    let metadata = fs::symlink_metadata(path).map_err(|_| layout_error())?;
    if !metadata.is_dir() || is_reparse(&metadata) {
        return Err(layout_error());
    }
    Ok(())
}

fn reject_ancestry(path: &Path) -> Result<(), ToolError> {
    for ancestor in path.ancestors() {
        if ancestor.exists() {
            reject_object(ancestor)?;
        }
    }
    Ok(())
}

fn reject_object(path: &Path) -> Result<(), ToolError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| layout_error())?;
    if metadata.file_type().is_symlink() || is_reparse(&metadata) {
        return Err(layout_error());
    }
    Ok(())
}

#[cfg(windows)]
fn is_reparse(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_attributes() & 0x400 != 0
}

#[cfg(not(windows))]
fn is_reparse(_: &fs::Metadata) -> bool {
    false
}

async fn probe(
    git: &Path,
    root: &Path,
    arguments: &[&str],
    stdout_limit: usize,
) -> Result<Vec<u8>, ToolError> {
    let policy = HostExecutionPolicy::new(
        git,
        HostArgumentPolicy::Exact(arguments.iter().map(|value| (*value).into()).collect()),
        root,
        ".",
    )?
    .with_environment(repository_observer_environment(root)?)?
    .with_timeout(PROBE_TIMEOUT)?
    .with_output_limits(OutputLimits {
        stdout_bytes: stdout_limit,
        stderr_bytes: PROBE_STDERR_LIMIT,
        combined_bytes: stdout_limit.saturating_add(PROBE_STDERR_LIMIT),
    })?;
    let output = policy.execute_process(&ToolInput(json!({}))).await?;
    successful_output(output, stdout_limit)
}

fn successful_output(output: HostProcessOutput, limit: usize) -> Result<Vec<u8>, ToolError> {
    if output.exit_code != Some(0)
        || output.timed_out
        || output.overflow.is_some()
        || output.stdout.len() > limit
    {
        return Err(layout_error());
    }
    Ok(output.stdout)
}

fn parse_line(bytes: &[u8]) -> Result<&str, ToolError> {
    let bytes = bytes.strip_suffix(b"\n").unwrap_or(bytes);
    if bytes.is_empty() || bytes.contains(&b'\n') || bytes.contains(&0) {
        return Err(layout_error());
    }
    std::str::from_utf8(bytes).map_err(|_| layout_error())
}

fn canonical_probe_path(bytes: &[u8]) -> Result<PathBuf, ToolError> {
    let path = PathBuf::from(parse_line(bytes)?);
    fs::canonicalize(path).map_err(|_| layout_error())
}

fn selected_worktree_registered(bytes: &[u8], root: &Path) -> Result<bool, ToolError> {
    let fields = bytes.split(|byte| *byte == 0).collect::<Vec<_>>();
    if fields.last().is_some_and(|field| !field.is_empty()) {
        return Err(layout_error());
    }
    let mut current = Vec::new();
    let mut matches = 0usize;
    for field in fields {
        if field.is_empty() {
            if !current.is_empty() {
                if worktree_record_matches(&current, root)? {
                    matches += 1;
                }
                current.clear();
            }
        } else {
            current.push(field);
        }
    }
    if !current.is_empty() && worktree_record_matches(&current, root)? {
        matches += 1;
    }
    Ok(matches == 1)
}

fn worktree_record_matches(fields: &[&[u8]], root: &Path) -> Result<bool, ToolError> {
    let mut path = None;
    let mut seen = std::collections::BTreeSet::new();
    let mut prunable = false;
    let mut bare = false;
    let mut head_valid = false;
    let mut attached = false;
    let mut detached = false;
    for field in fields {
        let (key, value) = match field.iter().position(|byte| *byte == b' ') {
            Some(space) => (&field[..space], Some(&field[space + 1..])),
            None => (*field, None),
        };
        if !seen.insert(key.to_vec()) {
            return Err(layout_error());
        }
        match (key, value) {
            (b"worktree", Some(value)) => {
                let text = std::str::from_utf8(value).map_err(|_| layout_error())?;
                path = Some(PathBuf::from(text));
            }
            (b"HEAD", Some(value)) => {
                let oid = std::str::from_utf8(value).map_err(|_| layout_error())?;
                head_valid = (oid.len() == 40 || oid.len() == 64)
                    && oid.bytes().all(|byte| byte.is_ascii_hexdigit());
            }
            (b"branch", Some(value)) => {
                attached = value.starts_with(b"refs/heads/") && value.len() > 11;
            }
            (b"locked", None | Some(_)) => {}
            (b"detached", None) => detached = true,
            (b"bare", None) => bare = true,
            (b"prunable", Some(_)) => prunable = true,
            _ => return Err(layout_error()),
        }
    }
    let Some(path) = path else {
        return Err(layout_error());
    };
    if !head_valid || attached == detached {
        return Err(layout_error());
    }
    let canonical = match fs::canonicalize(&path) {
        Ok(canonical) => canonical,
        Err(_) => return Ok(false),
    };
    if canonical == root && (prunable || bare) {
        return Err(layout_error());
    }
    Ok(canonical == root)
}

fn layout_error() -> ToolError {
    git_error("repository Git layout identity validation failed")
}

#[cfg(test)]
pub(crate) mod test_fixture {
    use std::{
        fs,
        path::{Path, PathBuf},
        process::{Command, Output},
        sync::atomic::{AtomicU64, Ordering},
    };

    static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

    pub(crate) struct WorktreeFixture {
        pub(crate) base: PathBuf,
        pub(crate) git: PathBuf,
        pub(crate) main: PathBuf,
        pub(crate) linked_a: PathBuf,
        pub(crate) linked_b: PathBuf,
    }

    impl WorktreeFixture {
        pub(crate) fn new() -> Self {
            let id = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
            let base = std::env::temp_dir().join(format!(
                "rah-linked-worktree-{}-{id}-{}",
                std::process::id(),
                uuid::Uuid::new_v4()
            ));
            let main = base.join("main");
            let linked_a = base.join("linked-a");
            let linked_b = base.join("linked-b");
            fs::create_dir_all(&main).expect("create isolated worktree fixture root");
            let git = native_git();
            run(&git, &main, &["init", "--quiet", "--initial-branch=main"]);
            run(&git, &main, &["config", "user.name", "RAH Test"]);
            run(
                &git,
                &main,
                &["config", "user.email", "rah@example.invalid"],
            );
            run(&git, &main, &["config", "core.autocrlf", "false"]);
            fs::write(main.join("tracked.txt"), b"initial\n").expect("write tracked fixture");
            run(&git, &main, &["add", "--", "tracked.txt"]);
            run(&git, &main, &["commit", "--quiet", "-m", "initial"]);
            run(
                &git,
                &main,
                &[
                    "worktree",
                    "add",
                    "--quiet",
                    "-b",
                    "linked-a",
                    linked_a.to_str().unwrap(),
                ],
            );
            run(
                &git,
                &main,
                &[
                    "worktree",
                    "add",
                    "--quiet",
                    "-b",
                    "linked-b",
                    linked_b.to_str().unwrap(),
                ],
            );
            Self {
                base,
                git,
                main,
                linked_a,
                linked_b,
            }
        }
    }

    impl Drop for WorktreeFixture {
        fn drop(&mut self) {
            if self.base.exists() {
                fs::remove_dir_all(&self.base).expect("remove only this linked-worktree fixture");
            }
        }
    }

    pub(crate) fn native_git() -> PathBuf {
        #[cfg(windows)]
        let output = Command::new("where.exe").arg("git.exe").output().unwrap();
        #[cfg(not(windows))]
        let output = Command::new("which").arg("git").output().unwrap();
        assert!(
            output.status.success(),
            "native Git is required by deterministic fixtures"
        );
        let path = String::from_utf8(output.stdout).unwrap();
        fs::canonicalize(path.lines().next().unwrap()).unwrap()
    }

    pub(crate) fn run(git: &Path, root: &Path, arguments: &[&str]) -> Output {
        let output = Command::new(git)
            .args(arguments)
            .current_dir(root)
            .output()
            .expect("run native Git fixture command");
        assert!(
            output.status.success(),
            "Git fixture command failed: {:?}: {}",
            arguments,
            String::from_utf8_lossy(&output.stderr)
        );
        output
    }
}

#[cfg(test)]
mod tests {
    use std::process::Command;

    use super::{RepositoryGitLayout, selected_worktree_registered, test_fixture::WorktreeFixture};
    use crate::{RepositoryAdmissionIdentity, RepositoryAdmissionRelation};

    struct ParserRoot(std::path::PathBuf);

    impl ParserRoot {
        fn new() -> Self {
            let path = std::env::temp_dir()
                .join(format!("rah-worktree-list-parser-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir(&path).unwrap();
            Self(std::fs::canonicalize(path).unwrap())
        }
    }

    impl Drop for ParserRoot {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).unwrap();
        }
    }

    fn porcelain_record(root: &std::path::Path, tail: &str) -> String {
        format!(
            "worktree {}\0HEAD {}\0{}\0\0",
            root.display(),
            "0123456789abcdef0123456789abcdef01234567",
            tail
        )
    }

    #[test]
    fn worktree_list_parser_rejects_malformed_and_ambiguous_records() {
        let root = ParserRoot::new();
        let valid = porcelain_record(&root.0, "branch refs/heads/topic");
        assert!(selected_worktree_registered(valid.as_bytes(), &root.0).unwrap());
        assert!(
            selected_worktree_registered(
                porcelain_record(&root.0, "branch refs/heads/topic\0locked portable media")
                    .as_bytes(),
                &root.0
            )
            .unwrap()
        );

        let missing_worktree = format!(
            "HEAD {}\0branch refs/heads/topic\0\0",
            "0123456789abcdef0123456789abcdef01234567"
        );
        let missing_head = format!("worktree {}\0branch refs/heads/topic\0\0", root.0.display());
        let duplicate_head = format!(
            "worktree {}\0HEAD {}\0HEAD {}\0branch refs/heads/topic\0\0",
            root.0.display(),
            "0123456789abcdef0123456789abcdef01234567",
            "0123456789abcdef0123456789abcdef01234567"
        );
        let malformed_oid = format!(
            "worktree {}\0HEAD not-an-object-id\0branch refs/heads/topic\0\0",
            root.0.display()
        );
        let both_attached_and_detached = format!(
            "worktree {}\0HEAD {}\0branch refs/heads/topic\0detached\0\0",
            root.0.display(),
            "0123456789abcdef0123456789abcdef01234567"
        );
        let neither_attached_nor_detached = format!(
            "worktree {}\0HEAD {}\0\0",
            root.0.display(),
            "0123456789abcdef0123456789abcdef01234567"
        );
        let unknown_field = format!(
            "worktree {}\0HEAD {}\0branch refs/heads/topic\0unknown-field\0\0",
            root.0.display(),
            "0123456789abcdef0123456789abcdef01234567"
        );
        let prunable = porcelain_record(&root.0, "branch refs/heads/topic\0prunable missing");
        let truncated = valid.trim_end_matches('\0').to_owned();
        for malformed in [
            missing_worktree,
            missing_head,
            duplicate_head,
            malformed_oid,
            both_attached_and_detached,
            neither_attached_nor_detached,
            unknown_field,
            prunable,
            truncated,
        ] {
            assert!(
                selected_worktree_registered(malformed.as_bytes(), &root.0).is_err(),
                "malformed worktree list record unexpectedly passed: {malformed:?}"
            );
        }

        assert!(
            !selected_worktree_registered(format!("{valid}{valid}").as_bytes(), &root.0).unwrap()
        );
        assert!(selected_worktree_registered(b"", &root.0).is_ok_and(|matched| !matched));
    }

    #[cfg(unix)]
    #[test]
    fn worktree_list_parser_fails_closed_for_non_utf8_path_fields() {
        let root = ParserRoot::new();
        let mut record = b"worktree ".to_vec();
        record.extend_from_slice(&[0xff]);
        record.extend_from_slice(
            b"\0HEAD 0123456789abcdef0123456789abcdef01234567\0branch refs/heads/topic\0\0",
        );
        assert!(selected_worktree_registered(&record, &root.0).is_err());
    }

    #[tokio::test]
    async fn relative_linked_gitfile_and_backlink_are_validated_by_git_semantics() {
        let fixture = WorktreeFixture::new();
        let layout = RepositoryGitLayout::capture(&fixture.git, &fixture.linked_a).unwrap();
        let private = layout.git_dir().to_path_buf();
        let root_gitfile = fixture.linked_a.join(".git");
        let backlink = private.join("gitdir");
        let original_gitfile = std::fs::read(&root_gitfile).unwrap();
        let original_backlink = std::fs::read(&backlink).unwrap();

        let registration = private.file_name().unwrap().to_string_lossy();
        std::fs::write(
            &root_gitfile,
            format!("gitdir: ../main/.git/worktrees/{registration}\n"),
        )
        .unwrap();
        std::fs::write(&backlink, b"../../../../linked-a/.git\n").unwrap();

        let identity = RepositoryAdmissionIdentity::capture(&fixture.git, &fixture.linked_a)
            .expect("relative Git linking records resolve to the retained standard layout");
        identity
            .validate_git()
            .await
            .expect("native Git semantic probes should confirm relative linking records");
        identity
            .revalidate(&fixture.git, &fixture.linked_a)
            .expect("relative linking record bytes should remain current");

        std::fs::write(&root_gitfile, original_gitfile).unwrap();
        std::fs::write(&backlink, original_backlink).unwrap();
    }

    #[tokio::test]
    async fn real_main_and_linked_worktrees_validate_and_share_only_common_identity() {
        let fixture = WorktreeFixture::new();
        let identities = [&fixture.main, &fixture.linked_a, &fixture.linked_b]
            .map(|root| RepositoryAdmissionIdentity::capture(&fixture.git, root).unwrap());
        for identity in &identities {
            identity.validate_git().await.unwrap_or_else(|_| {
                panic!(
                    "identity probe failed for {}",
                    identity.canonical_root().display()
                )
            });
        }
        let alias = RepositoryAdmissionIdentity::capture(
            &fixture.git,
            fixture.linked_a.join("..").join("linked-a"),
        )
        .unwrap();
        assert_eq!(
            identities[1].relation(&alias),
            RepositoryAdmissionRelation::Same
        );
        for left in 0..identities.len() {
            for right in left + 1..identities.len() {
                assert_eq!(
                    identities[left].relation(&identities[right]),
                    RepositoryAdmissionRelation::Distinct
                );
            }
        }
        let layouts = [&fixture.main, &fixture.linked_a, &fixture.linked_b]
            .map(|root| RepositoryGitLayout::capture(&fixture.git, root).unwrap());
        assert_eq!(layouts[0].common_git_dir(), layouts[1].common_git_dir());
        assert_eq!(layouts[1].common_git_dir(), layouts[2].common_git_dir());
        assert_ne!(layouts[0].git_dir(), layouts[1].git_dir());
        assert_ne!(layouts[1].git_dir(), layouts[2].git_dir());
    }

    #[test]
    fn gitfile_content_change_is_detected_even_when_the_file_object_is_retained() {
        let fixture = WorktreeFixture::new();
        let identity =
            RepositoryAdmissionIdentity::capture(&fixture.git, &fixture.linked_a).unwrap();
        let gitfile = fixture.linked_a.join(".git");
        let old = std::fs::read(&gitfile).unwrap();
        let file_identity = super::ObjectIdentity::capture(&gitfile).unwrap();
        let offset = old.len() - 2;
        let replacement = if old[offset] == b'a' { b'b' } else { b'a' };
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .open(&gitfile)
            .unwrap();
        use std::io::{Seek, SeekFrom, Write};
        file.seek(SeekFrom::Start(offset as u64)).unwrap();
        file.write_all(&[replacement]).unwrap();
        file.sync_all().unwrap();
        assert!(file_identity.same_object(&super::ObjectIdentity::capture(&gitfile).unwrap()));
        drop(file);
        assert!(
            identity
                .revalidate(&fixture.git, &fixture.linked_a)
                .is_err()
        );
        std::fs::write(&gitfile, old).unwrap();
        identity
            .revalidate(&fixture.git, &fixture.linked_a)
            .unwrap();
    }

    #[test]
    fn root_gitfile_and_git_executable_replacements_stale_retained_identity() {
        let fixture = WorktreeFixture::new();
        let identity =
            RepositoryAdmissionIdentity::capture(&fixture.git, &fixture.linked_a).unwrap();

        let gitfile = fixture.linked_a.join(".git");
        let retained_gitfile = fixture.base.join("retained-linked-a-gitfile");
        std::fs::rename(&gitfile, &retained_gitfile).unwrap();
        std::fs::copy(&retained_gitfile, &gitfile).unwrap();
        assert!(
            identity
                .revalidate(&fixture.git, &fixture.linked_a)
                .is_err()
        );

        let root_backup = fixture.base.join("retained-linked-a-root");
        std::fs::rename(&fixture.linked_a, &root_backup).unwrap();
        std::fs::create_dir(&fixture.linked_a).unwrap();
        std::fs::copy(root_backup.join(".git"), fixture.linked_a.join(".git")).unwrap();
        assert!(
            identity
                .revalidate(&fixture.git, &fixture.linked_a)
                .is_err()
        );

        let copied_git = fixture.base.join("git-binding.exe");
        std::fs::copy(&fixture.git, &copied_git).unwrap();
        let git_identity =
            RepositoryAdmissionIdentity::capture(&copied_git, &fixture.linked_b).unwrap();
        let retained_git = fixture.base.join("retained-git-binding.exe");
        std::fs::rename(&copied_git, &retained_git).unwrap();
        std::fs::copy(&fixture.git, &copied_git).unwrap();
        assert!(
            git_identity
                .revalidate(&copied_git, &fixture.linked_b)
                .is_err()
        );
    }

    #[test]
    fn copied_fabricated_malformed_and_separate_git_dir_forms_fail_closed() {
        let fixture = WorktreeFixture::new();
        let copied_root = fixture.base.join("copied-root");
        std::fs::create_dir(&copied_root).unwrap();
        std::fs::copy(fixture.linked_a.join(".git"), copied_root.join(".git")).unwrap();
        let copied_git_recognition = Command::new(&fixture.git)
            .args(["rev-parse", "--is-inside-work-tree"])
            .current_dir(&copied_root)
            .output()
            .unwrap();
        assert!(copied_git_recognition.status.success());
        assert!(RepositoryGitLayout::capture(&fixture.git, &copied_root).is_err());

        let fabricated_root = fixture.base.join("fabricated-root");
        std::fs::create_dir(&fabricated_root).unwrap();
        std::fs::write(
            fabricated_root.join(".git"),
            format!("gitdir: {}\n", fixture.main.join(".git").display()),
        )
        .unwrap();
        assert!(RepositoryGitLayout::capture(&fixture.git, &fabricated_root).is_err());

        let malformed_root = fixture.base.join("malformed-root");
        std::fs::create_dir(&malformed_root).unwrap();
        std::fs::write(malformed_root.join(".git"), b"gitdir: one\ngitdir: two\n").unwrap();
        assert!(RepositoryGitLayout::capture(&fixture.git, &malformed_root).is_err());
        std::fs::write(
            malformed_root.join(".git"),
            [b"gitdir: ".as_slice(), &[0], b"x\n"].concat(),
        )
        .unwrap();
        assert!(RepositoryGitLayout::capture(&fixture.git, &malformed_root).is_err());
        std::fs::write(
            malformed_root.join(".git"),
            vec![b'x'; super::GITFILE_LIMIT + 1],
        )
        .unwrap();
        assert!(RepositoryGitLayout::capture(&fixture.git, &malformed_root).is_err());

        let separate_root = fixture.base.join("separate-root");
        let separate_git = fixture.base.join("separate-metadata");
        std::fs::create_dir(&separate_root).unwrap();
        super::test_fixture::run(
            &fixture.git,
            &separate_root,
            &[
                "init",
                "--quiet",
                "--separate-git-dir",
                separate_git.to_str().unwrap(),
            ],
        );
        let separate_git_recognition = Command::new(&fixture.git)
            .args(["rev-parse", "--is-inside-work-tree"])
            .current_dir(&separate_root)
            .output()
            .unwrap();
        assert!(separate_git_recognition.status.success());
        assert!(RepositoryGitLayout::capture(&fixture.git, &separate_root).is_err());
    }

    #[test]
    fn real_modern_submodule_is_not_misclassified_as_a_linked_worktree() {
        let fixture = WorktreeFixture::new();
        let submodule_source = fixture.base.join("submodule-source");
        std::fs::create_dir(&submodule_source).unwrap();
        super::test_fixture::run(&fixture.git, &submodule_source, &["init", "--quiet"]);
        super::test_fixture::run(
            &fixture.git,
            &submodule_source,
            &["config", "user.name", "RAH Test"],
        );
        super::test_fixture::run(
            &fixture.git,
            &submodule_source,
            &["config", "user.email", "rah@example.invalid"],
        );
        std::fs::write(submodule_source.join("module.txt"), b"module\n").unwrap();
        super::test_fixture::run(
            &fixture.git,
            &submodule_source,
            &["add", "--", "module.txt"],
        );
        super::test_fixture::run(
            &fixture.git,
            &submodule_source,
            &["commit", "--quiet", "-m", "module"],
        );
        super::test_fixture::run(
            &fixture.git,
            &fixture.main,
            &[
                "-c",
                "protocol.file.allow=always",
                "submodule",
                "add",
                "--quiet",
                submodule_source.to_str().unwrap(),
                "nested-module",
            ],
        );
        super::test_fixture::run(
            &fixture.git,
            &fixture.main,
            &["add", "--", ".gitmodules", "nested-module"],
        );
        super::test_fixture::run(
            &fixture.git,
            &fixture.main,
            &["commit", "--quiet", "-m", "add submodule"],
        );
        let submodule = fixture.main.join("nested-module");
        let recognized = Command::new(&fixture.git)
            .args(["rev-parse", "--show-superproject-working-tree"])
            .current_dir(&submodule)
            .output()
            .unwrap();
        assert!(recognized.status.success());
        assert!(!recognized.stdout.is_empty());
        assert!(RepositoryGitLayout::capture(&fixture.git, &submodule).is_err());
    }

    #[tokio::test]
    async fn old_form_nested_submodule_with_dot_git_directory_is_rejected() {
        let fixture = WorktreeFixture::new();
        let submodule = fixture.main.join("old-form-module");
        std::fs::create_dir(&submodule).unwrap();
        super::test_fixture::run(&fixture.git, &submodule, &["init", "--quiet"]);
        super::test_fixture::run(
            &fixture.git,
            &submodule,
            &["config", "user.name", "RAH Test"],
        );
        super::test_fixture::run(
            &fixture.git,
            &submodule,
            &["config", "user.email", "rah@example.invalid"],
        );
        std::fs::write(submodule.join("module.txt"), b"old-form module\n").unwrap();
        super::test_fixture::run(&fixture.git, &submodule, &["add", "--", "module.txt"]);
        super::test_fixture::run(
            &fixture.git,
            &submodule,
            &["commit", "--quiet", "-m", "old-form module"],
        );
        let submodule_oid = String::from_utf8(
            super::test_fixture::run(&fixture.git, &submodule, &["rev-parse", "HEAD"]).stdout,
        )
        .unwrap()
        .trim()
        .to_owned();
        std::fs::write(
            fixture.main.join(".gitmodules"),
            b"[submodule \"old-form-module\"]\n\tpath = old-form-module\n\turl = ../old-form-module\n",
        )
        .unwrap();
        let gitlink = format!("160000,{submodule_oid},old-form-module");
        super::test_fixture::run(
            &fixture.git,
            &fixture.main,
            &["update-index", "--add", "--cacheinfo", &gitlink],
        );
        super::test_fixture::run(&fixture.git, &fixture.main, &["add", "--", ".gitmodules"]);
        super::test_fixture::run(
            &fixture.git,
            &fixture.main,
            &["commit", "--quiet", "-m", "add old-form submodule"],
        );

        assert!(submodule.join(".git").is_dir());
        let recognized = Command::new(&fixture.git)
            .args(["rev-parse", "--show-superproject-working-tree"])
            .current_dir(&submodule)
            .output()
            .unwrap();
        assert!(recognized.status.success());
        assert!(!recognized.stdout.is_empty());
        let identity = RepositoryAdmissionIdentity::capture(&fixture.git, &submodule).unwrap();
        assert!(identity.validate_git().await.is_err());
    }

    #[cfg(windows)]
    #[test]
    fn reparse_root_git_entry_private_common_and_registration_are_rejected() {
        fn junction(path: &std::path::Path, target: &std::path::Path) {
            let status = Command::new("cmd.exe")
                .args(["/c", "mklink", "/J"])
                .arg(path)
                .arg(target)
                .status()
                .expect("mklink should start");
            assert!(status.success(), "junction fixture should be created");
        }

        let fixture = WorktreeFixture::new();
        let root_alias = fixture.base.join("root-junction");
        junction(&root_alias, &fixture.linked_a);
        assert!(RepositoryGitLayout::capture(&fixture.git, &root_alias).is_err());
        std::fs::remove_dir(&root_alias).unwrap();

        let candidate = fixture.base.join("git-junction-root");
        std::fs::create_dir(&candidate).unwrap();
        junction(&candidate.join(".git"), &fixture.main.join(".git"));
        assert!(RepositoryGitLayout::capture(&fixture.git, &candidate).is_err());
        std::fs::remove_dir(candidate.join(".git")).unwrap();

        let original_layout =
            RepositoryGitLayout::capture(&fixture.git, &fixture.linked_a).unwrap();
        let private = original_layout.git_dir().to_path_buf();
        let private_backup = private.with_file_name("registration-backup");
        std::fs::rename(&private, &private_backup).unwrap();
        junction(&private, &private_backup);
        let private_rejected =
            RepositoryGitLayout::capture(&fixture.git, &fixture.linked_a).is_err();
        std::fs::remove_dir(&private).unwrap();
        std::fs::rename(&private_backup, &private).unwrap();
        assert!(private_rejected);

        let common = original_layout.common_git_dir().to_path_buf();
        let common_backup = common.with_file_name("common-backup");
        std::fs::rename(&common, &common_backup).unwrap();
        junction(&common, &common_backup);
        let common_rejected =
            RepositoryGitLayout::capture(&fixture.git, &fixture.linked_a).is_err();
        std::fs::remove_dir(&common).unwrap();
        std::fs::rename(&common_backup, &common).unwrap();
        assert!(common_rejected);
    }

    #[tokio::test]
    async fn linked_relationship_metadata_content_is_currentness_evidence() {
        let fixture = WorktreeFixture::new();
        let identity =
            RepositoryAdmissionIdentity::capture(&fixture.git, &fixture.linked_a).unwrap();
        let layout = RepositoryGitLayout::capture(&fixture.git, &fixture.linked_a).unwrap();
        let private = layout.git_dir().to_path_buf();
        for metadata in [private.join("commondir"), private.join("gitdir")] {
            let original = std::fs::read(&metadata).unwrap();
            let mut changed = original.clone();
            let last = changed.len() - 2;
            changed[last] = if changed[last] == b'x' { b'y' } else { b'x' };
            std::fs::write(&metadata, &changed).unwrap();
            assert!(
                identity
                    .revalidate(&fixture.git, &fixture.linked_a)
                    .is_err()
            );
            std::fs::write(&metadata, original).unwrap();
            identity
                .revalidate(&fixture.git, &fixture.linked_a)
                .unwrap();
        }
    }

    #[test]
    fn stale_identity_errors_do_not_include_private_git_paths() {
        let fixture = WorktreeFixture::new();
        let identity =
            RepositoryAdmissionIdentity::capture(&fixture.git, &fixture.linked_a).unwrap();
        let layout = RepositoryGitLayout::capture(&fixture.git, &fixture.linked_a).unwrap();
        let private = layout.git_dir().to_path_buf();
        let common = layout.common_git_dir().to_path_buf();
        std::fs::write(
            private.join("commondir"),
            b"RAH_V030_COMMON_PATH_SENTINEL\n",
        )
        .unwrap();
        let error = identity
            .revalidate(&fixture.git, &fixture.linked_a)
            .unwrap_err();
        let debug = format!("{error:?}");
        let display = error.to_string();
        let forbidden = [
            "RAH_V030_COMMON_PATH_SENTINEL".to_owned(),
            private.display().to_string(),
            common.display().to_string(),
            fixture.linked_a.display().to_string(),
        ];
        for forbidden in &forbidden {
            assert!(!debug.contains(forbidden));
            assert!(!display.contains(forbidden));
        }
    }

    #[test]
    fn private_common_and_registration_replacement_stales_retained_identity() {
        let fixture = WorktreeFixture::new();
        let identity =
            RepositoryAdmissionIdentity::capture(&fixture.git, &fixture.linked_a).unwrap();
        let layout = RepositoryGitLayout::capture(&fixture.git, &fixture.linked_a).unwrap();
        let private = layout.git_dir().to_path_buf();
        let private_backup = fixture.base.join("private-backup");
        std::fs::rename(&private, &private_backup).unwrap();
        std::fs::create_dir(&private).unwrap();
        assert!(
            identity
                .revalidate(&fixture.git, &fixture.linked_a)
                .is_err()
        );
        std::fs::remove_dir(&private).unwrap();
        std::fs::rename(&private_backup, &private).unwrap();
        identity
            .revalidate(&fixture.git, &fixture.linked_a)
            .unwrap();

        let common = layout.common_git_dir().to_path_buf();
        let common_backup = fixture.base.join("common-backup");
        std::fs::rename(&common, &common_backup).unwrap();
        std::fs::create_dir(&common).unwrap();
        assert!(
            identity
                .revalidate(&fixture.git, &fixture.linked_a)
                .is_err()
        );
        std::fs::remove_dir(&common).unwrap();
        std::fs::rename(&common_backup, &common).unwrap();
        identity
            .revalidate(&fixture.git, &fixture.linked_a)
            .unwrap();
    }

    #[tokio::test]
    async fn coherent_locked_worktree_is_admitted_without_unlocking() {
        let fixture = WorktreeFixture::new();
        let identity =
            RepositoryAdmissionIdentity::capture(&fixture.git, &fixture.linked_a).unwrap();
        super::test_fixture::run(
            &fixture.git,
            &fixture.main,
            &["worktree", "lock", fixture.linked_a.to_str().unwrap()],
        );
        identity.validate_git().await.unwrap();
    }

    #[tokio::test]
    async fn externally_removed_linked_registration_stales_retained_identity() {
        let fixture = WorktreeFixture::new();
        let identity =
            RepositoryAdmissionIdentity::capture(&fixture.git, &fixture.linked_a).unwrap();
        super::test_fixture::run(
            &fixture.git,
            &fixture.main,
            &[
                "worktree",
                "remove",
                "--force",
                fixture.linked_a.to_str().unwrap(),
            ],
        );
        assert!(
            identity
                .revalidate_git(&fixture.git, &fixture.linked_a)
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn externally_moved_linked_worktree_does_not_migrate_retained_identity() {
        let fixture = WorktreeFixture::new();
        let identity =
            RepositoryAdmissionIdentity::capture(&fixture.git, &fixture.linked_a).unwrap();
        let moved = fixture.base.join("moved-a");
        super::test_fixture::run(
            &fixture.git,
            &fixture.main,
            &[
                "worktree",
                "move",
                fixture.linked_a.to_str().unwrap(),
                moved.to_str().unwrap(),
            ],
        );
        assert!(
            identity
                .revalidate_git(&fixture.git, &fixture.linked_a)
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn detached_linked_worktree_is_valid_but_does_not_gain_commit_support() {
        let fixture = WorktreeFixture::new();
        let detached = fixture.base.join("detached");
        super::test_fixture::run(
            &fixture.git,
            &fixture.main,
            &[
                "worktree",
                "add",
                "--quiet",
                "--detach",
                detached.to_str().unwrap(),
                "HEAD",
            ],
        );
        let identity = RepositoryAdmissionIdentity::capture(&fixture.git, &detached).unwrap();
        identity.validate_git().await.unwrap();
        let (_tool, control) = crate::RepositoryCommitTool::compose(
            &fixture.git,
            &detached,
            "RAH Test".into(),
            "rah@example.invalid".into(),
        )
        .unwrap();
        assert!(control.review_current_staged_snapshot().await.is_err());
    }
}
