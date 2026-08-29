#[cfg(any(test, feature = "live-test-support"))]
use std::sync::atomic::Ordering;
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Component, Path, PathBuf},
};

#[cfg(windows)]
use std::fs::File;

use async_trait::async_trait;
use futures::lock::{Mutex as AsyncMutex, MutexGuard};
use rah_protocol::{PermissionLevel, ToolContent, ToolDefinition, ToolInput, ToolName, ToolOutput};
use rah_sandbox::HostProcessOutput;
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    HostArgumentPolicy, HostExecutionPolicy, Tool, ToolContext, ToolError,
    git_support::{git_environment, git_error},
    host_execute::{is_beneath, paths_equivalent},
};

/// Stable name for the bounded repository worktree text replacement capability.
pub const REPOSITORY_WORKTREE_PATCH_TOOL_NAME: &str = "repo.patch";

const MAX_SERIALIZED_REQUEST_BYTES: usize = 64 * 1024;
const MAX_PATH_BYTES: usize = 1024;
const MAX_TEXT_BYTES: usize = 64 * 1024;
const MAX_FILE_BYTES: usize = 1024 * 1024;
const MAX_REPLACEMENTS: usize = 16;
const BOM: &[u8] = b"\xef\xbb\xbf";

#[cfg(feature = "live-test-support")]
static LIVE_REPLACEMENT_ATTEMPTS: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

/// Resets the opt-in process-local replacement-attempt observation counter.
///
/// This is compiled only for the live validation fixture. It is not a policy,
/// audit, or execution surface and cannot alter `repo.patch` authority.
#[cfg(feature = "live-test-support")]
pub fn reset_live_test_replacement_attempts() {
    LIVE_REPLACEMENT_ATTEMPTS.store(0, Ordering::Relaxed);
}

/// Returns the opt-in process-local native replacement-attempt observation.
///
/// This is compiled only for the live validation fixture and is deliberately
/// unavailable from the default production crate build.
#[cfg(feature = "live-test-support")]
#[must_use]
pub fn live_test_replacement_attempts() -> usize {
    LIVE_REPLACEMENT_ATTEMPTS.load(Ordering::Relaxed)
}

/// Host-constructed, bounded worktree text replacement capability.
///
/// The policy that grants this authority remains crate-private. Construction
/// fixes the Git executable and repository root; model input may only request
/// the one exact conditional replacement described by this tool's schema.
pub struct RepositoryWorktreePatchTool {
    policy: RepositoryWorktreeMutationPolicy,
}

impl RepositoryWorktreePatchTool {
    /// Creates a `repo.patch` tool for one trusted non-bare repository root.
    pub fn new(
        git_executable: impl AsRef<Path>,
        repository_root: impl AsRef<Path>,
    ) -> Result<Self, ToolError> {
        Ok(Self {
            policy: RepositoryWorktreeMutationPolicy::new(
                git_executable.as_ref(),
                repository_root.as_ref(),
                PatchLimits::default(),
            )?,
        })
    }
}

#[async_trait]
impl Tool for RepositoryWorktreePatchTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: ToolName::new(REPOSITORY_WORKTREE_PATCH_TOOL_NAME),
            description: "Replaces one or more uniquely matched literal text fragments in one clean tracked worktree file."
                .to_owned(),
            input_schema: json!({
                "oneOf": [{
                    "type": "object",
                    "properties": {
                        "path": {"type": "string", "minLength": 1, "maxLength": MAX_PATH_BYTES},
                        "expected_file_sha256": {"type": "string", "pattern": "^[0-9a-f]{64}$"},
                        "expected_file_byte_length": {"type": "integer", "minimum": 0, "maximum": MAX_FILE_BYTES},
                        "expected_old_text": {"type": "string", "minLength": 1, "maxLength": MAX_TEXT_BYTES},
                        "replacement_text": {"type": "string", "maxLength": MAX_TEXT_BYTES}
                    },
                    "required": ["path", "expected_file_sha256", "expected_file_byte_length", "expected_old_text", "replacement_text"],
                    "additionalProperties": false
                }, {
                    "type": "object",
                    "properties": {
                        "path": {"type": "string", "minLength": 1, "maxLength": MAX_PATH_BYTES},
                        "expected_file_sha256": {"type": "string", "pattern": "^[0-9a-f]{64}$"},
                        "expected_file_byte_length": {"type": "integer", "minimum": 0, "maximum": MAX_FILE_BYTES},
                        "replacements": {
                            "type": "array", "minItems": 1, "maxItems": MAX_REPLACEMENTS,
                            "items": {
                                "type": "object",
                                "properties": {
                                    "expected_old_text": {"type": "string", "minLength": 1, "maxLength": MAX_TEXT_BYTES},
                                    "replacement_text": {"type": "string", "maxLength": MAX_TEXT_BYTES}
                                },
                                "required": ["expected_old_text", "replacement_text"],
                                "additionalProperties": false
                            }
                        }
                    },
                    "required": ["path", "expected_file_sha256", "expected_file_byte_length", "replacements"],
                    "additionalProperties": false
                }]
            }),
            permission: PermissionLevel::Execute,
        }
    }

    async fn execute(
        &self,
        input: ToolInput,
        _context: ToolContext,
    ) -> Result<ToolOutput, ToolError> {
        let request = PatchRequest::parse(&input, self.policy.limits)?;
        let _lease = self.policy.acquire_lease().await;
        let outcome = self.policy.execute_once(request).await;
        Ok(outcome.into_tool_output())
    }
}

/// Private, host-owned authority for one complete conditional worktree replacement.
///
/// It is intentionally distinct from `RepositoryMutationPolicy`,
/// `WorkspacePolicy`, and `HostExecutionPolicy`. The latter is only used here
/// as a bounded host-side Git observation mechanism.
struct RepositoryWorktreeMutationPolicy {
    git: PathBuf,
    git_identity: FileIdentity,
    root: PathBuf,
    root_identity: FileIdentity,
    dot_git_identity: FileIdentity,
    lease: std::sync::Arc<AsyncMutex<()>>,
    limits: PatchLimits,
    #[cfg(test)]
    test_hook: TestHook,
}

impl RepositoryWorktreeMutationPolicy {
    fn new(git: &Path, root: &Path, limits: PatchLimits) -> Result<Self, ToolError> {
        if !root.is_absolute() {
            return Err(policy_error("repository root must be an absolute path"));
        }
        reject_reparse_ancestry(root, "repository root")?;
        let root = canonical_directory(root, "repository root")?;
        validate_directory_path(&root, &root, "repository root")?;
        let dot_git = root.join(".git");
        reject_link_or_reparse(&dot_git, "repository metadata")?;
        if !fs::metadata(&dot_git).map_err(fs_error)?.is_dir() {
            return Err(policy_error(
                "repository metadata must be a directory; linked worktrees are unsupported",
            ));
        }
        let git = canonical_git_executable(git)?;
        Ok(Self {
            git_identity: FileIdentity::capture(&git)?,
            git,
            root_identity: FileIdentity::capture(&root)?,
            dot_git_identity: FileIdentity::capture(&dot_git)?,
            lease: crate::git_stage::repository_lease(&root),
            root,
            limits,
            #[cfg(test)]
            test_hook: TestHook::default(),
        })
    }

    async fn acquire_lease(&self) -> MutexGuard<'_, ()> {
        self.lease.lock().await
    }

    async fn execute_once(&self, request: PatchRequest) -> MutationOutcome {
        let pre = match self.capture_preimage(&request).await {
            Ok(pre) => pre,
            Err(reason) => return MutationOutcome::Refused { reason },
        };

        let postimage = match build_postimage(&pre, &request, self.limits) {
            Ok(postimage) => postimage,
            Err(reason) => return MutationOutcome::Refused { reason },
        };
        #[cfg(test)]
        self.test_hook.run(
            TestPhase::AfterExpectedTextValidation,
            &pre.target.path,
            None,
        );
        #[cfg(test)]
        if self.test_hook.should_fail(TestPhase::BeforeTemporaryWrite) {
            return MutationOutcome::Refused {
                reason: RefusalReason::Temporary("injected pre-temporary failure".to_owned()),
            };
        }
        let temporary = match self.write_temporary(&pre.target, &postimage.bytes) {
            Ok(temporary) => temporary,
            Err(reason) => return MutationOutcome::Refused { reason },
        };
        #[cfg(test)]
        self.test_hook.run(
            TestPhase::AfterTemporaryWrite,
            &pre.target.path,
            Some(&temporary.path),
        );
        #[cfg(test)]
        if self.test_hook.should_fail(TestPhase::AfterTemporaryWrite) {
            return self.refuse_after_temp(
                RefusalReason::Temporary("injected post-temporary failure".to_owned()),
                temporary,
                &postimage,
            );
        }

        if let Err(reason) = self
            .revalidate_before_commit(&pre, &temporary, &postimage)
            .await
        {
            return self.refuse_after_temp(reason, temporary, &postimage);
        }
        #[cfg(test)]
        self.test_hook.run(
            TestPhase::BeforeReplacement,
            &pre.target.path,
            Some(&temporary.path),
        );
        #[cfg(test)]
        if self.test_hook.should_fail(TestPhase::BeforeReplacement) {
            return self.refuse_after_temp(
                RefusalReason::Temporary("injected pre-replacement failure".to_owned()),
                temporary,
                &postimage,
            );
        }
        #[cfg(test)]
        self.test_hook
            .replacement_attempts
            .fetch_add(1, Ordering::Relaxed);
        #[cfg(feature = "live-test-support")]
        LIVE_REPLACEMENT_ATTEMPTS.fetch_add(1, Ordering::Relaxed);

        #[cfg(test)]
        let replacement = self
            .test_hook
            .replace_once(&temporary.path, &pre.target.path);
        #[cfg(not(test))]
        let replacement = replace_once(&temporary.path, &pre.target.path);
        match replacement {
            Ok(()) => {
                #[cfg(test)]
                self.test_hook.run(
                    TestPhase::AfterReplacement,
                    &pre.target.path,
                    Some(&temporary.path),
                );
                #[cfg(test)]
                if self
                    .test_hook
                    .should_fail(TestPhase::BeforePostVerification)
                {
                    return MutationOutcome::Uncertain {
                        evidence: MutationEvidence::from_images(
                            &pre,
                            &postimage,
                            MutationResultClass::Uncertain,
                        ),
                        reason: RefusalReason::PostObservation(
                            "injected post-replacement observation failure",
                        ),
                    };
                }
                match self.verify_postimage(&pre, &postimage, &temporary).await {
                    Ok(()) => MutationOutcome::Success {
                        evidence: MutationEvidence::from_images(
                            &pre,
                            &postimage,
                            MutationResultClass::Success,
                        ),
                    },
                    Err(reason) => MutationOutcome::Uncertain {
                        evidence: MutationEvidence::from_images(
                            &pre,
                            &postimage,
                            MutationResultClass::Uncertain,
                        ),
                        reason,
                    },
                }
            }
            Err(error) => {
                #[cfg(test)]
                self.test_hook.run(
                    TestPhase::AfterReplacement,
                    &pre.target.path,
                    Some(&temporary.path),
                );
                self.classify_replacement_failure(&pre, &postimage, &temporary, error)
            }
        }
    }

    fn refuse_after_temp(
        &self,
        reason: RefusalReason,
        temporary: Temporary,
        postimage: &Postimage,
    ) -> MutationOutcome {
        match remove_temporary(&temporary, &postimage.bytes, self.limits) {
            Ok(()) => MutationOutcome::Refused { reason },
            Err(cleanup) => MutationOutcome::Uncertain {
                evidence: MutationEvidence::empty(MutationResultClass::Uncertain),
                reason: RefusalReason::TemporaryCleanup(cleanup),
            },
        }
    }

    async fn capture_preimage(&self, request: &PatchRequest) -> Result<Preimage, RefusalReason> {
        self.revalidate_repository()
            .map_err(RefusalReason::Repository)?;
        let target = Target::capture(&self.root, &request.path).map_err(RefusalReason::Path)?;
        #[cfg(test)]
        self.test_hook
            .run(TestPhase::AfterInitialPathValidation, &target.path, None);
        let git = self
            .git_state(&target)
            .await
            .map_err(RefusalReason::Repository)?;
        self.git_worktree_clean(&target)
            .await
            .map_err(RefusalReason::Repository)?;
        #[cfg(test)]
        self.test_hook
            .run(TestPhase::AfterGitValidation, &target.path, None);
        let bytes = read_bounded(&target.path, self.limits.max_file_bytes)
            .map_err(|_| RefusalReason::Precondition("could not read bounded target preimage"))?;
        validate_preconditions(&bytes, request, self.limits)
            .map_err(RefusalReason::Precondition)?;
        #[cfg(test)]
        self.test_hook
            .run(TestPhase::AfterPreimageValidation, &target.path, None);
        Ok(Preimage { target, git, bytes })
    }

    async fn revalidate_before_commit(
        &self,
        pre: &Preimage,
        temporary: &Temporary,
        postimage: &Postimage,
    ) -> Result<(), RefusalReason> {
        self.revalidate_repository()
            .map_err(RefusalReason::Repository)?;
        pre.target
            .revalidate(&self.root)
            .map_err(RefusalReason::Path)?;
        #[cfg(test)]
        self.test_hook.run(
            TestPhase::AfterFinalTargetIdentityRevalidation,
            &pre.target.path,
            Some(&temporary.path),
        );
        self.revalidate_repository()
            .map_err(RefusalReason::Repository)?;
        pre.target
            .revalidate(&self.root)
            .map_err(RefusalReason::Path)?;
        let git = self
            .git_state(&pre.target)
            .await
            .map_err(RefusalReason::Repository)?;
        if git != pre.git {
            return Err(RefusalReason::Repository(policy_error(
                "repository state changed before replacement",
            )));
        }
        self.git_worktree_clean(&pre.target)
            .await
            .map_err(RefusalReason::Repository)?;
        let current = read_bounded(&pre.target.path, self.limits.max_file_bytes)
            .map_err(|_| RefusalReason::Precondition("could not reread bounded target preimage"))?;
        if current != pre.bytes {
            return Err(RefusalReason::Precondition(
                "target preimage changed before replacement",
            ));
        }
        temporary
            .revalidate(&postimage.bytes, self.limits)
            .map_err(RefusalReason::Temporary)?;
        Ok(())
    }

    async fn verify_postimage(
        &self,
        pre: &Preimage,
        postimage: &Postimage,
        temporary: &Temporary,
    ) -> Result<(), RefusalReason> {
        self.revalidate_repository()
            .map_err(RefusalReason::Repository)?;
        Target::verify_replaced(&self.root, &pre.target, &temporary.identity)
            .map_err(RefusalReason::Path)?;
        let git = self
            .git_state(&pre.target)
            .await
            .map_err(RefusalReason::Repository)?;
        if git != pre.git {
            return Err(RefusalReason::Repository(policy_error(
                "repository state changed during replacement",
            )));
        }
        let current = read_bounded(&pre.target.path, self.limits.max_file_bytes).map_err(|_| {
            RefusalReason::PostObservation("could not read bounded target after replacement")
        })?;
        if current != postimage.bytes {
            return Err(RefusalReason::PostObservation(
                "postimage did not match constructed content",
            ));
        }
        Ok(())
    }

    fn classify_replacement_failure(
        &self,
        pre: &Preimage,
        postimage: &Postimage,
        temporary: &Temporary,
        _error: std::io::Error,
    ) -> MutationOutcome {
        let target_intact = self
            .revalidate_repository()
            .and_then(|()| pre.target.revalidate(&self.root))
            .and_then(|()| {
                read_bounded(&pre.target.path, self.limits.max_file_bytes).map_err(fs_error)
            })
            .is_ok_and(|current| current == pre.bytes);
        let temporary_intact = temporary.revalidate(&postimage.bytes, self.limits).is_ok();
        let cleanup = if temporary_intact {
            remove_temporary(temporary, &postimage.bytes, self.limits)
        } else {
            Err("temporary replacement file could not be proven intact".to_owned())
        };
        if target_intact && temporary_intact && cleanup.is_ok() {
            MutationOutcome::KnownReplacementFailure {
                evidence: MutationEvidence::from_images(
                    pre,
                    postimage,
                    MutationResultClass::KnownReplacementFailure,
                ),
            }
        } else {
            MutationOutcome::Uncertain {
                evidence: MutationEvidence::from_images(
                    pre,
                    postimage,
                    MutationResultClass::Uncertain,
                ),
                reason: RefusalReason::PostObservation(
                    "replacement failure could not prove the preimage remained intact",
                ),
            }
        }
    }

    fn write_temporary(&self, target: &Target, bytes: &[u8]) -> Result<Temporary, RefusalReason> {
        for _ in 0..32 {
            let path = temporary_path(&target.parent);
            let mut file = match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(file) => file,
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(RefusalReason::Temporary(error.to_string())),
            };
            let temporary = match Temporary::capture_identity(&path) {
                Ok(temporary) => temporary,
                Err(reason) => {
                    drop(file);
                    return Err(RefusalReason::TemporaryCleanup(reason));
                }
            };
            #[cfg(unix)]
            if let Err(error) = set_temporary_permissions(&file, target.unix_mode) {
                drop(file);
                return match remove_temporary_identity(&temporary) {
                    Ok(()) => Err(RefusalReason::Temporary(error.to_string())),
                    Err(cleanup) => Err(RefusalReason::TemporaryCleanup(cleanup)),
                };
            }
            if let Err(error) = file.write_all(bytes).and_then(|()| file.sync_all()) {
                drop(file);
                return match remove_temporary_identity(&temporary) {
                    Ok(()) => Err(RefusalReason::Temporary(error.to_string())),
                    Err(cleanup) => Err(RefusalReason::TemporaryCleanup(cleanup)),
                };
            }
            drop(file);
            match temporary.revalidate(bytes, self.limits) {
                Ok(()) => return Ok(temporary),
                Err(reason) => {
                    return match remove_temporary_identity(&temporary) {
                        Ok(()) => Err(RefusalReason::Temporary(reason)),
                        Err(cleanup) => Err(RefusalReason::TemporaryCleanup(cleanup)),
                    };
                }
            }
        }
        Err(RefusalReason::Temporary(
            "could not allocate an exclusive temporary replacement file".to_owned(),
        ))
    }

    fn revalidate_repository(&self) -> Result<(), ToolError> {
        reject_reparse_ancestry(&self.root, "repository root")?;
        validate_directory_path(&self.root, &self.root, "repository root")?;
        let current = canonical_directory(&self.root, "repository root")?;
        let dot_git = current.join(".git");
        reject_link_or_reparse(&dot_git, "repository metadata")?;
        if !paths_equivalent(&current, &self.root)
            || FileIdentity::capture(&current)? != self.root_identity
            || !fs::metadata(&dot_git).map_err(fs_error)?.is_dir()
            || FileIdentity::capture(&dot_git)? != self.dot_git_identity
        {
            return Err(policy_error("repository identity changed"));
        }
        Ok(())
    }

    async fn git_state(&self, target: &Target) -> Result<GitState, ToolError> {
        let top = self
            .git_output(vec!["rev-parse", "--show-toplevel"])
            .await?;
        let top = std::str::from_utf8(&top)
            .map_err(|_| git_error("Git repository root response was not UTF-8"))?
            .trim_end_matches(['\r', '\n']);
        if !paths_equivalent(&fs::canonicalize(top).map_err(fs_error)?, &self.root) {
            return Err(git_error("configured root is not Git's worktree root"));
        }
        let bare = self
            .git_output(vec!["rev-parse", "--is-bare-repository"])
            .await?;
        if bare.as_slice() != b"false\n" && bare.as_slice() != b"false\r\n" {
            return Err(git_error("bare repositories are unsupported"));
        }
        let head = self
            .git_output(vec!["rev-parse", "--verify", "HEAD"])
            .await?;
        let head_entry = self
            .git_output(vec![
                "--literal-pathspecs",
                "ls-tree",
                "-z",
                "HEAD",
                "--",
                &target.git_path,
            ])
            .await?;
        let index_entry = self
            .git_output(vec![
                "--literal-pathspecs",
                "ls-files",
                "-s",
                "-z",
                "--",
                &target.git_path,
            ])
            .await?;
        let tag = self
            .git_output(vec![
                "--literal-pathspecs",
                "ls-files",
                "-v",
                "-z",
                "--",
                &target.git_path,
            ])
            .await?;
        let refs = self
            .git_output(vec![
                "for-each-ref",
                "--format=%(refname)%00%(objectname)%00",
            ])
            .await?;
        let head_entry = parse_head_entry(&head_entry, target.git_path.as_bytes())?;
        let index_entry = parse_index_entry(&index_entry, target.git_path.as_bytes())?;
        if head_entry != index_entry {
            return Err(git_error("target has staged or index divergence"));
        }
        require_normal_index_tag(&tag, target.git_path.as_bytes())?;
        Ok(GitState {
            head,
            head_entry,
            refs,
        })
    }

    async fn git_output(&self, arguments: Vec<&str>) -> Result<Vec<u8>, ToolError> {
        let output = self.git_process(arguments).await?;
        if output.exit_code != Some(0) {
            return Err(git_error(
                "bounded Git state observation did not complete successfully",
            ));
        }
        Ok(output.stdout)
    }

    async fn git_process(&self, arguments: Vec<&str>) -> Result<HostProcessOutput, ToolError> {
        self.revalidate_git()?;
        let policy = HostExecutionPolicy::new(
            &self.git,
            HostArgumentPolicy::Exact(arguments.into_iter().map(str::to_owned).collect()),
            &self.root,
            ".",
        )?
        .with_environment(git_environment())?;
        let output = policy.execute_process(&ToolInput(json!({}))).await?;
        if output.timed_out || output.overflow.is_some() {
            return Err(git_error(
                "bounded Git state observation did not complete successfully",
            ));
        }
        Ok(output)
    }

    /// Checks only the host-selected target using a fixed Git observation.
    /// A nonzero result is deliberately not exposed outside this policy.
    async fn git_worktree_clean(&self, target: &Target) -> Result<(), ToolError> {
        let output = self
            .git_process(vec![
                "--literal-pathspecs",
                "diff-files",
                "--quiet",
                "--",
                &target.git_path,
            ])
            .await?;
        match output.exit_code {
            Some(0) => Ok(()),
            Some(1) => Err(policy_error("target has unstaged worktree changes")),
            _ => Err(git_error(
                "bounded Git worktree observation did not complete successfully",
            )),
        }
    }

    fn revalidate_git(&self) -> Result<(), ToolError> {
        let current = canonical_git_executable(&self.git)?;
        if !paths_equivalent(&current, &self.git)
            || FileIdentity::capture(&current)? != self.git_identity
        {
            return Err(policy_error("configured Git executable identity changed"));
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
struct PatchLimits {
    max_serialized_request_bytes: usize,
    max_path_bytes: usize,
    max_text_bytes: usize,
    max_file_bytes: usize,
}

impl Default for PatchLimits {
    fn default() -> Self {
        Self {
            max_serialized_request_bytes: MAX_SERIALIZED_REQUEST_BYTES,
            max_path_bytes: MAX_PATH_BYTES,
            max_text_bytes: MAX_TEXT_BYTES,
            max_file_bytes: MAX_FILE_BYTES,
        }
    }
}

struct PatchRequest {
    path: PathBuf,
    expected_sha256: String,
    expected_length: usize,
    replacements: Vec<Replacement>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Replacement {
    old: String,
    new: String,
}

impl PatchRequest {
    fn parse(input: &ToolInput, limits: PatchLimits) -> Result<Self, ToolError> {
        let serialized = serde_json::to_vec(&input.0).map_err(|error| ToolError::InvalidInput {
            message: format!("input could not be serialized: {error}"),
        })?;
        if serialized.len() > limits.max_serialized_request_bytes {
            return Err(ToolError::InvalidInput {
                message: "input exceeds the repository patch request limit".to_owned(),
            });
        }
        let object = input.0.as_object().ok_or_else(|| ToolError::InvalidInput {
            message: "input must be an object".to_owned(),
        })?;
        reject_unknown_fields(
            object,
            [
                "path",
                "expected_file_sha256",
                "expected_file_byte_length",
                "expected_old_text",
                "replacement_text",
                "replacements",
            ],
        )?;
        let path = required_string(object, "path")?;
        let expected_sha256 = required_string(object, "expected_file_sha256")?;
        let expected_length = object
            .get("expected_file_byte_length")
            .and_then(Value::as_u64)
            .and_then(|value| usize::try_from(value).ok())
            .ok_or_else(|| ToolError::InvalidInput {
                message: "`expected_file_byte_length` must be a nonnegative integer".to_owned(),
            })?;
        if expected_length > limits.max_file_bytes {
            return Err(ToolError::InvalidInput {
                message: "`expected_file_byte_length` exceeds the file size limit".to_owned(),
            });
        }
        let path = parse_logical_path(path, limits.max_path_bytes)?;
        if !is_lower_sha256(expected_sha256) {
            return Err(ToolError::InvalidInput {
                message: "`expected_file_sha256` must be 64 lowercase hexadecimal characters"
                    .to_owned(),
            });
        }
        let legacy_old = object.get("expected_old_text");
        let legacy_new = object.get("replacement_text");
        let replacements = object.get("replacements");
        let replacements = match (legacy_old, legacy_new, replacements) {
            (Some(_), Some(_), None) => vec![Replacement {
                old: required_string(object, "expected_old_text")?.to_owned(),
                new: required_string(object, "replacement_text")?.to_owned(),
            }],
            (None, None, Some(value)) => parse_replacements(value)?,
            (Some(_), None, None) | (None, Some(_), None) => {
                return Err(ToolError::InvalidInput {
                    message: "legacy replacement fields must be supplied together".to_owned(),
                });
            }
            _ => {
                return Err(ToolError::InvalidInput {
                    message: "input must use exactly one replacement form".to_owned(),
                });
            }
        };
        validate_replacements(&replacements, limits)?;
        Ok(Self {
            path,
            expected_sha256: expected_sha256.to_owned(),
            expected_length,
            replacements,
        })
    }
}

fn parse_replacements(value: &Value) -> Result<Vec<Replacement>, ToolError> {
    let items = value.as_array().ok_or_else(|| ToolError::InvalidInput {
        message: "`replacements` must be an array".to_owned(),
    })?;
    if items.is_empty() || items.len() > MAX_REPLACEMENTS {
        return Err(ToolError::InvalidInput {
            message: "`replacements` must contain between 1 and 16 items".to_owned(),
        });
    }
    items
        .iter()
        .map(|item| {
            let object = item.as_object().ok_or_else(|| ToolError::InvalidInput {
                message: "each replacement must be an object".to_owned(),
            })?;
            reject_unknown_fields(object, ["expected_old_text", "replacement_text"])?;
            Ok(Replacement {
                old: required_string(object, "expected_old_text")?.to_owned(),
                new: required_string(object, "replacement_text")?.to_owned(),
            })
        })
        .collect()
}

fn validate_replacements(
    replacements: &[Replacement],
    limits: PatchLimits,
) -> Result<(), ToolError> {
    let mut aggregate = 0usize;
    for replacement in replacements {
        for (name, text, empty_allowed) in [
            ("expected_old_text", replacement.old.as_str(), false),
            ("replacement_text", replacement.new.as_str(), true),
        ] {
            if text.len() > limits.max_text_bytes
                || text.contains('\0')
                || (!empty_allowed && text.is_empty())
                || text.contains('\u{feff}')
            {
                return Err(ToolError::InvalidInput {
                    message: format!("`{name}` exceeds limits or contains unsupported content"),
                });
            }
            aggregate =
                aggregate
                    .checked_add(text.len())
                    .ok_or_else(|| ToolError::InvalidInput {
                        message: "aggregate replacement text exceeds the request limit".to_owned(),
                    })?;
        }
    }
    if aggregate > limits.max_text_bytes {
        return Err(ToolError::InvalidInput {
            message: "aggregate replacement text exceeds the request limit".to_owned(),
        });
    }
    Ok(())
}

struct Target {
    path: PathBuf,
    parent: PathBuf,
    parent_identity: FileIdentity,
    identity: FileIdentity,
    git_path: String,
    #[cfg(unix)]
    unix_mode: u32,
}

impl Target {
    fn capture(root: &Path, relative: &Path) -> Result<Self, ToolError> {
        let path = validate_existing_target(root, relative)?;
        let parent = path
            .parent()
            .ok_or_else(|| policy_error("target has no parent"))?
            .to_path_buf();
        let identity = FileIdentity::capture(&path)?;
        if identity.link_count > 1 {
            return Err(policy_error("hard-linked targets are unsupported"));
        }
        #[cfg(unix)]
        let unix_mode = unix_permission_mode(&fs::metadata(&path).map_err(fs_error)?);
        Ok(Self {
            git_path: relative.to_string_lossy().replace('\\', "/"),
            parent_identity: FileIdentity::capture(&parent)?,
            parent,
            identity,
            path,
            #[cfg(unix)]
            unix_mode,
        })
    }

    fn revalidate(&self, root: &Path) -> Result<(), ToolError> {
        let current = validate_existing_target(root, Path::new(&self.git_path))?;
        let parent = current
            .parent()
            .ok_or_else(|| policy_error("target has no parent"))?;
        let identity = FileIdentity::capture(&current)?;
        if !paths_equivalent(&current, &self.path)
            || FileIdentity::capture(parent)? != self.parent_identity
            || identity != self.identity
            || identity.link_count > 1
        {
            return Err(policy_error("target identity changed"));
        }
        #[cfg(unix)]
        if unix_permission_mode(&fs::metadata(&current).map_err(fs_error)?) != self.unix_mode {
            return Err(policy_error("target permissions changed"));
        }
        Ok(())
    }

    fn verify_replaced(
        root: &Path,
        previous: &Self,
        expected_identity: &FileIdentity,
    ) -> Result<(), ToolError> {
        let current = validate_existing_target(root, Path::new(&previous.git_path))?;
        let parent = current
            .parent()
            .ok_or_else(|| policy_error("target has no parent"))?;
        let identity = FileIdentity::capture(&current)?;
        if !paths_equivalent(&current, &previous.path)
            || FileIdentity::capture(parent)? != previous.parent_identity
            || identity != *expected_identity
            || identity.link_count > 1
        {
            return Err(policy_error("target path safety changed after replacement"));
        }
        #[cfg(unix)]
        if unix_permission_mode(&fs::metadata(&current).map_err(fs_error)?) != previous.unix_mode {
            return Err(policy_error(
                "target permissions were not preserved after replacement",
            ));
        }
        Ok(())
    }
}

/// Host-named postimage staging file and the identity captured after its bytes
/// were flushed. Its identity must become the target identity on success.
struct Temporary {
    path: PathBuf,
    identity: FileIdentity,
}

impl Temporary {
    fn capture_identity(path: &Path) -> Result<Self, String> {
        reject_link_or_reparse(path, "temporary replacement file")
            .map_err(|error| error.to_string())?;
        let metadata = fs::metadata(path).map_err(|error| error.to_string())?;
        if !metadata.is_file() {
            return Err("temporary replacement file must be a regular file".to_owned());
        }
        reject_unsupported_file_attributes(&metadata).map_err(|error| error.to_string())?;
        let identity = FileIdentity::capture(path).map_err(|error| error.to_string())?;
        if identity.link_count > 1 {
            return Err("temporary replacement file has unsupported hard links".to_owned());
        }
        Ok(Self {
            path: path.to_path_buf(),
            identity,
        })
    }

    fn revalidate(&self, expected: &[u8], limits: PatchLimits) -> Result<(), String> {
        self.revalidate_identity()?;
        validate_temporary(&self.path, expected, limits).map_err(|reason| reason.to_string())?;
        Ok(())
    }

    fn revalidate_identity(&self) -> Result<(), String> {
        let identity = FileIdentity::capture(&self.path).map_err(|error| error.to_string())?;
        if identity != self.identity || identity.link_count > 1 {
            return Err("temporary replacement file identity changed".to_owned());
        }
        Ok(())
    }
}

struct Preimage {
    target: Target,
    git: GitState,
    bytes: Vec<u8>,
}

struct Postimage {
    bytes: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct GitState {
    head: Vec<u8>,
    head_entry: GitEntry,
    refs: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct GitEntry {
    mode: Vec<u8>,
    object: Vec<u8>,
}

#[derive(Debug)]
enum RefusalReason {
    Path(ToolError),
    Repository(ToolError),
    Precondition(&'static str),
    Temporary(String),
    TemporaryCleanup(String),
    PostObservation(&'static str),
}

impl std::fmt::Display for RefusalReason {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Path(error) | Self::Repository(error) => error.fmt(formatter),
            Self::Precondition(message) | Self::PostObservation(message) => {
                formatter.write_str(message)
            }
            Self::Temporary(message) | Self::TemporaryCleanup(message) => {
                formatter.write_str(message)
            }
        }
    }
}

enum MutationOutcome {
    Refused {
        reason: RefusalReason,
    },
    Success {
        evidence: MutationEvidence,
    },
    KnownReplacementFailure {
        evidence: MutationEvidence,
    },
    Uncertain {
        evidence: MutationEvidence,
        reason: RefusalReason,
    },
}

impl MutationOutcome {
    fn into_tool_output(self) -> ToolOutput {
        let (status, changed, uncertain, is_error, _evidence, reason) = match self {
            Self::Refused { reason } => (
                "precondition_failed",
                false,
                false,
                true,
                None,
                redacted_refusal_reason(&reason),
            ),
            Self::Success { evidence } => ("ok", true, false, false, Some(evidence), "none"),
            Self::KnownReplacementFailure { evidence } => (
                "replacement_failed_known",
                false,
                false,
                true,
                Some(evidence),
                "replacement",
            ),
            Self::Uncertain { evidence, reason } => (
                "uncertain",
                false,
                true,
                true,
                Some(evidence),
                redacted_refusal_reason(&reason),
            ),
        };
        ToolOutput {
            content: vec![ToolContent::Json(json!({
                "status": status,
                "changed": changed,
                "uncertain": uncertain,
                "reason": reason,
            }))],
            is_error,
        }
    }
}

/// Private audit evidence: only identities, hashes, sizes, and outcome class.
#[allow(dead_code)]
struct MutationEvidence {
    target_identity: FileIdentity,
    preimage_sha256: String,
    preimage_length: usize,
    postimage_sha256: String,
    postimage_length: usize,
    result: MutationResultClass,
}

#[allow(dead_code)]
#[derive(Clone, Copy)]
enum MutationResultClass {
    Success,
    KnownReplacementFailure,
    Uncertain,
}

impl MutationEvidence {
    fn from_images(pre: &Preimage, post: &Postimage, result: MutationResultClass) -> Self {
        Self {
            target_identity: pre.target.identity.clone(),
            preimage_sha256: sha256_hex(&pre.bytes),
            preimage_length: pre.bytes.len(),
            postimage_sha256: sha256_hex(&post.bytes),
            postimage_length: post.bytes.len(),
            result,
        }
    }

    fn empty(result: MutationResultClass) -> Self {
        Self {
            target_identity: FileIdentity::empty(),
            preimage_sha256: String::new(),
            preimage_length: 0,
            postimage_sha256: String::new(),
            postimage_length: 0,
            result,
        }
    }
}

fn build_postimage(
    pre: &Preimage,
    request: &PatchRequest,
    limits: PatchLimits,
) -> Result<Postimage, RefusalReason> {
    let text = std::str::from_utf8(&pre.bytes)
        .map_err(|_| RefusalReason::Precondition("target is not strict UTF-8"))?;
    let (bom, body) = if let Some(body) = text.strip_prefix('\u{feff}') {
        (BOM, body)
    } else {
        (&[][..], text)
    };
    let mut ranges = Vec::with_capacity(request.replacements.len());
    for replacement in &request.replacements {
        let matches = body.match_indices(&replacement.old).collect::<Vec<_>>();
        if matches.is_empty() {
            return Err(RefusalReason::Precondition("expected old text is missing"));
        }
        if matches.len() != 1 {
            return Err(RefusalReason::Precondition(
                "expected old text occurs more than once",
            ));
        }
        if replacement.old == replacement.new {
            return Err(RefusalReason::Precondition(
                "replacement is a verified no-op",
            ));
        }
        let (start, matched) = matches[0];
        ranges.push(ResolvedReplacement {
            start,
            end: start + matched.len(),
            new: replacement.new.as_str(),
        });
    }
    ranges.sort_by_key(|range| range.start);
    for pair in ranges.windows(2) {
        if pair[1].start < pair[0].end {
            return Err(RefusalReason::Precondition(
                "replacement ranges overlap or duplicate",
            ));
        }
    }
    let estimated_body_length = ranges.iter().fold(body.len(), |length, range| {
        length
            .saturating_sub(range.end - range.start)
            .saturating_add(range.new.len())
    });
    let mut body_postimage = Vec::with_capacity(estimated_body_length);
    let mut cursor = 0;
    for range in ranges {
        body_postimage.extend_from_slice(&body.as_bytes()[cursor..range.start]);
        body_postimage.extend_from_slice(range.new.as_bytes());
        cursor = range.end;
    }
    body_postimage.extend_from_slice(&body.as_bytes()[cursor..]);
    let length = bom.len().saturating_add(body_postimage.len());
    if length > limits.max_file_bytes {
        return Err(RefusalReason::Precondition(
            "resulting postimage exceeds the file size limit",
        ));
    }
    let mut bytes = Vec::with_capacity(length);
    bytes.extend_from_slice(bom);
    bytes.extend_from_slice(&body_postimage);
    Ok(Postimage { bytes })
}

struct ResolvedReplacement<'a> {
    start: usize,
    end: usize,
    new: &'a str,
}

fn validate_preconditions(
    bytes: &[u8],
    request: &PatchRequest,
    limits: PatchLimits,
) -> Result<(), &'static str> {
    if bytes.len() > limits.max_file_bytes {
        return Err("target exceeds the file size limit");
    }
    if bytes.contains(&0) {
        return Err("target contains NUL and is unsupported");
    }
    if bytes.len() != request.expected_length {
        return Err("target byte length does not match the request precondition");
    }
    if sha256_hex(bytes) != request.expected_sha256 {
        return Err("target SHA-256 does not match the request precondition");
    }
    if std::str::from_utf8(bytes).is_err() {
        return Err("target is not strict UTF-8");
    }
    Ok(())
}

fn parse_logical_path(value: &str, limit: usize) -> Result<PathBuf, ToolError> {
    if value.is_empty()
        || value.len() > limit
        || value.contains('\0')
        || value.contains('\\')
        || value.contains(':')
        || value.starts_with('/')
        || value.starts_with("//")
        || Path::new(value).is_absolute()
    {
        return Err(ToolError::InvalidInput {
            message: "`path` must be a bounded logical relative path with slash separators"
                .to_owned(),
        });
    }
    for component in value.split('/') {
        if component.is_empty()
            || component == "."
            || component == ".."
            || component.eq_ignore_ascii_case(".git")
        {
            return Err(ToolError::InvalidInput {
                message: "`path` contains an unsupported component".to_owned(),
            });
        }
    }
    let path = PathBuf::from(value);
    if path
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(ToolError::InvalidInput {
            message: "`path` must contain only normal components".to_owned(),
        });
    }
    Ok(path)
}

fn validate_existing_target(root: &Path, relative: &Path) -> Result<PathBuf, ToolError> {
    validate_directory_path(root, root, "repository root")?;
    let mut current = root.to_path_buf();
    for component in relative.components() {
        let Component::Normal(component) = component else {
            return Err(policy_error("target contains a non-normal path component"));
        };
        current.push(component);
        reject_link_or_reparse(&current, "target path component")?;
    }
    let canonical = fs::canonicalize(&current).map_err(fs_error)?;
    if !is_beneath(&canonical, root) || !paths_equivalent(&canonical, &current) {
        return Err(policy_error(
            "target path aliases or escapes the repository root",
        ));
    }
    let metadata = fs::metadata(&canonical).map_err(fs_error)?;
    if !metadata.is_file() {
        return Err(policy_error("target must be an existing regular file"));
    }
    reject_unsupported_file_attributes(&metadata)?;
    Ok(canonical)
}

fn validate_directory_path(root: &Path, directory: &Path, label: &str) -> Result<(), ToolError> {
    if !is_beneath(directory, root) {
        return Err(policy_error(format!("{label} escapes the repository root")));
    }
    reject_link_or_reparse(directory, label)?;
    if !fs::metadata(directory).map_err(fs_error)?.is_dir() {
        return Err(policy_error(format!("{label} must be a directory")));
    }
    Ok(())
}

fn temporary_path(parent: &Path) -> PathBuf {
    parent.join(format!(".rah-repo-patch-{}.tmp", Uuid::new_v4()))
}

fn validate_temporary(
    path: &Path,
    expected: &[u8],
    limits: PatchLimits,
) -> Result<(), RefusalReason> {
    reject_link_or_reparse(path, "temporary replacement file")
        .map_err(|error| RefusalReason::Temporary(error.to_string()))?;
    let metadata =
        fs::metadata(path).map_err(|error| RefusalReason::Temporary(error.to_string()))?;
    if !metadata.is_file() {
        return Err(RefusalReason::Temporary(
            "temporary replacement file must be a regular file".to_owned(),
        ));
    }
    reject_unsupported_file_attributes(&metadata)
        .map_err(|error| RefusalReason::Temporary(error.to_string()))?;
    let bytes = read_bounded(path, limits.max_file_bytes)
        .map_err(|error| RefusalReason::Temporary(error.to_string()))?;
    if bytes != expected {
        return Err(RefusalReason::Temporary(
            "temporary postimage verification failed".to_owned(),
        ));
    }
    Ok(())
}

fn remove_temporary(
    temporary: &Temporary,
    expected: &[u8],
    limits: PatchLimits,
) -> Result<(), String> {
    temporary.revalidate(expected, limits)?;
    remove_temporary_identity(temporary)
}

fn remove_temporary_identity(temporary: &Temporary) -> Result<(), String> {
    temporary.revalidate_identity()?;
    match fs::remove_file(&temporary.path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Err("temporary replacement file disappeared during cleanup".to_owned())
        }
        Err(error) => Err(error.to_string()),
    }
}

fn read_bounded(path: &Path, max: usize) -> Result<Vec<u8>, std::io::Error> {
    let metadata = fs::metadata(path)?;
    if metadata.len() > u64::try_from(max).unwrap_or(u64::MAX) {
        return Err(std::io::Error::other("target exceeds the file size limit"));
    }
    let bytes = fs::read(path)?;
    if bytes.len() > max {
        return Err(std::io::Error::other("target exceeds the file size limit"));
    }
    Ok(bytes)
}

fn canonical_git_executable(path: &Path) -> Result<PathBuf, ToolError> {
    if !path.is_absolute() {
        return Err(policy_error(
            "Git executable must be an absolute host-selected path",
        ));
    }
    reject_link_or_reparse(path, "Git executable")?;
    let canonical = fs::canonicalize(path).map_err(fs_error)?;
    if !fs::metadata(&canonical).map_err(fs_error)?.is_file() {
        return Err(policy_error("Git executable must be a regular file"));
    }
    Ok(canonical)
}

fn canonical_directory(path: &Path, label: &str) -> Result<PathBuf, ToolError> {
    let canonical = fs::canonicalize(path).map_err(fs_error)?;
    if !canonical.is_dir() {
        return Err(policy_error(format!(
            "{label} must be an existing directory"
        )));
    }
    Ok(canonical)
}

fn reject_reparse_ancestry(path: &Path, label: &str) -> Result<(), ToolError> {
    for ancestor in path.ancestors() {
        if ancestor.exists() {
            reject_link_or_reparse(ancestor, label)?;
        }
    }
    Ok(())
}

fn reject_link_or_reparse(path: &Path, label: &str) -> Result<(), ToolError> {
    let metadata = fs::symlink_metadata(path).map_err(fs_error)?;
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

fn reject_unsupported_file_attributes(metadata: &fs::Metadata) -> Result<(), ToolError> {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_READONLY: u32 = 0x1;
        const FILE_ATTRIBUTE_COMPRESSED: u32 = 0x800;
        const FILE_ATTRIBUTE_ENCRYPTED: u32 = 0x4000;
        if metadata.file_attributes()
            & (FILE_ATTRIBUTE_READONLY | FILE_ATTRIBUTE_COMPRESSED | FILE_ATTRIBUTE_ENCRYPTED)
            != 0
        {
            return Err(policy_error(
                "target has unsupported Windows file attributes",
            ));
        }
    }
    #[cfg(not(windows))]
    let _ = metadata;
    Ok(())
}

fn replace_once(temporary: &Path, target: &Path) -> Result<(), std::io::Error> {
    #[cfg(windows)]
    {
        windows_replace_once(temporary, target)
    }
    #[cfg(not(windows))]
    {
        fs::rename(temporary, target)
    }
}

#[cfg(unix)]
fn unix_permission_mode(metadata: &fs::Metadata) -> u32 {
    use std::os::unix::fs::MetadataExt;
    metadata.mode() & 0o7777
}

#[cfg(unix)]
fn set_temporary_permissions(file: &fs::File, mode: u32) -> Result<(), std::io::Error> {
    use std::os::unix::fs::PermissionsExt;
    file.set_permissions(fs::Permissions::from_mode(mode))
}

#[cfg(windows)]
fn windows_replace_once(temporary: &Path, target: &Path) -> Result<(), std::io::Error> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
    };

    let temporary = temporary
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let target = target
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    // MoveFileExW is the one native commit attempt. It is deliberately never retried.
    let result = unsafe {
        MoveFileExW(
            temporary.as_ptr(),
            target.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if result == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

fn parse_head_entry(bytes: &[u8], expected_path: &[u8]) -> Result<GitEntry, ToolError> {
    let records = records(bytes)?;
    let [record] = records.as_slice() else {
        return Err(git_error("target must have exactly one regular HEAD entry"));
    };
    let Some(tab) = record.iter().position(|byte| *byte == b'\t') else {
        return Err(git_error("Git HEAD entry was malformed"));
    };
    if &record[tab + 1..] != expected_path {
        return Err(git_error("Git HEAD entry selected an unexpected target"));
    }
    let fields = record[..tab]
        .split(|byte| *byte == b' ')
        .collect::<Vec<_>>();
    let [mode, kind, object] = fields.as_slice() else {
        return Err(git_error("Git HEAD entry was malformed"));
    };
    if *kind != b"blob" || !matches!(*mode, b"100644" | b"100755") {
        return Err(git_error("target must be a regular HEAD tree entry"));
    }
    Ok(GitEntry {
        mode: mode.to_vec(),
        object: object.to_vec(),
    })
}

fn parse_index_entry(bytes: &[u8], expected_path: &[u8]) -> Result<GitEntry, ToolError> {
    let records = records(bytes)?;
    let [record] = records.as_slice() else {
        return Err(git_error(
            "target must have exactly one stage-0 index entry",
        ));
    };
    let Some(tab) = record.iter().position(|byte| *byte == b'\t') else {
        return Err(git_error("Git index entry was malformed"));
    };
    if &record[tab + 1..] != expected_path {
        return Err(git_error("Git index entry selected an unexpected target"));
    }
    let fields = record[..tab]
        .split(|byte| *byte == b' ')
        .collect::<Vec<_>>();
    let [mode, object, stage] = fields.as_slice() else {
        return Err(git_error("Git index entry was malformed"));
    };
    if *stage != b"0" || !matches!(*mode, b"100644" | b"100755") {
        return Err(git_error("target must be one regular stage-0 index entry"));
    }
    Ok(GitEntry {
        mode: mode.to_vec(),
        object: object.to_vec(),
    })
}

fn require_normal_index_tag(bytes: &[u8], expected_path: &[u8]) -> Result<(), ToolError> {
    let records = records(bytes)?;
    let [record] = records.as_slice() else {
        return Err(git_error("Git index tag observation was malformed"));
    };
    if record.first() != Some(&b'H')
        || record.get(1) != Some(&b' ')
        || &record[2..] != expected_path
    {
        return Err(git_error("target has unsupported sparse or index flags"));
    }
    Ok(())
}

fn records(bytes: &[u8]) -> Result<Vec<&[u8]>, ToolError> {
    if !bytes.ends_with(&[0]) {
        return Err(git_error("Git NUL-delimited observation was malformed"));
    }
    Ok(bytes[..bytes.len() - 1].split(|byte| *byte == 0).collect())
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut text = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(text, "{byte:02x}");
    }
    text
}

fn is_lower_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn required_string<'a>(object: &'a Map<String, Value>, name: &str) -> Result<&'a str, ToolError> {
    object
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| ToolError::InvalidInput {
            message: format!("`{name}` must be a string"),
        })
}

fn reject_unknown_fields<const N: usize>(
    object: &Map<String, Value>,
    allowed: [&str; N],
) -> Result<(), ToolError> {
    if let Some(name) = object.keys().find(|name| !allowed.contains(&name.as_str())) {
        return Err(ToolError::InvalidInput {
            message: format!("unknown field `{name}`"),
        });
    }
    Ok(())
}

fn redacted_refusal_reason(reason: &RefusalReason) -> &'static str {
    match reason {
        RefusalReason::Path(_) => "path_or_filesystem",
        RefusalReason::Repository(_) => "repository_state",
        RefusalReason::Temporary(_) | RefusalReason::TemporaryCleanup(_) => "temporary",
        RefusalReason::PostObservation(_) => "replacement",
        RefusalReason::Precondition(message) => redacted_precondition_reason(message),
    }
}

fn redacted_precondition_reason(reason: &str) -> &'static str {
    if reason.contains("SHA-256")
        || reason.contains("byte length")
        || reason.contains("expected old text")
        || reason.contains("UTF-8")
    {
        "precondition"
    } else if reason.contains("replacement") {
        "replacement"
    } else if reason.contains("temporary") {
        "temporary"
    } else if reason.contains("Git") || reason.contains("repository") || reason.contains("index") {
        "repository_state"
    } else {
        "path_or_filesystem"
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FileIdentity {
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
    #[cfg(windows)]
    volume_serial: u32,
    #[cfg(windows)]
    file_index: u64,
    link_count: u32,
}

impl FileIdentity {
    fn capture(path: &Path) -> Result<Self, ToolError> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let metadata = fs::metadata(path).map_err(fs_error)?;
            Ok(Self {
                device: metadata.dev(),
                inode: metadata.ino(),
                link_count: u32::try_from(metadata.nlink()).unwrap_or(u32::MAX),
            })
        }
        #[cfg(windows)]
        {
            capture_windows_file_identity(path)
        }
        #[cfg(not(any(unix, windows)))]
        {
            let _ = path;
            Ok(Self { link_count: 1 })
        }
    }

    fn empty() -> Self {
        Self {
            #[cfg(unix)]
            device: 0,
            #[cfg(unix)]
            inode: 0,
            #[cfg(windows)]
            volume_serial: 0,
            #[cfg(windows)]
            file_index: 0,
            link_count: 0,
        }
    }
}

#[cfg(windows)]
fn capture_windows_file_identity(path: &Path) -> Result<FileIdentity, ToolError> {
    use std::os::windows::{ffi::OsStrExt, io::FromRawHandle};
    use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
    use windows_sys::Win32::Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, CreateFileW, FILE_FLAG_BACKUP_SEMANTICS, FILE_READ_ATTRIBUTES,
        FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, GetFileInformationByHandle,
        OPEN_EXISTING,
    };

    let path = path
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let handle = unsafe {
        CreateFileW(
            path.as_ptr(),
            FILE_READ_ATTRIBUTES,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            std::ptr::null(),
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS,
            std::ptr::null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(fs_error(std::io::Error::last_os_error()));
    }
    let _file = unsafe { File::from_raw_handle(handle) };
    let mut information = std::mem::MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::zeroed();
    let result = unsafe { GetFileInformationByHandle(handle, information.as_mut_ptr()) };
    if result == 0 {
        return Err(fs_error(std::io::Error::last_os_error()));
    }
    let information = unsafe { information.assume_init() };
    Ok(FileIdentity {
        volume_serial: information.dwVolumeSerialNumber,
        file_index: (u64::from(information.nFileIndexHigh) << 32)
            | u64::from(information.nFileIndexLow),
        link_count: information.nNumberOfLinks,
    })
}

fn fs_error(error: impl std::fmt::Display) -> ToolError {
    policy_error(error.to_string())
}

fn policy_error(message: impl Into<String>) -> ToolError {
    ToolError::Execution {
        message: format!(
            "repository worktree mutation policy rejected capability: {}",
            message.into()
        ),
    }
}

#[cfg(test)]
#[derive(Default)]
struct TestHook {
    actions: std::sync::Mutex<Vec<TestAction>>,
    failures: std::sync::Mutex<Vec<TestPhase>>,
    force_replacement_failure: std::sync::atomic::AtomicBool,
    replacement_attempts: std::sync::atomic::AtomicUsize,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TestPhase {
    AfterInitialPathValidation,
    AfterGitValidation,
    AfterPreimageValidation,
    AfterExpectedTextValidation,
    BeforeTemporaryWrite,
    AfterTemporaryWrite,
    AfterFinalTargetIdentityRevalidation,
    BeforeReplacement,
    AfterReplacement,
    BeforePostVerification,
}

#[cfg(test)]
type TestActionFunction = dyn Fn(&Path, Option<&Path>) + Send;

#[cfg(test)]
struct TestAction {
    phase: TestPhase,
    action: Box<TestActionFunction>,
}

#[cfg(test)]
impl TestHook {
    fn fail_at(&self, phase: TestPhase) {
        self.failures
            .lock()
            .expect("test hook mutex poisoned")
            .push(phase);
    }

    fn should_fail(&self, phase: TestPhase) -> bool {
        let mut failures = self.failures.lock().expect("test hook mutex poisoned");
        failures
            .iter()
            .position(|configured| *configured == phase)
            .map(|index| {
                failures.remove(index);
                true
            })
            .unwrap_or(false)
    }
    fn install<F>(&self, phase: TestPhase, action: F)
    where
        F: Fn(&Path, Option<&Path>) + Send + 'static,
    {
        self.actions
            .lock()
            .expect("test hook mutex poisoned")
            .push(TestAction {
                phase,
                action: Box::new(action),
            });
    }

    fn run(&self, phase: TestPhase, target: &Path, temporary: Option<&Path>) {
        let mut actions = self.actions.lock().expect("test hook mutex poisoned");
        let action = actions
            .iter()
            .position(|action| action.phase == phase)
            .map(|index| actions.remove(index));
        drop(actions);
        if let Some(action) = action {
            (action.action)(target, temporary);
        }
    }

    fn replace_once(&self, temporary: &Path, target: &Path) -> Result<(), std::io::Error> {
        if self
            .force_replacement_failure
            .swap(false, Ordering::Relaxed)
        {
            return Err(std::io::Error::other("injected replacement failure"));
        }
        replace_once(temporary, target)
    }
}

#[cfg(test)]
mod tests {
    #[cfg(windows)]
    use std::sync::{Arc, Mutex};
    use std::{
        fs,
        path::{Path, PathBuf},
        process::{Command, Stdio},
        sync::atomic::{AtomicU64, Ordering as AtomicOrdering},
    };

    use rah_protocol::ToolInput;
    use serde_json::{Value, json};

    use super::{
        MAX_FILE_BYTES, PatchLimits, PatchRequest, REPOSITORY_WORKTREE_PATCH_TOOL_NAME,
        RepositoryWorktreeMutationPolicy, RepositoryWorktreePatchTool, TestPhase, sha256_hex,
    };
    use crate::{Tool, ToolContext};

    static NEXT_TEST_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new(label: &str) -> Self {
            let id = NEXT_TEST_DIRECTORY.fetch_add(1, AtomicOrdering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "rah-repo-patch-{label}-{}-{id}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir(&path).expect("test directory should be created");
            Self(path)
        }

        fn repository(&self) -> PathBuf {
            let root = self.0.join("repository");
            fs::create_dir(&root).expect("repository directory should be created");
            git(&root, &["init", "--quiet"]);
            git(&root, &["config", "user.name", "RAH Test"]);
            git(&root, &["config", "user.email", "rah@example.invalid"]);
            git(&root, &["config", "core.autocrlf", "false"]);
            git(&root, &["config", "core.filemode", "true"]);
            fs::write(root.join("target.txt"), b"alpha\nold\nomega\n")
                .expect("target should be written");
            fs::write(root.join("other.txt"), b"other\n").expect("other should be written");
            git(&root, &["add", "--", "target.txt", "other.txt"]);
            git(&root, &["commit", "--quiet", "-m", "initial"]);
            root
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn replaces_one_clean_tracked_utf8_file_and_preserves_the_index() {
        let base = TestDirectory::new("success");
        let root = base.repository();
        let before_index = git_output(&root, &["ls-files", "-s", "-z"]);
        let other_before = fs::read(root.join("other.txt")).unwrap();
        let input = request("target.txt", b"alpha\nold\nomega\n", "old", "new");
        let tool = RepositoryWorktreePatchTool::new(git_executable(), &root).unwrap();

        let output = run(&tool, input).await;

        assert_eq!(content(&output)["status"], "ok");
        assert_eq!(content(&output)["changed"], true);
        assert_eq!(
            fs::read(root.join("target.txt")).unwrap(),
            b"alpha\nnew\nomega\n"
        );
        assert_eq!(fs::read(root.join("other.txt")).unwrap(), other_before);
        assert_eq!(git_output(&root, &["ls-files", "-s", "-z"]), before_index);
        assert_eq!(
            tool.definition().name.as_str(),
            REPOSITORY_WORKTREE_PATCH_TOOL_NAME
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn rejects_a_fresh_snapshot_of_an_externally_dirty_target_without_attempting_replacement()
    {
        let base = TestDirectory::new("dirty-fresh-snapshot");
        let root = base.repository();
        let dirty = b"alpha\nold\nexternal dirty\n";
        fs::write(root.join("target.txt"), dirty).unwrap();
        let before_index = git_output(&root, &["ls-files", "-s", "-z"]);
        let before_head = git_output(&root, &["rev-parse", "--verify", "HEAD"]);
        let before_refs = git_output(
            &root,
            &["for-each-ref", "--format=%(refname)%00%(objectname)%00"],
        );
        let tool = RepositoryWorktreePatchTool::new(git_executable(), &root).unwrap();

        let output = run(&tool, request("target.txt", dirty, "old", "new")).await;

        assert_eq!(content(&output)["status"], "precondition_failed");
        assert_eq!(fs::read(root.join("target.txt")).unwrap(), dirty);
        assert_eq!(git_output(&root, &["ls-files", "-s", "-z"]), before_index);
        assert_eq!(
            git_output(&root, &["rev-parse", "--verify", "HEAD"]),
            before_head
        );
        assert_eq!(
            git_output(
                &root,
                &["for-each-ref", "--format=%(refname)%00%(objectname)%00"]
            ),
            before_refs
        );
        assert_eq!(
            tool.policy
                .test_hook
                .replacement_attempts
                .load(AtomicOrdering::Relaxed),
            0
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn preserves_one_leading_bom_and_crlf_without_normalization() {
        let base = TestDirectory::new("bom-crlf");
        let root = base.repository();
        let bytes = b"\xef\xbb\xbfalpha\r\nold\r\nomega\r\n";
        replace_and_commit(&root, "target.txt", bytes);
        let tool = RepositoryWorktreePatchTool::new(git_executable(), &root).unwrap();

        let output = run(&tool, request("target.txt", bytes, "old", "new")).await;

        assert_eq!(content(&output)["status"], "ok");
        assert_eq!(
            fs::read(root.join("target.txt")).unwrap(),
            b"\xef\xbb\xbfalpha\r\nnew\r\nomega\r\n"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn rejects_wrong_digest_length_missing_or_duplicate_text_without_writing() {
        let base = TestDirectory::new("preconditions");
        let root = base.repository();
        let original = fs::read(root.join("target.txt")).unwrap();
        let tool = RepositoryWorktreePatchTool::new(git_executable(), &root).unwrap();
        let mut wrong_hash = request("target.txt", &original, "old", "new");
        wrong_hash["expected_file_sha256"] = json!("0".repeat(64));
        let mut wrong_length = request("target.txt", &original, "old", "new");
        wrong_length["expected_file_byte_length"] = json!(original.len() + 1);
        let missing = request("target.txt", &original, "absent", "new");
        for input in [wrong_hash, wrong_length, missing] {
            let output = run(&tool, input).await;
            assert_eq!(content(&output)["status"], "precondition_failed");
        }
        assert_eq!(fs::read(root.join("target.txt")).unwrap(), original);

        let duplicate_bytes = b"old\nold\n";
        replace_and_commit(&root, "target.txt", duplicate_bytes);
        let duplicate = request("target.txt", duplicate_bytes, "old", "new");
        let output = run(&tool, duplicate).await;
        assert_eq!(content(&output)["status"], "precondition_failed");
        assert_eq!(fs::read(root.join("target.txt")).unwrap(), duplicate_bytes);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn rejects_malformed_utf8_and_oversized_postimage() {
        let base = TestDirectory::new("encoding-size");
        let root = base.repository();
        let invalid = b"old\xff";
        replace_and_commit(&root, "target.txt", invalid);
        let tool = RepositoryWorktreePatchTool::new(git_executable(), &root).unwrap();
        let malformed = run(&tool, request("target.txt", invalid, "old", "new")).await;
        assert_eq!(content(&malformed)["status"], "precondition_failed");
        assert_eq!(fs::read(root.join("target.txt")).unwrap(), invalid);

        let large = vec![b'a'; MAX_FILE_BYTES - 1];
        replace_and_commit(&root, "target.txt", &large);
        let oversized = run(&tool, request("target.txt", &large, "a", "bbbb")).await;
        assert_eq!(content(&oversized)["status"], "precondition_failed");
        assert_eq!(fs::read(root.join("target.txt")).unwrap(), large);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn rejects_untracked_staged_and_non_stage_zero_targets() {
        let base = TestDirectory::new("git-state");
        let root = base.repository();
        let tool = RepositoryWorktreePatchTool::new(git_executable(), &root).unwrap();

        let untracked = b"old\n";
        fs::write(root.join("untracked.txt"), untracked).unwrap();
        let output = run(&tool, request("untracked.txt", untracked, "old", "new")).await;
        assert_eq!(content(&output)["status"], "precondition_failed");

        let staged = b"alpha\nstaged\nomega\n";
        fs::write(root.join("target.txt"), staged).unwrap();
        git(&root, &["add", "--", "target.txt"]);
        let output = run(&tool, request("target.txt", staged, "staged", "new")).await;
        assert_eq!(content(&output)["reason"], "repository_state");

        git(&root, &["reset", "--quiet", "HEAD", "--", "target.txt"]);
        fs::write(root.join("target.txt"), b"alpha\nold\nomega\n").unwrap();
        let object = git_output(&root, &["rev-parse", "HEAD:target.txt"]);
        let object = String::from_utf8(object).unwrap().trim().to_owned();
        git_with_input(
            &root,
            &["update-index", "--index-info"],
            format!("100644 {object} 1\ttarget.txt\n").as_bytes(),
        );
        let current = fs::read(root.join("target.txt")).unwrap();
        let output = run(&tool, request("target.txt", &current, "old", "new")).await;
        assert_eq!(content(&output)["reason"], "repository_state");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn rejects_directory_outside_and_alias_paths() {
        let base = TestDirectory::new("paths");
        let root = base.repository();
        fs::create_dir(root.join("directory")).unwrap();
        let tool = RepositoryWorktreePatchTool::new(git_executable(), &root).unwrap();
        let directory = request("directory", b"", "old", "new");
        let output = run(&tool, directory).await;
        assert_eq!(content(&output)["reason"], "path_or_filesystem");

        for path in [
            root.join("target.txt").to_string_lossy().into_owned(),
            "../target.txt".to_owned(),
            ".git/config".to_owned(),
            "target.txt:stream".to_owned(),
            "dir//target.txt".to_owned(),
            "./target.txt".to_owned(),
            r"\\?\C:\target.txt".to_owned(),
            r"\\server\share\target.txt".to_owned(),
        ] {
            let error = tool
                .execute(
                    ToolInput(request_value(&path, b"old", "old", "new")),
                    ToolContext::default(),
                )
                .await
                .expect_err("unsafe logical paths must be rejected before mutation");
            assert!(error.to_string().contains("path"));
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn stale_target_between_validation_phases_fails_closed_without_attempting_replacement() {
        let base = TestDirectory::new("stale");
        let root = base.repository();
        let policy =
            RepositoryWorktreeMutationPolicy::new(&git_executable(), &root, PatchLimits::default())
                .unwrap();
        policy
            .test_hook
            .install(TestPhase::AfterPreimageValidation, |target, _| {
                fs::write(target, b"external\n").unwrap();
            });
        let request = PatchRequest::parse(
            &ToolInput(request("target.txt", b"alpha\nold\nomega\n", "old", "new")),
            PatchLimits::default(),
        )
        .unwrap();

        let output = policy.execute_once(request).await.into_tool_output();

        assert_eq!(content(&output)["status"], "precondition_failed");
        assert_eq!(fs::read(root.join("target.txt")).unwrap(), b"external\n");
        assert_eq!(
            policy
                .test_hook
                .replacement_attempts
                .load(AtomicOrdering::Relaxed),
            0
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn uses_one_replacement_attempt_and_cleans_temporary_after_known_failure() {
        let base = TestDirectory::new("one-attempt");
        let root = base.repository();
        let policy =
            RepositoryWorktreeMutationPolicy::new(&git_executable(), &root, PatchLimits::default())
                .unwrap();
        policy
            .test_hook
            .force_replacement_failure
            .store(true, AtomicOrdering::Relaxed);
        let request = PatchRequest::parse(
            &ToolInput(request("target.txt", b"alpha\nold\nomega\n", "old", "new")),
            PatchLimits::default(),
        )
        .unwrap();

        let output = policy.execute_once(request).await.into_tool_output();

        assert_eq!(content(&output)["status"], "replacement_failed_known");
        assert_eq!(
            policy
                .test_hook
                .replacement_attempts
                .load(AtomicOrdering::Relaxed),
            1
        );
        assert_eq!(
            fs::read(root.join("target.txt")).unwrap(),
            b"alpha\nold\nomega\n"
        );
        assert!(fs::read_dir(&root).unwrap().all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".rah-repo-patch-")
        }));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn races_after_each_precommit_validation_phase_refuse_without_replacement() {
        for phase in [
            TestPhase::AfterInitialPathValidation,
            TestPhase::AfterGitValidation,
            TestPhase::AfterPreimageValidation,
            TestPhase::AfterExpectedTextValidation,
            TestPhase::AfterTemporaryWrite,
            TestPhase::AfterFinalTargetIdentityRevalidation,
        ] {
            let base = TestDirectory::new("phase-race");
            let root = base.repository();
            let policy = RepositoryWorktreeMutationPolicy::new(
                &git_executable(),
                &root,
                PatchLimits::default(),
            )
            .unwrap();
            policy.test_hook.install(phase, |target, _| {
                let displaced = target.with_extension("external");
                fs::rename(target, &displaced).unwrap();
                fs::write(target, b"alpha\nold\nomega\n").unwrap();
            });
            let request = parse_request();

            let output = policy.execute_once(request).await.into_tool_output();

            assert_eq!(
                content(&output)["status"],
                "precondition_failed",
                "{phase:?}"
            );
            assert_eq!(
                policy
                    .test_hook
                    .replacement_attempts
                    .load(AtomicOrdering::Relaxed),
                0,
                "{phase:?} must not attempt replacement"
            );
            assert_eq!(
                fs::read(root.join("target.txt")).unwrap(),
                b"alpha\nold\nomega\n"
            );
            assert_no_patch_temporary(&root);
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn index_and_head_races_after_git_observation_refuse_without_replacement() {
        let base = TestDirectory::new("index-race");
        let root = base.repository();
        let policy =
            RepositoryWorktreeMutationPolicy::new(&git_executable(), &root, PatchLimits::default())
                .unwrap();
        let git_root = root.clone();
        policy
            .test_hook
            .install(TestPhase::AfterGitValidation, move |target, _| {
                fs::write(target, b"alpha\nstaged\nomega\n").unwrap();
                git(&git_root, &["add", "--", "target.txt"]);
                fs::write(target, b"alpha\nold\nomega\n").unwrap();
            });
        let output = policy
            .execute_once(parse_request())
            .await
            .into_tool_output();
        assert_eq!(content(&output)["status"], "precondition_failed");
        assert_eq!(content(&output)["reason"], "repository_state");
        assert_eq!(
            policy
                .test_hook
                .replacement_attempts
                .load(AtomicOrdering::Relaxed),
            0
        );

        let base = TestDirectory::new("conflict-race");
        let root = base.repository();
        let policy =
            RepositoryWorktreeMutationPolicy::new(&git_executable(), &root, PatchLimits::default())
                .unwrap();
        let git_root = root.clone();
        policy
            .test_hook
            .install(TestPhase::AfterGitValidation, move |_, _| {
                let object =
                    String::from_utf8(git_output(&git_root, &["rev-parse", "HEAD:target.txt"]))
                        .unwrap()
                        .trim()
                        .to_owned();
                git_with_input(
                    &git_root,
                    &["update-index", "--index-info"],
                    format!("100644 {object} 1\ttarget.txt\n").as_bytes(),
                );
            });
        let output = policy
            .execute_once(parse_request())
            .await
            .into_tool_output();
        assert_eq!(content(&output)["status"], "precondition_failed");
        assert_eq!(content(&output)["reason"], "repository_state");
        assert_eq!(
            policy
                .test_hook
                .replacement_attempts
                .load(AtomicOrdering::Relaxed),
            0
        );

        let base = TestDirectory::new("head-race");
        let root = base.repository();
        let policy =
            RepositoryWorktreeMutationPolicy::new(&git_executable(), &root, PatchLimits::default())
                .unwrap();
        let git_root = root.clone();
        policy
            .test_hook
            .install(TestPhase::AfterGitValidation, move |_, _| {
                fs::write(git_root.join("other.txt"), b"new head\n").unwrap();
                git(&git_root, &["add", "--", "other.txt"]);
                git(
                    &git_root,
                    &["commit", "--quiet", "-m", "external head change"],
                );
            });
        let output = policy
            .execute_once(parse_request())
            .await
            .into_tool_output();
        assert_eq!(content(&output)["status"], "precondition_failed");
        assert_eq!(content(&output)["reason"], "repository_state");
        assert_eq!(
            policy
                .test_hook
                .replacement_attempts
                .load(AtomicOrdering::Relaxed),
            0
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn deleted_target_after_text_validation_refuses_without_replacement() {
        let base = TestDirectory::new("deleted-target");
        let root = base.repository();
        let policy =
            RepositoryWorktreeMutationPolicy::new(&git_executable(), &root, PatchLimits::default())
                .unwrap();
        policy
            .test_hook
            .install(TestPhase::AfterExpectedTextValidation, |target, _| {
                fs::remove_file(target).unwrap();
            });

        let output = policy
            .execute_once(parse_request())
            .await
            .into_tool_output();

        assert_eq!(content(&output)["status"], "precondition_failed");
        assert_eq!(
            policy
                .test_hook
                .replacement_attempts
                .load(AtomicOrdering::Relaxed),
            0
        );
        assert!(!root.join("target.txt").exists());
        assert_no_patch_temporary(&root);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn temporary_tampering_before_commit_is_uncertain_and_preserves_evidence() {
        let base = TestDirectory::new("temporary-tamper");
        let root = base.repository();
        let policy =
            RepositoryWorktreeMutationPolicy::new(&git_executable(), &root, PatchLimits::default())
                .unwrap();
        policy
            .test_hook
            .install(TestPhase::AfterTemporaryWrite, |_, temporary| {
                fs::write(temporary.unwrap(), b"tampered\n").unwrap();
            });

        let output = policy
            .execute_once(parse_request())
            .await
            .into_tool_output();

        assert_eq!(content(&output)["status"], "uncertain");
        assert_eq!(
            policy
                .test_hook
                .replacement_attempts
                .load(AtomicOrdering::Relaxed),
            0
        );
        assert_eq!(
            fs::read(root.join("target.txt")).unwrap(),
            b"alpha\nold\nomega\n"
        );
        assert!(has_patch_temporary(&root));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn missing_temporary_after_a_replacement_error_is_uncertain_without_replay() {
        let base = TestDirectory::new("missing-temporary");
        let root = base.repository();
        let policy =
            RepositoryWorktreeMutationPolicy::new(&git_executable(), &root, PatchLimits::default())
                .unwrap();
        policy
            .test_hook
            .force_replacement_failure
            .store(true, AtomicOrdering::Relaxed);
        policy
            .test_hook
            .install(TestPhase::AfterReplacement, |_, temporary| {
                fs::remove_file(temporary.unwrap()).unwrap();
            });

        let output = policy
            .execute_once(parse_request())
            .await
            .into_tool_output();

        assert_eq!(content(&output)["status"], "uncertain");
        assert_eq!(
            policy
                .test_hook
                .replacement_attempts
                .load(AtomicOrdering::Relaxed),
            1
        );
        assert_eq!(
            fs::read(root.join("target.txt")).unwrap(),
            b"alpha\nold\nomega\n"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn post_replacement_identity_change_with_same_content_is_uncertain() {
        let base = TestDirectory::new("post-identity");
        let root = base.repository();
        let policy =
            RepositoryWorktreeMutationPolicy::new(&git_executable(), &root, PatchLimits::default())
                .unwrap();
        policy
            .test_hook
            .install(TestPhase::AfterReplacement, |target, _| {
                let displaced = target.with_extension("replaced");
                fs::rename(target, displaced).unwrap();
                fs::write(target, b"alpha\nnew\nomega\n").unwrap();
            });

        let output = policy
            .execute_once(parse_request())
            .await
            .into_tool_output();

        assert_eq!(content(&output)["status"], "uncertain");
        assert_eq!(
            policy
                .test_hook
                .replacement_attempts
                .load(AtomicOrdering::Relaxed),
            1
        );
        assert_eq!(
            fs::read(root.join("target.txt")).unwrap(),
            b"alpha\nnew\nomega\n"
        );
    }

    #[cfg(windows)]
    #[tokio::test(flavor = "current_thread")]
    async fn windows_delete_sharing_locks_have_conservative_replacement_outcomes() {
        let base = TestDirectory::new("windows-lock-release");
        let root = base.repository();
        let held = Arc::new(Mutex::new(Some(open_without_delete_share(
            &root.join("target.txt"),
        ))));
        let policy =
            RepositoryWorktreeMutationPolicy::new(&git_executable(), &root, PatchLimits::default())
                .unwrap();
        let release = held.clone();
        policy
            .test_hook
            .install(TestPhase::BeforeReplacement, move |_, _| {
                drop(release.lock().unwrap().take());
            });
        let output = policy
            .execute_once(parse_request())
            .await
            .into_tool_output();
        assert_eq!(content(&output)["status"], "ok");
        assert_eq!(
            policy
                .test_hook
                .replacement_attempts
                .load(AtomicOrdering::Relaxed),
            1
        );

        let base = TestDirectory::new("windows-lock-known");
        let root = base.repository();
        let _lock = open_without_delete_share(&root.join("target.txt"));
        let policy =
            RepositoryWorktreeMutationPolicy::new(&git_executable(), &root, PatchLimits::default())
                .unwrap();
        let output = policy
            .execute_once(parse_request())
            .await
            .into_tool_output();
        assert_eq!(content(&output)["status"], "replacement_failed_known");
        assert_eq!(
            fs::read(root.join("target.txt")).unwrap(),
            b"alpha\nold\nomega\n"
        );
        assert_eq!(
            policy
                .test_hook
                .replacement_attempts
                .load(AtomicOrdering::Relaxed),
            1
        );

        let base = TestDirectory::new("windows-lock-uncertain");
        let root = base.repository();
        let _lock = open_without_delete_share(&root.join("target.txt"));
        let policy =
            RepositoryWorktreeMutationPolicy::new(&git_executable(), &root, PatchLimits::default())
                .unwrap();
        policy
            .test_hook
            .install(TestPhase::AfterReplacement, |_, temporary| {
                fs::remove_file(temporary.unwrap()).unwrap();
            });
        let output = policy
            .execute_once(parse_request())
            .await
            .into_tool_output();
        assert_eq!(content(&output)["status"], "uncertain");
        assert_eq!(
            policy
                .test_hook
                .replacement_attempts
                .load(AtomicOrdering::Relaxed),
            1
        );
    }

    #[cfg(any(unix, windows))]
    #[tokio::test(flavor = "current_thread")]
    async fn rejects_symbolic_link_targets_when_supported() {
        let base = TestDirectory::new("symlink");
        let root = base.repository();
        fs::remove_file(root.join("target.txt")).unwrap();
        if create_symlink(Path::new("other.txt"), &root.join("target.txt")).is_err() {
            return;
        }
        let tool = RepositoryWorktreePatchTool::new(git_executable(), &root).unwrap();
        let output = run(&tool, request("target.txt", b"other\n", "other", "new")).await;
        assert_eq!(content(&output)["reason"], "path_or_filesystem");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn rejects_hard_link_targets_when_the_platform_reports_them() {
        let base = TestDirectory::new("hard-link");
        let root = base.repository();
        if fs::hard_link(root.join("target.txt"), root.join("target-alias.txt")).is_err() {
            return;
        }
        let tool = RepositoryWorktreePatchTool::new(git_executable(), &root).unwrap();
        let bytes = fs::read(root.join("target.txt")).unwrap();
        let output = run(&tool, request("target.txt", &bytes, "old", "new")).await;
        assert_eq!(content(&output)["reason"], "path_or_filesystem");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn applies_multiple_original_snapshot_replacements_in_offset_order() {
        let base = TestDirectory::new("multiple");
        let root = base.repository();
        let bytes = b"first A, second B, third C\r\n";
        fs::write(root.join("target.txt"), bytes).unwrap();
        git(&root, &["add", "--", "target.txt"]);
        git(&root, &["commit", "--quiet", "-m", "multiple"]);
        let tool = RepositoryWorktreePatchTool::new(git_executable(), &root).unwrap();
        let output = run(
            &tool,
            request_multiple("target.txt", bytes, &[("C", "3"), ("A", "1"), ("B", "2")]),
        )
        .await;
        assert_eq!(content(&output)["status"], "ok");
        assert_eq!(
            fs::read(root.join("target.txt")).unwrap(),
            b"first 1, second 2, third 3\r\n"
        );
        assert_eq!(fs::read(root.join("other.txt")).unwrap(), b"other\n");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn rejects_mixed_duplicate_overlap_and_generated_match_forms() {
        let base = TestDirectory::new("multiple-rejections");
        let root = base.repository();
        let tool = RepositoryWorktreePatchTool::new(git_executable(), &root).unwrap();
        let bytes = b"A B\n";
        fs::write(root.join("target.txt"), bytes).unwrap();
        git(&root, &["add", "--", "target.txt"]);
        git(&root, &["commit", "--quiet", "-m", "replace"]);
        let mixed = json!({
            "path": "target.txt", "expected_file_sha256": sha256_hex(bytes),
            "expected_file_byte_length": bytes.len(), "expected_old_text": "A",
            "replacement_text": "X", "replacements": []
        });
        assert!(
            tool.execute(ToolInput(mixed), ToolContext::default())
                .await
                .is_err()
        );
        for replacements in [
            vec![("A", "X"), ("A", "Y")],
            vec![("A", "X"), ("A B", "Y")],
            vec![("A", "X"), ("X", "Y")],
        ] {
            let output = run(&tool, request_multiple("target.txt", bytes, &replacements)).await;
            assert_eq!(content(&output)["status"], "precondition_failed");
            assert_eq!(fs::read(root.join("target.txt")).unwrap(), bytes);
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn private_faults_preserve_precommit_and_report_postcommit_uncertainty() {
        let base = TestDirectory::new("faults");
        let root = base.repository();
        for phase in [
            TestPhase::BeforeTemporaryWrite,
            TestPhase::AfterTemporaryWrite,
            TestPhase::BeforeReplacement,
        ] {
            let policy = RepositoryWorktreeMutationPolicy::new(
                &git_executable(),
                &root,
                PatchLimits::default(),
            )
            .unwrap();
            policy.test_hook.fail_at(phase);
            let output = policy
                .execute_once(parse_request())
                .await
                .into_tool_output();
            assert_eq!(content(&output)["status"], "precondition_failed");
            assert_eq!(
                fs::read(root.join("target.txt")).unwrap(),
                b"alpha\nold\nomega\n"
            );
            assert_no_patch_temporary(&root);
        }
        let policy =
            RepositoryWorktreeMutationPolicy::new(&git_executable(), &root, PatchLimits::default())
                .unwrap();
        policy.test_hook.fail_at(TestPhase::BeforePostVerification);
        let output = policy
            .execute_once(parse_request())
            .await
            .into_tool_output();
        assert_eq!(content(&output)["status"], "uncertain");
        assert_eq!(
            fs::read(root.join("target.txt")).unwrap(),
            b"alpha\nnew\nomega\n"
        );
    }

    #[cfg(unix)]
    #[tokio::test(flavor = "current_thread")]
    async fn preserves_unix_executable_and_nonexecutable_modes() {
        use std::os::unix::fs::PermissionsExt;

        for mode in [0o755, 0o644] {
            let base = TestDirectory::new("unix-mode");
            let root = base.repository();
            let target = root.join("target.txt");
            fs::set_permissions(&target, fs::Permissions::from_mode(mode)).unwrap();
            let chmod = if mode & 0o111 != 0 { "+x" } else { "-x" };
            git(
                &root,
                &["add", &format!("--chmod={chmod}"), "--", "target.txt"],
            );
            git(&root, &["commit", "--quiet", "-m", "normalize target mode"]);
            assert_unix_mode_baseline(&root, mode);

            let bytes = fs::read(root.join("target.txt")).unwrap();
            let head_before = git_output(&root, &["ls-tree", "-z", "HEAD", "--", "target.txt"]);
            let index_before =
                git_output(&root, &["ls-files", "--stage", "-z", "--", "target.txt"]);
            let tool = RepositoryWorktreePatchTool::new(git_executable(), &root).unwrap();
            let output = run(&tool, request("target.txt", &bytes, "old", "new")).await;
            assert_eq!(
                content(&output)["status"],
                "ok",
                "{}",
                unix_mode_diagnostics(&root, mode, Some(&output))
            );
            assert_eq!(fs::read(&target).unwrap(), b"alpha\nnew\nomega\n");
            assert_eq!(
                fs::metadata(&target).unwrap().permissions().mode() & 0o111,
                mode & 0o111
            );
            assert_eq!(
                git_output(&root, &["ls-tree", "-z", "HEAD", "--", "target.txt"]),
                head_before
            );
            assert_eq!(
                git_output(&root, &["ls-files", "--stage", "-z", "--", "target.txt"]),
                index_before
            );
            assert_no_patch_temporary(&root);
        }
    }

    #[cfg(unix)]
    fn assert_unix_mode_baseline(root: &Path, mode: u32) {
        use std::os::unix::fs::PermissionsExt;

        let expected_git_mode = if mode & 0o111 != 0 {
            b"100755"
        } else {
            b"100644"
        };
        let target = root.join("target.txt");
        let diagnostics = unix_mode_diagnostics(root, mode, None);
        assert_eq!(
            fs::metadata(&target).unwrap().permissions().mode() & 0o111,
            mode & 0o111,
            "{diagnostics}"
        );
        assert!(
            git_output(root, &["ls-tree", "HEAD", "--", "target.txt"])
                .starts_with(expected_git_mode),
            "{diagnostics}"
        );
        assert!(
            git_output(root, &["ls-files", "--stage", "--", "target.txt"])
                .starts_with(expected_git_mode),
            "{diagnostics}"
        );
        assert_eq!(
            git_exit_code(
                root,
                &[
                    "--literal-pathspecs",
                    "diff-files",
                    "--quiet",
                    "--",
                    "target.txt"
                ]
            ),
            Some(0),
            "{diagnostics}"
        );
        assert!(
            git_output(
                root,
                &["status", "--porcelain=v1", "--untracked-files=all", "-z"]
            )
            .is_empty(),
            "{diagnostics}"
        );
    }

    #[cfg(unix)]
    fn unix_mode_diagnostics(
        root: &Path,
        requested_mode: u32,
        output: Option<&crate::ToolOutput>,
    ) -> String {
        use std::os::unix::fs::PermissionsExt;

        let filesystem_executable = fs::metadata(root.join("target.txt"))
            .map(|metadata| metadata.permissions().mode() & 0o111 != 0)
            .unwrap_or(false);
        let (returned_status, returned_reason) = output
            .map(|output| {
                (
                    content(output)["status"].to_string(),
                    content(output)["reason"].to_string(),
                )
            })
            .unwrap_or_else(|| ("setup".to_owned(), "not_run".to_owned()));
        format!(
            "requested_mode={requested_mode:o}; filesystem_executable={filesystem_executable}; head={:?}; index={:?}; diff_files_exit={:?}; status={:?}; returned_status={}; returned_reason={}",
            String::from_utf8_lossy(&git_output(root, &["ls-tree", "HEAD", "--", "target.txt"])),
            String::from_utf8_lossy(&git_output(
                root,
                &["ls-files", "--stage", "--", "target.txt"]
            )),
            git_exit_code(
                root,
                &[
                    "--literal-pathspecs",
                    "diff-files",
                    "--quiet",
                    "--",
                    "target.txt"
                ]
            ),
            String::from_utf8_lossy(&git_output(
                root,
                &["status", "--porcelain=v1", "--untracked-files=all", "-z"]
            )),
            returned_status,
            returned_reason,
        )
    }

    fn request(path: &str, bytes: &[u8], old: &str, replacement: &str) -> Value {
        request_value(path, bytes, old, replacement)
    }

    fn request_multiple(path: &str, bytes: &[u8], replacements: &[(&str, &str)]) -> Value {
        json!({
            "path": path,
            "expected_file_sha256": sha256_hex(bytes),
            "expected_file_byte_length": bytes.len(),
            "replacements": replacements.iter().map(|(old, new)| json!({
                "expected_old_text": old,
                "replacement_text": new,
            })).collect::<Vec<_>>(),
        })
    }

    fn parse_request() -> PatchRequest {
        PatchRequest::parse(
            &ToolInput(request("target.txt", b"alpha\nold\nomega\n", "old", "new")),
            PatchLimits::default(),
        )
        .unwrap()
    }

    fn has_patch_temporary(root: &Path) -> bool {
        fs::read_dir(root).unwrap().any(|entry| {
            entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".rah-repo-patch-")
        })
    }

    fn assert_no_patch_temporary(root: &Path) {
        assert!(!has_patch_temporary(root));
    }

    fn request_value(path: &str, bytes: &[u8], old: &str, replacement: &str) -> Value {
        json!({
            "path": path,
            "expected_file_sha256": sha256_hex(bytes),
            "expected_file_byte_length": bytes.len(),
            "expected_old_text": old,
            "replacement_text": replacement,
        })
    }

    async fn run(tool: &RepositoryWorktreePatchTool, input: Value) -> crate::ToolOutput {
        tool.execute(ToolInput(input), ToolContext::default())
            .await
            .expect("well-formed patch request should return a bounded outcome")
    }

    fn content(output: &crate::ToolOutput) -> &Value {
        match output.content.as_slice() {
            [rah_protocol::ToolContent::Json(value)] => value,
            _ => panic!("outcome should contain one JSON object"),
        }
    }

    fn replace_and_commit(root: &Path, path: &str, bytes: &[u8]) {
        fs::write(root.join(path), bytes).unwrap();
        git(root, &["add", "--", path]);
        git(root, &["commit", "--quiet", "-m", "replace target"]);
    }

    fn git(root: &Path, arguments: &[&str]) {
        let output = Command::new(git_executable())
            .args(arguments)
            .current_dir(root)
            .output()
            .expect("Git command should start");
        assert!(
            output.status.success(),
            "Git command {:?} failed: {}",
            arguments,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn git_output(root: &Path, arguments: &[&str]) -> Vec<u8> {
        let output = Command::new(git_executable())
            .args(arguments)
            .current_dir(root)
            .output()
            .expect("Git command should start");
        assert!(output.status.success());
        output.stdout
    }

    #[cfg(unix)]
    fn git_exit_code(root: &Path, arguments: &[&str]) -> Option<i32> {
        Command::new(git_executable())
            .args(arguments)
            .current_dir(root)
            .status()
            .expect("Git command should start")
            .code()
    }

    fn git_with_input(root: &Path, arguments: &[&str], input: &[u8]) {
        let mut child = Command::new(git_executable())
            .args(arguments)
            .current_dir(root)
            .stdin(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("Git command should start");
        use std::io::Write as _;
        child.stdin.as_mut().unwrap().write_all(input).unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "Git command {:?} failed: {}",
            arguments,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn git_executable() -> PathBuf {
        #[cfg(windows)]
        let output = Command::new("where.exe").arg("git.exe").output().unwrap();
        #[cfg(not(windows))]
        let output = Command::new("which").arg("git").output().unwrap();
        assert!(
            output.status.success(),
            "Git executable must be available for tests"
        );
        let path = String::from_utf8(output.stdout).unwrap();
        fs::canonicalize(path.lines().next().unwrap()).unwrap()
    }

    #[cfg(windows)]
    fn open_without_delete_share(path: &Path) -> fs::File {
        use std::os::windows::{ffi::OsStrExt, io::FromRawHandle};
        use windows_sys::Win32::{
            Foundation::INVALID_HANDLE_VALUE,
            Storage::FileSystem::{
                CreateFileW, FILE_READ_ATTRIBUTES, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
            },
        };

        let path = path
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect::<Vec<_>>();
        let handle = unsafe {
            CreateFileW(
                path.as_ptr(),
                FILE_READ_ATTRIBUTES,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                std::ptr::null(),
                OPEN_EXISTING,
                0,
                std::ptr::null_mut(),
            )
        };
        assert_ne!(handle, INVALID_HANDLE_VALUE, "test lock handle should open");
        unsafe { fs::File::from_raw_handle(handle) }
    }

    #[cfg(unix)]
    fn create_symlink(target: &Path, link: &Path) -> std::io::Result<()> {
        std::os::unix::fs::symlink(target, link)
    }

    #[cfg(windows)]
    fn create_symlink(target: &Path, link: &Path) -> std::io::Result<()> {
        std::os::windows::fs::symlink_file(target, link)
    }
}
