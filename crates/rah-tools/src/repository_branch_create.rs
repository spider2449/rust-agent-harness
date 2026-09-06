//! Private, host-owned foundation for one local branch creation.
//!
//! This module deliberately has no public constructor, Tool implementation,
//! or registry integration.  Task 226 owns the future host composition.  The
//! only mutation permitted here is one fixed, expected-absence `update-ref`
//! invocation for one host-validated local branch.

#![allow(dead_code)]

use std::sync::Arc;
#[cfg(test)]
use std::sync::atomic::{AtomicUsize, Ordering};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

use futures::lock::{Mutex as AsyncMutex, MutexGuard};
use rah_sandbox::{HostProcessOutput, OutputLimits};

use crate::{
    HostArgumentPolicy, HostExecutionPolicy, ToolError,
    git_stage::repository_lease,
    git_support::{git_environment, git_error},
    host_execute::paths_equivalent,
    repository_observer::{FileIdentity, RepositoryIdentity, reject_link_or_reparse},
};

const COMMAND_TIMEOUT: Duration = Duration::from_secs(10);
/// Maximum number of local heads admitted by one bounded observation.
const MAX_LOCAL_HEADS: usize = 4096;
/// Fixed Git stdout bound, including local-head snapshots.
const OUTPUT_LIMITS: OutputLimits = OutputLimits {
    stdout_bytes: 512 * 1024,
    stderr_bytes: 32 * 1024,
    combined_bytes: 544 * 1024,
};
const REFLOG_MESSAGE: &str = "RAH create local branch";
const HOST_COMMITTER_NAME: &str = "RAH Host";
const HOST_COMMITTER_EMAIL: &str = "rah-host@example.invalid";

/// The closed result taxonomy for one bounded branch-create attempt.
#[derive(Clone, Debug, Eq, PartialEq)]
enum BranchCreationDisposition {
    InvalidInput,
    PreconditionFailed,
    KnownNoEffect,
    BranchCreatedVerified { name: String, oid: String },
    DesiredStateObservedAfterUncertainAttempt { name: String, oid: String },
    Uncertain,
}

/// A host-bound, reusable policy.  Construction is crate-private and no
/// public API exposes the raw Git fields or generic ref mutation.
struct RepositoryBranchCreationPolicy {
    repository: RepositoryIdentity,
    git: PathBuf,
    git_binding: HostExecutionPolicy,
    hooks: PathBuf,
    hooks_identity: FileIdentity,
    lease: Arc<AsyncMutex<()>>,
    generation: uuid::Uuid,
    #[cfg(test)]
    attempts: AtomicUsize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RepositorySnapshot {
    branch: String,
    oid: String,
    branch_oid: String,
    local_heads: Vec<LocalHeadEntry>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LocalHeadEntry {
    name: String,
    oid: String,
}

impl RepositoryBranchCreationPolicy {
    fn new(git: &Path, root: &Path) -> Result<Self, ToolError> {
        let repository = RepositoryIdentity::capture(root)?;
        let dot_git = repository.root().join(".git");
        reject_link_or_reparse(&dot_git, "repository metadata")?;
        if !fs::metadata(&dot_git).map_err(io_error)?.is_dir() || dot_git.join("commondir").exists()
        {
            return Err(git_error(
                "branch creation requires a normal repository with a real .git directory",
            ));
        }
        let hooks = unique_empty_hooks_directory()?;
        let hooks_identity = FileIdentity::capture(&hooks)?;
        let git = fs::canonicalize(git).map_err(io_error)?;
        let git_binding = HostExecutionPolicy::new(
            &git,
            HostArgumentPolicy::Exact(vec!["--version".into()]),
            repository.root(),
            ".",
        )?
        .with_environment(git_environment_for(repository.root(), Path::new("."))?)?;
        let lease = repository_lease(repository.root());
        Ok(Self {
            repository,
            git,
            git_binding,
            hooks,
            hooks_identity,
            lease,
            generation: uuid::Uuid::new_v4(),
            #[cfg(test)]
            attempts: AtomicUsize::new(0),
        })
    }

    async fn acquire_lease(&self) -> MutexGuard<'_, ()> {
        self.lease.lock().await
    }

    async fn create(&self, name: String) -> BranchCreationDisposition {
        if validate_branch_name(&name).is_err() {
            return BranchCreationDisposition::InvalidInput;
        }
        let _lease = self.acquire_lease().await;
        if self.capture_snapshot(&name).await.is_err() {
            return BranchCreationDisposition::PreconditionFailed;
        }
        // Repeat the complete admission immediately before the sole mutating
        // process.  The lease serializes RAH-owned work, while this fresh
        // capture and the CAS protect against external changes.
        let before = match self.capture_snapshot(&name).await {
            Ok(snapshot) => snapshot,
            Err(_error) => return BranchCreationDisposition::PreconditionFailed,
        };
        let ref_name = format!("refs/heads/{name}");
        #[cfg(test)]
        if test_phase::before_spawn(self, &ref_name, &before.oid).is_err() {
            return BranchCreationDisposition::PreconditionFailed;
        }
        #[cfg(test)]
        self.attempts.fetch_add(1, Ordering::Relaxed);
        let process = {
            #[cfg(test)]
            if test_phase::spawn_failure(self) {
                Err(git_error("test-only mutation spawn failure"))
            } else {
                self.run_update_ref(&ref_name, &before.oid).await
            }
            #[cfg(not(test))]
            {
                self.run_update_ref(&ref_name, &before.oid).await
            }
        };
        #[cfg(test)]
        let process_is_uncertain = test_phase::after_spawn(self);
        #[cfg(not(test))]
        let process_is_uncertain = false;
        #[cfg(test)]
        let post_observation_failed = test_phase::post_observation_is_unknown(self);
        #[cfg(not(test))]
        let post_observation_failed = false;

        let post = if process_is_uncertain {
            Err(git_error("test-only lost mutation result"))
        } else if post_observation_failed {
            Err(git_error("test-only post-observation failure"))
        } else {
            self.capture_post_state(&name).await
        };
        let process_succeeded = process.as_ref().is_ok_and(|output| {
            output.exit_code == Some(0) && !output.timed_out && output.overflow.is_none()
        });
        match post {
            Ok(post) if process_succeeded && self.verified(&before, &post, &name).await => {
                BranchCreationDisposition::BranchCreatedVerified {
                    name,
                    oid: before.oid,
                }
            }
            Ok(post) if self.desired_state(&before, &post, &name) => {
                BranchCreationDisposition::DesiredStateObservedAfterUncertainAttempt {
                    name,
                    oid: before.oid,
                }
            }
            Ok(post) if self.no_effect(&before, &post, &name) => {
                BranchCreationDisposition::KnownNoEffect
            }
            Err(_) => match self.observe_desired(&before, &name).await {
                Ok(true) => BranchCreationDisposition::DesiredStateObservedAfterUncertainAttempt {
                    name,
                    oid: before.oid,
                },
                _ => BranchCreationDisposition::Uncertain,
            },
            _ => BranchCreationDisposition::Uncertain,
        }
    }

    async fn capture_snapshot(&self, name: &str) -> Result<RepositorySnapshot, ToolError> {
        self.revalidate_static().await?;
        validate_branch_name(name)?;
        self.check_ref_format(name).await?;
        let branch = self.output(&["symbolic-ref", "--quiet", "HEAD"]).await?;
        if !branch.starts_with("refs/heads/") || branch == "refs/heads/" {
            return Err(git_error("HEAD is not an attached local branch"));
        }
        let oid = self
            .output(&["rev-parse", "--verify", "--quiet", "HEAD"])
            .await?;
        validate_oid(&oid)?;
        let branch_oid = self
            .output(&["rev-parse", "--verify", "--quiet", &branch])
            .await?;
        if branch_oid != oid {
            return Err(git_error("attached branch does not equal HEAD"));
        }
        if self.output(&["cat-file", "-t", &oid]).await? != "commit" {
            return Err(git_error("HEAD is not a commit object"));
        }
        let local_heads = self.local_heads().await?;
        reject_collisions(name, &local_heads)?;
        self.require_absent_reflog(name).await?;
        Ok(RepositorySnapshot {
            branch,
            oid,
            branch_oid,
            local_heads,
        })
    }

    async fn capture_post_state(&self, _name: &str) -> Result<RepositorySnapshot, ToolError> {
        self.revalidate_static().await?;
        let branch = self.output(&["symbolic-ref", "--quiet", "HEAD"]).await?;
        let oid = self
            .output(&["rev-parse", "--verify", "--quiet", "HEAD"])
            .await?;
        validate_oid(&oid)?;
        let branch_oid = self
            .output(&["rev-parse", "--verify", "--quiet", &branch])
            .await?;
        if branch_oid != oid {
            return Err(git_error("attached branch changed after mutation"));
        }
        Ok(RepositorySnapshot {
            branch,
            oid,
            branch_oid,
            local_heads: self.local_heads().await?,
        })
    }

    async fn verified(
        &self,
        before: &RepositorySnapshot,
        post: &RepositorySnapshot,
        name: &str,
    ) -> bool {
        if !self.desired_state(before, post, name)
            || before.branch != post.branch
            || before.oid != post.oid
            || before.branch_oid != post.branch_oid
            || post.branch_oid != post.oid
        {
            return false;
        }
        let target = format!("refs/heads/{name}");
        if heads_without(&post.local_heads, &target) != before.local_heads {
            return false;
        }
        let reflog = self.target_reflog(name).await;
        matches!(reflog, Ok(Some((oid, message, author, email)))
            if oid == before.oid
                && message == REFLOG_MESSAGE
                && author == HOST_COMMITTER_NAME
                && email == HOST_COMMITTER_EMAIL)
    }

    fn desired_state(
        &self,
        before: &RepositorySnapshot,
        post: &RepositorySnapshot,
        name: &str,
    ) -> bool {
        let target = format!("refs/heads/{name}");
        post.branch == before.branch
            && post.oid == before.oid
            && post.branch_oid == before.branch_oid
            && post.branch_oid == post.oid
            && post
                .local_heads
                .iter()
                .any(|entry| entry.name == target && entry.oid == before.oid)
    }

    fn no_effect(
        &self,
        before: &RepositorySnapshot,
        post: &RepositorySnapshot,
        name: &str,
    ) -> bool {
        let target = format!("refs/heads/{name}");
        post.branch == before.branch
            && post.oid == before.oid
            && post.branch_oid == before.branch_oid
            && post.branch_oid == post.oid
            && heads_without(&post.local_heads, &target) == before.local_heads
            && !post.local_heads.iter().any(|entry| entry.name == target)
    }

    async fn observe_desired(
        &self,
        before: &RepositorySnapshot,
        name: &str,
    ) -> Result<bool, ToolError> {
        #[cfg(test)]
        if test_phase::post_observation_is_unknown(self) {
            return Err(git_error("test-only post-observation failure"));
        }
        let post = self.capture_post_state(name).await?;
        Ok(self.desired_state(before, &post, name))
    }

    async fn run_update_ref(
        &self,
        ref_name: &str,
        oid: &str,
    ) -> Result<HostProcessOutput, ToolError> {
        let zero = zero_oid(oid)?;
        let arguments = vec![
            "update-ref".into(),
            "--create-reflog".into(),
            "-m".into(),
            REFLOG_MESSAGE.into(),
            ref_name.into(),
            oid.into(),
            zero,
        ];
        self.run(arguments).await
    }

    async fn check_ref_format(&self, name: &str) -> Result<(), ToolError> {
        let output = self
            .run(vec![
                "check-ref-format".into(),
                "--branch".into(),
                name.into(),
            ])
            .await?;
        if output.exit_code == Some(0) && !output.timed_out && output.overflow.is_none() {
            Ok(())
        } else {
            Err(git_error("Git rejected the validated branch name"))
        }
    }

    async fn require_absent_reflog(&self, name: &str) -> Result<(), ToolError> {
        let reference = format!("refs/heads/{name}");
        let output = self
            .run(vec!["reflog".into(), "exists".into(), reference])
            .await?;
        if output.timed_out || output.overflow.is_some() {
            return Err(git_error("reflog admission observation was incomplete"));
        }
        match output.exit_code {
            Some(1) => Ok(()),
            Some(0) => Err(git_error("target branch has existing reflog state")),
            _ => Err(git_error("reflog admission observation failed")),
        }
    }

    async fn target_reflog(
        &self,
        name: &str,
    ) -> Result<Option<(String, String, String, String)>, ToolError> {
        let reference = format!("refs/heads/{name}");
        let output = self
            .run(vec![
                "reflog".into(),
                "show".into(),
                "--max-count=1".into(),
                "--format=%H%x00%gs%x00%gN%x00%gE".into(),
                reference,
            ])
            .await?;
        if output.exit_code == Some(1) && output.stdout.is_empty() {
            return Ok(None);
        }
        let text = successful_utf8(output)?;
        let fields = text.split('\0').collect::<Vec<_>>();
        if fields.len() != 4 || fields.iter().any(|field| field.is_empty()) {
            return Err(git_error("target reflog observation was malformed"));
        }
        Ok(Some((
            fields[0].into(),
            fields[1].into(),
            fields[2].into(),
            fields[3].into(),
        )))
    }

    async fn local_heads(&self) -> Result<Vec<LocalHeadEntry>, ToolError> {
        let output = self
            .run(vec![
                "for-each-ref".into(),
                "--format=%(refname)%00%(objectname)%00".into(),
                "refs/heads/".into(),
            ])
            .await?;
        let output = successful(output)?;
        let mut fields = output.stdout.split(|byte| *byte == 0).collect::<Vec<_>>();
        if let Some(last) = fields.last_mut() {
            *last = trim_line_end(last);
        }
        if fields.last().is_some_and(|field| field.is_empty()) {
            fields.pop();
        }
        if fields.len() % 2 != 0 {
            return Err(git_error("local-head observation was malformed"));
        }
        let count = fields.len() / 2;
        if count > MAX_LOCAL_HEADS {
            return Err(git_error("local-head observation exceeded its bound"));
        }
        let (pairs, remainder) = fields.as_slice().as_chunks::<2>();
        debug_assert!(remainder.is_empty());
        pairs
            .iter()
            .map(|pair| {
                let name = std::str::from_utf8(trim_line_end(pair[0]))
                    .map_err(|_| git_error("local-head name was not UTF-8"))?;
                if !name.starts_with("refs/heads/") || name == "refs/heads/" {
                    return Err(git_error("Git returned a malformed local-head ref"));
                }
                let oid = std::str::from_utf8(trim_line_end(pair[1]))
                    .map_err(|_| git_error("local-head OID was not UTF-8"))?;
                validate_oid(oid)?;
                Ok(LocalHeadEntry {
                    name: name.into(),
                    oid: oid.into(),
                })
            })
            .collect()
    }

    async fn revalidate_static(&self) -> Result<(), ToolError> {
        self.repository.revalidate()?;
        self.git_binding.revalidate()?;
        let dot_git = self.repository.root().join(".git");
        if !fs::metadata(&dot_git).map_err(io_error)?.is_dir() || dot_git.join("commondir").exists()
        {
            return Err(git_error(
                "repository topology is not a normal .git directory",
            ));
        }
        self.revalidate_hooks()?;
        if self.output(&["rev-parse", "--is-bare-repository"]).await? != "false" {
            return Err(git_error("bare repositories are unsupported"));
        }
        if self
            .optional_output(&["config", "--get", "extensions.refStorage"])
            .await?
            .is_some_and(|storage| !storage.is_empty())
        {
            return Err(git_error(
                "reftable and unknown ref backends are unsupported",
            ));
        }
        let refs = self.output(&["rev-parse", "--git-path", "refs"]).await?;
        let refs = PathBuf::from(refs);
        let refs = if refs.is_absolute() {
            refs
        } else {
            self.repository.root().join(refs)
        };
        let refs = fs::canonicalize(refs).map_err(io_error)?;
        let expected = fs::canonicalize(dot_git.join("refs")).map_err(io_error)?;
        if !paths_equivalent(&refs, &expected) || !refs.is_dir() {
            return Err(git_error(
                "repository ref backend is not the ordinary files backend",
            ));
        }
        for marker in [
            "MERGE_HEAD",
            "CHERRY_PICK_HEAD",
            "REVERT_HEAD",
            "SQUASH_MSG",
            "BISECT_LOG",
            "rebase-apply",
            "rebase-merge",
            "sequencer",
        ] {
            if dot_git.join(marker).exists() {
                return Err(git_error("repository is in an unsupported special state"));
            }
        }
        Ok(())
    }

    fn revalidate_hooks(&self) -> Result<(), ToolError> {
        let canonical = fs::canonicalize(&self.hooks).map_err(io_error)?;
        if !paths_equivalent(&canonical, &self.hooks)
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

    async fn output(&self, arguments: &[&str]) -> Result<String, ToolError> {
        self.output_owned(arguments.iter().map(|value| (*value).into()).collect())
            .await
    }

    async fn optional_output(&self, arguments: &[&str]) -> Result<Option<String>, ToolError> {
        let output = self
            .run(arguments.iter().map(|value| (*value).into()).collect())
            .await?;
        if output.exit_code == Some(1) && !output.timed_out && output.overflow.is_none() {
            return Ok(None);
        }
        Ok(Some(successful_utf8(output)?))
    }

    async fn output_owned(&self, arguments: Vec<String>) -> Result<String, ToolError> {
        successful_utf8(self.run(arguments).await?)
    }

    async fn run(&self, arguments: Vec<String>) -> Result<HostProcessOutput, ToolError> {
        self.git_binding.revalidate()?;
        HostExecutionPolicy::new(
            &self.git,
            HostArgumentPolicy::Exact(arguments),
            self.repository.root(),
            ".",
        )?
        .with_environment(git_environment_for(self.repository.root(), &self.hooks)?)?
        .with_timeout(COMMAND_TIMEOUT)?
        .with_output_limits(OUTPUT_LIMITS)?
        .execute_process(&crate::ToolInput(serde_json::json!({})))
        .await
    }
}

fn trim_line_end(bytes: &[u8]) -> &[u8] {
    let bytes = bytes
        .strip_prefix(b"\r\n")
        .or_else(|| bytes.strip_prefix(b"\n"))
        .unwrap_or(bytes);
    bytes
        .strip_suffix(b"\r\n")
        .or_else(|| bytes.strip_suffix(b"\n"))
        .unwrap_or(bytes)
}

impl Drop for RepositoryBranchCreationPolicy {
    fn drop(&mut self) {
        if let Ok(canonical) = fs::canonicalize(&self.hooks)
            && paths_equivalent(&canonical, &self.hooks)
            && canonical.is_dir()
            && FileIdentity::capture(&canonical).ok().as_ref() == Some(&self.hooks_identity)
            && fs::read_dir(&canonical).is_ok_and(|mut entries| entries.next().is_none())
        {
            let _ = fs::remove_dir(&canonical);
        }
    }
}

fn validate_branch_name(name: &str) -> Result<(), ToolError> {
    if name.is_empty() || name.len() > 128 || !name.is_ascii() || name.starts_with("refs/") {
        return Err(git_error(
            "branch name is outside the closed ASCII contract",
        ));
    }
    let components = name.split('/').collect::<Vec<_>>();
    if components.is_empty() || components.len() > 8 {
        return Err(git_error("branch name has an invalid component count"));
    }
    for component in components {
        if component.is_empty()
            || component.len() > 48
            || component == "."
            || component == ".."
            || component.starts_with('.')
            || component.starts_with('-')
            || component.ends_with('.')
            || component.ends_with(' ')
            || component.ends_with(".lock")
            || windows_reserved_alias(component)
        {
            return Err(git_error("branch name contains a forbidden component"));
        }
        let bytes = component.as_bytes();
        if !bytes
            .first()
            .is_some_and(|byte| byte.is_ascii_alphanumeric())
            || !bytes
                .last()
                .is_some_and(|byte| byte.is_ascii_alphanumeric())
            || bytes
                .iter()
                .any(|byte| !byte.is_ascii_alphanumeric() && !matches!(*byte, b'.' | b'_' | b'-'))
        {
            return Err(git_error("branch name contains a forbidden character"));
        }
    }
    if name.contains("..")
        || name.contains("@{")
        || name.contains('@')
        || name.contains(['~', '^', ':', '?', '*', '[', '\\', ' '])
        || name.bytes().any(|byte| byte < 0x20 || byte == 0x7f)
    {
        return Err(git_error("branch name contains a forbidden Git spelling"));
    }
    Ok(())
}

fn windows_reserved_alias(component: &str) -> bool {
    let upper = component.to_ascii_uppercase();
    matches!(upper.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (upper.len() == 4
            && (upper.starts_with("COM") || upper.starts_with("LPT"))
            && upper.as_bytes()[3].is_ascii_digit()
            && upper.as_bytes()[3] != b'0')
}

fn reject_collisions(name: &str, heads: &[LocalHeadEntry]) -> Result<(), ToolError> {
    let target = format!("refs/heads/{name}");
    let folded = ascii_fold(&target);
    for reference in heads {
        let other = ascii_fold(&reference.name);
        if other == folded
            || other.starts_with(&(folded.clone() + "/"))
            || folded.starts_with(&(other + "/"))
        {
            return Err(git_error(
                "branch name collides with an existing local head",
            ));
        }
    }
    Ok(())
}

fn heads_without(heads: &[LocalHeadEntry], excluded: &str) -> Vec<LocalHeadEntry> {
    heads
        .iter()
        .filter(|entry| entry.name != excluded)
        .cloned()
        .collect()
}

fn ascii_fold(value: &str) -> String {
    value
        .bytes()
        .map(|byte| byte.to_ascii_lowercase() as char)
        .collect()
}

fn validate_oid(oid: &str) -> Result<(), ToolError> {
    if !matches!(oid.len(), 40 | 64) || !oid.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(git_error("Git returned an invalid object id"));
    }
    Ok(())
}

fn zero_oid(oid: &str) -> Result<String, ToolError> {
    validate_oid(oid)?;
    Ok("0".repeat(oid.len()))
}

fn successful(output: HostProcessOutput) -> Result<HostProcessOutput, ToolError> {
    if output.exit_code == Some(0) && !output.timed_out && output.overflow.is_none() {
        Ok(output)
    } else {
        Err(git_error("fixed Git observation failed"))
    }
}

fn successful_utf8(output: HostProcessOutput) -> Result<String, ToolError> {
    let output = successful(output)?;
    let text = std::str::from_utf8(&output.stdout)
        .map_err(|_| git_error("Git output was not UTF-8"))?
        .trim_end_matches(['\r', '\n'])
        .to_owned();
    if text.is_empty() {
        Err(git_error("Git output was empty"))
    } else {
        Ok(text)
    }
}

fn git_environment_for(
    root: &Path,
    hooks: &Path,
) -> Result<BTreeMap<OsString, OsString>, ToolError> {
    let mut environment = git_environment();
    let entries = [
        ("core.fsmonitor", OsString::from("false")),
        ("core.untrackedCache", OsString::from("false")),
        ("safe.directory", root.as_os_str().to_owned()),
        ("core.hooksPath", hooks.as_os_str().to_owned()),
        ("core.logAllRefUpdates", OsString::from("false")),
        ("core.preloadIndex", OsString::from("false")),
        ("user.name", OsString::from(HOST_COMMITTER_NAME)),
        ("user.email", OsString::from(HOST_COMMITTER_EMAIL)),
    ];
    environment.insert("GIT_CONFIG_COUNT".into(), entries.len().to_string().into());
    for (index, (key, value)) in entries.into_iter().enumerate() {
        environment.insert(format!("GIT_CONFIG_KEY_{index}").into(), key.into());
        environment.insert(format!("GIT_CONFIG_VALUE_{index}").into(), value);
    }
    environment.insert("GIT_COMMITTER_NAME".into(), HOST_COMMITTER_NAME.into());
    environment.insert("GIT_COMMITTER_EMAIL".into(), HOST_COMMITTER_EMAIL.into());
    Ok(environment)
}

fn io_error(error: std::io::Error) -> ToolError {
    git_error(error.to_string())
}

fn unique_empty_hooks_directory() -> Result<PathBuf, ToolError> {
    let path = std::env::temp_dir().join(format!(
        "rah-branch-hooks-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4()
    ));
    fs::create_dir(&path).map_err(io_error)?;
    fs::canonicalize(path).map_err(io_error)
}

#[cfg(test)]
mod test_phase {
    use super::{RepositoryBranchCreationPolicy, git_error};
    use std::sync::{Mutex, OnceLock};

    #[derive(Clone, Copy)]
    enum Fault {
        CasRace,
        LostResult,
        UnknownPostState,
        SpawnFailure,
    }
    static FAULT: OnceLock<Mutex<Vec<(uuid::Uuid, Fault)>>> = OnceLock::new();
    pub(super) struct Guard(uuid::Uuid);
    impl Drop for Guard {
        fn drop(&mut self) {
            FAULT
                .get_or_init(|| Mutex::new(Vec::new()))
                .lock()
                .unwrap()
                .retain(|(generation, _)| *generation != self.0);
        }
    }
    pub(super) fn install(policy: &RepositoryBranchCreationPolicy, fault: &'static str) -> Guard {
        let fault = match fault {
            "cas_race" => Fault::CasRace,
            "lost_result" => Fault::LostResult,
            "unknown_post_state" => Fault::UnknownPostState,
            "spawn_failure" => Fault::SpawnFailure,
            _ => panic!("unknown branch test fault"),
        };
        FAULT
            .get_or_init(|| Mutex::new(Vec::new()))
            .lock()
            .unwrap()
            .push((policy.generation, fault));
        Guard(policy.generation)
    }
    pub(super) fn before_spawn(
        policy: &RepositoryBranchCreationPolicy,
        reference: &str,
        oid: &str,
    ) -> Result<(), crate::ToolError> {
        if FAULT
            .get_or_init(|| Mutex::new(Vec::new()))
            .lock()
            .unwrap()
            .iter()
            .any(|(generation, fault)| {
                *generation == policy.generation && matches!(fault, Fault::CasRace)
            })
        {
            let output = std::process::Command::new(&policy.git)
                .args(["update-ref", reference, oid])
                .current_dir(policy.repository.root())
                .output()
                .map_err(|error| git_error(error.to_string()))?;
            if !output.status.success() {
                return Err(git_error("CAS race fixture failed"));
            }
        }
        Ok(())
    }
    pub(super) fn after_spawn(policy: &RepositoryBranchCreationPolicy) -> bool {
        FAULT
            .get_or_init(|| Mutex::new(Vec::new()))
            .lock()
            .unwrap()
            .iter()
            .any(|(generation, fault)| {
                *generation == policy.generation && matches!(fault, Fault::LostResult)
            })
    }
    pub(super) fn spawn_failure(policy: &RepositoryBranchCreationPolicy) -> bool {
        FAULT
            .get_or_init(|| Mutex::new(Vec::new()))
            .lock()
            .unwrap()
            .iter()
            .any(|(generation, fault)| {
                *generation == policy.generation && matches!(fault, Fault::SpawnFailure)
            })
    }
    #[allow(dead_code)]
    pub(super) fn post_observation_is_unknown(policy: &RepositoryBranchCreationPolicy) -> bool {
        FAULT
            .get_or_init(|| Mutex::new(Vec::new()))
            .lock()
            .unwrap()
            .iter()
            .any(|(generation, fault)| {
                *generation == policy.generation && matches!(fault, Fault::UnknownPostState)
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        io::Write,
        path::PathBuf,
        process::{Command, Stdio},
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
            "rah-branch-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let git = git();
        for args in [
            ["init", "--quiet"].as_slice(),
            ["config", "user.name", "ambient"].as_slice(),
            ["config", "user.email", "ambient@example.invalid"].as_slice(),
            ["commit", "--allow-empty", "--quiet", "-m", "base"].as_slice(),
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
        (git, fs::canonicalize(root).unwrap())
    }
    fn policy(git: &Path, root: &Path) -> RepositoryBranchCreationPolicy {
        RepositoryBranchCreationPolicy::new(git, root).unwrap()
    }
    fn git_ok(git: &Path, root: &Path, args: &[&str]) {
        assert!(
            Command::new(git)
                .args(args)
                .current_dir(root)
                .status()
                .unwrap()
                .success(),
            "{args:?}"
        );
    }
    fn stdout(git: &Path, root: &Path, args: &[&str]) -> String {
        String::from_utf8(
            Command::new(git)
                .args(args)
                .current_dir(root)
                .output()
                .unwrap()
                .stdout,
        )
        .unwrap()
        .trim()
        .into()
    }
    fn git_stdin(git: &Path, root: &Path, args: &[&str], input: &str) {
        let mut child = Command::new(git)
            .args(args)
            .current_dir(root)
            .stdin(Stdio::piped())
            .spawn()
            .unwrap();
        let mut stdin = child.stdin.take().unwrap();
        stdin.write_all(input.as_bytes()).unwrap();
        drop(stdin);
        assert!(child.wait().unwrap().success(), "{args:?}");
    }
    fn cleanup(root: PathBuf) {
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn closed_name_contract_and_zero_oid_are_exact() {
        for name in [
            "",
            "a/",
            "/a",
            "a//b",
            "-a",
            ".a",
            "a.",
            ".",
            "..",
            "a.lock",
            "a..b",
            "a@b",
            "a@{b",
            "a b",
            "a~b",
            "a\\b",
            "refs/heads/a",
            "é",
            "a/b/c/d/e/f/g/h/i",
        ] {
            assert!(validate_branch_name(name).is_err(), "accepted {name:?}");
        }
        for name in ["a", "feature/test", "A_1-x.y", "a/b/c"] {
            assert!(validate_branch_name(name).is_ok(), "rejected {name:?}");
        }
        for name in ["CON", "prn", "Aux", "nul", "COM1", "com9", "LPT1", "lpt9"] {
            assert!(validate_branch_name(name).is_err());
        }
        assert_eq!(zero_oid(&"a".repeat(40)).unwrap(), "0".repeat(40));
        assert_eq!(zero_oid(&"b".repeat(64)).unwrap(), "0".repeat(64));
        assert!(zero_oid("abc").is_err());
        assert!(zero_oid(&"z".repeat(40)).is_err());
    }

    #[tokio::test]
    async fn successful_create_preserves_all_repository_planes_and_tracking_config() {
        let (git, root) = fixture();
        git_ok(&git, &root, &["config", "branch.main.remote", "ambient"]);
        git_ok(
            &git,
            &root,
            &["config", "branch.main.merge", "refs/heads/main"],
        );
        fs::write(root.join("dirty.txt"), "unstaged\n").unwrap();
        let before_head = stdout(&git, &root, &["symbolic-ref", "-q", "HEAD"]);
        let before_oid = stdout(&git, &root, &["rev-parse", "HEAD"]);
        let before_refs = stdout(&git, &root, &["show-ref"]);
        let before_status = stdout(&git, &root, &["status", "--porcelain=v1"]);
        let before_index = fs::read(root.join(".git/index")).unwrap();
        let before_config = stdout(
            &git,
            &root,
            &["config", "--local", "--get-regexp", "^branch\\."],
        );
        let result = policy(&git, &root).create("feature/test".into()).await;
        assert!(
            matches!(
                result,
                BranchCreationDisposition::BranchCreatedVerified { .. }
            ),
            "{result:?}"
        );
        assert_eq!(
            stdout(&git, &root, &["symbolic-ref", "-q", "HEAD"]),
            before_head
        );
        assert_eq!(stdout(&git, &root, &["rev-parse", "HEAD"]), before_oid);
        let after_refs = stdout(&git, &root, &["show-ref"]);
        let existing_after = after_refs
            .lines()
            .filter(|line| !line.ends_with(" refs/heads/feature/test"))
            .collect::<Vec<_>>();
        assert_eq!(existing_after, before_refs.lines().collect::<Vec<_>>());
        assert_eq!(
            stdout(&git, &root, &["status", "--porcelain=v1"]),
            before_status
        );
        assert_eq!(fs::read(root.join(".git/index")).unwrap(), before_index);
        assert_eq!(
            stdout(
                &git,
                &root,
                &["config", "--local", "--get-regexp", "^branch\\."]
            ),
            before_config
        );
        assert!(
            stdout(
                &git,
                &root,
                &["config", "--local", "--get", "branch.feature/test.remote"]
            )
            .is_empty()
        );
        assert!(
            stdout(
                &git,
                &root,
                &["config", "--local", "--get", "branch.feature/test.merge"]
            )
            .is_empty()
        );
        cleanup(root);
    }

    #[tokio::test]
    async fn dirty_staged_and_mixed_states_are_allowed() {
        for state in 0..3 {
            let (git, root) = fixture();
            fs::write(root.join("tracked"), "base\n").unwrap();
            git_ok(&git, &root, &["add", "tracked"]);
            git_ok(&git, &root, &["commit", "--quiet", "-m", "tracked"]);
            if state != 0 {
                fs::write(root.join("tracked"), "changed\n").unwrap();
            }
            if state == 2 {
                git_ok(&git, &root, &["add", "tracked"]);
                fs::write(root.join("tracked"), "mixed\n").unwrap();
            }
            let result = policy(&git, &root).create(format!("feature/{state}")).await;
            assert!(
                matches!(
                    &result,
                    BranchCreationDisposition::BranchCreatedVerified { .. }
                ),
                "{result:?}"
            );
            cleanup(root);
        }
    }

    #[tokio::test]
    async fn exact_prefix_case_and_packed_collisions_are_rejected() {
        for (existing, candidate) in [
            ("feature", "feature"),
            ("foo", "foo/bar"),
            ("foo/bar", "foo"),
            ("Feature", "feature"),
        ] {
            let (git, root) = fixture();
            git_ok(
                &git,
                &root,
                &["update-ref", &format!("refs/heads/{existing}"), "HEAD"],
            );
            let result = policy(&git, &root).create(candidate.into()).await;
            assert!(
                matches!(result, BranchCreationDisposition::PreconditionFailed),
                "{result:?}"
            );
            cleanup(root);
        }
        let (git, root) = fixture();
        git_ok(&git, &root, &["update-ref", "refs/heads/packed", "HEAD"]);
        git_ok(&git, &root, &["pack-refs", "--all"]);
        assert!(matches!(
            policy(&git, &root).create("packed".into()).await,
            BranchCreationDisposition::PreconditionFailed
        ));
        cleanup(root);
    }

    #[tokio::test]
    async fn tags_and_remote_tracking_heads_do_not_collide_or_change() {
        let (git, root) = fixture();
        let oid = stdout(&git, &root, &["rev-parse", "HEAD"]);
        git_ok(&git, &root, &["tag", "feature", "HEAD"]);
        git_ok(
            &git,
            &root,
            &["update-ref", "refs/remotes/origin/feature", &oid],
        );
        let before_refs = stdout(&git, &root, &["show-ref"]);
        let p = policy(&git, &root);
        assert!(matches!(
            p.create("feature".into()).await,
            BranchCreationDisposition::BranchCreatedVerified { .. }
        ));
        assert_eq!(p.attempts.load(Ordering::Relaxed), 1);
        let after_refs = stdout(&git, &root, &["show-ref"]);
        let existing_after = after_refs
            .lines()
            .filter(|line| !line.ends_with(" refs/heads/feature"))
            .collect::<Vec<_>>();
        assert_eq!(existing_after, before_refs.lines().collect::<Vec<_>>());
        cleanup(root);
    }

    #[tokio::test]
    async fn local_head_count_overflow_fails_before_mutation() {
        let (git, root) = fixture();
        let oid = stdout(&git, &root, &["rev-parse", "HEAD"]);
        let mut input = String::new();
        for index in 0..=MAX_LOCAL_HEADS {
            input.push_str(&format!("create refs/heads/overflow/{index:04} {oid}\n"));
        }
        git_stdin(&git, &root, &["update-ref", "--stdin"], &input);
        let p = policy(&git, &root);
        assert!(matches!(
            p.create("count-overflow".into()).await,
            BranchCreationDisposition::PreconditionFailed
        ));
        assert_eq!(p.attempts.load(Ordering::Relaxed), 0);
        assert!(
            stdout(
                &git,
                &root,
                &["show-ref", "--verify", "refs/heads/count-overflow"]
            )
            .is_empty()
        );
        cleanup(root);
    }

    #[tokio::test]
    async fn local_head_output_overflow_fails_before_mutation() {
        let (git, root) = fixture();
        let oid = stdout(&git, &root, &["rev-parse", "HEAD"]);
        let mut input = String::new();
        for index in 0..(MAX_LOCAL_HEADS - 1) {
            let name = format!("wide/{index:04}-{}", "a".repeat(110));
            input.push_str(&format!("create refs/heads/{name} {oid}\n"));
        }
        git_stdin(&git, &root, &["update-ref", "--stdin"], &input);
        let p = policy(&git, &root);
        assert!(matches!(
            p.create("output-overflow".into()).await,
            BranchCreationDisposition::PreconditionFailed
        ));
        assert_eq!(p.attempts.load(Ordering::Relaxed), 0);
        assert!(
            stdout(
                &git,
                &root,
                &["show-ref", "--verify", "refs/heads/output-overflow"]
            )
            .is_empty()
        );
        cleanup(root);
    }

    #[tokio::test]
    async fn detached_unborn_bare_linked_and_special_state_are_rejected() {
        let (git, root) = fixture();
        git_ok(&git, &root, &["checkout", "--detach", "HEAD"]);
        assert!(matches!(
            policy(&git, &root).create("x".into()).await,
            BranchCreationDisposition::PreconditionFailed
        ));
        cleanup(root);
        let (git, root) = fixture();
        let linked = std::env::temp_dir().join(format!(
            "rah-branch-linked-{}",
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&linked);
        git_ok(
            &git,
            &root,
            &[
                "worktree",
                "add",
                "--quiet",
                linked.to_str().unwrap(),
                "HEAD",
            ],
        );
        assert!(RepositoryBranchCreationPolicy::new(&git, &linked).is_err());
        cleanup(linked);
        cleanup(root);
        let root = std::env::temp_dir().join(format!(
            "rah-branch-unborn-{}",
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        git_ok(&git, &root, &["init", "--quiet"]);
        assert!(matches!(
            policy(&git, &root).create("x".into()).await,
            BranchCreationDisposition::PreconditionFailed
        ));
        cleanup(root);
        let bare = std::env::temp_dir().join(format!(
            "rah-branch-bare-{}",
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        git_ok(
            &git,
            Path::new("."),
            &["init", "--bare", "--quiet", bare.to_str().unwrap()],
        );
        assert!(RepositoryBranchCreationPolicy::new(&git, &bare).is_err());
        cleanup(bare);
        let (git, root) = fixture();
        fs::write(root.join(".git/MERGE_HEAD"), b"x\n").unwrap();
        assert!(matches!(
            policy(&git, &root).create("x".into()).await,
            BranchCreationDisposition::PreconditionFailed
        ));
        cleanup(root);
    }

    #[tokio::test]
    async fn hooks_are_confined_and_tampering_fails_closed() {
        let (git, root) = fixture();
        let marker = root.join("marker");
        #[cfg(unix)]
        {
            fs::write(
                root.join(".git/hooks/reference-transaction"),
                format!("#!/bin/sh\ntouch {}\n", marker.display()),
            )
            .unwrap();
        }
        let p = policy(&git, &root);
        let result = p.create("safe".into()).await;
        assert!(
            matches!(
                &result,
                BranchCreationDisposition::BranchCreatedVerified { .. }
            ),
            "{result:?}"
        );
        assert!(!marker.exists());
        cleanup(root);
        let (git, root) = fixture();
        let p = policy(&git, &root);
        fs::write(p.hooks.join("unexpected"), b"tampered").unwrap();
        assert!(matches!(
            p.create("tampered".into()).await,
            BranchCreationDisposition::PreconditionFailed
        ));
        cleanup(root);
    }

    #[tokio::test]
    async fn repository_local_hooks_path_cannot_override_host_confinement() {
        let (git, root) = fixture();
        let hostile = root.join("hostile-hooks");
        fs::create_dir(&hostile).unwrap();
        let marker = root.join("hostile-marker");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let hook = hostile.join("reference-transaction");
            fs::write(&hook, format!("#!/bin/sh\ntouch {}\n", marker.display())).unwrap();
            fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();
        }
        git_ok(
            &git,
            &root,
            &["config", "core.hooksPath", hostile.to_str().unwrap()],
        );
        let result = policy(&git, &root).create("confined".into()).await;
        assert!(matches!(
            result,
            BranchCreationDisposition::BranchCreatedVerified { .. }
        ));
        assert!(!marker.exists());
        cleanup(root);
    }

    #[tokio::test]
    async fn invalid_input_is_rejected_without_a_mutating_attempt() {
        let (git, root) = fixture();
        let p = policy(&git, &root);
        assert!(matches!(
            p.create("CON".into()).await,
            BranchCreationDisposition::InvalidInput
        ));
        assert_eq!(p.attempts.load(Ordering::Relaxed), 0);
        cleanup(root);
    }

    #[tokio::test]
    async fn cas_race_is_not_retried_and_lost_result_is_not_verified() {
        let (git, root) = fixture();
        let p = policy(&git, &root);
        let guard = test_phase::install(&p, "cas_race");
        let result = p.create("race".into()).await;
        assert!(
            matches!(
                &result,
                BranchCreationDisposition::DesiredStateObservedAfterUncertainAttempt { .. }
                    | BranchCreationDisposition::Uncertain
            ),
            "{result:?}"
        );
        assert_eq!(p.attempts.load(Ordering::Relaxed), 1);
        drop(guard);
        cleanup(root);
        let (git, root) = fixture();
        let p = policy(&git, &root);
        let guard = test_phase::install(&p, "lost_result");
        let result = p.create("lost".into()).await;
        assert!(
            matches!(
                &result,
                BranchCreationDisposition::DesiredStateObservedAfterUncertainAttempt { .. }
            ),
            "{result:?}"
        );
        assert_eq!(p.attempts.load(Ordering::Relaxed), 1);
        drop(guard);
        cleanup(root);
    }

    #[tokio::test]
    async fn spawn_failure_is_known_no_effect_and_observer_failure_is_uncertain() {
        let (git, root) = fixture();
        let p = policy(&git, &root);
        let guard = test_phase::install(&p, "spawn_failure");
        let result = p.create("no-effect".into()).await;
        assert!(
            matches!(&result, BranchCreationDisposition::KnownNoEffect),
            "{result:?}"
        );
        assert_eq!(p.attempts.load(Ordering::Relaxed), 1);
        drop(guard);
        cleanup(root);

        let (git, root) = fixture();
        let p = policy(&git, &root);
        let guard = test_phase::install(&p, "unknown_post_state");
        let result = p.create("uncertain".into()).await;
        assert!(
            matches!(&result, BranchCreationDisposition::Uncertain),
            "{result:?}"
        );
        assert_eq!(p.attempts.load(Ordering::Relaxed), 1);
        drop(guard);
        cleanup(root);
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn windows_extension_device_spellings_fail_without_branch_mutation() {
        for name in ["CON.txt", "con.txt", "NUL.log", "COM1.foo"] {
            let (git, root) = fixture();
            assert!(validate_branch_name(name).is_ok());
            let result = policy(&git, &root).create(name.into()).await;
            assert!(matches!(result, BranchCreationDisposition::KnownNoEffect));
            assert!(
                !stdout(
                    &git,
                    &root,
                    &["show-ref", "--verify", &format!("refs/heads/{name}")]
                )
                .contains(name)
            );
            cleanup(root);
        }
    }

    #[tokio::test]
    async fn policy_is_reusable_but_each_call_has_one_attempt() {
        let (git, root) = fixture();
        let p = policy(&git, &root);
        let first = p.create("one".into()).await;
        assert!(
            matches!(
                &first,
                BranchCreationDisposition::BranchCreatedVerified { .. }
            ),
            "{first:?}"
        );
        assert!(matches!(
            p.create("two".into()).await,
            BranchCreationDisposition::BranchCreatedVerified { .. }
        ));
        assert_eq!(p.attempts.load(Ordering::Relaxed), 2);
        cleanup(root);
    }
}
