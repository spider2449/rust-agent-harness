//! Private, host-owned foundation for one reviewed normal Git commit.
//!
//! This module intentionally has no tool or public API.  A later composition
//! task may adapt its narrow internal authority; it must not turn this into a
//! generic Git executor.

#![allow(dead_code)] // Private foundation awaiting host-only composition in Task 137.

use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

use futures::lock::MutexGuard;
use rah_sandbox::{HostProcessOutput, OutputLimits};
use sha2::{Digest, Sha256};

use crate::{
    HostArgumentPolicy, HostExecutionPolicy, ToolError,
    git_stage::repository_lease,
    git_support::git_error,
    repository_observer::{FileIdentity, RepositoryIdentity},
};

const MESSAGE_LIMIT: usize = 16 * 1024;
const COMMIT_TIMEOUT: Duration = Duration::from_secs(15);
const OUTPUT_LIMITS: OutputLimits = OutputLimits {
    stdout_bytes: 32 * 1024,
    stderr_bytes: 32 * 1024,
    combined_bytes: 64 * 1024,
};

/// The only private outcomes of one bounded commit attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CommitDisposition {
    InvalidInput,
    PreconditionFailed,
    KnownNoEffect,
    CommittedVerified,
    Uncertain,
}

#[derive(Clone, Debug)]
struct HostIdentity {
    name: String,
    email: String,
}

impl HostIdentity {
    fn new(name: String, email: String) -> Result<Self, ToolError> {
        for (label, value) in [("name", &name), ("email", &email)] {
            if value.is_empty() || value.trim().is_empty() || value.contains('\0') {
                return Err(git_error(format!("host commit {label} is invalid")));
            }
        }
        Ok(Self { name, email })
    }
}

/// Host-created, in-memory authorization for exactly one reviewed index state.
/// It is consumed by value, cannot be reconstructed from hashes, and remains
/// private until a future host-only composition task needs it.
struct ReviewedCommitAuthorization {
    generation: uuid::Uuid,
    branch: String,
    old_head: String,
    raw_index_sha256: [u8; 32],
    staged_entries_sha256: [u8; 32],
    tree: String,
}

struct CommitSnapshot {
    branch: String,
    old_head: String,
    raw_index_sha256: [u8; 32],
    staged_entries_sha256: [u8; 32],
    tree: String,
}

/// One host-selected repository, executable, identity, and empty hook root.
/// Construction is deliberately private: no model input can select any field.
struct RepositoryCommitPolicy {
    repository: RepositoryIdentity,
    git: PathBuf,
    git_binding: HostExecutionPolicy,
    identity: HostIdentity,
    hooks: PathBuf,
    hooks_identity: FileIdentity,
    generation: uuid::Uuid,
    lease: std::sync::Arc<futures::lock::Mutex<()>>,
    #[cfg(test)]
    attempts: std::sync::atomic::AtomicUsize,
}

impl RepositoryCommitPolicy {
    fn new(git: &Path, root: &Path, name: String, email: String) -> Result<Self, ToolError> {
        let repository = RepositoryIdentity::capture(root)?;
        let dot_git = repository.root().join(".git");
        if !fs::metadata(&dot_git).map_err(io_error)?.is_dir() {
            return Err(git_error(
                "linked worktrees and .git indirection are unsupported",
            ));
        }
        let hooks = unique_empty_hooks_directory()?;
        // HostExecutionPolicy captures and validates the exact native executable.
        let git_binding = HostExecutionPolicy::new(
            git,
            HostArgumentPolicy::Exact(vec!["--version".into()]),
            repository.root(),
            ".",
        )?;
        let lease = repository_lease(repository.root());
        Ok(Self {
            repository,
            git: fs::canonicalize(git).map_err(io_error)?,
            git_binding,
            identity: HostIdentity::new(name, email)?,
            hooks_identity: FileIdentity::capture(&hooks)?,
            hooks,
            generation: uuid::Uuid::new_v4(),
            lease,
            #[cfg(test)]
            attempts: std::sync::atomic::AtomicUsize::new(0),
        })
    }

    async fn acquire_lease(&self) -> MutexGuard<'_, ()> {
        self.lease.lock().await
    }

    async fn authorize(&self) -> Result<ReviewedCommitAuthorization, ToolError> {
        let _lease = self.acquire_lease().await;
        let snapshot = self.capture_snapshot().await?;
        #[cfg(test)]
        test_phase::hit(test_phase::Phase::AfterAuthorization, self)?;
        Ok(ReviewedCommitAuthorization {
            generation: self.generation,
            branch: snapshot.branch,
            old_head: snapshot.old_head,
            raw_index_sha256: snapshot.raw_index_sha256,
            staged_entries_sha256: snapshot.staged_entries_sha256,
            tree: snapshot.tree,
        })
    }

    /// Performs at most one mutating process spawn.  Consuming `authorization`
    /// before the spawn structurally prevents replay after every outcome.
    async fn commit(
        &self,
        authorization: ReviewedCommitAuthorization,
        message: String,
    ) -> CommitDisposition {
        if validate_message(&message).is_err() {
            return CommitDisposition::InvalidInput;
        }
        let _lease = self.acquire_lease().await;
        #[cfg(test)]
        if test_phase::hit(test_phase::Phase::AfterLease, self).is_err() {
            return CommitDisposition::Uncertain;
        }
        if authorization.generation != self.generation {
            return CommitDisposition::PreconditionFailed;
        }
        #[cfg(test)]
        if test_phase::hit(test_phase::Phase::BeforeFinalRevalidation, self).is_err() {
            return CommitDisposition::Uncertain;
        }
        if self.matches_authorization(&authorization).await.is_err() {
            return CommitDisposition::PreconditionFailed;
        }
        #[cfg(test)]
        if test_phase::spawn_failure(self) {
            return match self.proves_no_effect(&authorization).await {
                Ok(true) => CommitDisposition::KnownNoEffect,
                _ => CommitDisposition::Uncertain,
            };
        }
        #[cfg(test)]
        if test_phase::hit(test_phase::Phase::BeforeSpawn, self).is_err() {
            return CommitDisposition::Uncertain;
        }
        #[cfg(test)]
        self.attempts
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let spawned = self.run_commit(&message).await;
        #[cfg(test)]
        if test_phase::hit(test_phase::Phase::AfterSpawn, self).is_err() {
            return CommitDisposition::Uncertain;
        }
        #[cfg(test)]
        if test_phase::hit(test_phase::Phase::BeforePostObservation, self).is_err() {
            return CommitDisposition::Uncertain;
        }
        match self.verify_committed(&authorization, &message).await {
            Ok(true) => CommitDisposition::CommittedVerified,
            Ok(false) => match self.proves_no_effect(&authorization).await {
                Ok(true) => CommitDisposition::KnownNoEffect,
                _ => CommitDisposition::Uncertain,
            },
            Err(_) if spawned.is_err() => CommitDisposition::Uncertain,
            Err(_) => CommitDisposition::Uncertain,
        }
    }

    async fn matches_authorization(
        &self,
        authorization: &ReviewedCommitAuthorization,
    ) -> Result<(), ToolError> {
        let current = self.capture_snapshot().await?;
        if current.branch != authorization.branch
            || current.old_head != authorization.old_head
            || current.raw_index_sha256 != authorization.raw_index_sha256
            || current.staged_entries_sha256 != authorization.staged_entries_sha256
            || current.tree != authorization.tree
        {
            return Err(git_error("reviewed commit snapshot changed"));
        }
        Ok(())
    }

    async fn capture_snapshot(&self) -> Result<CommitSnapshot, ToolError> {
        self.repository.revalidate()?;
        self.git_binding.revalidate()?;
        self.revalidate_hooks()?;
        self.require_ordinary_state()?;
        let branch = output_text(self.run(&["symbolic-ref", "-q", "HEAD"]).await?)?;
        if !valid_branch(&branch) {
            return Err(git_error("HEAD is not an attached refs/heads branch"));
        }
        let old_head = output_text(self.run(&["rev-parse", "--verify", "HEAD"]).await?)?;
        if !valid_oid(&old_head) {
            return Err(git_error("HEAD is not an existing commit object id"));
        }
        let branch_head = output_text(self.run(&["rev-parse", "--verify", &branch]).await?)?;
        if branch_head != old_head {
            return Err(git_error("attached branch does not equal HEAD"));
        }
        self.require_index_admission().await?;
        let staged = self
            .run(&["ls-files", "--stage", "-z", "--no-abbrev"])
            .await?;
        let staged = successful(staged)?;
        validate_stage_entries(&staged.stdout)?;
        let tree = output_text(self.run(&["write-tree"]).await?)?;
        if !valid_oid(&tree) {
            return Err(git_error("write-tree returned an invalid object id"));
        }
        let delta = self.run(&["diff", "--cached", "--quiet", "HEAD"]).await?;
        if delta.exit_code != Some(1) || delta.timed_out || delta.overflow.is_some() {
            return Err(git_error("staged index has no admissible tree delta"));
        }
        // Capture raw bytes last: fixed Git observations may refresh cache
        // extensions, while this value binds the state immediately before use.
        let index =
            fs::read(self.repository.root().join(".git").join("index")).map_err(io_error)?;
        if index.len() < 12 || &index[..4] != b"DIRC" {
            return Err(git_error("index is malformed or unsupported"));
        }
        Ok(CommitSnapshot {
            branch,
            old_head,
            raw_index_sha256: Sha256::digest(index).into(),
            staged_entries_sha256: Sha256::digest(&staged.stdout).into(),
            tree,
        })
    }

    fn require_ordinary_state(&self) -> Result<(), ToolError> {
        let git = self.repository.root().join(".git");
        for name in [
            "MERGE_HEAD",
            "CHERRY_PICK_HEAD",
            "REVERT_HEAD",
            "SQUASH_MSG",
            "BISECT_LOG",
        ] {
            if git.join(name).exists() {
                return Err(git_error("repository is in an unsupported special state"));
            }
        }
        for name in ["rebase-apply", "rebase-merge", "sequencer"] {
            if git.join(name).exists() {
                return Err(git_error("repository is in an unsupported special state"));
            }
        }
        Ok(())
    }

    async fn require_index_admission(&self) -> Result<(), ToolError> {
        let sparse = self
            .run(&["config", "--bool", "core.sparseCheckout"])
            .await?;
        if sparse.exit_code == Some(0) && output_text(sparse)? == "true" {
            return Err(git_error("sparse checkout is unsupported"));
        }
        if self
            .repository
            .root()
            .join(".git/info/sparse-checkout")
            .exists()
        {
            return Err(git_error("sparse checkout is unsupported"));
        }
        Ok(())
    }

    async fn run_commit(&self, message: &str) -> Result<HostProcessOutput, ToolError> {
        let mut args = self.commit_config();
        args.extend([
            "commit".into(),
            "--no-verify".into(),
            "--cleanup=verbatim".into(),
            "-m".into(),
            message.into(),
        ]);
        self.policy(args, COMMIT_TIMEOUT)?
            .execute_process(&crate::ToolInput(serde_json::json!({})))
            .await
    }

    async fn run(&self, arguments: &[&str]) -> Result<HostProcessOutput, ToolError> {
        self.policy(
            arguments.iter().map(|arg| (*arg).to_owned()).collect(),
            Duration::from_secs(5),
        )?
        .execute_process(&crate::ToolInput(serde_json::json!({})))
        .await
    }

    fn policy(
        &self,
        arguments: Vec<String>,
        timeout: Duration,
    ) -> Result<HostExecutionPolicy, ToolError> {
        self.git_binding.revalidate()?;
        HostExecutionPolicy::new(
            &self.git,
            HostArgumentPolicy::Exact(arguments),
            self.repository.root(),
            ".",
        )
        .and_then(|policy| policy.with_environment(self.environment()))
        .and_then(|policy| policy.with_timeout(timeout))
        .and_then(|policy| policy.with_output_limits(OUTPUT_LIMITS))
    }

    fn environment(&self) -> BTreeMap<OsString, OsString> {
        let mut environment = BTreeMap::new();
        environment.insert("GIT_CONFIG_NOSYSTEM".into(), "1".into());
        environment.insert("GIT_CONFIG_GLOBAL".into(), null_device().into());
        environment.insert("GIT_TERMINAL_PROMPT".into(), "0".into());
        environment.insert("GIT_CONFIG_COUNT".into(), "8".into());
        for (index, (key, value)) in self.config_entries().into_iter().enumerate() {
            environment.insert(format!("GIT_CONFIG_KEY_{index}").into(), key.into());
            environment.insert(format!("GIT_CONFIG_VALUE_{index}").into(), value.into());
        }
        environment
    }

    fn config_entries(&self) -> Vec<(String, String)> {
        vec![
            ("core.fsmonitor".into(), "false".into()),
            ("core.untrackedCache".into(), "false".into()),
            (
                "safe.directory".into(),
                self.repository.root().display().to_string(),
            ),
            ("core.hooksPath".into(), self.hooks.display().to_string()),
            ("commit.gpgSign".into(), "false".into()),
            ("user.useConfigOnly".into(), "true".into()),
            ("user.name".into(), self.identity.name.clone()),
            ("user.email".into(), self.identity.email.clone()),
        ]
    }

    fn commit_config(&self) -> Vec<String> {
        self.config_entries()
            .into_iter()
            .flat_map(|(key, value)| vec!["-c".into(), format!("{key}={value}")])
            .collect()
    }

    fn revalidate_hooks(&self) -> Result<(), ToolError> {
        let canonical = fs::canonicalize(&self.hooks).map_err(io_error)?;
        if canonical != self.hooks
            || !canonical.is_dir()
            || FileIdentity::capture(&canonical)? != self.hooks_identity
            || fs::read_dir(&canonical).map_err(io_error)?.next().is_some()
        {
            return Err(git_error(
                "host-owned hooks directory changed or is not empty",
            ));
        }
        Ok(())
    }

    async fn proves_no_effect(
        &self,
        authorization: &ReviewedCommitAuthorization,
    ) -> Result<bool, ToolError> {
        #[cfg(test)]
        test_phase::hit(test_phase::Phase::DuringNoEffectVerification, self)?;
        self.repository.revalidate()?;
        let branch = output_text(self.run(&["symbolic-ref", "-q", "HEAD"]).await?)?;
        let head = output_text(self.run(&["rev-parse", "--verify", "HEAD"]).await?)?;
        let branch_head = output_text(
            self.run(&["rev-parse", "--verify", &authorization.branch])
                .await?,
        )?;
        Ok(branch == authorization.branch
            && head == authorization.old_head
            && branch_head == authorization.old_head)
    }

    async fn verify_committed(
        &self,
        authorization: &ReviewedCommitAuthorization,
        message: &str,
    ) -> Result<bool, ToolError> {
        #[cfg(test)]
        test_phase::hit(test_phase::Phase::DuringCommittedVerification, self)?;
        self.repository.revalidate()?;
        let branch = output_text(self.run(&["symbolic-ref", "-q", "HEAD"]).await?)?;
        let new_head = output_text(self.run(&["rev-parse", "--verify", "HEAD"]).await?)?;
        let branch_head = output_text(
            self.run(&["rev-parse", "--verify", &authorization.branch])
                .await?,
        )?;
        if branch != authorization.branch
            || new_head == authorization.old_head
            || branch_head != new_head
        {
            return Ok(false);
        }
        if output_text(self.run(&["cat-file", "-t", &new_head]).await?)? != "commit" {
            return Ok(false);
        }
        let raw = successful(self.run(&["cat-file", "-p", &new_head]).await?)?.stdout;
        let (headers, body) = split_commit(&raw)?;
        let parents = headers
            .iter()
            .filter_map(|line| line.strip_prefix("parent "))
            .collect::<Vec<_>>();
        let trees = headers
            .iter()
            .filter_map(|line| line.strip_prefix("tree "))
            .collect::<Vec<_>>();
        let authors = headers
            .iter()
            .filter_map(|line| line.strip_prefix("author "))
            .collect::<Vec<_>>();
        let committers = headers
            .iter()
            .filter_map(|line| line.strip_prefix("committer "))
            .collect::<Vec<_>>();
        let signed = headers.iter().any(|line| line.starts_with("gpgsig "));
        let committed_message = if message.ends_with('\n') {
            message.as_bytes().to_vec()
        } else {
            format!("{message}\n").into_bytes()
        };
        if parents != [authorization.old_head.as_str()]
            || trees != [authorization.tree.as_str()]
            || signed
            || body != committed_message
        {
            return Ok(false);
        }
        let expected = format!("{} <{}> ", self.identity.name, self.identity.email);
        if authors.len() != 1
            || committers.len() != 1
            || !authors[0].starts_with(&expected)
            || !committers[0].starts_with(&expected)
        {
            return Ok(false);
        }
        let post_tree = output_text(self.run(&["write-tree"]).await?)?;
        Ok(post_tree == authorization.tree)
    }

    #[cfg(test)]
    fn attempts(&self) -> usize {
        self.attempts.load(std::sync::atomic::Ordering::Relaxed)
    }
}

impl Drop for RepositoryCommitPolicy {
    fn drop(&mut self) {
        // The directory is policy-owned and was created outside the repository.
        // Cleanup failure cannot broaden authority or affect commit verification.
        let _ = fs::remove_dir(&self.hooks);
    }
}

fn validate_message(message: &str) -> Result<(), ToolError> {
    if message.is_empty()
        || message.trim().is_empty()
        || message.contains('\0')
        || message.len() > MESSAGE_LIMIT
        || message.lines().next().is_none_or(str::is_empty)
    {
        return Err(git_error("commit message is invalid"));
    }
    Ok(())
}

fn valid_branch(value: &str) -> bool {
    value.starts_with("refs/heads/")
        && value.len() > "refs/heads/".len()
        && !value.contains([' ', '\0', '~', '^', ':', '?', '*', '[', '\\'])
        && !value.ends_with('.')
        && !value.contains("..")
}
fn valid_oid(value: &str) -> bool {
    matches!(value.len(), 40 | 64) && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}
fn successful(output: HostProcessOutput) -> Result<HostProcessOutput, ToolError> {
    if output.exit_code == Some(0) && !output.timed_out && output.overflow.is_none() {
        Ok(output)
    } else {
        Err(git_error("fixed Git observation failed"))
    }
}
fn output_text(output: HostProcessOutput) -> Result<String, ToolError> {
    let output = successful(output)?;
    let value = std::str::from_utf8(&output.stdout)
        .map_err(|_| git_error("Git output was not UTF-8"))?
        .trim_end_matches(['\r', '\n'])
        .to_owned();
    if value.is_empty() {
        Err(git_error("Git output was empty"))
    } else {
        Ok(value)
    }
}
fn io_error(error: std::io::Error) -> ToolError {
    git_error(error.to_string())
}
#[cfg(windows)]
fn null_device() -> &'static str {
    "NUL"
}
#[cfg(not(windows))]
fn null_device() -> &'static str {
    "/dev/null"
}

fn unique_empty_hooks_directory() -> Result<PathBuf, ToolError> {
    let path = std::env::temp_dir().join(format!(
        "rah-commit-hooks-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4()
    ));
    fs::create_dir(&path).map_err(io_error)?;
    fs::canonicalize(path).map_err(io_error)
}

fn validate_stage_entries(bytes: &[u8]) -> Result<(), ToolError> {
    if bytes.is_empty() {
        return Err(git_error("index has no staged entries"));
    }
    for entry in bytes
        .split(|byte| *byte == 0)
        .filter(|entry| !entry.is_empty())
    {
        let tab = entry
            .iter()
            .position(|byte| *byte == b'\t')
            .ok_or_else(|| git_error("malformed index entry"))?;
        let (meta, _path) = entry.split_at(tab);
        let parts = std::str::from_utf8(meta)
            .map_err(|_| git_error("non-UTF-8 index metadata"))?
            .split(' ')
            .collect::<Vec<_>>();
        if parts.len() != 3 || parts[2] != "0" || parts[0] == "160000" || parts[0] == "000000" {
            return Err(git_error(
                "index contains unsupported conflict, gitlink, or intent-to-add entry",
            ));
        }
    }
    Ok(())
}

fn split_commit(raw: &[u8]) -> Result<(Vec<&str>, &[u8]), ToolError> {
    let split = raw
        .windows(2)
        .position(|window| window == b"\n\n")
        .ok_or_else(|| git_error("malformed commit object"))?;
    let headers = std::str::from_utf8(&raw[..split])
        .map_err(|_| git_error("commit headers were not UTF-8"))?
        .lines()
        .collect::<Vec<_>>();
    if headers.iter().any(|line| line.is_empty()) {
        return Err(git_error("malformed commit headers"));
    }
    Ok((headers, &raw[split + 2..]))
}

// Deliberately finite, test-only fault points for the one-attempt state
// machine. They are not compiled into a production artifact and cannot become
// a runtime callback or plugin surface.
#[cfg(test)]
mod test_phase {
    use std::sync::{Mutex, OnceLock};

    use super::{RepositoryCommitPolicy, ToolError, git_error};

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub(super) enum Phase {
        AfterAuthorization,
        AfterLease,
        BeforeFinalRevalidation,
        BeforeSpawn,
        AfterSpawn,
        BeforePostObservation,
        DuringCommittedVerification,
        DuringNoEffectVerification,
    }

    #[derive(Clone, Copy)]
    enum Fault {
        Fail,
        SpawnFailure,
        IndexLock,
    }

    static FAULT: OnceLock<Mutex<Option<(uuid::Uuid, Phase, Fault)>>> = OnceLock::new();

    pub(super) struct Guard;

    impl Drop for Guard {
        fn drop(&mut self) {
            *FAULT.get_or_init(|| Mutex::new(None)).lock().unwrap() = None;
        }
    }

    pub(super) fn install(
        policy: &RepositoryCommitPolicy,
        phase: Phase,
        fault: &'static str,
    ) -> Guard {
        let fault = match fault {
            "fail" => Fault::Fail,
            "spawn_failure" => Fault::SpawnFailure,
            "index_lock" => Fault::IndexLock,
            _ => panic!("unknown repository commit test fault"),
        };
        let mut slot = FAULT.get_or_init(|| Mutex::new(None)).lock().unwrap();
        assert!(
            slot.replace((policy.generation, phase, fault)).is_none(),
            "test fault already installed"
        );
        Guard
    }

    pub(super) fn hit(phase: Phase, _policy: &RepositoryCommitPolicy) -> Result<(), ToolError> {
        if matches!(
            *FAULT.get_or_init(|| Mutex::new(None)).lock().unwrap(),
            Some((generation, selected, Fault::Fail)) if generation == _policy.generation && selected == phase
        ) {
            return Err(git_error("test-only postcondition observer failure"));
        }
        if matches!(
            *FAULT.get_or_init(|| Mutex::new(None)).lock().unwrap(),
            Some((generation, selected, Fault::IndexLock)) if generation == _policy.generation && selected == phase
        ) {
            std::fs::write(
                _policy.repository.root().join(".git/index.lock"),
                b"test-owned lock",
            )
            .map_err(super::io_error)?;
        }
        Ok(())
    }

    pub(super) fn spawn_failure(policy: &RepositoryCommitPolicy) -> bool {
        matches!(
            *FAULT.get_or_init(|| Mutex::new(None)).lock().unwrap(),
            Some((generation, Phase::BeforeSpawn, Fault::SpawnFailure)) if generation == policy.generation
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        path::PathBuf,
        process::Command,
        sync::atomic::{AtomicU64, Ordering},
    };
    static NEXT: AtomicU64 = AtomicU64::new(0);
    fn git() -> PathBuf {
        #[cfg(windows)]
        let output = Command::new("where.exe").arg("git.exe").output().unwrap();
        #[cfg(not(windows))]
        let output = Command::new("which").arg("git").output().unwrap();
        fs::canonicalize(
            String::from_utf8(output.stdout)
                .unwrap()
                .lines()
                .next()
                .unwrap(),
        )
        .unwrap()
    }
    fn fixture() -> (PathBuf, PathBuf) {
        let root = std::env::temp_dir().join(format!(
            "rah-commit-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        let git = git();
        for args in [
            ["init", "--quiet"].as_slice(),
            ["config", "user.name", "ambient"].as_slice(),
            ["config", "user.email", "ambient@example.invalid"].as_slice(),
        ] {
            assert!(
                Command::new(&git)
                    .args(args)
                    .current_dir(&root)
                    .status()
                    .unwrap()
                    .success()
            );
        }
        fs::write(root.join("tracked.txt"), b"base\n").unwrap();
        assert!(
            Command::new(&git)
                .args(["add", "tracked.txt"])
                .current_dir(&root)
                .status()
                .unwrap()
                .success()
        );
        assert!(
            Command::new(&git)
                .args(["commit", "--quiet", "-m", "base"])
                .current_dir(&root)
                .status()
                .unwrap()
                .success()
        );
        (git, fs::canonicalize(root).unwrap())
    }
    fn policy(git: &Path, root: &Path) -> RepositoryCommitPolicy {
        RepositoryCommitPolicy::new(
            git,
            root,
            "RAH Host".into(),
            "rah-host@example.invalid".into(),
        )
        .unwrap()
    }

    fn stage(git: &Path, root: &Path, bytes: &[u8]) {
        fs::write(root.join("tracked.txt"), bytes).unwrap();
        assert!(
            Command::new(git)
                .args(["add", "tracked.txt"])
                .current_dir(root)
                .status()
                .unwrap()
                .success()
        );
    }
    #[tokio::test]
    async fn normal_dirty_and_same_file_snapshots_commit_exact_index() {
        let (git, root) = fixture();
        fs::write(root.join("tracked.txt"), b"staged\n").unwrap();
        Command::new(&git)
            .args(["add", "tracked.txt"])
            .current_dir(&root)
            .status()
            .unwrap();
        fs::write(root.join("tracked.txt"), b"unstaged\n").unwrap();
        fs::write(root.join("untracked.txt"), b"u\n").unwrap();
        let policy = policy(&git, &root);
        let authorization = policy.authorize().await.unwrap();
        assert_eq!(
            policy
                .commit(authorization, "reviewed message".into())
                .await,
            CommitDisposition::CommittedVerified
        );
        assert_eq!(policy.attempts(), 1);
        assert_eq!(fs::read(root.join("tracked.txt")).unwrap(), b"unstaged\n");
        assert!(root.join("untracked.txt").exists());
        let show = Command::new(&git)
            .args(["show", "HEAD:tracked.txt"])
            .current_dir(&root)
            .output()
            .unwrap();
        assert_eq!(show.stdout, b"staged\n");
        fs::remove_dir_all(root).unwrap();
    }
    #[tokio::test]
    async fn invalid_messages_and_changed_index_refuse_before_spawn() {
        let (git, root) = fixture();
        fs::write(root.join("tracked.txt"), b"staged\n").unwrap();
        Command::new(&git)
            .args(["add", "tracked.txt"])
            .current_dir(&root)
            .status()
            .unwrap();
        let policy = policy(&git, &root);
        let authorization = policy.authorize().await.unwrap();
        assert_eq!(
            policy.commit(authorization, " ".into()).await,
            CommitDisposition::InvalidInput
        );
        assert_eq!(policy.attempts(), 0);
        let authorization = policy.authorize().await.unwrap();
        fs::write(root.join("tracked.txt"), b"changed\n").unwrap();
        Command::new(&git)
            .args(["add", "tracked.txt"])
            .current_dir(&root)
            .status()
            .unwrap();
        assert_eq!(
            policy.commit(authorization, "message".into()).await,
            CommitDisposition::PreconditionFailed
        );
        assert_eq!(policy.attempts(), 0);
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn no_delta_special_state_and_tampered_hooks_refuse_before_spawn() {
        let (git, root) = fixture();
        let first_policy = policy(&git, &root);
        assert!(first_policy.authorize().await.is_err(), "no staged delta");
        assert_eq!(first_policy.attempts(), 0);
        fs::write(root.join("tracked.txt"), b"staged\n").unwrap();
        Command::new(&git)
            .args(["add", "tracked.txt"])
            .current_dir(&root)
            .status()
            .unwrap();
        fs::write(first_policy.hooks.join("unexpected"), b"x").unwrap();
        assert!(
            first_policy.authorize().await.is_err(),
            "tampered hooks directory"
        );
        assert_eq!(first_policy.attempts(), 0);
        drop(first_policy);
        fs::remove_dir_all(root).unwrap();

        let (git, root) = fixture();
        fs::write(root.join("tracked.txt"), b"staged\n").unwrap();
        Command::new(&git)
            .args(["add", "tracked.txt"])
            .current_dir(&root)
            .status()
            .unwrap();
        fs::write(root.join(".git/MERGE_HEAD"), b"deadbeef\n").unwrap();
        let policy = policy(&git, &root);
        assert!(policy.authorize().await.is_err(), "merge state");
        assert_eq!(policy.attempts(), 0);
        fs::remove_dir_all(root).unwrap();
    }
    #[tokio::test]
    async fn repository_hooks_and_signing_config_do_not_control_commit() {
        let (git, root) = fixture();
        fs::write(root.join("tracked.txt"), b"staged\n").unwrap();
        Command::new(&git)
            .args(["add", "tracked.txt"])
            .current_dir(&root)
            .status()
            .unwrap();
        Command::new(&git)
            .args(["config", "commit.gpgSign", "true"])
            .current_dir(&root)
            .status()
            .unwrap();
        let marker = root.join("hook-marker");
        let _hook = root.join(".git/hooks/prepare-commit-msg");
        #[cfg(unix)]
        fs::write(&_hook, format!("#!/bin/sh\ntouch '{}'\n", marker.display())).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&_hook, fs::Permissions::from_mode(0o755)).unwrap();
        }
        let policy = policy(&git, &root);
        let authorization = policy.authorize().await.unwrap();
        assert_eq!(
            policy.commit(authorization, "message".into()).await,
            CommitDisposition::CommittedVerified
        );
        assert!(!marker.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn message_and_fixed_command_boundaries_are_exact() {
        assert!(validate_message("first\nbody").is_ok());
        assert!(validate_message(&"x".repeat(MESSAGE_LIMIT)).is_ok());
        assert!(validate_message(&"x".repeat(MESSAGE_LIMIT + 1)).is_err());
        for invalid in ["", " \t\n", "\nbody", "a\0b"] {
            assert!(validate_message(invalid).is_err());
        }
        let (git, root) = fixture();
        let policy = policy(&git, &root);
        let command = policy.commit_config();
        assert!(command.chunks_exact(2).all(|pair| pair[0] == "-c"));
        assert!(!command.iter().any(|arg| {
            [
                "-a",
                "--amend",
                "--allow-empty",
                "--allow-empty-message",
                "--author",
                "--date",
                "--gpg-sign",
                "--signoff",
                "-F",
                "-t",
                "-C",
                "--reuse-message",
                "--fixup",
                "--squash",
                "--trailer",
            ]
            .contains(&arg.as_str())
        }));
        drop(policy);
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn policy_generation_head_and_index_races_refuse_before_spawn() {
        let (git, root) = fixture();
        stage(&git, &root, b"staged\n");
        let first = policy(&git, &root);
        let second = policy(&git, &root);
        let authorization = first.authorize().await.unwrap();
        assert_eq!(
            second.commit(authorization, "message".into()).await,
            CommitDisposition::PreconditionFailed
        );
        assert_eq!(second.attempts(), 0);

        let authorization = first.authorize().await.unwrap();
        stage(&git, &root, b"different index\n");
        assert_eq!(
            first.commit(authorization, "message".into()).await,
            CommitDisposition::PreconditionFailed
        );
        assert_eq!(first.attempts(), 0);
        stage(&git, &root, b"staged again\n");
        let authorization = first.authorize().await.unwrap();
        let branch = String::from_utf8(
            Command::new(&git)
                .args(["symbolic-ref", "HEAD"])
                .current_dir(&root)
                .output()
                .unwrap()
                .stdout,
        )
        .unwrap();
        let branch = branch.trim();
        assert!(
            Command::new(&git)
                .args(["symbolic-ref", "HEAD", "refs/heads/rah-other"])
                .current_dir(&root)
                .status()
                .unwrap()
                .success()
        );
        assert_eq!(
            first.commit(authorization, "message".into()).await,
            CommitDisposition::PreconditionFailed
        );
        assert_eq!(first.attempts(), 0);
        assert!(
            Command::new(&git)
                .args(["symbolic-ref", "HEAD", branch])
                .current_dir(&root)
                .status()
                .unwrap()
                .success()
        );
        drop(second);
        drop(first);
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn spawn_failure_lock_refusal_and_post_spawn_observer_failure_are_classified_by_state() {
        let (git, root) = fixture();
        stage(&git, &root, b"one\n");
        let policy = policy(&git, &root);
        let authorization = policy.authorize().await.unwrap();
        let guard = test_phase::install(&policy, test_phase::Phase::BeforeSpawn, "spawn_failure");
        assert_eq!(
            policy.commit(authorization, "message".into()).await,
            CommitDisposition::KnownNoEffect
        );
        assert_eq!(policy.attempts(), 0);
        drop(guard);

        let authorization = policy.authorize().await.unwrap();
        let lock = root.join(".git/index.lock");
        let guard = test_phase::install(&policy, test_phase::Phase::BeforeSpawn, "index_lock");
        assert_eq!(
            policy.commit(authorization, "message".into()).await,
            CommitDisposition::KnownNoEffect
        );
        assert_eq!(policy.attempts(), 1);
        drop(guard);
        fs::remove_file(lock).unwrap();

        let authorization = policy.authorize().await.unwrap();
        let guard = test_phase::install(&policy, test_phase::Phase::BeforePostObservation, "fail");
        assert_eq!(
            policy.commit(authorization, "message".into()).await,
            CommitDisposition::Uncertain
        );
        assert_eq!(policy.attempts(), 2);
        drop(guard);
        drop(policy);
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn hostile_local_hooks_identity_and_editor_configuration_do_not_gain_authority() {
        let (git, root) = fixture();
        stage(&git, &root, b"staged\n");
        let hostile = root.join("hostile-hooks");
        fs::create_dir(&hostile).unwrap();
        let marker = root.join("hostile-marker");
        #[cfg(unix)]
        {
            let hook = hostile.join("prepare-commit-msg");
            fs::write(&hook, format!("#!/bin/sh\ntouch '{}'\n", marker.display())).unwrap();
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();
        }
        for (key, value) in [
            ("core.hooksPath", hostile.to_string_lossy().as_ref()),
            ("core.editor", "false"),
            ("commit.template", "missing-template"),
            ("user.name", "ambient attacker"),
            ("user.email", "attacker@example.invalid"),
        ] {
            assert!(
                Command::new(&git)
                    .args(["config", key, value])
                    .current_dir(&root)
                    .status()
                    .unwrap()
                    .success()
            );
        }
        let policy = policy(&git, &root);
        let authorization = policy.authorize().await.unwrap();
        assert_eq!(
            policy.commit(authorization, "exact message".into()).await,
            CommitDisposition::CommittedVerified
        );
        assert!(!marker.exists());
        let raw = Command::new(&git)
            .args(["cat-file", "-p", "HEAD"])
            .current_dir(&root)
            .output()
            .unwrap()
            .stdout;
        assert!(
            String::from_utf8(raw)
                .unwrap()
                .contains("author RAH Host <rah-host@example.invalid> ")
        );
        drop(policy);
        fs::remove_dir_all(root).unwrap();
    }
}
