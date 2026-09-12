//! Host-authorized movement of one clean HEAD-tracked repository file.

use std::{
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
    sync::Arc,
};

use async_trait::async_trait;
use futures::lock::Mutex as AsyncMutex;
use rah_protocol::{PermissionLevel, ToolContent, ToolDefinition, ToolInput, ToolName, ToolOutput};
use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    HostArgumentPolicy, HostExecutionPolicy, Tool, ToolContext, ToolError,
    git_support::git_environment,
    host_execute::{is_beneath, paths_equivalent},
    repository_boundary::RepositoryNestedBoundaryPolicy,
    repository_worktree_patch::{
        FileIdentity, parse_logical_path, reject_link_or_reparse, reject_reparse_ancestry,
        reject_unsupported_file_attributes, validate_directory_path, validate_existing_target,
    },
};

/// Stable name for the bounded repository file rename/move capability.
pub const REPOSITORY_RENAME_FILE_TOOL_NAME: &str = "repo.rename-file";
const MAX_PATH_BYTES: usize = 1024;
const MAX_FILE_BYTES: usize = 1024 * 1024;
const MAX_REVIEWED_FILE_BYTES: usize = 64 * 1024;
const MAX_PREPARATION_REQUEST_BYTES: usize = 8192;
const MAX_SERIALIZED_REVIEW_BYTES: usize = 256 * 1024;
const MAX_PREPARED_REPRESENTATION_BYTES: usize = 512 * 1024;

/// Typed human input for the future reviewed rename route.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RepositoryRenameFilePreparationRequest {
    /// Repository-relative logical source path.
    pub source_path: String,
    /// Repository-relative logical destination path.
    pub destination_path: String,
}

/// Sanitized errors from reviewed rename preparation and revalidation.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum RepositoryRenameFilePreparationError {
    /// The closed human request violates a route bound.
    #[error("invalid repository rename-file preparation input: {reason}")]
    InvalidInput { reason: &'static str },
    /// The reviewed route does not support the requested file or repository form.
    #[error("repository rename-file preparation is unsupported: {reason}")]
    Unsupported { reason: &'static str },
    /// The selected repository state or reviewed subset is not admissible.
    #[error("repository rename-file preparation precondition failed: {reason}")]
    PreconditionFailed { reason: &'static str },
    /// Complete review or private retained evidence exceeds its bound.
    #[error("repository rename-file review is too large")]
    ReviewTooLarge,
    /// Retained evidence no longer matches current host state.
    #[error("repository rename-file preparation is stale")]
    Stale,
}

/// Complete bounded review of one exact reviewed move.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RepositoryRenameFileReview {
    operation: &'static str,
    source_path: String,
    destination_path: String,
    source_byte_length: usize,
    source_sha256: String,
    source_content_escaped: String,
    source_format: &'static str,
    source_mode: String,
    expected_effect: &'static str,
    expected_git_consequence: &'static str,
    non_effects: Vec<&'static str>,
}

impl RepositoryRenameFileReview {
    /// Returns the reviewed operation name.
    #[must_use]
    pub fn operation(&self) -> &str {
        self.operation
    }
    /// Returns the logical source path.
    #[must_use]
    pub fn source_path(&self) -> &str {
        &self.source_path
    }
    /// Returns the logical destination path.
    #[must_use]
    pub fn destination_path(&self) -> &str {
        &self.destination_path
    }
    /// Returns the exact source byte length.
    #[must_use]
    pub fn source_byte_length(&self) -> usize {
        self.source_byte_length
    }
    /// Returns the host-derived source SHA-256.
    #[must_use]
    pub fn source_sha256(&self) -> &str {
        &self.source_sha256
    }
    /// Returns the complete escaped source content.
    #[must_use]
    pub fn source_content_escaped(&self) -> &str {
        &self.source_content_escaped
    }
    /// Returns explicit source encoding facts.
    #[must_use]
    pub fn source_format(&self) -> &str {
        self.source_format
    }
    /// Returns the protected Git file mode.
    #[must_use]
    pub fn source_mode(&self) -> &str {
        &self.source_mode
    }
    /// Returns the expected worktree effect.
    #[must_use]
    pub fn expected_effect(&self) -> &str {
        self.expected_effect
    }
    /// Returns the expected unstaged Git consequence.
    #[must_use]
    pub fn expected_git_consequence(&self) -> &str {
        self.expected_git_consequence
    }
    /// Returns effects explicitly excluded from this review.
    #[must_use]
    pub fn non_effects(&self) -> &[&'static str] {
        &self.non_effects
    }
    /// Compatibility alias for complete escaped source display.
    #[must_use]
    pub fn content_escaped(&self) -> &str {
        self.source_content_escaped()
    }
    /// Compatibility alias for the source length.
    #[must_use]
    pub fn content_byte_length(&self) -> usize {
        self.source_byte_length()
    }
    /// Compatibility alias for the source digest.
    #[must_use]
    pub fn content_sha256(&self) -> &str {
        self.source_sha256()
    }
}

/// Opaque host-retained evidence for one reviewed rename preparation.
pub struct RepositoryRenameFilePreparation {
    preparer_identity: Uuid,
    tool_input: ToolInput,
    review: RepositoryRenameFileReview,
    review_identity: String,
    source_sha256: String,
    source_byte_length: usize,
    root_identity: FileIdentity,
    dot_git_identity: FileIdentity,
    git_identity: FileIdentity,
    tool_definition: ToolDefinition,
    destination_ignored: bool,
    pre: Preimage,
}

impl RepositoryRenameFilePreparation {
    /// Returns the exact host-constructed ordinary ToolInput.
    #[must_use]
    pub fn tool_input(&self) -> &ToolInput {
        &self.tool_input
    }
    /// Returns the complete bounded review.
    #[must_use]
    pub fn review(&self) -> &RepositoryRenameFileReview {
        &self.review
    }
    /// Returns the deterministic identity of the complete reviewed operation.
    #[must_use]
    pub fn review_identity(&self) -> &str {
        &self.review_identity
    }
    /// Returns the host-derived source digest.
    #[must_use]
    pub fn source_sha256(&self) -> &str {
        &self.source_sha256
    }
    /// Returns the host-derived source length.
    #[must_use]
    pub fn source_byte_length(&self) -> usize {
        self.source_byte_length
    }
}

impl std::fmt::Debug for RepositoryRenameFilePreparation {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RepositoryRenameFilePreparation")
            .field("preparation", &"redacted")
            .field("review_identity", &self.review_identity)
            .finish_non_exhaustive()
    }
}

/// Independent classification of a later ordinary rename ToolOutput.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RepositoryRenameFileProof {
    /// Filesystem and protected Git evidence prove the reviewed move.
    ReviewedSuccess,
    /// The exact reviewed source preimage remains intact and destination absent.
    KnownNoEffect,
    /// The result or independent observations do not prove either state.
    Uncertain,
}

/// Strictly classifies the current ordinary `repo.rename-file` producer shape.
#[must_use]
pub fn classify_repository_rename_file_output(
    output: &ToolOutput,
    destination_path: &str,
) -> RepositoryRenameFileProof {
    let [ToolContent::Json(value)] = output.content.as_slice() else {
        return RepositoryRenameFileProof::Uncertain;
    };
    let Some(object) = value.as_object() else {
        return RepositoryRenameFileProof::Uncertain;
    };
    let Some(status) = object.get("status").and_then(Value::as_str) else {
        return RepositoryRenameFileProof::Uncertain;
    };
    let Some(uncertain) = object.get("uncertain").and_then(Value::as_bool) else {
        return RepositoryRenameFileProof::Uncertain;
    };
    match status {
        "renamed_verified" => {
            if output.is_error
                || uncertain
                || object.len() != 3
                || object.get("path").and_then(Value::as_str) != Some(destination_path)
            {
                RepositoryRenameFileProof::Uncertain
            } else {
                RepositoryRenameFileProof::ReviewedSuccess
            }
        }
        "known_no_effect" | "invalid_input" | "precondition_failed" => {
            if !output.is_error || uncertain || object.len() != 2 {
                RepositoryRenameFileProof::Uncertain
            } else {
                RepositoryRenameFileProof::KnownNoEffect
            }
        }
        "uncertain" => RepositoryRenameFileProof::Uncertain,
        _ => RepositoryRenameFileProof::Uncertain,
    }
}

/// Host-created authority for one selected repository.
pub struct RepositoryFileRenameTool {
    policy: Arc<RepositoryFileRenamePolicy>,
}

/// Opaque host-created rename authority.
#[derive(Clone)]
pub struct RepositoryFileRenameAuthority {
    policy: Arc<RepositoryFileRenamePolicy>,
}

/// Host-bound, zero-effect preparation and proof foundation for reviewed rename.
pub struct RepositoryRenameFilePreparer {
    identity: Uuid,
    policy: RepositoryFileRenamePolicy,
}

impl RepositoryRenameFilePreparer {
    /// Creates a preparer bound to one host-selected repository and Git binary.
    pub fn new(
        git_executable: impl AsRef<Path>,
        repository_root: impl AsRef<Path>,
    ) -> Result<Self, ToolError> {
        Ok(Self {
            identity: Uuid::new_v4(),
            policy: RepositoryFileRenamePolicy::new(
                git_executable.as_ref(),
                repository_root.as_ref(),
            )?,
        })
    }

    /// Captures a complete review using observation only.
    pub async fn prepare(
        &self,
        request: RepositoryRenameFilePreparationRequest,
    ) -> Result<RepositoryRenameFilePreparation, RepositoryRenameFilePreparationError> {
        let request = ReviewedRenameRequest::parse(request)?;
        let _lease = self.policy.lease.lock().await;
        let source = self
            .policy
            .reviewed_source_bytes(&request.source_path)
            .await
            .map_err(
                |_| RepositoryRenameFilePreparationError::PreconditionFailed {
                    reason: "reviewed_source",
                },
            )?;
        let ordinary_request = RenameRequest {
            source_path: request.source_path.clone(),
            destination_path: request.destination_path.clone(),
            sha256: sha256(&source),
            length: source.len(),
        };
        let pre = self.policy.capture(&ordinary_request).await.map_err(|_| {
            RepositoryRenameFilePreparationError::PreconditionFailed {
                reason: "repository_state",
            }
        })?;
        let destination_ignored = self
            .policy
            .destination_is_ignored(&request.destination_path)
            .await
            .map_err(
                |_| RepositoryRenameFilePreparationError::PreconditionFailed {
                    reason: "destination_ignore_state",
                },
            )?;
        if destination_ignored {
            return Err(RepositoryRenameFilePreparationError::PreconditionFailed {
                reason: "destination_ignored",
            });
        }
        let tool_input = canonical_reviewed_tool_input(&ordinary_request);
        let review = build_rename_review(&request, &pre)?;
        let review_identity = compute_rename_review_identity(
            self.identity,
            &self.policy,
            &ordinary_request,
            &pre,
            destination_ignored,
            &tool_input,
            &review,
        );
        let preparation = RepositoryRenameFilePreparation {
            preparer_identity: self.identity,
            tool_input,
            review,
            review_identity,
            source_sha256: ordinary_request.sha256,
            source_byte_length: ordinary_request.length,
            root_identity: self.policy.root_identity.clone(),
            dot_git_identity: self.policy.dot_git_identity.clone(),
            git_identity: self.policy.git_identity.clone(),
            tool_definition: ordinary_rename_tool_definition(),
            destination_ignored,
            pre,
        };
        if serialized_preparation_size(&preparation) > MAX_PREPARED_REPRESENTATION_BYTES {
            return Err(RepositoryRenameFilePreparationError::ReviewTooLarge);
        }
        Ok(preparation)
    }

    /// Revalidates the retained preparation without performing any mutation.
    pub async fn revalidate(
        &self,
        preparation: &RepositoryRenameFilePreparation,
    ) -> Result<(), RepositoryRenameFilePreparationError> {
        let _lease = self.policy.lease.lock().await;
        if preparation.preparer_identity != self.identity
            || preparation.root_identity != self.policy.root_identity
            || preparation.dot_git_identity != self.policy.dot_git_identity
            || preparation.git_identity != self.policy.git_identity
            || preparation.tool_definition != ordinary_rename_tool_definition()
        {
            return Err(RepositoryRenameFilePreparationError::Stale);
        }
        let request = RenameRequest::parse(&preparation.tool_input)
            .map_err(|_| RepositoryRenameFilePreparationError::Stale)?;
        if canonical_reviewed_tool_input(&request) != preparation.tool_input
            || request.length > MAX_REVIEWED_FILE_BYTES
        {
            return Err(RepositoryRenameFilePreparationError::Stale);
        }
        let current = self
            .policy
            .capture(&request)
            .await
            .map_err(|_| RepositoryRenameFilePreparationError::Stale)?;
        if current != preparation.pre {
            return Err(RepositoryRenameFilePreparationError::Stale);
        }
        let ignored = self
            .policy
            .destination_is_ignored(&request.destination_path)
            .await
            .map_err(|_| RepositoryRenameFilePreparationError::Stale)?;
        if ignored != preparation.destination_ignored || ignored {
            return Err(RepositoryRenameFilePreparationError::Stale);
        }
        let reviewed_request = ReviewedRenameRequest {
            source_path: request.source_path.clone(),
            destination_path: request.destination_path.clone(),
        };
        let review = build_rename_review(&reviewed_request, &current)
            .map_err(|_| RepositoryRenameFilePreparationError::Stale)?;
        if review != preparation.review
            || sha256(&current.bytes) != preparation.source_sha256
            || current.bytes.len() != preparation.source_byte_length
        {
            return Err(RepositoryRenameFilePreparationError::Stale);
        }
        let identity = compute_rename_review_identity(
            self.identity,
            &self.policy,
            &request,
            &current,
            ignored,
            &preparation.tool_input,
            &review,
        );
        if identity != preparation.review_identity {
            return Err(RepositoryRenameFilePreparationError::Stale);
        }
        Ok(())
    }

    /// Independently classifies a later ordinary ToolOutput and fresh state.
    pub async fn prove_result(
        &self,
        preparation: &RepositoryRenameFilePreparation,
        output: &ToolOutput,
    ) -> RepositoryRenameFileProof {
        let _lease = self.policy.lease.lock().await;
        if !self.preparation_resources_match(preparation) {
            return RepositoryRenameFileProof::Uncertain;
        }
        let Ok(request) = RenameRequest::parse(&preparation.tool_input) else {
            return RepositoryRenameFileProof::Uncertain;
        };
        let destination_path = request
            .destination_path
            .to_string_lossy()
            .replace('\\', "/");
        match classify_repository_rename_file_output(output, &destination_path) {
            RepositoryRenameFileProof::ReviewedSuccess => {
                if self.policy.verify_post(&preparation.pre).await.is_ok() {
                    RepositoryRenameFileProof::ReviewedSuccess
                } else {
                    RepositoryRenameFileProof::Uncertain
                }
            }
            RepositoryRenameFileProof::KnownNoEffect => {
                if self.policy.intact(&request, &preparation.pre).await {
                    RepositoryRenameFileProof::KnownNoEffect
                } else {
                    RepositoryRenameFileProof::Uncertain
                }
            }
            RepositoryRenameFileProof::Uncertain => RepositoryRenameFileProof::Uncertain,
        }
    }

    /// Proves the exact reviewed no-effect state after a possible boundary.
    pub async fn prove_known_no_effect(
        &self,
        preparation: &RepositoryRenameFilePreparation,
    ) -> bool {
        let _lease = self.policy.lease.lock().await;
        let Ok(request) = RenameRequest::parse(&preparation.tool_input) else {
            return false;
        };
        self.preparation_resources_match(preparation)
            && self.policy.intact(&request, &preparation.pre).await
    }

    /// Proves the reviewed post-effect state without relying on Tool status.
    pub async fn prove_success(&self, preparation: &RepositoryRenameFilePreparation) -> bool {
        let _lease = self.policy.lease.lock().await;
        self.preparation_resources_match(preparation)
            && self.policy.verify_post(&preparation.pre).await.is_ok()
    }

    fn preparation_resources_match(&self, preparation: &RepositoryRenameFilePreparation) -> bool {
        preparation.preparer_identity == self.identity
            && preparation.root_identity == self.policy.root_identity
            && preparation.dot_git_identity == self.policy.dot_git_identity
            && preparation.git_identity == self.policy.git_identity
    }
}

impl RepositoryFileRenameAuthority {
    /// Binds the authority to host-selected repository resources.
    pub fn new(git: impl AsRef<Path>, root: impl AsRef<Path>) -> Result<Self, ToolError> {
        Ok(Self {
            policy: Arc::new(RepositoryFileRenamePolicy::new(
                git.as_ref(),
                root.as_ref(),
            )?),
        })
    }
    /// Checks whether the authority still names the supplied host resources.
    pub fn matches_resources(&self, git: &Path, root: &Path) -> bool {
        self.policy.matches_resources(git, root)
    }
}

impl RepositoryFileRenameTool {
    /// Creates a tool from host-selected resources.
    pub fn new(git: impl AsRef<Path>, root: impl AsRef<Path>) -> Result<Self, ToolError> {
        Ok(Self::from_authority(RepositoryFileRenameAuthority::new(
            git, root,
        )?))
    }
    /// Creates a tool from an authority constructed by the host.
    #[must_use]
    pub fn from_authority(authority: RepositoryFileRenameAuthority) -> Self {
        Self {
            policy: authority.policy,
        }
    }
}

#[async_trait]
impl Tool for RepositoryFileRenameTool {
    fn definition(&self) -> ToolDefinition {
        ordinary_rename_tool_definition()
    }

    async fn execute(
        &self,
        input: ToolInput,
        _context: ToolContext,
    ) -> Result<ToolOutput, ToolError> {
        #[cfg(feature = "live-test-support")]
        live_test_rename_file_tool_executions::record(&self.policy.root);
        let request = match RenameRequest::parse(&input) {
            Ok(request) => request,
            Err(()) => return Ok(result("invalid_input", None, false)),
        };
        let _lease = self.policy.lease.lock().await;
        let pre = match self.policy.capture(&request).await {
            Ok(pre) => pre,
            Err(()) => return Ok(result("precondition_failed", None, false)),
        };
        #[cfg(test)]
        self.policy
            .test_hook
            .apply(&pre, &self.policy.root, &request.destination_path);
        if self.policy.revalidate(&request, &pre).await.is_err() {
            return Ok(result("precondition_failed", None, false));
        }
        #[cfg(test)]
        self.policy
            .rename_attempts
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        #[cfg(feature = "live-test-support")]
        live_test_rename_file_native_attempts::record(&self.policy.root);
        let native = {
            #[cfg(test)]
            if self
                .policy
                .force_native_failure
                .swap(false, std::sync::atomic::Ordering::SeqCst)
            {
                Err(std::io::Error::other("forced native failure"))
            } else {
                rename_once(&pre.source, &pre.destination)
            }
            #[cfg(not(test))]
            {
                rename_once(&pre.source, &pre.destination)
            }
        };
        #[cfg(test)]
        self.policy
            .test_hook
            .apply_after_attempt(&pre, &self.policy.root);
        if native.is_err() {
            return Ok(if self.policy.intact(&request, &pre).await {
                result("known_no_effect", None, false)
            } else {
                result("uncertain", None, true)
            });
        }
        #[cfg(test)]
        if self
            .policy
            .force_uncertain
            .swap(false, std::sync::atomic::Ordering::SeqCst)
        {
            return Ok(result("uncertain", None, true));
        }
        if self.policy.verify_post(&pre).await.is_ok() {
            Ok(result(
                "renamed_verified",
                Some(&request.destination_path),
                false,
            ))
        } else {
            Ok(result("uncertain", None, true))
        }
    }
}

#[cfg(feature = "live-test-support")]
pub mod live_test_rename_file_tool_executions {
    use std::{
        collections::HashMap,
        path::{Path, PathBuf},
        sync::{Mutex, OnceLock},
    };

    static EXECUTIONS: OnceLock<Mutex<HashMap<PathBuf, usize>>> = OnceLock::new();

    fn key(root: &Path) -> PathBuf {
        std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf())
    }

    pub fn record(root: &Path) {
        let mut executions = EXECUTIONS
            .get_or_init(|| Mutex::new(HashMap::new()))
            .lock()
            .unwrap();
        *executions.entry(key(root)).or_default() += 1;
    }

    pub fn count(root: &Path) -> usize {
        EXECUTIONS
            .get_or_init(|| Mutex::new(HashMap::new()))
            .lock()
            .unwrap()
            .get(&key(root))
            .copied()
            .unwrap_or(0)
    }

    pub fn clear(root: &Path) {
        EXECUTIONS
            .get_or_init(|| Mutex::new(HashMap::new()))
            .lock()
            .unwrap()
            .remove(&key(root));
    }
}

#[cfg(feature = "live-test-support")]
pub mod live_test_rename_file_native_attempts {
    use std::{
        collections::HashMap,
        path::{Path, PathBuf},
        sync::{Mutex, OnceLock},
    };

    static ATTEMPTS: OnceLock<Mutex<HashMap<PathBuf, usize>>> = OnceLock::new();

    fn key(root: &Path) -> PathBuf {
        std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf())
    }

    pub fn record(root: &Path) {
        let mut attempts = ATTEMPTS
            .get_or_init(|| Mutex::new(HashMap::new()))
            .lock()
            .unwrap();
        *attempts.entry(key(root)).or_default() += 1;
    }

    pub fn count(root: &Path) -> usize {
        ATTEMPTS
            .get_or_init(|| Mutex::new(HashMap::new()))
            .lock()
            .unwrap()
            .get(&key(root))
            .copied()
            .unwrap_or(0)
    }

    pub fn clear(root: &Path) {
        ATTEMPTS
            .get_or_init(|| Mutex::new(HashMap::new()))
            .lock()
            .unwrap()
            .remove(&key(root));
    }
}

/// Separate private authority; it does not compose creation, deletion, patch, index, or commit authority.
struct RepositoryFileRenamePolicy {
    git: PathBuf,
    root: PathBuf,
    root_identity: FileIdentity,
    git_identity: FileIdentity,
    dot_git_identity: FileIdentity,
    lease: Arc<AsyncMutex<()>>,
    #[cfg(test)]
    test_hook: TestHook,
    #[cfg(test)]
    rename_attempts: std::sync::atomic::AtomicUsize,
    #[cfg(test)]
    force_uncertain: std::sync::atomic::AtomicBool,
    #[cfg(test)]
    force_native_failure: std::sync::atomic::AtomicBool,
    #[cfg(test)]
    force_observation_unknown: std::sync::atomic::AtomicBool,
    #[cfg(test)]
    force_same_volume_mismatch: std::sync::atomic::AtomicBool,
    #[cfg(all(test, windows))]
    force_alias_ambiguity: std::sync::atomic::AtomicBool,
}

impl RepositoryFileRenamePolicy {
    fn new(git: &Path, root: &Path) -> Result<Self, ToolError> {
        if !git.is_absolute() || !root.is_absolute() {
            return Err(policy_error("host identities must be absolute"));
        }
        reject_reparse_ancestry(root, "repository root")?;
        let root = fs::canonicalize(root).map_err(fs_error)?;
        validate_directory_path(&root, &root, "repository root")?;
        let dot_git_identity = validate_supported_dot_git(&root)?;
        reject_reparse_ancestry(git, "Git executable")?;
        let git = fs::canonicalize(git).map_err(fs_error)?;
        if !fs::metadata(&git).map_err(fs_error)?.is_file() {
            return Err(policy_error("Git identity is invalid"));
        }
        Ok(Self {
            root_identity: FileIdentity::capture(&root)?,
            git_identity: FileIdentity::capture(&git)?,
            dot_git_identity,
            lease: crate::git_stage::repository_lease(&root),
            git,
            root,
            #[cfg(test)]
            test_hook: TestHook::default(),
            #[cfg(test)]
            rename_attempts: Default::default(),
            #[cfg(test)]
            force_uncertain: Default::default(),
            #[cfg(test)]
            force_native_failure: Default::default(),
            #[cfg(test)]
            force_observation_unknown: Default::default(),
            #[cfg(test)]
            force_same_volume_mismatch: Default::default(),
            #[cfg(all(test, windows))]
            force_alias_ambiguity: Default::default(),
        })
    }
    fn matches_resources(&self, git: &Path, root: &Path) -> bool {
        fs::canonicalize(git).ok().as_deref() == Some(&self.git)
            && fs::canonicalize(root).ok().as_deref() == Some(&self.root)
    }
    async fn capture(&self, request: &RenameRequest) -> Result<Preimage, ()> {
        self.repository_ok().map_err(|_| ())?;
        if paths_equivalent(
            &self.root.join(&request.source_path),
            &self.root.join(&request.destination_path),
        ) {
            return Err(());
        }
        let source = validate_existing_target(&self.root, &request.source_path).map_err(|_| ())?;
        let metadata = fs::metadata(&source).map_err(|_| ())?;
        reject_unsupported_file_attributes(&metadata).map_err(|_| ())?;
        let identity = FileIdentity::capture(&source).map_err(|_| ())?;
        if identity.link_count != 1 {
            return Err(());
        }
        let bytes = fs::read(&source).map_err(|_| ())?;
        if bytes.len() > MAX_FILE_BYTES
            || bytes.len() != request.length
            || sha256(&bytes) != request.sha256
        {
            return Err(());
        }
        let git = self.git_state(&request.source_path).await?;
        if git.blob != bytes || !worktree_mode_matches(&metadata, &git.head_entry.mode) {
            return Err(());
        }
        let source_parent = source.parent().ok_or(())?.to_path_buf();
        validate_ordinary_directory_ancestry(&self.root, &source_parent, "source parent")?;
        let source_parent_identity = FileIdentity::capture(&source_parent).map_err(|_| ())?;
        let destination = self.destination(&request.destination_path).await?;
        let destination_parent = destination.parent().ok_or(())?.to_path_buf();
        let destination_parent_identity =
            FileIdentity::capture(&destination_parent).map_err(|_| ())?;
        if !identity.same_volume(&destination_parent_identity) || {
            #[cfg(test)]
            {
                self.force_same_volume_mismatch
                    .load(std::sync::atomic::Ordering::SeqCst)
            }
            #[cfg(not(test))]
            {
                false
            }
        } {
            return Err(());
        }
        Ok(Preimage {
            source,
            destination,
            source_path: request.source_path.clone(),
            destination_path: request.destination_path.clone(),
            identity,
            source_parent,
            source_parent_identity,
            destination_parent,
            destination_parent_identity,
            bytes,
            git,
            index: fs::read(self.root.join(".git/index")).map_err(|_| ())?,
        })
    }
    async fn reviewed_source_bytes(&self, path: &Path) -> Result<Vec<u8>, ()> {
        self.repository_ok().map_err(|_| ())?;
        let source = validate_existing_target(&self.root, path).map_err(|_| ())?;
        let metadata = fs::metadata(&source).map_err(|_| ())?;
        reject_unsupported_file_attributes(&metadata).map_err(|_| ())?;
        if !reviewed_worktree_mode_supported(&metadata) {
            return Err(());
        }
        let identity = FileIdentity::capture(&source).map_err(|_| ())?;
        if identity.link_count != 1 {
            return Err(());
        }
        let bytes = fs::read(source).map_err(|_| ())?;
        if bytes.len() > MAX_REVIEWED_FILE_BYTES
            || std::str::from_utf8(&bytes).is_err()
            || bytes.contains(&0)
        {
            return Err(());
        }
        Ok(bytes)
    }
    async fn destination(&self, relative: &Path) -> Result<PathBuf, ()> {
        let destination = self.root.join(relative);
        let parent = destination.parent().ok_or(())?;
        validate_ordinary_directory_ancestry(&self.root, parent, "destination parent")?;
        match observe_absence(&destination) {
            AbsenceObservation::Absent => {}
            AbsenceObservation::Present | AbsenceObservation::Unknown => return Err(()),
        }
        self.destination_git_absent(relative).await?;
        Ok(destination)
    }
    async fn revalidate(&self, request: &RenameRequest, pre: &Preimage) -> Result<(), ()> {
        let current = self.capture(request).await?;
        if !paths_equivalent(&current.source, &pre.source)
            || current.identity != pre.identity
            || current.source_parent_identity != pre.source_parent_identity
            || current.destination_parent_identity != pre.destination_parent_identity
            || current.bytes != pre.bytes
            || current.git != pre.git
            || current.index != pre.index
        {
            return Err(());
        }
        Ok(())
    }
    async fn intact(&self, request: &RenameRequest, pre: &Preimage) -> bool {
        self.capture(request)
            .await
            .is_ok_and(|current| current.matches(pre))
            && self.observe_absence(&pre.destination) == AbsenceObservation::Absent
    }
    async fn verify_post(&self, pre: &Preimage) -> Result<(), ()> {
        self.repository_ok().map_err(|_| ())?;
        validate_ordinary_directory_ancestry(&self.root, &pre.source_parent, "source parent")?;
        validate_ordinary_directory_ancestry(
            &self.root,
            &pre.destination_parent,
            "destination parent",
        )?;
        if FileIdentity::capture(&pre.source_parent).map_err(|_| ())? != pre.source_parent_identity
            || FileIdentity::capture(&pre.destination_parent).map_err(|_| ())?
                != pre.destination_parent_identity
            || self.observe_absence(&pre.source) != AbsenceObservation::Absent
        {
            return Err(());
        }
        if self.observe_absence(&pre.destination) != AbsenceObservation::Present {
            return Err(());
        }
        #[cfg(windows)]
        {
            #[cfg(test)]
            if self
                .force_alias_ambiguity
                .load(std::sync::atomic::Ordering::SeqCst)
            {
                verify_post_aliases_with(pre, |parent| {
                    if paths_equivalent(parent, &pre.source_parent) {
                        Ok(vec![pre.source_parent.join("fOo.rs")])
                    } else if paths_equivalent(parent, &pre.destination_parent) {
                        Ok(vec![
                            pre.destination_parent.join("Target.rs"),
                            pre.destination_parent.join("tArGeT.rs"),
                        ])
                    } else {
                        Err(())
                    }
                })?;
            } else {
                verify_post_aliases(pre)?;
            }
            #[cfg(not(test))]
            verify_post_aliases(pre)?;
        }
        let destination =
            validate_existing_target(&self.root, &pre.destination_path).map_err(|_| ())?;
        if !paths_equivalent(&destination, &pre.destination) {
            return Err(());
        }
        let destination_metadata = fs::metadata(&destination).map_err(|_| ())?;
        let destination_bytes = fs::read(&destination).map_err(|_| ())?;
        if !destination_metadata.is_file()
            || destination_bytes != pre.bytes
            || destination_bytes.len() != pre.bytes.len()
            || sha256(&destination_bytes) != sha256(&pre.bytes)
        {
            return Err(());
        }
        let destination_identity = FileIdentity::capture(&destination).map_err(|_| ())?;
        if destination_identity.link_count != 1 || !destination_identity.same_object(&pre.identity)
        {
            return Err(());
        }
        let git = self.git_state(&pre.source_path).await?;
        if git != pre.git || fs::read(self.root.join(".git/index")).map_err(|_| ())? != pre.index {
            return Err(());
        }
        Ok(())
    }
    fn observe_absence(&self, path: &Path) -> AbsenceObservation {
        #[cfg(test)]
        if self
            .force_observation_unknown
            .load(std::sync::atomic::Ordering::SeqCst)
        {
            return AbsenceObservation::Unknown;
        }
        observe_absence(path)
    }
    fn repository_ok(&self) -> Result<(), ToolError> {
        reject_reparse_ancestry(&self.root, "repository root")?;
        let root = fs::canonicalize(&self.root).map_err(fs_error)?;
        let dot_git_identity = validate_supported_dot_git(&self.root)?;
        if !paths_equivalent(&root, &self.root)
            || FileIdentity::capture(&root)? != self.root_identity
            || dot_git_identity != self.dot_git_identity
            || FileIdentity::capture(&self.git)? != self.git_identity
        {
            return Err(policy_error("repository identity changed"));
        }
        Ok(())
    }
    async fn git_state(&self, path: &Path) -> Result<GitState, ()> {
        self.require_supported_repository_state().await?;
        let target = path.to_string_lossy().replace('\\', "/");
        let head = self
            .git_output(vec!["rev-parse", "--verify", "HEAD"])
            .await?;
        let branch = self
            .git_output(vec!["symbolic-ref", "--quiet", "--short", "HEAD"])
            .await?;
        let tree = self
            .git_output(vec![
                "--literal-pathspecs",
                "ls-tree",
                "-z",
                "HEAD",
                "--",
                &target,
            ])
            .await?;
        let index = self
            .git_output(vec![
                "--literal-pathspecs",
                "ls-files",
                "-s",
                "-z",
                "--",
                &target,
            ])
            .await?;
        let head_entry = parse_tree_entry(&tree, target.as_bytes())?;
        let index_entry = parse_index_entry(&index, target.as_bytes())?;
        if index_entry != head_entry {
            return Err(());
        }
        let tag = self
            .git_output(vec!["ls-files", "-v", "-z", "--", &target])
            .await?;
        if tag != [b"H ", target.as_bytes(), &[0]].concat() {
            return Err(());
        }
        let blob = self
            .git_output(vec!["show", &format!("HEAD:{target}")])
            .await?;
        let refs = self
            .git_output(vec![
                "for-each-ref",
                "--format=%(refname)%00%(objectname)%00",
            ])
            .await?;
        let sparse = [
            self.git_optional_output(vec!["config", "--bool", "core.sparseCheckout"])
                .await?,
            self.git_optional_output(vec!["config", "--bool", "index.sparse"])
                .await?,
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
        if sparse.iter().any(|value| value.starts_with(b"true")) {
            return Err(());
        }
        Ok(GitState {
            blob,
            head_entry,
            index_entry,
            fingerprint: [head, branch, tree, index, refs, sparse.concat()].concat(),
        })
    }
    async fn destination_git_absent(&self, path: &Path) -> Result<(), ()> {
        let target = path.to_string_lossy().replace('\\', "/");
        #[cfg(windows)]
        let tree_conflict = self.windows_tree_destination_conflict(path).await?;
        #[cfg(not(windows))]
        let tree_conflict = {
            let tree = self
                .git_output(vec![
                    "--literal-pathspecs",
                    "ls-tree",
                    "-z",
                    "HEAD",
                    "--",
                    target.as_str(),
                ])
                .await?;
            git_tree_candidates_conflict(&tree, path)?
        };

        #[cfg(windows)]
        let mut index_args = vec!["--icase-pathspecs"];
        #[cfg(not(windows))]
        let mut index_args = vec!["--literal-pathspecs"];
        index_args.extend(["ls-files", "-s", "-z", "--", target.as_str()]);
        let index = self.git_output(index_args).await?;

        let index_conflict = git_index_candidates_conflict(&index, path)?;
        if tree_conflict || index_conflict {
            return Err(());
        }
        Ok(())
    }
    #[cfg(windows)]
    async fn windows_tree_destination_conflict(&self, destination: &Path) -> Result<bool, ()> {
        let target_components = destination.components().collect::<Vec<_>>();
        let mut parent: Option<PathBuf> = None;
        for depth in 0..target_components.len() {
            let pathspec = match parent.as_ref() {
                Some(path) => format!("{}/", path.to_str().ok_or(())?),
                None => ".".to_owned(),
            };
            let tree = self
                .git_output(vec![
                    "--literal-pathspecs",
                    "ls-tree",
                    "-z",
                    "HEAD",
                    "--",
                    pathspec.as_str(),
                ])
                .await?;
            let target_prefix =
                target_components[..=depth]
                    .iter()
                    .fold(PathBuf::new(), |mut path, component| {
                        path.push(component.as_os_str());
                        path
                    });
            let mut next_parent = None;
            let mut conflict = false;
            for record in nul_records(&tree)? {
                let (candidate, is_tree) = parse_tree_candidate(record)?;
                if git_candidate_is_destination_or_descendant(&candidate, destination) {
                    conflict = true;
                }
                if is_tree
                    && paths_equivalent(&candidate, &target_prefix)
                    && next_parent.replace(candidate).is_some()
                {
                    return Err(());
                }
            }
            if conflict {
                return Ok(true);
            }
            parent = next_parent;
            if parent.is_none() {
                return Ok(false);
            }
        }
        Ok(false)
    }
    async fn destination_is_ignored(&self, path: &Path) -> Result<bool, ()> {
        let target = path.to_string_lossy().replace('\\', "/");
        let output = HostExecutionPolicy::new(
            &self.git,
            HostArgumentPolicy::Exact(vec![
                "check-ignore".to_owned(),
                "--no-index".to_owned(),
                "--quiet".to_owned(),
                "--".to_owned(),
                target,
            ]),
            &self.root,
            ".",
        )
        .map_err(|_| ())?
        .with_environment(git_environment())
        .map_err(|_| ())?
        .execute_process(&ToolInput(json!({})))
        .await
        .map_err(|_| ())?;
        if output.timed_out || output.overflow.is_some() {
            return Err(());
        }
        match output.exit_code {
            Some(0) => Ok(true),
            Some(1) => Ok(false),
            _ => Err(()),
        }
    }
    async fn require_supported_repository_state(&self) -> Result<(), ()> {
        for marker in [
            "MERGE_HEAD",
            "CHERRY_PICK_HEAD",
            "REVERT_HEAD",
            "REBASE_HEAD",
            "SQUASH_MSG",
            "BISECT_LOG",
            "BISECT_START",
            "sequencer",
            "rebase-merge",
            "rebase-apply",
        ] {
            match fs::symlink_metadata(self.root.join(".git").join(marker)) {
                Ok(_) => return Err(()),
                Err(error) if error.kind() == ErrorKind::NotFound => {}
                Err(_) => return Err(()),
            }
        }
        Ok(())
    }
    async fn git_optional_output(&self, args: Vec<&str>) -> Result<Option<Vec<u8>>, ()> {
        let output = HostExecutionPolicy::new(
            &self.git,
            HostArgumentPolicy::Exact(args.into_iter().map(str::to_owned).collect()),
            &self.root,
            ".",
        )
        .map_err(|_| ())?
        .with_environment(git_environment())
        .map_err(|_| ())?
        .execute_process(&ToolInput(json!({})))
        .await
        .map_err(|_| ())?;
        if output.timed_out || output.overflow.is_some() {
            return Err(());
        }
        match output.exit_code {
            Some(0) => Ok(Some(output.stdout)),
            Some(1) => Ok(None),
            _ => Err(()),
        }
    }
    async fn git_output(&self, args: Vec<&str>) -> Result<Vec<u8>, ()> {
        let output = HostExecutionPolicy::new(
            &self.git,
            HostArgumentPolicy::Exact(args.into_iter().map(str::to_owned).collect()),
            &self.root,
            ".",
        )
        .map_err(|_| ())?
        .with_environment(git_environment())
        .map_err(|_| ())?
        .execute_process(&ToolInput(json!({})))
        .await
        .map_err(|_| ())?;
        if output.exit_code == Some(0) && !output.timed_out && output.overflow.is_none() {
            Ok(output.stdout)
        } else {
            Err(())
        }
    }
}

fn validate_supported_dot_git(root: &Path) -> Result<FileIdentity, ToolError> {
    let dot_git = root.join(".git");
    reject_link_or_reparse(&dot_git, "repository metadata")?;
    if !fs::metadata(&dot_git).map_err(fs_error)?.is_dir() {
        return Err(policy_error("linked worktrees are unsupported"));
    }
    FileIdentity::capture(&dot_git)
}

#[derive(PartialEq, Eq)]
struct Preimage {
    source: PathBuf,
    destination: PathBuf,
    source_path: PathBuf,
    destination_path: PathBuf,
    identity: FileIdentity,
    source_parent: PathBuf,
    source_parent_identity: FileIdentity,
    destination_parent: PathBuf,
    destination_parent_identity: FileIdentity,
    bytes: Vec<u8>,
    git: GitState,
    index: Vec<u8>,
}
impl Preimage {
    fn matches(&self, other: &Self) -> bool {
        paths_equivalent(&self.source, &other.source)
            && paths_equivalent(&self.destination, &other.destination)
            && self.identity == other.identity
            && self.source_parent_identity == other.source_parent_identity
            && self.destination_parent_identity == other.destination_parent_identity
            && self.bytes == other.bytes
            && self.git == other.git
            && self.index == other.index
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct GitState {
    blob: Vec<u8>,
    head_entry: GitEntry,
    index_entry: GitEntry,
    fingerprint: Vec<u8>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct GitEntry {
    mode: Vec<u8>,
    object: Vec<u8>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AbsenceObservation {
    Absent,
    Present,
    Unknown,
}
struct RenameRequest {
    source_path: PathBuf,
    destination_path: PathBuf,
    sha256: String,
    length: usize,
}

struct ReviewedRenameRequest {
    source_path: PathBuf,
    destination_path: PathBuf,
}

impl ReviewedRenameRequest {
    fn parse(
        request: RepositoryRenameFilePreparationRequest,
    ) -> Result<Self, RepositoryRenameFilePreparationError> {
        let serialized = serde_json::to_vec(&request).map_err(|_| {
            RepositoryRenameFilePreparationError::InvalidInput {
                reason: "request_serialization",
            }
        })?;
        if serialized.len() > MAX_PREPARATION_REQUEST_BYTES {
            return Err(RepositoryRenameFilePreparationError::InvalidInput {
                reason: "request_too_large",
            });
        }
        if request.source_path.is_empty() || request.destination_path.is_empty() {
            return Err(RepositoryRenameFilePreparationError::InvalidInput {
                reason: "empty_path",
            });
        }
        let source_path = parse_rename_path(&request.source_path).map_err(|_| {
            RepositoryRenameFilePreparationError::InvalidInput {
                reason: "source_path",
            }
        })?;
        let destination_path = parse_rename_path(&request.destination_path).map_err(|_| {
            RepositoryRenameFilePreparationError::InvalidInput {
                reason: "destination_path",
            }
        })?;
        if source_path == destination_path {
            return Err(RepositoryRenameFilePreparationError::InvalidInput {
                reason: "paths_must_differ",
            });
        }
        Ok(Self {
            source_path,
            destination_path,
        })
    }
}

fn canonical_reviewed_tool_input(request: &RenameRequest) -> ToolInput {
    ToolInput(json!({
        "source_path": request.source_path.to_string_lossy().replace('\\', "/"),
        "destination_path": request.destination_path.to_string_lossy().replace('\\', "/"),
        "expected_source_file_sha256": request.sha256,
        "expected_source_file_byte_length": request.length,
    }))
}

fn build_rename_review(
    request: &ReviewedRenameRequest,
    pre: &Preimage,
) -> Result<RepositoryRenameFileReview, RepositoryRenameFilePreparationError> {
    let source = std::str::from_utf8(&pre.bytes).map_err(|_| {
        RepositoryRenameFilePreparationError::PreconditionFailed {
            reason: "source_utf8",
        }
    })?;
    let mode = String::from_utf8(pre.git.head_entry.mode.clone()).map_err(|_| {
        RepositoryRenameFilePreparationError::PreconditionFailed {
            reason: "source_mode",
        }
    })?;
    let review = RepositoryRenameFileReview {
        operation: REPOSITORY_RENAME_FILE_TOOL_NAME,
        source_path: request.source_path.to_string_lossy().replace('\\', "/"),
        destination_path: request
            .destination_path
            .to_string_lossy()
            .replace('\\', "/"),
        source_byte_length: pre.bytes.len(),
        source_sha256: sha256(&pre.bytes),
        source_content_escaped: escape_reviewed_text(source),
        source_format: "strict UTF-8, NUL-free, complete escaped source",
        source_mode: mode,
        expected_effect: "one reviewed source file moves to the absent destination",
        expected_git_consequence: "one unstaged worktree rename-like change; HEAD, index, refs, and history remain unchanged",
        non_effects: vec![
            "no content rewrite",
            "no staging",
            "no unstaging",
            "no commit",
            "no branch/ref/history mutation",
            "no directory creation",
            "no overwrite",
            "no import/reference rewrite",
            "no automatic retry",
            "no replay",
            "no rollback",
            "no compensation",
        ],
    };
    if serde_json::to_vec(&review)
        .map(|bytes| bytes.len() <= MAX_SERIALIZED_REVIEW_BYTES)
        .unwrap_or(false)
    {
        Ok(review)
    } else {
        Err(RepositoryRenameFilePreparationError::ReviewTooLarge)
    }
}

fn escape_reviewed_text(source: &str) -> String {
    use std::fmt::Write as _;
    let mut escaped = String::with_capacity(source.len());
    for character in source.chars() {
        match character {
            '\r' => escaped.push_str("\\r"),
            '\n' => escaped.push_str("\\n"),
            '\t' => escaped.push_str("\\t"),
            ' ' => escaped.push_str("\\u{20}"),
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            character
                if character.is_control() || character == '\u{7f}' || !character.is_ascii() =>
            {
                let _ = write!(escaped, "\\u{{{:x}}}", character as u32);
            }
            character => escaped.push(character),
        }
    }
    escaped
}

fn compute_rename_review_identity(
    preparer_identity: Uuid,
    policy: &RepositoryFileRenamePolicy,
    request: &RenameRequest,
    pre: &Preimage,
    destination_ignored: bool,
    tool_input: &ToolInput,
    review: &RepositoryRenameFileReview,
) -> String {
    let mut digest = Sha256::new();
    digest.update(b"rah-repository-rename-file-preparation-v1\0");
    digest.update(preparer_identity.as_bytes());
    update_serialized(&mut digest, tool_input);
    update_serialized(&mut digest, review);
    update_serialized(&mut digest, &ordinary_rename_tool_definition());
    digest.update(request.source_path.to_string_lossy().as_bytes());
    digest.update(request.destination_path.to_string_lossy().as_bytes());
    digest.update(&pre.bytes);
    digest.update(&pre.index);
    digest.update(&pre.git.fingerprint);
    digest.update([u8::from(destination_ignored)]);
    digest.update(format!("{:?}", policy.root_identity).as_bytes());
    digest.update(format!("{:?}", policy.dot_git_identity).as_bytes());
    digest.update(format!("{:?}", policy.git_identity).as_bytes());
    digest.update(format!("{:?}", pre.identity).as_bytes());
    digest.update(format!("{:?}", pre.source_parent_identity).as_bytes());
    digest.update(format!("{:?}", pre.destination_parent_identity).as_bytes());
    hex_digest(digest.finalize())
}

fn update_serialized<T: Serialize>(digest: &mut Sha256, value: &T) {
    if let Ok(bytes) = serde_json::to_vec(value) {
        digest.update(bytes);
    }
}

fn hex_digest(bytes: impl IntoIterator<Item = u8>) -> String {
    use std::fmt::Write as _;
    let mut result = String::with_capacity(64);
    for byte in bytes {
        let _ = write!(result, "{byte:02x}");
    }
    result
}

fn serialized_preparation_size(preparation: &RepositoryRenameFilePreparation) -> usize {
    let public_size = serde_json::to_vec(&json!({
        "tool_input": &preparation.tool_input,
        "review": &preparation.review,
        "review_identity": &preparation.review_identity,
        "source_sha256": &preparation.source_sha256,
        "source_byte_length": preparation.source_byte_length,
        "tool_definition": &preparation.tool_definition,
    }))
    .map(|bytes| bytes.len())
    .unwrap_or(usize::MAX);
    public_size
        .saturating_add(preparation.pre.bytes.len())
        .saturating_add(preparation.pre.index.len())
        .saturating_add(preparation.pre.git.fingerprint.len())
        .saturating_add(4096)
}

impl RenameRequest {
    fn parse(input: &ToolInput) -> Result<Self, ()> {
        let object = input.0.as_object().ok_or(())?;
        if object.len() != 4 {
            return Err(());
        }
        let source_path = parse_rename_path(
            object
                .get("source_path")
                .and_then(Value::as_str)
                .ok_or(())?,
        )
        .map_err(|_| ())?;
        let destination_path = parse_rename_path(
            object
                .get("destination_path")
                .and_then(Value::as_str)
                .ok_or(())?,
        )
        .map_err(|_| ())?;
        let sha256 = object
            .get("expected_source_file_sha256")
            .and_then(Value::as_str)
            .ok_or(())?;
        if sha256.len() != 64
            || !sha256
                .bytes()
                .all(|b| b.is_ascii_digit() || matches!(b, b'a'..=b'f'))
        {
            return Err(());
        }
        let length = object
            .get("expected_source_file_byte_length")
            .and_then(Value::as_u64)
            .and_then(|n| usize::try_from(n).ok())
            .filter(|n| *n <= MAX_FILE_BYTES)
            .ok_or(())?;
        Ok(Self {
            source_path,
            destination_path,
            sha256: sha256.to_owned(),
            length,
        })
    }
}
fn parse_rename_path(value: &str) -> Result<PathBuf, ()> {
    let path = parse_logical_path(value, MAX_PATH_BYTES).map_err(|_| ())?;
    for component in path.components() {
        let name = component.as_os_str().to_str().ok_or(())?;
        if name.contains(['*', '?', '[', ']'])
            || name.ends_with(['.', ' '])
            || reserved_windows_name(name)
        {
            return Err(());
        }
    }
    Ok(path)
}
fn reserved_windows_name(component: &str) -> bool {
    let stem = component
        .trim_end_matches(['.', ' '])
        .split('.')
        .next()
        .unwrap_or("")
        .to_ascii_uppercase();
    matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (stem.len() == 4
            && matches!(&stem[..3], "COM" | "LPT")
            && matches!(stem.as_bytes()[3], b'1'..=b'9'))
}
fn parse_tree_entry(bytes: &[u8], path: &[u8]) -> Result<GitEntry, ()> {
    let records = nul_records(bytes)?;
    let matching = records
        .iter()
        .filter(|record| record_path(record) == Some(path))
        .copied()
        .collect::<Vec<_>>();
    let [record] = matching.as_slice() else {
        return Err(());
    };
    let tab = record.iter().position(|byte| *byte == b'\t').ok_or(())?;
    let fields = record[..tab]
        .split(|byte| *byte == b' ')
        .collect::<Vec<_>>();
    let [mode, kind, object] = fields.as_slice() else {
        return Err(());
    };
    if *kind != b"blob" || !matches!(*mode, b"100644" | b"100755") {
        return Err(());
    }
    Ok(GitEntry {
        mode: mode.to_vec(),
        object: object.to_vec(),
    })
}

fn parse_index_entry(bytes: &[u8], path: &[u8]) -> Result<GitEntry, ()> {
    let records = nul_records(bytes)?;
    let [record] = records.as_slice() else {
        return Err(());
    };
    let tab = record.iter().position(|byte| *byte == b'\t').ok_or(())?;
    if &record[tab + 1..] != path {
        return Err(());
    }
    let fields = record[..tab]
        .split(|byte| *byte == b' ')
        .collect::<Vec<_>>();
    let [mode, object, stage] = fields.as_slice() else {
        return Err(());
    };
    if *stage != b"0" || !matches!(*mode, b"100644" | b"100755") {
        return Err(());
    }
    if object.iter().all(|byte| *byte == b'0') {
        return Err(());
    }
    Ok(GitEntry {
        mode: mode.to_vec(),
        object: object.to_vec(),
    })
}

fn git_index_candidates_conflict(bytes: &[u8], destination: &Path) -> Result<bool, ()> {
    let mut conflict = false;
    for record in nul_records(bytes)? {
        let candidate = parse_index_candidate(record)?;
        conflict |= git_candidate_is_destination_or_descendant(&candidate, destination);
    }
    Ok(conflict)
}

#[cfg(not(windows))]
fn git_tree_candidates_conflict(bytes: &[u8], destination: &Path) -> Result<bool, ()> {
    let mut conflict = false;
    for record in nul_records(bytes)? {
        let (candidate, _) = parse_tree_candidate(record)?;
        conflict |= git_candidate_is_destination_or_descendant(&candidate, destination);
    }
    Ok(conflict)
}

fn parse_tree_candidate(record: &[u8]) -> Result<(PathBuf, bool), ()> {
    let tab = record.iter().position(|byte| *byte == b'\t').ok_or(())?;
    let fields = record[..tab]
        .split(|byte| *byte == b' ')
        .collect::<Vec<_>>();
    let [mode, kind, object] = fields.as_slice() else {
        return Err(());
    };
    if !matches!(
        (*kind, *mode),
        (b"blob", b"100644" | b"100755" | b"120000")
            | (b"tree", b"040000")
            | (b"commit", b"160000")
    ) || !valid_git_object_id(object)
    {
        return Err(());
    }
    Ok((git_candidate_path(&record[tab + 1..])?, *kind == b"tree"))
}

fn parse_index_candidate(record: &[u8]) -> Result<PathBuf, ()> {
    let tab = record.iter().position(|byte| *byte == b'\t').ok_or(())?;
    let fields = record[..tab]
        .split(|byte| *byte == b' ')
        .collect::<Vec<_>>();
    let [mode, object, stage] = fields.as_slice() else {
        return Err(());
    };
    if !matches!(*mode, b"100644" | b"100755" | b"120000" | b"160000")
        || !valid_git_object_id(object)
        || !matches!(*stage, b"0" | b"1" | b"2" | b"3")
    {
        return Err(());
    }
    git_candidate_path(&record[tab + 1..])
}

fn valid_git_object_id(object: &[u8]) -> bool {
    matches!(object.len(), 40 | 64)
        && object
            .iter()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f' | b'A'..=b'F'))
}

fn git_candidate_path(path: &[u8]) -> Result<PathBuf, ()> {
    let path = std::str::from_utf8(path).map_err(|_| ())?;
    parse_rename_path(path)
}

fn git_candidate_is_destination_or_descendant(candidate: &Path, destination: &Path) -> bool {
    let candidate_components = candidate.components().collect::<Vec<_>>();
    let destination_components = destination.components().collect::<Vec<_>>();
    candidate_components.len() >= destination_components.len()
        && candidate_components.iter().zip(destination_components).all(
            |(candidate, destination)| {
                let mut candidate_prefix = PathBuf::new();
                candidate_prefix.push(candidate.as_os_str());
                let mut destination_prefix = PathBuf::new();
                destination_prefix.push(destination.as_os_str());
                paths_equivalent(&candidate_prefix, &destination_prefix)
            },
        )
}

fn record_path(record: &[u8]) -> Option<&[u8]> {
    record
        .iter()
        .position(|byte| *byte == b'\t')
        .map(|tab| &record[tab + 1..])
}

fn nul_records(bytes: &[u8]) -> Result<Vec<&[u8]>, ()> {
    if bytes.is_empty() {
        return Ok(Vec::new());
    }
    let bytes = bytes.strip_suffix(&[0]).ok_or(())?;
    Ok(bytes.split(|byte| *byte == 0).collect())
}

fn observe_absence(path: &Path) -> AbsenceObservation {
    match fs::symlink_metadata(path) {
        Ok(_) => AbsenceObservation::Present,
        Err(error) if error.kind() == ErrorKind::NotFound => AbsenceObservation::Absent,
        Err(_) => AbsenceObservation::Unknown,
    }
}

fn validate_ordinary_directory_ancestry(
    root: &Path,
    directory: &Path,
    label: &str,
) -> Result<(), ()> {
    validate_directory_path(root, directory, label).map_err(|_| ())?;
    RepositoryNestedBoundaryPolicy::new(root)
        .validate_existing(directory)
        .map_err(|_| ())?;
    let mount_points = observed_mount_points()?;
    let mut current = directory.to_path_buf();
    loop {
        reject_link_or_reparse(&current, label).map_err(|_| ())?;
        if !fs::metadata(&current).map_err(|_| ())?.is_dir() {
            return Err(());
        }
        if paths_equivalent(&current, root) {
            return Ok(());
        }
        reject_nested_repository_boundary(&current)?;
        if mount_points
            .iter()
            .any(|mount_point| paths_equivalent(mount_point, &current))
        {
            return Err(());
        }
        let parent = current.parent().ok_or(())?;
        if !is_beneath(parent, root) {
            return Err(());
        }
        current = parent.to_path_buf();
    }
}

fn reject_nested_repository_boundary(directory: &Path) -> Result<(), ()> {
    match fs::symlink_metadata(directory.join(".git")) {
        Ok(_) => Err(()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(_) => Err(()),
    }
}

fn observed_mount_points() -> Result<Vec<PathBuf>, ()> {
    #[cfg(target_os = "linux")]
    {
        let contents = fs::read_to_string("/proc/self/mountinfo").map_err(|_| ())?;
        contents
            .lines()
            .map(|line| {
                let fields = line.split_whitespace().collect::<Vec<_>>();
                let mount_point = fields.get(4).ok_or(())?;
                decode_mountinfo_path(mount_point)
            })
            .collect()
    }
    #[cfg(not(target_os = "linux"))]
    {
        Ok(Vec::new())
    }
}

#[cfg(target_os = "linux")]
fn decode_mountinfo_path(value: &str) -> Result<PathBuf, ()> {
    use std::os::unix::ffi::OsStringExt;

    let mut bytes = Vec::with_capacity(value.len());
    let encoded = value.as_bytes();
    let mut index = 0;
    while index < encoded.len() {
        if encoded[index] == b'\\' {
            let digits = encoded.get(index + 1..index + 4).ok_or(())?;
            if digits.iter().all(|digit| matches!(digit, b'0'..=b'7')) {
                let byte = digits
                    .iter()
                    .fold(0, |value, digit| value * 8 + (*digit - b'0'));
                bytes.push(byte);
                index += 4;
                continue;
            }
            return Err(());
        }
        bytes.push(encoded[index]);
        index += 1;
    }
    let path = PathBuf::from(std::ffi::OsString::from_vec(bytes));
    path.is_absolute().then_some(path).ok_or(())
}

#[cfg(windows)]
fn verify_post_aliases(pre: &Preimage) -> Result<(), ()> {
    verify_post_aliases_with(pre, read_directory_entries)
}

#[cfg(windows)]
fn verify_post_aliases_with<F>(pre: &Preimage, mut read_entries: F) -> Result<(), ()>
where
    F: FnMut(&Path) -> Result<Vec<PathBuf>, ()>,
{
    let source_entries = read_entries(&pre.source_parent)?;
    if source_entries
        .iter()
        .any(|entry| paths_equivalent(entry, &pre.source))
    {
        return Err(());
    }
    let destination_entries = if paths_equivalent(&pre.source_parent, &pre.destination_parent) {
        source_entries
    } else {
        read_entries(&pre.destination_parent)?
    };
    let destination_matches = destination_entries
        .iter()
        .filter(|entry| paths_equivalent(entry, &pre.destination))
        .count();
    if destination_matches != 1 {
        return Err(());
    }
    Ok(())
}

#[cfg(windows)]
fn read_directory_entries(parent: &Path) -> Result<Vec<PathBuf>, ()> {
    fs::read_dir(parent)
        .map_err(|_| ())?
        .map(|entry| entry.map(|entry| entry.path()).map_err(|_| ()))
        .collect()
}

fn worktree_mode_matches(metadata: &fs::Metadata, mode: &[u8]) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let executable = metadata.permissions().mode() & 0o111 != 0;
        executable == (mode == b"100755")
    }
    #[cfg(not(unix))]
    {
        let _ = (metadata, mode);
        true
    }
}

fn reviewed_worktree_mode_supported(metadata: &fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        matches!(metadata.permissions().mode() & 0o777, 0o644 | 0o755)
    }
    #[cfg(not(unix))]
    {
        let _ = metadata;
        true
    }
}

fn rename_once(source: &Path, destination: &Path) -> Result<(), std::io::Error> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Storage::FileSystem::MoveFileExW;
        let source = source
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect::<Vec<_>>();
        let destination = destination
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect::<Vec<_>>();
        let ok = unsafe { MoveFileExW(source.as_ptr(), destination.as_ptr(), 0) };
        if ok == 0 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::ffi::OsStrExt;
        let source = std::ffi::CString::new(source.as_os_str().as_bytes())
            .map_err(|_| std::io::ErrorKind::InvalidInput)?;
        let destination = std::ffi::CString::new(destination.as_os_str().as_bytes())
            .map_err(|_| std::io::ErrorKind::InvalidInput)?;
        let result = unsafe {
            libc::renameat2(
                libc::AT_FDCWD,
                source.as_ptr(),
                libc::AT_FDCWD,
                destination.as_ptr(),
                libc::RENAME_NOREPLACE,
            )
        };
        if result == 0 {
            Ok(())
        } else {
            Err(std::io::Error::last_os_error())
        }
    }
    #[cfg(all(unix, not(target_os = "linux")))]
    {
        let _ = (source, destination);
        Err(std::io::Error::other(
            "no supported no-replace rename primitive",
        ))
    }
}
fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn ordinary_rename_tool_definition() -> ToolDefinition {
    ToolDefinition {
        name: ToolName::new(REPOSITORY_RENAME_FILE_TOOL_NAME),
        description: "Renames one clean HEAD-tracked file to an absent path within the same bounded repository.".to_owned(),
        input_schema: json!({"type":"object","properties":{"source_path":{"type":"string","minLength":1,"maxLength":MAX_PATH_BYTES},"destination_path":{"type":"string","minLength":1,"maxLength":MAX_PATH_BYTES},"expected_source_file_sha256":{"type":"string","pattern":"^[0-9a-f]{64}$"},"expected_source_file_byte_length":{"type":"integer","minimum":0,"maximum":MAX_FILE_BYTES}},"required":["source_path","destination_path","expected_source_file_sha256","expected_source_file_byte_length"],"additionalProperties":false}),
        permission: PermissionLevel::Execute,
    }
}

fn result(status: &str, path: Option<&Path>, uncertain: bool) -> ToolOutput {
    let mut value = json!({"status":status,"uncertain":uncertain});
    if let Some(path) = path {
        value["path"] = Value::String(path.to_string_lossy().replace('\\', "/"));
    }
    ToolOutput {
        content: vec![ToolContent::Json(value)],
        is_error: status != "renamed_verified",
    }
}
fn policy_error(message: impl Into<String>) -> ToolError {
    ToolError::Execution {
        message: format!(
            "repository file rename policy rejected capability: {}",
            message.into()
        ),
    }
}
fn fs_error(error: impl std::fmt::Display) -> ToolError {
    policy_error(error.to_string())
}

#[cfg(test)]
#[derive(Default)]
struct TestHook {
    modify_source: std::sync::atomic::AtomicBool,
    create_destination: std::sync::atomic::AtomicBool,
    replace_source_parent: std::sync::atomic::AtomicBool,
    replace_destination_parent: std::sync::atomic::AtomicBool,
    create_nested_boundary: std::sync::atomic::AtomicBool,
    #[cfg(any(unix, windows))]
    replace_dot_git_before_revalidation: std::sync::atomic::AtomicBool,
    replace_source_after_attempt: std::sync::atomic::AtomicBool,
    replace_destination_after_attempt: std::sync::atomic::AtomicBool,
    replace_source_parent_after_attempt: std::sync::atomic::AtomicBool,
    replace_destination_parent_after_attempt: std::sync::atomic::AtomicBool,
    create_nested_boundary_after_attempt: std::sync::atomic::AtomicBool,
    #[cfg(any(unix, windows))]
    replace_dot_git_after_attempt: std::sync::atomic::AtomicBool,
    #[cfg(any(unix, windows))]
    redirect_destination_parent_after_attempt: std::sync::atomic::AtomicBool,
    #[cfg(unix)]
    replace_destination_with_symlink_after_attempt: std::sync::atomic::AtomicBool,
}
#[cfg(test)]
impl TestHook {
    fn apply(&self, pre: &Preimage, root: &Path, destination: &Path) {
        use std::sync::atomic::Ordering;
        if self.modify_source.swap(false, Ordering::SeqCst) {
            fs::write(&pre.source, b"changed").unwrap();
        }
        if self.create_destination.swap(false, Ordering::SeqCst) {
            fs::write(root.join(destination), b"external").unwrap();
        }
        if self.replace_source_parent.swap(false, Ordering::SeqCst) {
            replace_parent(&pre.source_parent);
        }
        if self
            .replace_destination_parent
            .swap(false, Ordering::SeqCst)
        {
            replace_parent(&pre.destination_parent);
        }
        if self.create_nested_boundary.swap(false, Ordering::SeqCst) {
            fs::create_dir(pre.destination_parent.join(".git")).unwrap();
        }
        #[cfg(any(unix, windows))]
        if self
            .replace_dot_git_before_revalidation
            .swap(false, Ordering::SeqCst)
        {
            replace_dot_git_with_redirect(&root.join(".git"));
        }
    }

    fn apply_after_attempt(&self, pre: &Preimage, _root: &Path) {
        use std::sync::atomic::Ordering;
        if self
            .replace_source_after_attempt
            .swap(false, Ordering::SeqCst)
        {
            if pre.source.exists() {
                let backup = pre.source.with_extension("rah-rename-replaced");
                fs::rename(&pre.source, backup).unwrap();
            }
            fs::write(&pre.source, &pre.bytes).unwrap();
        }
        if self
            .replace_destination_after_attempt
            .swap(false, Ordering::SeqCst)
        {
            let backup = pre.destination.with_extension("rah-rename-replaced");
            fs::rename(&pre.destination, backup).unwrap();
            fs::write(&pre.destination, &pre.bytes).unwrap();
        }
        if self
            .replace_source_parent_after_attempt
            .swap(false, Ordering::SeqCst)
        {
            replace_parent(&pre.source_parent);
        }
        if self
            .replace_destination_parent_after_attempt
            .swap(false, Ordering::SeqCst)
        {
            replace_parent(&pre.destination_parent);
        }
        if self
            .create_nested_boundary_after_attempt
            .swap(false, Ordering::SeqCst)
        {
            fs::create_dir(pre.destination_parent.join(".git")).unwrap();
        }
        #[cfg(any(unix, windows))]
        if self
            .replace_dot_git_after_attempt
            .swap(false, Ordering::SeqCst)
        {
            replace_dot_git_with_redirect(&_root.join(".git"));
        }
        #[cfg(any(unix, windows))]
        if self
            .redirect_destination_parent_after_attempt
            .swap(false, Ordering::SeqCst)
        {
            redirect_parent(&pre.destination_parent);
        }
        #[cfg(unix)]
        if self
            .replace_destination_with_symlink_after_attempt
            .swap(false, Ordering::SeqCst)
        {
            use std::os::unix::fs::symlink;
            let backup = pre.destination.with_extension("rah-rename-link-target");
            fs::rename(&pre.destination, &backup).unwrap();
            symlink(&backup, &pre.destination).unwrap();
        }
    }
}

#[cfg(test)]
fn replace_parent(path: &Path) {
    let backup = path.with_file_name(format!(
        "{}.rah-rename-replaced",
        path.file_name().unwrap().to_string_lossy()
    ));
    fs::rename(path, backup).unwrap();
    fs::create_dir(path).unwrap();
}

#[cfg(all(test, any(unix, windows)))]
fn redirect_parent(path: &Path) {
    let backup = path.with_file_name(format!(
        "{}.rah-rename-redirect-target",
        path.file_name().unwrap().to_string_lossy()
    ));
    fs::rename(path, &backup).unwrap();
    create_directory_redirect(path, &backup);
}

#[cfg(all(test, any(unix, windows)))]
fn replace_dot_git_with_redirect(dot_git: &Path) {
    let target = dot_git.with_file_name(".git.rah-rename-metadata-target");
    fs::rename(dot_git, &target).unwrap();
    create_directory_redirect(dot_git, &target);
}

#[cfg(all(test, unix))]
fn create_directory_redirect(alias: &Path, target: &Path) {
    std::os::unix::fs::symlink(target, alias).unwrap();
}

#[cfg(all(test, windows))]
fn create_directory_redirect(alias: &Path, target: &Path) {
    assert!(
        std::process::Command::new("cmd.exe")
            .args([
                "/c",
                "mklink",
                "/J",
                alias.to_str().unwrap(),
                target.to_str().unwrap(),
            ])
            .status()
            .unwrap()
            .success()
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::Write,
        process::Command,
        time::{SystemTime, UNIX_EPOCH},
    };
    struct Fixture {
        root: PathBuf,
        git: PathBuf,
    }
    impl Fixture {
        fn new() -> Self {
            Self::with_source("old.txt")
        }
        fn nested() -> Self {
            Self::with_source("source/old.txt")
        }
        fn with_source(source: &str) -> Self {
            let root = std::env::temp_dir().join(format!(
                "rah-rename-{}",
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir_all(root.join("existing")).unwrap();
            let git = git_path();
            for args in [
                ["init", "--quiet"].as_slice(),
                ["config", "user.name", "RAH Test"].as_slice(),
                ["config", "user.email", "rah@example.invalid"].as_slice(),
            ] {
                run(&git, &root, args);
            }
            let source = Path::new(source);
            if let Some(parent) = source.parent() {
                fs::create_dir_all(root.join(parent)).unwrap();
            }
            fs::write(root.join(source), b"rename bytes").unwrap();
            run(&git, &root, &["add", source.to_str().unwrap()]);
            run(&git, &root, &["commit", "--quiet", "-m", "base"]);
            Self { root, git }
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
    fn git_path() -> PathBuf {
        #[cfg(windows)]
        let command = ("where.exe", "git.exe");
        #[cfg(not(windows))]
        let command = ("which", "git");
        let out = Command::new(command.0).arg(command.1).output().unwrap();
        fs::canonicalize(
            String::from_utf8(out.stdout)
                .unwrap()
                .lines()
                .next()
                .unwrap(),
        )
        .unwrap()
    }
    fn run(git: &Path, root: &Path, args: &[&str]) {
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
    fn output(git: &Path, root: &Path, args: &[&str]) -> Vec<u8> {
        let output = Command::new(git)
            .args(args)
            .current_dir(root)
            .output()
            .unwrap();
        assert!(output.status.success(), "{args:?}");
        output.stdout
    }
    fn execute(tool: &RepositoryFileRenameTool, input: Value) -> Value {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let output = runtime
            .block_on(tool.execute(ToolInput(input), ToolContext::default()))
            .unwrap();
        let [ToolContent::Json(value)] = output.content.as_slice() else {
            panic!("JSON result required")
        };
        value.clone()
    }
    fn request(source: &str, destination: &str) -> Value {
        json!({"source_path":source,"destination_path":destination,"expected_source_file_sha256":sha256(b"rename bytes"),"expected_source_file_byte_length":12})
    }
    fn attempts(tool: &RepositoryFileRenameTool) -> usize {
        tool.policy
            .rename_attempts
            .load(std::sync::atomic::Ordering::SeqCst)
    }
    #[test]
    fn same_directory_rename_preserves_bytes_and_attempt_count() {
        let f = Fixture::new();
        let t = RepositoryFileRenameTool::new(&f.git, &f.root).unwrap();
        assert_eq!(
            execute(&t, request("old.txt", "renamed.txt"))["status"],
            "renamed_verified"
        );
        assert_eq!(
            fs::read(f.root.join("renamed.txt")).unwrap(),
            b"rename bytes"
        );
        assert!(!f.root.join("old.txt").exists());
        assert_eq!(
            t.policy
                .rename_attempts
                .load(std::sync::atomic::Ordering::SeqCst),
            1
        );
    }
    #[test]
    fn cross_directory_move_is_supported() {
        let f = Fixture::new();
        let t = RepositoryFileRenameTool::new(&f.git, &f.root).unwrap();
        assert_eq!(
            execute(&t, request("old.txt", "existing/moved.txt"))["status"],
            "renamed_verified"
        );
        assert_eq!(
            fs::read(f.root.join("existing/moved.txt")).unwrap(),
            b"rename bytes"
        );
    }

    #[cfg(any(unix, windows))]
    #[test]
    fn same_target_dot_git_link_before_initial_execution_is_rejected_without_effect() {
        let f = Fixture::new();
        let t = RepositoryFileRenameTool::new(&f.git, &f.root).unwrap();
        let expected = t.policy.dot_git_identity.clone();
        replace_dot_git_with_redirect(&f.root.join(".git"));
        assert_eq!(
            FileIdentity::capture(&f.root.join(".git")).unwrap(),
            expected
        );
        assert!(reject_link_or_reparse(&f.root.join(".git"), "repository metadata").is_err());
        assert_eq!(
            execute(&t, request("old.txt", "renamed.txt"))["status"],
            "precondition_failed"
        );
        assert_eq!(attempts(&t), 0);
        assert!(f.root.join("old.txt").exists());
        assert!(!f.root.join("renamed.txt").exists());
    }

    #[cfg(any(unix, windows))]
    #[test]
    fn same_target_dot_git_link_between_capture_and_revalidation_is_rejected_without_effect() {
        let f = Fixture::new();
        let t = RepositoryFileRenameTool::new(&f.git, &f.root).unwrap();
        let expected = t.policy.dot_git_identity.clone();
        t.policy
            .test_hook
            .replace_dot_git_before_revalidation
            .store(true, std::sync::atomic::Ordering::SeqCst);
        assert_eq!(
            execute(&t, request("old.txt", "renamed.txt"))["status"],
            "precondition_failed"
        );
        assert_eq!(attempts(&t), 0);
        assert_eq!(
            FileIdentity::capture(&f.root.join(".git")).unwrap(),
            expected
        );
        assert!(reject_link_or_reparse(&f.root.join(".git"), "repository metadata").is_err());
        assert!(f.root.join("old.txt").exists());
        assert!(!f.root.join("renamed.txt").exists());
    }

    #[cfg(any(unix, windows))]
    #[test]
    fn same_target_dot_git_link_after_possible_effect_is_uncertain_without_replay() {
        let f = Fixture::new();
        let t = RepositoryFileRenameTool::new(&f.git, &f.root).unwrap();
        let expected = t.policy.dot_git_identity.clone();
        t.policy
            .test_hook
            .replace_dot_git_after_attempt
            .store(true, std::sync::atomic::Ordering::SeqCst);
        assert_eq!(
            execute(&t, request("old.txt", "renamed.txt"))["status"],
            "uncertain"
        );
        assert_eq!(attempts(&t), 1);
        assert_eq!(
            FileIdentity::capture(&f.root.join(".git")).unwrap(),
            expected
        );
        assert!(reject_link_or_reparse(&f.root.join(".git"), "repository metadata").is_err());
        assert!(!f.root.join("old.txt").exists());
        assert!(f.root.join("renamed.txt").exists());
    }

    #[test]
    fn removed_or_replaced_dot_git_directory_is_rejected_without_effect() {
        for replace in [false, true] {
            let f = Fixture::new();
            let t = RepositoryFileRenameTool::new(&f.git, &f.root).unwrap();
            let dot_git = f.root.join(".git");
            if replace {
                fs::rename(&dot_git, f.root.join(".git.rah-rename-different")).unwrap();
                fs::create_dir(&dot_git).unwrap();
            } else {
                fs::remove_dir_all(&dot_git).unwrap();
            }
            assert_eq!(
                execute(&t, request("old.txt", "renamed.txt"))["status"],
                "precondition_failed"
            );
            assert_eq!(attempts(&t), 0);
            assert!(f.root.join("old.txt").exists());
            assert!(!f.root.join("renamed.txt").exists());
        }
    }

    #[test]
    fn linked_worktree_gitfile_remains_unsupported() {
        let f = Fixture::new();
        let dot_git = f.root.join(".git");
        fs::remove_dir_all(&dot_git).unwrap();
        fs::write(&dot_git, "gitdir: metadata").unwrap();
        assert!(RepositoryFileRenameTool::new(&f.git, &f.root).is_err());
    }

    #[test]
    fn tracked_destination_missing_from_worktree_is_rejected() {
        let f = Fixture::new();
        fs::write(f.root.join("tracked-destination.txt"), b"tracked").unwrap();
        run(&f.git, &f.root, &["add", "tracked-destination.txt"]);
        run(
            &f.git,
            &f.root,
            &["commit", "--quiet", "-m", "tracked-destination"],
        );
        fs::remove_file(f.root.join("tracked-destination.txt")).unwrap();
        let t = RepositoryFileRenameTool::new(&f.git, &f.root).unwrap();
        assert_eq!(
            execute(&t, request("old.txt", "tracked-destination.txt"))["status"],
            "precondition_failed"
        );
        assert_eq!(attempts(&t), 0);
    }

    #[cfg(windows)]
    #[test]
    fn case_equivalent_tracked_destination_missing_from_worktree_is_rejected() {
        let f = Fixture::new();
        fs::write(f.root.join("README.md"), b"tracked").unwrap();
        run(&f.git, &f.root, &["add", "README.md"]);
        run(
            &f.git,
            &f.root,
            &["commit", "--quiet", "-m", "tracked-destination"],
        );
        fs::remove_file(f.root.join("README.md")).unwrap();
        let index_before = fs::read(f.root.join(".git/index")).unwrap();
        let head_before = output(&f.git, &f.root, &["rev-parse", "HEAD"]);
        let t = RepositoryFileRenameTool::new(&f.git, &f.root).unwrap();

        assert_eq!(
            execute(&t, request("old.txt", "readme.md"))["status"],
            "precondition_failed"
        );
        assert_eq!(attempts(&t), 0);
        assert_eq!(fs::read(f.root.join("old.txt")).unwrap(), b"rename bytes");
        assert!(!f.root.join("readme.md").exists());
        assert_eq!(fs::read(f.root.join(".git/index")).unwrap(), index_before);
        assert_eq!(output(&f.git, &f.root, &["rev-parse", "HEAD"]), head_before);
    }

    #[cfg(windows)]
    #[test]
    fn case_equivalent_index_destination_collision_is_rejected_without_effect() {
        let f = Fixture::new();
        fs::write(f.root.join("IndexOnly.md"), b"index").unwrap();
        run(&f.git, &f.root, &["add", "IndexOnly.md"]);
        fs::remove_file(f.root.join("IndexOnly.md")).unwrap();
        let index_before = fs::read(f.root.join(".git/index")).unwrap();
        let t = RepositoryFileRenameTool::new(&f.git, &f.root).unwrap();

        assert_eq!(
            execute(&t, request("old.txt", "indexonly.md"))["status"],
            "precondition_failed"
        );
        assert_eq!(attempts(&t), 0);
        assert!(f.root.join("old.txt").exists());
        assert!(!f.root.join("indexonly.md").exists());
        assert_eq!(fs::read(f.root.join(".git/index")).unwrap(), index_before);
    }

    #[cfg(windows)]
    #[test]
    fn case_equivalent_intent_to_add_destination_collision_is_rejected_without_effect() {
        let f = Fixture::new();
        fs::write(f.root.join("IntentOnly.md"), b"intent").unwrap();
        run(&f.git, &f.root, &["add", "-N", "IntentOnly.md"]);
        fs::remove_file(f.root.join("IntentOnly.md")).unwrap();
        let t = RepositoryFileRenameTool::new(&f.git, &f.root).unwrap();

        assert_eq!(
            execute(&t, request("old.txt", "intentonly.md"))["status"],
            "precondition_failed"
        );
        assert_eq!(attempts(&t), 0);
        assert!(f.root.join("old.txt").exists());
        assert!(!f.root.join("intentonly.md").exists());
    }

    #[cfg(windows)]
    #[test]
    fn case_equivalent_conflict_destination_collision_is_rejected_without_effect() {
        let f = Fixture::new();
        let mut objects = Vec::new();
        for (name, bytes) in [
            ("one", b"one".as_slice()),
            ("two", b"two"),
            ("three", b"three"),
        ] {
            fs::write(f.root.join(name), bytes).unwrap();
            objects.push(
                String::from_utf8(output(&f.git, &f.root, &["hash-object", "-w", name]))
                    .unwrap()
                    .trim()
                    .to_owned(),
            );
        }
        let input = objects
            .iter()
            .enumerate()
            .map(|(index, object)| format!("100644 {object} {}\tConflict.md\n", index + 1))
            .collect::<String>();
        let mut child = Command::new(&f.git)
            .args(["update-index", "--index-info"])
            .current_dir(&f.root)
            .stdin(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
        assert!(child.wait().unwrap().success());
        let t = RepositoryFileRenameTool::new(&f.git, &f.root).unwrap();

        assert_eq!(
            execute(&t, request("old.txt", "conflict.md"))["status"],
            "precondition_failed"
        );
        assert_eq!(attempts(&t), 0);
        assert!(f.root.join("old.txt").exists());
        assert!(!f.root.join("conflict.md").exists());
    }

    #[cfg(not(windows))]
    #[test]
    fn distinct_case_spelling_remains_admissible_on_case_sensitive_platforms() {
        let f = Fixture::new();
        fs::write(f.root.join("README.md"), b"tracked").unwrap();
        run(&f.git, &f.root, &["add", "README.md"]);
        run(
            &f.git,
            &f.root,
            &["commit", "--quiet", "-m", "tracked-destination"],
        );
        fs::remove_file(f.root.join("README.md")).unwrap();
        let t = RepositoryFileRenameTool::new(&f.git, &f.root).unwrap();

        assert_eq!(
            execute(&t, request("old.txt", "readme.md"))["status"],
            "renamed_verified"
        );
        assert_eq!(attempts(&t), 1);
        assert!(!f.root.join("old.txt").exists());
        assert_eq!(fs::read(f.root.join("readme.md")).unwrap(), b"rename bytes");
    }

    #[test]
    fn malformed_git_candidate_records_fail_closed() {
        let valid_object = b"0000000000000000000000000000000000000000";
        let destination = Path::new("target.md");
        assert!(parse_tree_candidate(b"100644 blob	target.md\0").is_err());
        assert!(parse_index_candidate(b"100644 0000\ttarget.md\0").is_err());
        assert!(
            parse_tree_candidate(
                [&b"100644 blob "[..], &valid_object[..], &b"\ttarget.md"[..],]
                    .concat()
                    .as_slice()
            )
            .is_ok()
        );
        assert!(
            git_index_candidates_conflict(
                b"100644 0000000000000000000000000000000000000000 0\ttarget.md",
                destination,
            )
            .is_err()
        );
        #[cfg(not(windows))]
        assert!(
            git_tree_candidates_conflict(
                b"100644 blob 0000000000000000000000000000000000000000\ttarget.md",
                destination,
            )
            .is_err()
        );
    }

    #[test]
    fn staged_or_intent_to_add_destination_collision_is_rejected() {
        for intent_to_add in [false, true] {
            let f = Fixture::new();
            if intent_to_add {
                fs::write(f.root.join("collision.txt"), b"intent").unwrap();
                run(&f.git, &f.root, &["add", "-N", "collision.txt"]);
                fs::remove_file(f.root.join("collision.txt")).unwrap();
            } else {
                fs::write(f.root.join("collision.txt"), b"staged").unwrap();
                run(&f.git, &f.root, &["add", "collision.txt"]);
                fs::remove_file(f.root.join("collision.txt")).unwrap();
            }
            let t = RepositoryFileRenameTool::new(&f.git, &f.root).unwrap();
            assert_eq!(
                execute(&t, request("old.txt", "collision.txt"))["status"],
                "precondition_failed"
            );
            assert_eq!(attempts(&t), 0);
        }
    }

    #[test]
    fn source_staged_content_replacement_is_rejected_when_worktree_is_head_bytes() {
        let f = Fixture::new();
        fs::write(f.root.join("old.txt"), b"staged replacement").unwrap();
        run(&f.git, &f.root, &["add", "old.txt"]);
        fs::write(f.root.join("old.txt"), b"rename bytes").unwrap();
        let t = RepositoryFileRenameTool::new(&f.git, &f.root).unwrap();
        assert_eq!(
            execute(&t, request("old.txt", "renamed.txt"))["status"],
            "precondition_failed"
        );
        assert_eq!(attempts(&t), 0);
    }

    #[test]
    fn source_staged_mode_replacement_is_rejected() {
        let f = Fixture::new();
        run(&f.git, &f.root, &["update-index", "--chmod=+x", "old.txt"]);
        let t = RepositoryFileRenameTool::new(&f.git, &f.root).unwrap();
        assert_eq!(
            execute(&t, request("old.txt", "renamed.txt"))["status"],
            "precondition_failed"
        );
        assert_eq!(attempts(&t), 0);
    }

    #[test]
    fn source_conflict_stages_are_rejected() {
        let f = Fixture::new();
        let mut objects = Vec::new();
        for (name, bytes) in [
            ("one", b"one".as_slice()),
            ("two", b"two"),
            ("three", b"three"),
        ] {
            fs::write(f.root.join(name), bytes).unwrap();
            objects.push(
                String::from_utf8(output(&f.git, &f.root, &["hash-object", "-w", name]))
                    .unwrap()
                    .trim()
                    .to_owned(),
            );
        }
        run(
            &f.git,
            &f.root,
            &["update-index", "--force-remove", "old.txt"],
        );
        let input = objects
            .iter()
            .enumerate()
            .map(|(index, object)| format!("100644 {object} {}\told.txt\n", index + 1))
            .collect::<String>();
        let mut child = Command::new(&f.git)
            .args(["update-index", "--index-info"])
            .current_dir(&f.root)
            .stdin(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
        assert!(child.wait().unwrap().success());
        let t = RepositoryFileRenameTool::new(&f.git, &f.root).unwrap();
        assert_eq!(
            execute(&t, request("old.txt", "renamed.txt"))["status"],
            "precondition_failed"
        );
        assert_eq!(attempts(&t), 0);
    }

    #[test]
    fn active_git_operation_markers_are_rejected_without_effect() {
        for marker in [
            "MERGE_HEAD",
            "CHERRY_PICK_HEAD",
            "REVERT_HEAD",
            "REBASE_HEAD",
            "BISECT_LOG",
            "sequencer",
            "rebase-merge",
            "rebase-apply",
        ] {
            let f = Fixture::new();
            let path = f.root.join(".git").join(marker);
            if marker == "sequencer" || marker.starts_with("rebase-") {
                fs::create_dir(path).unwrap();
            } else {
                fs::write(path, b"marker").unwrap();
            }
            let t = RepositoryFileRenameTool::new(&f.git, &f.root).unwrap();
            assert_eq!(
                execute(&t, request("old.txt", "renamed.txt"))["status"],
                "precondition_failed",
                "marker {marker}"
            );
            assert_eq!(attempts(&t), 0, "marker {marker}");
            assert!(f.root.join("old.txt").exists());
            assert!(!f.root.join("renamed.txt").exists());
        }
    }

    #[test]
    fn source_and_destination_parent_replacement_are_rejected() {
        let source_fixture = Fixture::nested();
        let source_tool =
            RepositoryFileRenameTool::new(&source_fixture.git, &source_fixture.root).unwrap();
        source_tool
            .policy
            .test_hook
            .replace_source_parent
            .store(true, std::sync::atomic::Ordering::SeqCst);
        assert_eq!(
            execute(&source_tool, request("source/old.txt", "renamed.txt"))["status"],
            "precondition_failed"
        );
        assert_eq!(attempts(&source_tool), 0);

        let destination_fixture = Fixture::new();
        let destination_tool =
            RepositoryFileRenameTool::new(&destination_fixture.git, &destination_fixture.root)
                .unwrap();
        destination_tool
            .policy
            .test_hook
            .replace_destination_parent
            .store(true, std::sync::atomic::Ordering::SeqCst);
        assert_eq!(
            execute(&destination_tool, request("old.txt", "existing/moved.txt"),)["status"],
            "precondition_failed"
        );
        assert_eq!(attempts(&destination_tool), 0);
    }

    #[test]
    fn existing_nested_repository_destination_ancestry_is_rejected() {
        let f = Fixture::new();
        let nested = f.root.join("nested");
        fs::create_dir_all(nested.join("sub")).unwrap();
        run(&f.git, &nested, &["init", "--quiet"]);
        let t = RepositoryFileRenameTool::new(&f.git, &f.root).unwrap();
        assert_eq!(
            execute(&t, request("old.txt", "nested/sub/moved.txt"))["status"],
            "precondition_failed"
        );
        assert_eq!(attempts(&t), 0);
        assert!(f.root.join("old.txt").exists());
        assert!(!nested.join("sub/moved.txt").exists());
    }

    #[test]
    fn outer_tracked_source_inside_nested_repository_is_rejected() {
        let f = Fixture::nested();
        run(&f.git, &f.root.join("source"), &["init", "--quiet"]);
        let t = RepositoryFileRenameTool::new(&f.git, &f.root).unwrap();
        assert_eq!(
            execute(&t, request("source/old.txt", "moved.txt"))["status"],
            "precondition_failed"
        );
        assert_eq!(attempts(&t), 0);
        assert!(f.root.join("source/old.txt").exists());
        assert!(!f.root.join("moved.txt").exists());
    }

    #[test]
    fn source_and_destination_inside_nested_repository_are_rejected() {
        let f = Fixture::nested();
        fs::create_dir_all(f.root.join("source/sub")).unwrap();
        run(&f.git, &f.root.join("source"), &["init", "--quiet"]);
        let t = RepositoryFileRenameTool::new(&f.git, &f.root).unwrap();

        assert_eq!(
            execute(&t, request("source/old.txt", "source/sub/moved.txt"))["status"],
            "precondition_failed"
        );
        assert_eq!(attempts(&t), 0);
        assert!(f.root.join("source/old.txt").exists());
        assert!(!f.root.join("source/sub/moved.txt").exists());
    }

    #[test]
    fn nested_repository_boundary_appearing_between_capture_and_revalidation_is_rejected() {
        let f = Fixture::new();
        let t = RepositoryFileRenameTool::new(&f.git, &f.root).unwrap();
        t.policy
            .test_hook
            .create_nested_boundary
            .store(true, std::sync::atomic::Ordering::SeqCst);
        assert_eq!(
            execute(&t, request("old.txt", "existing/moved.txt"))["status"],
            "precondition_failed"
        );
        assert_eq!(attempts(&t), 0);
        assert!(f.root.join("old.txt").exists());
        assert!(!f.root.join("existing/moved.txt").exists());
        assert!(f.root.join("existing/.git").is_dir());
    }

    #[test]
    fn nested_repository_boundary_after_possible_effect_is_uncertain_without_replay() {
        let f = Fixture::new();
        let t = RepositoryFileRenameTool::new(&f.git, &f.root).unwrap();
        t.policy
            .test_hook
            .create_nested_boundary_after_attempt
            .store(true, std::sync::atomic::Ordering::SeqCst);
        assert_eq!(
            execute(&t, request("old.txt", "existing/moved.txt"))["status"],
            "uncertain"
        );
        assert_eq!(attempts(&t), 1);
        assert!(!f.root.join("old.txt").exists());
        assert!(f.root.join("existing/moved.txt").exists());
        assert!(f.root.join("existing/.git").is_dir());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_mountinfo_path_escapes_are_decoded_for_ancestry_proof() {
        assert_eq!(
            decode_mountinfo_path(r"/tmp/with\040space\134name").unwrap(),
            PathBuf::from("/tmp/with space\\name")
        );
    }

    #[test]
    fn destination_parent_replacement_after_effect_is_uncertain() {
        let f = Fixture::new();
        let t = RepositoryFileRenameTool::new(&f.git, &f.root).unwrap();
        t.policy
            .test_hook
            .replace_destination_parent_after_attempt
            .store(true, std::sync::atomic::Ordering::SeqCst);
        assert_eq!(
            execute(&t, request("old.txt", "existing/moved.txt"))["status"],
            "uncertain"
        );
        assert_eq!(attempts(&t), 1);
    }

    #[cfg(any(unix, windows))]
    #[test]
    fn destination_intermediate_redirect_is_rejected_before_native_attempt() {
        let f = Fixture::new();
        let redirect_target = f.root.join("redirect-target");
        fs::create_dir_all(redirect_target.join("sub")).unwrap();
        create_directory_redirect(&f.root.join("redirect"), &redirect_target);
        let t = RepositoryFileRenameTool::new(&f.git, &f.root).unwrap();
        assert_eq!(
            execute(&t, request("old.txt", "redirect/sub/moved.txt"))["status"],
            "precondition_failed"
        );
        assert_eq!(attempts(&t), 0);
        assert!(f.root.join("old.txt").exists());
        assert!(!redirect_target.join("sub/moved.txt").exists());
    }

    #[cfg(any(unix, windows))]
    #[test]
    fn destination_parent_redirect_after_effect_is_uncertain() {
        let f = Fixture::new();
        let t = RepositoryFileRenameTool::new(&f.git, &f.root).unwrap();
        t.policy
            .test_hook
            .redirect_destination_parent_after_attempt
            .store(true, std::sync::atomic::Ordering::SeqCst);
        assert_eq!(
            execute(&t, request("old.txt", "existing/moved.txt"))["status"],
            "uncertain"
        );
        assert_eq!(attempts(&t), 1);
        assert!(!f.root.join("old.txt").exists());
        assert!(
            f.root
                .join("existing.rah-rename-redirect-target/moved.txt")
                .exists()
        );
    }

    #[test]
    fn same_volume_mismatch_fails_closed_before_the_native_attempt() {
        let f = Fixture::new();
        let t = RepositoryFileRenameTool::new(&f.git, &f.root).unwrap();
        t.policy
            .force_same_volume_mismatch
            .store(true, std::sync::atomic::Ordering::SeqCst);
        assert_eq!(
            execute(&t, request("old.txt", "renamed.txt"))["status"],
            "precondition_failed"
        );
        assert_eq!(attempts(&t), 0);
    }

    #[test]
    fn native_failure_with_identity_equal_preimage_is_known_no_effect() {
        let f = Fixture::new();
        let t = RepositoryFileRenameTool::new(&f.git, &f.root).unwrap();
        t.policy
            .force_native_failure
            .store(true, std::sync::atomic::Ordering::SeqCst);
        assert_eq!(
            execute(&t, request("old.txt", "renamed.txt"))["status"],
            "known_no_effect"
        );
        assert_eq!(attempts(&t), 1);
        assert!(f.root.join("old.txt").exists());
        assert!(!f.root.join("renamed.txt").exists());
    }

    #[test]
    fn same_byte_source_identity_replacement_is_uncertain_after_failure() {
        let f = Fixture::new();
        let t = RepositoryFileRenameTool::new(&f.git, &f.root).unwrap();
        t.policy
            .force_native_failure
            .store(true, std::sync::atomic::Ordering::SeqCst);
        t.policy
            .test_hook
            .replace_source_after_attempt
            .store(true, std::sync::atomic::Ordering::SeqCst);
        assert_eq!(
            execute(&t, request("old.txt", "renamed.txt"))["status"],
            "uncertain"
        );
        assert_eq!(attempts(&t), 1);
        assert_eq!(fs::read(f.root.join("old.txt")).unwrap(), b"rename bytes");
    }

    #[test]
    fn generic_absence_observation_failure_is_uncertain() {
        let f = Fixture::new();
        let t = RepositoryFileRenameTool::new(&f.git, &f.root).unwrap();
        t.policy
            .force_observation_unknown
            .store(true, std::sync::atomic::Ordering::SeqCst);
        assert_eq!(
            execute(&t, request("old.txt", "renamed.txt"))["status"],
            "uncertain"
        );
        assert_eq!(attempts(&t), 1);
    }

    #[test]
    fn destination_identity_mismatch_is_uncertain_after_possible_effect() {
        let f = Fixture::new();
        let t = RepositoryFileRenameTool::new(&f.git, &f.root).unwrap();
        t.policy
            .test_hook
            .replace_destination_after_attempt
            .store(true, std::sync::atomic::Ordering::SeqCst);
        assert_eq!(
            execute(&t, request("old.txt", "renamed.txt"))["status"],
            "uncertain"
        );
        assert_eq!(attempts(&t), 1);
        assert_eq!(
            fs::read(f.root.join("renamed.txt")).unwrap(),
            b"rename bytes"
        );
    }

    #[test]
    fn same_name_source_replacement_is_uncertain_after_possible_effect() {
        let f = Fixture::new();
        let t = RepositoryFileRenameTool::new(&f.git, &f.root).unwrap();
        t.policy
            .test_hook
            .replace_source_after_attempt
            .store(true, std::sync::atomic::Ordering::SeqCst);
        assert_eq!(
            execute(&t, request("old.txt", "renamed.txt"))["status"],
            "uncertain"
        );
        assert_eq!(attempts(&t), 1);
        assert_eq!(fs::read(f.root.join("old.txt")).unwrap(), b"rename bytes");
    }

    #[cfg(unix)]
    #[test]
    fn destination_symlink_substitution_is_uncertain_after_possible_effect() {
        let f = Fixture::new();
        let t = RepositoryFileRenameTool::new(&f.git, &f.root).unwrap();
        t.policy
            .test_hook
            .replace_destination_with_symlink_after_attempt
            .store(true, std::sync::atomic::Ordering::SeqCst);
        assert_eq!(
            execute(&t, request("old.txt", "renamed.txt"))["status"],
            "uncertain"
        );
        assert_eq!(attempts(&t), 1);
    }

    #[cfg(windows)]
    #[test]
    fn windows_case_only_rename_is_rejected_without_effect() {
        let f = Fixture::with_source("Foo.rs");
        let t = RepositoryFileRenameTool::new(&f.git, &f.root).unwrap();
        assert_eq!(
            execute(&t, request("Foo.rs", "foo.rs"))["status"],
            "precondition_failed"
        );
        assert_eq!(attempts(&t), 0);
    }

    #[cfg(windows)]
    #[test]
    fn windows_post_effect_alias_ambiguity_is_uncertain_without_replay() {
        let f = Fixture::with_source("Foo.rs");
        let t = RepositoryFileRenameTool::new(&f.git, &f.root).unwrap();
        t.policy
            .force_alias_ambiguity
            .store(true, std::sync::atomic::Ordering::SeqCst);
        assert_eq!(
            execute(&t, request("Foo.rs", "existing/Target.rs"))["status"],
            "uncertain"
        );
        assert_eq!(attempts(&t), 1);
        assert!(!f.root.join("Foo.rs").exists());
        assert_eq!(
            fs::read(f.root.join("existing/Target.rs")).unwrap(),
            b"rename bytes"
        );
    }

    #[test]
    fn malformed_and_colliding_requests_make_no_attempt() {
        let f = Fixture::new();
        let t = RepositoryFileRenameTool::new(&f.git, &f.root).unwrap();
        assert_eq!(
            execute(&t, json!({"source_path":"old.txt"}))["status"],
            "invalid_input"
        );
        fs::write(f.root.join("existing.txt"), b"x").unwrap();
        assert_eq!(
            execute(&t, request("old.txt", "existing.txt"))["status"],
            "precondition_failed"
        );
        assert_eq!(
            t.policy
                .rename_attempts
                .load(std::sync::atomic::Ordering::SeqCst),
            0
        );
    }
    #[test]
    fn missing_parent_directory_and_directory_collision_are_refused() {
        let f = Fixture::new();
        let t = RepositoryFileRenameTool::new(&f.git, &f.root).unwrap();
        assert_eq!(
            execute(&t, request("old.txt", "missing/moved.txt"))["status"],
            "precondition_failed"
        );
        assert_eq!(
            execute(&t, request("old.txt", "existing"))["status"],
            "precondition_failed"
        );
        assert_eq!(
            t.policy
                .rename_attempts
                .load(std::sync::atomic::Ordering::SeqCst),
            0
        );
    }
    #[test]
    fn stale_preconditions_and_dirty_source_are_effect_free() {
        let f = Fixture::new();
        let t = RepositoryFileRenameTool::new(&f.git, &f.root).unwrap();
        let mut stale = request("old.txt", "renamed.txt");
        stale["expected_source_file_sha256"] = json!(sha256(b"stale"));
        assert_eq!(execute(&t, stale)["status"], "precondition_failed");
        fs::write(f.root.join("old.txt"), b"dirty").unwrap();
        assert_eq!(
            execute(&t, request("old.txt", "renamed.txt"))["status"],
            "precondition_failed"
        );
        assert_eq!(
            t.policy
                .rename_attempts
                .load(std::sync::atomic::Ordering::SeqCst),
            0
        );
    }
    #[test]
    fn destination_race_is_refused_after_capture_without_attempt() {
        let f = Fixture::new();
        let t = RepositoryFileRenameTool::new(&f.git, &f.root).unwrap();
        t.policy
            .test_hook
            .create_destination
            .store(true, std::sync::atomic::Ordering::SeqCst);
        assert_eq!(
            execute(&t, request("old.txt", "race.txt"))["status"],
            "precondition_failed"
        );
        assert_eq!(fs::read(f.root.join("race.txt")).unwrap(), b"external");
        assert_eq!(
            t.policy
                .rename_attempts
                .load(std::sync::atomic::Ordering::SeqCst),
            0
        );
    }
    #[test]
    fn path_aliases_are_invalid_input() {
        let f = Fixture::new();
        let t = RepositoryFileRenameTool::new(&f.git, &f.root).unwrap();
        for (source, destination) in [
            ("../old.txt", "new.txt"),
            ("old.txt", ".git/x"),
            ("old.txt", "a/*"),
            ("old.txt", "CON"),
        ] {
            assert_eq!(
                execute(&t, request(source, destination))["status"],
                "invalid_input"
            );
        }
        assert_eq!(
            t.policy
                .rename_attempts
                .load(std::sync::atomic::Ordering::SeqCst),
            0
        );
    }
    #[test]
    fn changed_source_after_capture_is_refused_without_attempt() {
        let f = Fixture::new();
        let t = RepositoryFileRenameTool::new(&f.git, &f.root).unwrap();
        t.policy
            .test_hook
            .modify_source
            .store(true, std::sync::atomic::Ordering::SeqCst);
        assert_eq!(
            execute(&t, request("old.txt", "renamed.txt"))["status"],
            "precondition_failed"
        );
        assert_eq!(
            t.policy
                .rename_attempts
                .load(std::sync::atomic::Ordering::SeqCst),
            0
        );
    }
    #[test]
    fn uncertain_post_observation_is_not_retried() {
        let f = Fixture::new();
        let t = RepositoryFileRenameTool::new(&f.git, &f.root).unwrap();
        t.policy
            .force_uncertain
            .store(true, std::sync::atomic::Ordering::SeqCst);
        assert_eq!(
            execute(&t, request("old.txt", "renamed.txt"))["status"],
            "uncertain"
        );
        assert_eq!(
            t.policy
                .rename_attempts
                .load(std::sync::atomic::Ordering::SeqCst),
            1
        );
    }
}
