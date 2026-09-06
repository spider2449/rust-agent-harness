//! Host-owned bounded creation of one local branch.
//!
//! The mutation policy remains private.  The public surface is an opaque
//! host-created authority and a first-party Tool that delegates to it.  The
//! only mutation permitted here is one fixed, expected-absence `update-ref`
//! invocation for one host-validated local branch.

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
use rah_protocol::{PermissionLevel, ToolContent, ToolDefinition, ToolInput, ToolName, ToolOutput};
use rah_sandbox::{HostProcessOutput, OutputLimits};
use serde_json::{Value, json};

use crate::{
    HostArgumentPolicy, HostExecutionPolicy, Tool, ToolContext, ToolError,
    git_stage::repository_lease,
    git_support::{git_environment, git_error},
    host_execute::paths_equivalent,
    repository_observer::{FileIdentity, RepositoryIdentity, reject_link_or_reparse},
};

/// Stable name for the bounded repository local-branch creation capability.
pub const REPOSITORY_CREATE_BRANCH_TOOL_NAME: &str = "repo.create-branch";

/// Opaque host-created authority for one selected repository and Git executable.
#[derive(Clone)]
pub struct RepositoryBranchCreationAuthority {
    policy: Arc<RepositoryBranchCreationPolicy>,
}

impl RepositoryBranchCreationAuthority {
    /// Binds branch-creation authority to host-selected resources.
    pub fn new(
        git_executable: impl AsRef<Path>,
        repository_root: impl AsRef<Path>,
    ) -> Result<Self, ToolError> {
        Ok(Self {
            policy: Arc::new(RepositoryBranchCreationPolicy::new(
                git_executable.as_ref(),
                repository_root.as_ref(),
            )?),
        })
    }

    /// Confirms that the authority is bound to the selected resources.
    #[must_use]
    pub fn matches_resources(&self, git_executable: &Path, repository_root: &Path) -> bool {
        self.policy
            .matches_resources(git_executable, repository_root)
    }
}

/// First-party Tool for creating one local branch without switching branches.
pub struct RepositoryBranchCreationTool {
    authority: RepositoryBranchCreationAuthority,
}

impl RepositoryBranchCreationTool {
    /// Constructs a Tool from host-selected resources through the same opaque
    /// authority boundary as [`Self::from_authority`].
    pub fn new(
        git_executable: impl AsRef<Path>,
        repository_root: impl AsRef<Path>,
    ) -> Result<Self, ToolError> {
        Ok(Self::from_authority(
            RepositoryBranchCreationAuthority::new(git_executable, repository_root)?,
        ))
    }

    /// Constructs a Tool from an authority explicitly composed by the host.
    #[must_use]
    pub fn from_authority(authority: RepositoryBranchCreationAuthority) -> Self {
        Self { authority }
    }
}

#[async_trait::async_trait]
impl Tool for RepositoryBranchCreationTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: ToolName::new(REPOSITORY_CREATE_BRANCH_TOOL_NAME),
            description: "Creates one new local branch at the repository's current committed HEAD without switching branches.".to_owned(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "minLength": 1,
                        "maxLength": 128
                    }
                },
                "required": ["name"],
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
        let request = match BranchCreationRequest::parse(&input) {
            Ok(request) => request,
            Err(()) => return Ok(tool_result("invalid_input", false, None, None)),
        };
        Ok(tool_result_from_disposition(
            self.authority.policy.create(request.name).await,
        ))
    }
}

struct BranchCreationRequest {
    name: String,
}

impl BranchCreationRequest {
    fn parse(input: &ToolInput) -> Result<Self, ()> {
        let Value::Object(object) = &input.0 else {
            return Err(());
        };
        if object.len() != 1 {
            return Err(());
        }
        let Some(Value::String(name)) = object.get("name") else {
            return Err(());
        };
        Ok(Self { name: name.clone() })
    }
}

fn tool_result_from_disposition(disposition: BranchCreationDisposition) -> ToolOutput {
    match disposition {
        BranchCreationDisposition::InvalidInput => tool_result("invalid_input", false, None, None),
        BranchCreationDisposition::PreconditionFailed => {
            tool_result("precondition_failed", false, None, None)
        }
        BranchCreationDisposition::KnownNoEffect => {
            tool_result("known_no_effect", false, None, None)
        }
        BranchCreationDisposition::BranchCreatedVerified { name, oid } => {
            tool_result("branch_created_verified", false, Some(&name), Some(&oid))
        }
        BranchCreationDisposition::DesiredStateObservedAfterUncertainAttempt { name, oid } => {
            tool_result(
                "desired_state_observed_after_uncertain_attempt",
                true,
                Some(&name),
                Some(&oid),
            )
        }
        BranchCreationDisposition::Uncertain => tool_result("uncertain", true, None, None),
    }
}

fn tool_result(status: &str, uncertain: bool, name: Option<&str>, oid: Option<&str>) -> ToolOutput {
    let mut value = json!({"status": status, "uncertain": uncertain});
    if let Some(name) = name {
        value["name"] = Value::String(name.to_owned());
    }
    if let Some(oid) = oid {
        value["oid"] = Value::String(oid.to_owned());
    }
    ToolOutput {
        content: vec![ToolContent::Json(value)],
        is_error: status != "branch_created_verified",
    }
}

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
    #[cfg(test)]
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
            #[cfg(test)]
            generation: uuid::Uuid::new_v4(),
            #[cfg(test)]
            attempts: AtomicUsize::new(0),
        })
    }

    fn matches_resources(&self, git: &Path, root: &Path) -> bool {
        fs::canonicalize(git)
            .ok()
            .is_some_and(|candidate| paths_equivalent(&candidate, &self.git))
            && fs::canonicalize(root)
                .ok()
                .is_some_and(|candidate| paths_equivalent(&candidate, self.repository.root()))
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
    use crate::{Tool, ToolContext, ToolRegistry};
    use rah_protocol::{PermissionLevel, ToolCall, ToolCallId, ToolContent, ToolInput, ToolOutput};
    use serde_json::json;
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

    fn json_result(output: &ToolOutput) -> Value {
        let [ToolContent::Json(value)] = output.content.as_slice() else {
            panic!("branch creation must return one JSON result")
        };
        value.clone()
    }

    async fn execute_tool_output(tool: &RepositoryBranchCreationTool, value: Value) -> ToolOutput {
        tool.execute(ToolInput(value), ToolContext::default())
            .await
            .unwrap()
    }

    async fn execute_tool(tool: &RepositoryBranchCreationTool, value: Value) -> Value {
        json_result(&execute_tool_output(tool, value).await)
    }

    fn registry_call(value: Value) -> ToolCall {
        ToolCall {
            id: ToolCallId::new(),
            name: ToolName::new(REPOSITORY_CREATE_BRANCH_TOOL_NAME),
            input: ToolInput(value),
        }
    }

    #[test]
    fn public_definition_is_closed_and_execute_gated() {
        let (git, root) = fixture();
        let tool = RepositoryBranchCreationTool::new(&git, &root).unwrap();
        assert_eq!(tool.definition().name.as_str(), "repo.create-branch");
        assert_eq!(
            tool.definition().description,
            "Creates one new local branch at the repository's current committed HEAD without switching branches."
        );
        assert_eq!(tool.definition().permission, PermissionLevel::Execute);
        assert_eq!(
            tool.definition().input_schema,
            json!({
                "type":"object",
                "properties":{"name":{"type":"string","minLength":1,"maxLength":128}},
                "required":["name"],
                "additionalProperties":false
            })
        );
        cleanup(root);
    }

    #[test]
    fn authority_resource_matching_is_bound_without_spawning_git() {
        let (git_a, root_a) = fixture();
        let (_git_b, root_b) = fixture();
        let authority = RepositoryBranchCreationAuthority::new(&git_a, &root_a).unwrap();
        let different_git = root_a.join("different-git");
        fs::copy(&git_a, &different_git).unwrap();

        assert!(authority.matches_resources(&git_a, &root_a));
        assert!(!authority.matches_resources(&different_git, &root_a));
        assert!(!authority.matches_resources(&git_a, &root_b));

        cleanup(root_a);
        cleanup(root_b);
    }

    #[test]
    fn registry_has_no_implicit_branch_authority() {
        let registry = ToolRegistry::new();
        assert!(
            registry
                .definitions()
                .iter()
                .all(|definition| definition.name.as_str() != REPOSITORY_CREATE_BRANCH_TOOL_NAME)
        );
    }

    #[tokio::test]
    async fn explicit_authority_tool_registry_composition_creates_exact_branch_without_switching() {
        let (git, root) = fixture();
        let authority = RepositoryBranchCreationAuthority::new(&git, &root).unwrap();
        let tool = RepositoryBranchCreationTool::from_authority(authority);
        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(tool)).unwrap();

        let definition = registry
            .definitions()
            .into_iter()
            .find(|definition| definition.name.as_str() == REPOSITORY_CREATE_BRANCH_TOOL_NAME)
            .unwrap();
        assert_eq!(definition.permission, PermissionLevel::Execute);
        let before_head = stdout(&git, &root, &["symbolic-ref", "--quiet", "HEAD"]);
        let before_oid = stdout(&git, &root, &["rev-parse", "HEAD"]);
        let output = registry
            .execute(
                registry_call(json!({"name":"feature/test"})),
                ToolContext::default(),
            )
            .await
            .unwrap();
        let [ToolContent::Json(value)] = output.content.as_slice() else {
            panic!("branch creation must return one JSON result")
        };
        assert_eq!(
            value,
            &json!({
                "status":"branch_created_verified",
                "uncertain":false,
                "name":"feature/test",
                "oid":before_oid
            })
        );
        assert!(!output.is_error);
        assert_eq!(
            stdout(&git, &root, &["symbolic-ref", "--quiet", "HEAD"]),
            before_head
        );
        assert_eq!(
            stdout(&git, &root, &["rev-parse", "refs/heads/feature/test"]),
            before_oid
        );
        cleanup(root);
    }

    #[tokio::test]
    async fn authority_tool_is_reusable_and_duplicate_target_is_not_overwritten() {
        let (git, root) = fixture();
        let authority = RepositoryBranchCreationAuthority::new(&git, &root).unwrap();
        let tool = RepositoryBranchCreationTool::from_authority(authority);
        let first = execute_tool(&tool, json!({"name":"feature/one"})).await;
        let second = execute_tool(&tool, json!({"name":"feature/two"})).await;
        assert_eq!(first["status"], "branch_created_verified");
        assert_eq!(second["status"], "branch_created_verified");
        assert_eq!(first["oid"], second["oid"]);
        assert_eq!(
            stdout(&git, &root, &["rev-parse", "refs/heads/feature/one"]),
            first["oid"]
        );
        assert_eq!(
            stdout(&git, &root, &["rev-parse", "refs/heads/feature/two"]),
            second["oid"]
        );

        let duplicate = execute_tool(&tool, json!({"name":"feature/one"})).await;
        assert_eq!(duplicate["status"], "precondition_failed");
        assert_eq!(duplicate["uncertain"], false);
        assert!(duplicate.get("name").is_none());
        cleanup(root);
    }

    #[tokio::test]
    async fn tool_rejects_closed_input_without_echoing_invalid_name() {
        let (git, root) = fixture();
        let tool = RepositoryBranchCreationTool::new(&git, &root).unwrap();
        for input in [
            json!({}),
            json!({"name":"valid","extra":true}),
            json!({"name":17}),
            json!("name"),
            json!(null),
            json!({"name":""}),
            json!({"name":"feature?bad"}),
            json!({"name":"é"}),
            json!({"name":"refs/heads/foo"}),
        ] {
            let output = execute_tool(&tool, input).await;
            assert_eq!(output["status"], "invalid_input");
            assert_eq!(output["uncertain"], false);
            assert!(output.get("name").is_none());
            assert!(output.get("oid").is_none());
        }
        let serialized = serde_json::to_string(
            &execute_tool(&tool, json!({"name":"secret-invalid-value?"})).await,
        )
        .unwrap();
        assert!(!serialized.contains("secret-invalid-value?"));
        cleanup(root);
    }

    #[tokio::test]
    async fn tool_maps_closed_uncertain_and_no_effect_dispositions() {
        let (git, root) = fixture();
        let tool = RepositoryBranchCreationTool::new(&git, &root).unwrap();
        let guard = test_phase::install(tool.authority.policy.as_ref(), "spawn_failure");
        let known_no_effect = execute_tool_output(&tool, json!({"name":"known-no-effect"})).await;
        assert!(known_no_effect.is_error);
        let known_no_effect_value = json_result(&known_no_effect);
        assert_eq!(known_no_effect_value["status"], "known_no_effect");
        assert_eq!(known_no_effect_value["uncertain"], false);
        drop(guard);
        cleanup(root);

        let (git, root) = fixture();
        let tool = RepositoryBranchCreationTool::new(&git, &root).unwrap();
        let guard = test_phase::install(tool.authority.policy.as_ref(), "lost_result");
        let desired_after_uncertain =
            execute_tool_output(&tool, json!({"name":"lost-result"})).await;
        assert!(desired_after_uncertain.is_error);
        let desired_after_uncertain = json_result(&desired_after_uncertain);
        assert_eq!(
            desired_after_uncertain["status"],
            "desired_state_observed_after_uncertain_attempt"
        );
        assert_eq!(desired_after_uncertain["uncertain"], true);
        assert_eq!(desired_after_uncertain["name"], "lost-result");
        assert!(desired_after_uncertain.get("oid").is_some());
        drop(guard);
        cleanup(root);

        let (git, root) = fixture();
        let tool = RepositoryBranchCreationTool::new(&git, &root).unwrap();
        let guard = test_phase::install(tool.authority.policy.as_ref(), "unknown_post_state");
        let uncertain = execute_tool_output(&tool, json!({"name":"uncertain"})).await;
        assert!(uncertain.is_error);
        let uncertain = json_result(&uncertain);
        assert_eq!(uncertain["status"], "uncertain");
        assert_eq!(uncertain["uncertain"], true);
        assert!(uncertain.get("name").is_none());
        assert!(uncertain.get("oid").is_none());
        drop(guard);
        cleanup(root);
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
