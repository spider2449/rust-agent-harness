//! Host-authorized deletion of one clean HEAD-tracked repository file.

use std::{
    fs,
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
    host_execute::paths_equivalent,
    repository_boundary::RepositoryNestedBoundaryPolicy,
    repository_worktree_patch::{
        FileIdentity, parse_logical_path, reject_link_or_reparse, reject_reparse_ancestry,
        reject_unsupported_file_attributes, validate_directory_path, validate_existing_target,
    },
};

/// Stable name for the bounded repository file-deletion capability.
pub const REPOSITORY_DELETE_FILE_TOOL_NAME: &str = "repo.delete-file";
const MAX_PATH_BYTES: usize = 1024;
const MAX_FILE_BYTES: usize = 1024 * 1024;
const MAX_PREPARATION_REQUEST_BYTES: usize = 8192;
const MAX_REVIEWED_FILE_BYTES: usize = 65536;
const MAX_SERIALIZED_REVIEW_BYTES: usize = 262144;
const MAX_PREPARED_REPRESENTATION_BYTES: usize = 524288;

/// Typed host input for reviewed deletion preparation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepositoryDeleteFilePreparationRequest {
    /// Validated repository-relative logical path.
    pub path: String,
}

/// Sanitized failure classes for reviewed deletion preparation.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum RepositoryDeleteFilePreparationError {
    /// The closed request violated an input bound or rule.
    #[error("invalid repository delete-file preparation input: {reason}")]
    InvalidInput { reason: &'static str },
    /// The repository or requested target is not admissible.
    #[error("repository delete-file preparation precondition failed: {reason}")]
    PreconditionFailed { reason: &'static str },
    /// The complete review or retained preparation cannot fit its bound.
    #[error("repository delete-file review is too large")]
    ReviewTooLarge,
    /// The retained preparation no longer matches current host state.
    #[error("repository delete-file preparation is stale")]
    Stale,
}

/// BOM state of the exact protected source bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RepositoryDeleteFileBomState {
    Present,
    Absent,
}

/// Deterministic facts about the exact protected source bytes.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RepositoryDeleteFileContentFacts {
    /// Whether the source contains at least one carriage return byte.
    pub contains_cr: bool,
    /// Whether the source contains at least one line-feed byte.
    pub contains_lf: bool,
    /// Whether the source contains at least one CRLF pair.
    pub contains_crlf: bool,
    /// Number of carriage-return bytes.
    pub carriage_returns: usize,
    /// Number of line-feed bytes.
    pub line_feeds: usize,
    /// Number of CRLF pairs.
    pub crlf_pairs: usize,
    /// Whether the source ends in CR or LF.
    pub ends_with_newline: bool,
    /// Stable end-of-file marker.
    pub final_eof: &'static str,
    /// Whether the source contains a tab.
    pub contains_tab: bool,
    /// Whether the source contains a space immediately before a line ending or EOF.
    pub contains_trailing_space: bool,
    /// Whether control or Unicode format characters require escapes.
    pub contains_control_or_format_escape: bool,
    /// Number of control characters represented by escapes or short escapes.
    pub control_characters: usize,
    /// Number of Unicode format characters represented by code-point escapes.
    pub format_characters: usize,
    /// Whether the source is empty.
    pub empty: bool,
}

/// Complete, deterministic review for one exact tracked-file deletion.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RepositoryDeleteFileReview {
    operation: &'static str,
    target_count: usize,
    path: String,
    tracked_state: &'static str,
    file_mode: &'static str,
    file_intent: &'static str,
    preimage_encoding: &'static str,
    preimage: String,
    content_byte_length: usize,
    content_sha256: String,
    bom: RepositoryDeleteFileBomState,
    content_facts: RepositoryDeleteFileContentFacts,
    head_blob_relationship: &'static str,
    index_relationship: &'static str,
    expected_effect: &'static str,
    post_delete_git_meaning: &'static str,
    non_effects: Vec<&'static str>,
    warnings: Vec<&'static str>,
}

impl RepositoryDeleteFileReview {
    /// Returns the fixed operation name.
    #[must_use]
    pub fn operation(&self) -> &str {
        self.operation
    }

    /// Returns the fixed target count.
    #[must_use]
    pub fn target_count(&self) -> usize {
        self.target_count
    }

    /// Returns the validated logical path.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Returns the protected clean tracked-state description.
    #[must_use]
    pub fn tracked_state(&self) -> &str {
        self.tracked_state
    }

    /// Returns the derived HEAD file mode.
    #[must_use]
    pub fn file_mode(&self) -> &str {
        self.file_mode
    }

    /// Returns the deletion intent.
    #[must_use]
    pub fn file_intent(&self) -> &str {
        self.file_intent
    }

    /// Returns the complete deterministic escaped preimage.
    #[must_use]
    pub fn preimage(&self) -> &str {
        &self.preimage
    }

    /// Returns the complete escaped preimage under the content-oriented alias.
    #[must_use]
    pub fn content_escaped(&self) -> &str {
        &self.preimage
    }

    /// Returns the display encoding name.
    #[must_use]
    pub fn preimage_encoding(&self) -> &str {
        self.preimage_encoding
    }

    /// Returns the exact source byte length.
    #[must_use]
    pub fn content_byte_length(&self) -> usize {
        self.content_byte_length
    }

    /// Returns the SHA-256 of the exact source bytes.
    #[must_use]
    pub fn content_sha256(&self) -> &str {
        &self.content_sha256
    }

    /// Returns the exact source BOM state.
    #[must_use]
    pub fn bom(&self) -> RepositoryDeleteFileBomState {
        self.bom
    }

    /// Returns deterministic newline and content facts.
    #[must_use]
    pub fn content_facts(&self) -> &RepositoryDeleteFileContentFacts {
        &self.content_facts
    }

    /// Returns the protected HEAD/blob relationship.
    #[must_use]
    pub fn head_blob_relationship(&self) -> &str {
        self.head_blob_relationship
    }

    /// Returns the protected index relationship.
    #[must_use]
    pub fn index_relationship(&self) -> &str {
        self.index_relationship
    }

    /// Returns the expected worktree effect and Git meaning.
    #[must_use]
    pub fn expected_effect(&self) -> &str {
        self.expected_effect
    }

    /// Returns the post-delete Git meaning.
    #[must_use]
    pub fn post_delete_git_meaning(&self) -> &str {
        self.post_delete_git_meaning
    }

    /// Returns effects excluded by this review.
    #[must_use]
    pub fn non_effects(&self) -> &[&'static str] {
        &self.non_effects
    }

    /// Returns warnings that must accompany the destructive review.
    #[must_use]
    pub fn warnings(&self) -> &[&'static str] {
        &self.warnings
    }
}

/// Opaque bounded preparation retained by a trusted host.
pub struct RepositoryDeleteFilePreparation {
    preparer_identity: Uuid,
    tool_input: ToolInput,
    review: RepositoryDeleteFileReview,
    review_identity: String,
    source_sha256: String,
    source_byte_length: usize,
    root_identity: FileIdentity,
    dot_git_identity: FileIdentity,
    git_identity: FileIdentity,
    pre: Preimage,
}

impl RepositoryDeleteFilePreparation {
    /// Returns the exact host-reconstructed ordinary Tool input.
    #[must_use]
    pub fn tool_input(&self) -> &ToolInput {
        &self.tool_input
    }

    /// Returns the complete bounded destructive review.
    #[must_use]
    pub fn review(&self) -> &RepositoryDeleteFileReview {
        &self.review
    }
}

impl std::fmt::Debug for RepositoryDeleteFilePreparation {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RepositoryDeleteFilePreparation")
            .field("preparation", &"redacted")
            .finish_non_exhaustive()
    }
}

/// Host-bound, non-effectful preparation and revalidation for file deletion.
pub struct RepositoryDeleteFilePreparer {
    identity: Uuid,
    policy: RepositoryFileDeletionPolicy,
}

impl RepositoryDeleteFilePreparer {
    /// Creates a preparer bound to one host-selected repository and Git binary.
    pub fn new(
        git_executable: impl AsRef<Path>,
        repository_root: impl AsRef<Path>,
    ) -> Result<Self, ToolError> {
        Ok(Self {
            identity: Uuid::new_v4(),
            policy: RepositoryFileDeletionPolicy::new(
                git_executable.as_ref(),
                repository_root.as_ref(),
            )?,
        })
    }

    /// Captures a complete review without executing a Tool or deleting anything.
    pub async fn prepare(
        &self,
        request: RepositoryDeleteFilePreparationRequest,
    ) -> Result<RepositoryDeleteFilePreparation, RepositoryDeleteFilePreparationError> {
        let request = DeletePreparationRequest::parse(request)?;
        let _lease = self.policy.lease.lock().await;
        let pre = self
            .policy
            .capture_reviewed(&request.path, &request.logical_path)
            .await
            .map_err(
                |_| RepositoryDeleteFilePreparationError::PreconditionFailed {
                    reason: "repository_state",
                },
            )?;
        let tool_input = canonical_preparation_tool_input(&request, &pre);
        let review = build_delete_review(&request.logical_path, &pre)?;
        let source_sha256 = sha256(&pre.bytes);
        let source_byte_length = pre.bytes.len();
        let review_identity = compute_delete_review_identity(
            self.identity,
            &self.policy,
            &request.logical_path,
            &pre,
            &tool_input,
            &review,
        );
        let preparation = RepositoryDeleteFilePreparation {
            preparer_identity: self.identity,
            tool_input,
            review,
            review_identity,
            source_sha256,
            source_byte_length,
            root_identity: self.policy.root_identity.clone(),
            dot_git_identity: self.policy.dot_git_identity.clone(),
            git_identity: self.policy.git_identity.clone(),
            pre,
        };
        if serialized_preparation_size(&preparation) > MAX_PREPARED_REPRESENTATION_BYTES {
            return Err(RepositoryDeleteFilePreparationError::ReviewTooLarge);
        }
        Ok(preparation)
    }

    /// Revalidates the retained preparation without executing a Tool or deleting anything.
    pub async fn revalidate(
        &self,
        preparation: &RepositoryDeleteFilePreparation,
    ) -> Result<(), RepositoryDeleteFilePreparationError> {
        let _lease = self.policy.lease.lock().await;
        if preparation.preparer_identity != self.identity
            || preparation.root_identity != self.policy.root_identity
            || preparation.dot_git_identity != self.policy.dot_git_identity
            || preparation.git_identity != self.policy.git_identity
        {
            return Err(RepositoryDeleteFilePreparationError::Stale);
        }
        let request = DeleteRequest::parse(&preparation.tool_input)
            .map_err(|_| RepositoryDeleteFilePreparationError::Stale)?;
        if canonical_delete_tool_input(&request) != preparation.tool_input {
            return Err(RepositoryDeleteFilePreparationError::Stale);
        }
        let current = self
            .policy
            .capture_reviewed(&request.path, &request.logical_path)
            .await
            .map_err(|_| RepositoryDeleteFilePreparationError::Stale)?;
        if current != preparation.pre {
            return Err(RepositoryDeleteFilePreparationError::Stale);
        }
        let review = build_delete_review(&request.logical_path, &current)
            .map_err(|_| RepositoryDeleteFilePreparationError::Stale)?;
        if review != preparation.review
            || sha256(&current.bytes) != preparation.source_sha256
            || current.bytes.len() != preparation.source_byte_length
        {
            return Err(RepositoryDeleteFilePreparationError::Stale);
        }
        let identity = compute_delete_review_identity(
            self.identity,
            &self.policy,
            &request.logical_path,
            &current,
            &preparation.tool_input,
            &review,
        );
        if identity != preparation.review_identity {
            return Err(RepositoryDeleteFilePreparationError::Stale);
        }
        Ok(())
    }

    /// Independently proves that the reviewed target remains the exact protected preimage.
    pub async fn prove_known_no_effect(
        &self,
        preparation: &RepositoryDeleteFilePreparation,
    ) -> bool {
        let _lease = self.policy.lease.lock().await;
        if !self.preparation_resources_match(preparation) {
            return false;
        }
        let Ok(request) = DeleteRequest::parse(&preparation.tool_input) else {
            return false;
        };
        let Ok(current) = self
            .policy
            .capture_reviewed(&request.path, &request.logical_path)
            .await
        else {
            return false;
        };
        current == preparation.pre
            && current.identity.same_object(&preparation.pre.identity)
            && current.identity.link_count == preparation.pre.identity.link_count
    }

    /// Independently proves one immediate confirmed-absence postcondition after deletion.
    pub async fn prove_deleted_verified(
        &self,
        preparation: &RepositoryDeleteFilePreparation,
    ) -> bool {
        let _lease = self.policy.lease.lock().await;
        if !self.preparation_resources_match(preparation) || self.policy.repository_ok().is_err() {
            return false;
        }
        let Some(parent) = preparation.pre.path.parent() else {
            return false;
        };
        if validate_directory_path(&self.policy.root, parent, "target parent").is_err()
            || !matches!(
                preparation.pre.parent_identity.as_ref(),
                Some(expected)
                    if FileIdentity::capture(parent)
                        .map(|current| current == *expected && current.same_object(expected))
                        .unwrap_or(false)
            )
        {
            return false;
        }
        if !confirmed_target_absence(&preparation.pre.path)
            || same_name_entry_exists(parent, preparation.pre.path.file_name())
        {
            return false;
        }
        if fs::read(self.policy.root.join(".git/index")).ok() != Some(preparation.pre.index.clone())
        {
            return false;
        }
        let Ok(current_git) = self
            .policy
            .git_state(Path::new(&preparation.pre.git_path), false)
            .await
        else {
            return false;
        };
        current_git == preparation.pre.git
    }

    fn preparation_resources_match(&self, preparation: &RepositoryDeleteFilePreparation) -> bool {
        preparation.preparer_identity == self.identity
            && preparation.root_identity == self.policy.root_identity
            && preparation.dot_git_identity == self.policy.dot_git_identity
            && preparation.git_identity == self.policy.git_identity
    }
}

/// Host-constructed authority for exactly one protected repository file.
pub struct RepositoryFileDeletionTool {
    policy: Arc<RepositoryFileDeletionPolicy>,
}

/// Opaque host-created authority for one bounded repository file deletion.
#[derive(Clone)]
pub struct RepositoryFileDeletionAuthority {
    policy: Arc<RepositoryFileDeletionPolicy>,
}

impl RepositoryFileDeletionAuthority {
    /// Constructs deletion authority from host-selected repository identities.
    pub fn new(
        git_executable: impl AsRef<Path>,
        repository_root: impl AsRef<Path>,
    ) -> Result<Self, ToolError> {
        Ok(Self {
            policy: Arc::new(RepositoryFileDeletionPolicy::new(
                git_executable.as_ref(),
                repository_root.as_ref(),
            )?),
        })
    }

    /// Tests whether this host authority is bound to the selected resources.
    pub fn matches_resources(&self, git_executable: &Path, repository_root: &Path) -> bool {
        self.policy
            .matches_resources(git_executable, repository_root)
    }
}

impl RepositoryFileDeletionTool {
    /// Constructs deletion authority from host-selected identities.
    pub fn new(
        git_executable: impl AsRef<Path>,
        repository_root: impl AsRef<Path>,
    ) -> Result<Self, ToolError> {
        Ok(Self::from_authority(RepositoryFileDeletionAuthority::new(
            git_executable,
            repository_root,
        )?))
    }

    /// Constructs a tool from an authority already created by the trusted host.
    #[must_use]
    pub fn from_authority(authority: RepositoryFileDeletionAuthority) -> Self {
        Self {
            policy: authority.policy,
        }
    }
}

#[async_trait]
impl Tool for RepositoryFileDeletionTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: ToolName::new(REPOSITORY_DELETE_FILE_TOOL_NAME),
            description: "Deletes one clean HEAD-tracked repository-relative file.".to_owned(),
            input_schema: json!({"type":"object","properties":{"path":{"type":"string","minLength":1,"maxLength":MAX_PATH_BYTES},"expected_file_sha256":{"type":"string","pattern":"^[0-9a-f]{64}$"},"expected_file_byte_length":{"type":"integer","minimum":0,"maximum":MAX_FILE_BYTES}},"required":["path","expected_file_sha256","expected_file_byte_length"],"additionalProperties":false}),
            permission: PermissionLevel::Execute,
        }
    }

    async fn execute(
        &self,
        input: ToolInput,
        _context: ToolContext,
    ) -> Result<ToolOutput, ToolError> {
        #[cfg(feature = "live-test-support")]
        live_test_delete_file_tool_executions::record(&self.policy.root);
        let request = match DeleteRequest::parse(&input) {
            Ok(request) => request,
            Err(()) => return Ok(result("invalid_input", None, false)),
        };
        let _lease = self.policy.lease.lock().await;
        let pre = match self.policy.capture(&request).await {
            Ok(pre) => pre,
            Err(()) => return Ok(result("precondition_failed", Some(&request.path), false)),
        };
        #[cfg(test)]
        if self
            .policy
            .test_modify_before_delete
            .swap(false, std::sync::atomic::Ordering::SeqCst)
        {
            fs::write(&pre.path, b"changed externally").expect("test mutation should work");
        }
        if self.policy.revalidate(&request, &pre).await.is_err() {
            return Ok(result("precondition_failed", Some(&request.path), false));
        }
        #[cfg(test)]
        self.policy
            .delete_attempts
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        #[cfg(feature = "live-test-support")]
        live_test_delete_file_native_attempts::record(&self.policy.root);
        let native = delete_once(&pre.path);
        if native.is_err() {
            let intact = self.policy.intact(&request, &pre).await;
            return Ok(result(
                if intact {
                    "known_no_effect"
                } else {
                    "uncertain"
                },
                Some(&request.path),
                !intact,
            ));
        }
        #[cfg(test)]
        if self
            .policy
            .force_uncertain
            .swap(false, std::sync::atomic::Ordering::SeqCst)
        {
            return Ok(result("uncertain", None, true));
        }
        if self.policy.deleted_verified(&pre).await.is_ok() {
            Ok(result("deleted_verified", Some(&request.path), false))
        } else {
            Ok(result("uncertain", None, true))
        }
    }
}

#[cfg(feature = "live-test-support")]
pub mod live_test_delete_file_tool_executions {
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
pub mod live_test_delete_file_native_attempts {
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

/// Separate host-owned deletion policy; no other mutation policy grants it.
struct RepositoryFileDeletionPolicy {
    git: PathBuf,
    root: PathBuf,
    root_identity: FileIdentity,
    git_identity: FileIdentity,
    dot_git_identity: FileIdentity,
    boundary: RepositoryNestedBoundaryPolicy,
    lease: Arc<AsyncMutex<()>>,
    #[cfg(test)]
    test_modify_before_delete: std::sync::atomic::AtomicBool,
    #[cfg(test)]
    force_uncertain: std::sync::atomic::AtomicBool,
    #[cfg(test)]
    delete_attempts: std::sync::atomic::AtomicUsize,
}

impl RepositoryFileDeletionPolicy {
    fn matches_resources(&self, git: &Path, root: &Path) -> bool {
        fs::canonicalize(git).ok().as_deref() == Some(self.git.as_path())
            && fs::canonicalize(root).ok().as_deref() == Some(self.root.as_path())
    }

    fn new(git: &Path, root: &Path) -> Result<Self, ToolError> {
        if !root.is_absolute() || !git.is_absolute() {
            return Err(policy_error("host identities must be absolute"));
        }
        reject_reparse_ancestry(root, "repository root")?;
        let root = fs::canonicalize(root).map_err(fs_error)?;
        validate_directory_path(&root, &root, "repository root")?;
        let dot_git = root.join(".git");
        reject_link_or_reparse(&dot_git, "repository metadata")?;
        if !fs::metadata(&dot_git).map_err(fs_error)?.is_dir() {
            return Err(policy_error("linked worktrees are unsupported"));
        }
        let git = fs::canonicalize(git).map_err(fs_error)?;
        if !fs::metadata(&git).map_err(fs_error)?.is_file() {
            return Err(policy_error("Git identity is invalid"));
        }
        reject_link_or_reparse(&git, "Git executable")?;
        let lease = crate::git_stage::repository_lease(&root);
        Ok(Self {
            root_identity: FileIdentity::capture(&root)?,
            git_identity: FileIdentity::capture(&git)?,
            dot_git_identity: FileIdentity::capture(&dot_git)?,
            boundary: RepositoryNestedBoundaryPolicy::new(&root),
            git,
            root,
            lease,
            #[cfg(test)]
            test_modify_before_delete: Default::default(),
            #[cfg(test)]
            force_uncertain: Default::default(),
            #[cfg(test)]
            delete_attempts: Default::default(),
        })
    }

    async fn capture(&self, request: &DeleteRequest) -> Result<Preimage, ()> {
        let pre = self
            .capture_path(&request.path, &request.logical_path, false)
            .await?;
        if pre.bytes.len() > MAX_FILE_BYTES
            || pre.bytes.len() != request.length
            || sha256(&pre.bytes) != request.sha256
        {
            return Err(());
        }
        Ok(pre)
    }

    async fn capture_reviewed(&self, path: &Path, logical_path: &str) -> Result<Preimage, ()> {
        let pre = self.capture_path(path, logical_path, true).await?;
        if pre.bytes.len() > MAX_REVIEWED_FILE_BYTES
            || pre.bytes.contains(&0)
            || std::str::from_utf8(&pre.bytes).is_err()
            || !reviewed_mode_matches(&pre.path, &pre.git.head_entry)
        {
            return Err(());
        }
        Ok(pre)
    }

    async fn capture_path(
        &self,
        path: &Path,
        logical_path: &str,
        reviewed: bool,
    ) -> Result<Preimage, ()> {
        self.repository_ok().map_err(|_| ())?;
        let path = validate_existing_target(&self.root, path).map_err(|_| ())?;
        self.boundary.validate_existing(&path).map_err(|_| ())?;
        let metadata = fs::metadata(&path).map_err(|_| ())?;
        reject_unsupported_file_attributes(&metadata).map_err(|_| ())?;
        let identity = FileIdentity::capture(&path).map_err(|_| ())?;
        if identity.link_count != 1 {
            return Err(());
        }
        let bytes = fs::read(&path).map_err(|_| ())?;
        if bytes.len() > MAX_FILE_BYTES {
            return Err(());
        }
        let git = self.git_state(Path::new(logical_path), reviewed).await?;
        if git.blob != bytes {
            return Err(());
        }
        let parent_identity = if reviewed {
            let parent = path.parent().ok_or(())?;
            Some(FileIdentity::capture(parent).map_err(|_| ())?)
        } else {
            None
        };
        Ok(Preimage {
            path,
            git_path: logical_path.to_owned(),
            identity,
            bytes,
            git,
            index: fs::read(self.root.join(".git/index")).map_err(|_| ())?,
            parent_identity,
        })
    }

    async fn revalidate(&self, request: &DeleteRequest, pre: &Preimage) -> Result<(), ()> {
        let current = self.capture(request).await?;
        if !paths_equivalent(&current.path, &pre.path)
            || current.identity != pre.identity
            || current.git != pre.git
            || current.index != pre.index
            || current.bytes != pre.bytes
        {
            return Err(());
        }
        Ok(())
    }

    async fn intact(&self, request: &DeleteRequest, pre: &Preimage) -> bool {
        self.capture(request).await.is_ok() && fs::metadata(&pre.path).is_ok()
    }

    async fn deleted_verified(&self, pre: &Preimage) -> Result<(), ()> {
        self.repository_ok().map_err(|_| ())?;
        self.boundary
            .validate_existing(pre.path.parent().ok_or(())?)
            .map_err(|_| ())?;
        if fs::symlink_metadata(&pre.path).is_ok()
            || self.git_state(Path::new(&pre.git_path), false).await?.blob != pre.bytes
        {
            return Err(());
        }
        if fs::read(self.root.join(".git/index")).map_err(|_| ())? != pre.index
            || self
                .git_state(Path::new(&pre.git_path), false)
                .await?
                .fingerprint
                != pre.git.fingerprint
        {
            return Err(());
        }
        Ok(())
    }

    fn repository_ok(&self) -> Result<(), ToolError> {
        reject_reparse_ancestry(&self.root, "repository root")?;
        let current = fs::canonicalize(&self.root).map_err(fs_error)?;
        let dot_git = current.join(".git");
        if !paths_equivalent(&current, &self.root)
            || FileIdentity::capture(&current)? != self.root_identity
            || FileIdentity::capture(&dot_git)? != self.dot_git_identity
            || reject_link_or_reparse(&self.git, "Git executable").is_err()
            || fs::metadata(&self.git).map_err(fs_error)?.is_dir()
            || FileIdentity::capture(&self.git)? != self.git_identity
        {
            return Err(policy_error("repository identity changed"));
        }
        Ok(())
    }

    async fn git_state(&self, path: &Path, reviewed: bool) -> Result<GitState, ()> {
        if reviewed {
            self.require_supported_repository_state().await?;
        }
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
        if !tree.starts_with(b"100644 blob ") && !tree.starts_with(b"100755 blob ")
            || tree != index_to_tree(&index, &target).ok_or(())?
        {
            return Err(());
        }
        let tags = self
            .git_output(vec![
                "--literal-pathspecs",
                "ls-files",
                "-v",
                "-z",
                "--",
                &target,
            ])
            .await?;
        if !tags.starts_with(b"H ") {
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
        Ok(GitState {
            head: head.clone(),
            branch: branch.clone(),
            head_entry: tree.clone(),
            index_entry: index.clone(),
            blob,
            refs: refs.clone(),
            fingerprint: [head, branch, tree, index, refs].concat(),
        })
    }

    async fn require_supported_repository_state(&self) -> Result<(), ()> {
        let bare = self
            .git_output(vec!["rev-parse", "--is-bare-repository"])
            .await?;
        if bare != b"false\n" && bare != b"false\r\n" {
            return Err(());
        }
        if self
            .git_optional_output(vec!["config", "--bool", "core.sparseCheckout"])
            .await?
            .is_some_and(|output| output == b"true\n" || output == b"true\r\n")
        {
            return Err(());
        }
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
            if self.root.join(".git").join(marker).exists() {
                return Err(());
            }
        }
        Ok(())
    }

    async fn git_optional_output(&self, args: Vec<&str>) -> Result<Option<Vec<u8>>, ()> {
        let process = HostExecutionPolicy::new(
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
        if process.timed_out || process.overflow.is_some() {
            return Err(());
        }
        match process.exit_code {
            Some(0) => Ok(Some(process.stdout)),
            Some(1) => Ok(None),
            _ => Err(()),
        }
    }

    async fn git_output(&self, args: Vec<&str>) -> Result<Vec<u8>, ()> {
        let process = HostExecutionPolicy::new(
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
        if process.exit_code != Some(0) || process.timed_out || process.overflow.is_some() {
            Err(())
        } else {
            Ok(process.stdout)
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Preimage {
    path: PathBuf,
    git_path: String,
    identity: FileIdentity,
    bytes: Vec<u8>,
    git: GitState,
    index: Vec<u8>,
    parent_identity: Option<FileIdentity>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct GitState {
    head: Vec<u8>,
    branch: Vec<u8>,
    head_entry: Vec<u8>,
    index_entry: Vec<u8>,
    blob: Vec<u8>,
    refs: Vec<u8>,
    fingerprint: Vec<u8>,
}
struct DeleteRequest {
    path: PathBuf,
    logical_path: String,
    sha256: String,
    length: usize,
}
impl DeleteRequest {
    fn parse(input: &ToolInput) -> Result<Self, ()> {
        let object = input.0.as_object().ok_or(())?;
        if object.len() != 3 {
            return Err(());
        }
        let logical_path = object.get("path").and_then(Value::as_str).ok_or(())?;
        let path = parse_logical_path(logical_path, MAX_PATH_BYTES).map_err(|_| ())?;
        let sha256 = object
            .get("expected_file_sha256")
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
            .get("expected_file_byte_length")
            .and_then(Value::as_u64)
            .and_then(|v| usize::try_from(v).ok())
            .ok_or(())?;
        if length > MAX_FILE_BYTES {
            return Err(());
        }
        Ok(Self {
            path,
            logical_path: logical_path.to_owned(),
            sha256: sha256.to_owned(),
            length,
        })
    }
}

struct DeletePreparationRequest {
    path: PathBuf,
    logical_path: String,
}

impl DeletePreparationRequest {
    fn parse(
        request: RepositoryDeleteFilePreparationRequest,
    ) -> Result<Self, RepositoryDeleteFilePreparationError> {
        let input = json!({"path": request.path});
        if serde_json::to_vec(&input)
            .map(|bytes| bytes.len() > MAX_PREPARATION_REQUEST_BYTES)
            .unwrap_or(true)
        {
            return Err(RepositoryDeleteFilePreparationError::InvalidInput {
                reason: "request_bound",
            });
        }
        let logical_path = input.get("path").and_then(Value::as_str).ok_or(
            RepositoryDeleteFilePreparationError::InvalidInput {
                reason: "path_type",
            },
        )?;
        let path = parse_logical_path(logical_path, MAX_PATH_BYTES).map_err(|_| {
            RepositoryDeleteFilePreparationError::InvalidInput {
                reason: "path_or_bounds",
            }
        })?;
        Ok(Self {
            path,
            logical_path: logical_path.to_owned(),
        })
    }
}

fn canonical_preparation_tool_input(
    request: &DeletePreparationRequest,
    pre: &Preimage,
) -> ToolInput {
    ToolInput(json!({
        "path": request.logical_path,
        "expected_file_sha256": sha256(&pre.bytes),
        "expected_file_byte_length": pre.bytes.len(),
    }))
}

fn canonical_delete_tool_input(request: &DeleteRequest) -> ToolInput {
    ToolInput(json!({
        "path": request.logical_path,
        "expected_file_sha256": request.sha256,
        "expected_file_byte_length": request.length,
    }))
}

fn build_delete_review(
    logical_path: &str,
    pre: &Preimage,
) -> Result<RepositoryDeleteFileReview, RepositoryDeleteFilePreparationError> {
    let source = std::str::from_utf8(&pre.bytes).map_err(|_| {
        RepositoryDeleteFilePreparationError::PreconditionFailed {
            reason: "source_encoding",
        }
    })?;
    let file_mode = match pre.git.head_entry.get(..6) {
        Some(b"100644") => "100644",
        Some(b"100755") => "100755",
        _ => {
            return Err(RepositoryDeleteFilePreparationError::PreconditionFailed {
                reason: "file_mode",
            });
        }
    };
    let review = RepositoryDeleteFileReview {
        operation: REPOSITORY_DELETE_FILE_TOOL_NAME,
        target_count: 1,
        path: logical_path.to_owned(),
        tracked_state: "clean HEAD-tracked stage-0 regular file",
        file_mode,
        file_intent: "permanently remove this existing regular file",
        preimage_encoding: "complete_utf8_escaped",
        preimage: escape_delete_review_text(source),
        content_byte_length: pre.bytes.len(),
        content_sha256: sha256(&pre.bytes),
        bom: if pre.bytes.starts_with(b"\xef\xbb\xbf") {
            RepositoryDeleteFileBomState::Present
        } else {
            RepositoryDeleteFileBomState::Absent
        },
        content_facts: delete_content_facts(source),
        head_blob_relationship: "worktree bytes equal current HEAD blob",
        index_relationship: "exact stage-0 index entry equals HEAD tree entry and worktree",
        expected_effect: "one worktree file becomes absent; one unstaged deletion",
        post_delete_git_meaning: "index, HEAD, branch, refs, and history remain unchanged",
        non_effects: vec![
            "not Stage",
            "not Unstage",
            "not Commit",
            "does not modify the index",
            "does not modify HEAD, refs, or history",
            "does not rename or move",
            "does not restore or clean up",
            "does not delete another path",
        ],
        warnings: vec![
            "permanently removes the selected worktree file",
            "no Trash or Recycle Bin guarantee",
            "no backup",
            "no restore",
            "no rollback",
            "no retry or replay",
            "uncertainty may require manual inspection",
            "timeout, cancellation, or disconnect after a possible effect is not rollback",
        ],
    };
    if serde_json::to_vec(&review)
        .map(|serialized| serialized.len() <= MAX_SERIALIZED_REVIEW_BYTES)
        .unwrap_or(false)
    {
        Ok(review)
    } else {
        Err(RepositoryDeleteFilePreparationError::ReviewTooLarge)
    }
}

fn delete_content_facts(source: &str) -> RepositoryDeleteFileContentFacts {
    let bytes = source.as_bytes();
    let carriage_returns = bytes.iter().filter(|byte| **byte == b'\r').count();
    let line_feeds = bytes.iter().filter(|byte| **byte == b'\n').count();
    let crlf_pairs = bytes.windows(2).filter(|pair| *pair == b"\r\n").count();
    let contains_trailing_space = bytes.iter().enumerate().any(|(index, byte)| {
        *byte == b' ' && (index + 1 == bytes.len() || matches!(bytes[index + 1], b'\r' | b'\n'))
    });
    let contains_control_or_format_escape = source
        .chars()
        .any(|character| character.is_control() || delete_is_format_character(character));
    let control_characters = source
        .chars()
        .filter(|character| character.is_control())
        .count();
    let format_characters = source
        .chars()
        .filter(|character| delete_is_format_character(*character))
        .count();
    let ends_with_newline = bytes
        .last()
        .is_some_and(|byte| matches!(byte, b'\r' | b'\n'));
    RepositoryDeleteFileContentFacts {
        contains_cr: carriage_returns != 0,
        contains_lf: line_feeds != 0,
        contains_crlf: crlf_pairs != 0,
        carriage_returns,
        line_feeds,
        crlf_pairs,
        ends_with_newline,
        final_eof: if ends_with_newline {
            "final_newline"
        } else {
            "no_final_newline"
        },
        contains_tab: bytes.contains(&b'\t'),
        contains_trailing_space,
        contains_control_or_format_escape,
        control_characters,
        format_characters,
        empty: bytes.is_empty(),
    }
}

fn reviewed_mode_matches(path: &Path, head_entry: &[u8]) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let executable = head_entry.starts_with(b"100755");
        let mode = match fs::metadata(path) {
            Ok(metadata) => metadata.permissions().mode() & 0o111 != 0,
            Err(_) => return false,
        };
        mode == executable
    }
    #[cfg(not(unix))]
    {
        let _ = (path, head_entry);
        true
    }
}

fn escape_delete_review_text(source: &str) -> String {
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
                if character.is_control()
                    || character == '\u{7f}'
                    || delete_is_format_character(character)
                    || !character.is_ascii() =>
            {
                let _ = write!(escaped, "\\u{{{:x}}}", character as u32);
            }
            character => escaped.push(character),
        }
    }
    escaped
}

fn delete_is_format_character(character: char) -> bool {
    matches!(
        character as u32,
        0x00ad
            | 0x0600..=0x0605
            | 0x061c
            | 0x06dd
            | 0x070f
            | 0x0890..=0x0891
            | 0x08e2
            | 0x180e
            | 0x200b..=0x200f
            | 0x202a..=0x202e
            | 0x2060..=0x2064
            | 0x2066..=0x206f
            | 0xfeff
            | 0xfff9..=0xfffb
            | 0x110bd
            | 0x110cd
            | 0x13430..=0x1343f
            | 0x1bca0..=0x1bca3
            | 0x1d173..=0x1d17a
            | 0xe0001
            | 0xe0020..=0xe007f
    )
}

fn compute_delete_review_identity(
    preparer_identity: Uuid,
    policy: &RepositoryFileDeletionPolicy,
    logical_path: &str,
    pre: &Preimage,
    tool_input: &ToolInput,
    review: &RepositoryDeleteFileReview,
) -> String {
    let mut digest = Sha256::new();
    digest.update(b"rah-repository-delete-file-preparation-v1\0");
    digest.update(preparer_identity.as_bytes());
    update_serialized(&mut digest, tool_input);
    update_serialized(&mut digest, review);
    digest.update(logical_path.as_bytes());
    digest.update(&pre.bytes);
    update_identity(&mut digest, &policy.root_identity);
    update_identity(&mut digest, &policy.dot_git_identity);
    update_identity(&mut digest, &policy.git_identity);
    update_identity(&mut digest, &pre.identity);
    digest.update(&pre.index);
    digest.update(&pre.git.head);
    digest.update(&pre.git.branch);
    digest.update(&pre.git.head_entry);
    digest.update(&pre.git.index_entry);
    digest.update(&pre.git.refs);
    hex_digest(digest.finalize())
}

fn update_identity(digest: &mut Sha256, identity: &FileIdentity) {
    digest.update(format!("{identity:?}").as_bytes());
}

fn update_serialized<T: Serialize>(digest: &mut Sha256, value: &T) {
    if let Ok(serialized) = serde_json::to_vec(value) {
        digest.update(serialized);
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

fn serialized_preparation_size(preparation: &RepositoryDeleteFilePreparation) -> usize {
    let public_size = serde_json::to_vec(&json!({
        "tool_input": &preparation.tool_input,
        "review": &preparation.review,
        "review_identity": &preparation.review_identity,
        "source_sha256": &preparation.source_sha256,
        "source_byte_length": preparation.source_byte_length,
    }))
    .map(|serialized| serialized.len())
    .unwrap_or(usize::MAX);
    public_size
        .saturating_add(preparation.pre.bytes.len())
        .saturating_add(preparation.pre.index.len())
        .saturating_add(preparation.pre.git.head.len())
        .saturating_add(preparation.pre.git.branch.len())
        .saturating_add(preparation.pre.git.head_entry.len())
        .saturating_add(preparation.pre.git.index_entry.len())
        .saturating_add(preparation.pre.git.blob.len())
        .saturating_add(preparation.pre.git.refs.len())
        .saturating_add(preparation.pre.git.fingerprint.len())
        .saturating_add(preparation.pre.path.to_string_lossy().len())
        .saturating_add(preparation.pre.git_path.len())
        .saturating_add(4096)
}

fn confirmed_target_absence(path: &Path) -> bool {
    matches!(
        fs::symlink_metadata(path),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound
    )
}

fn same_name_entry_exists(parent: &Path, target_name: Option<&std::ffi::OsStr>) -> bool {
    let Some(target_name) = target_name else {
        return true;
    };
    let Ok(entries) = fs::read_dir(parent) else {
        return true;
    };
    for entry in entries {
        let Ok(entry) = entry else {
            return true;
        };
        let name = entry.file_name();
        #[cfg(windows)]
        let same_name =
            name.to_string_lossy().to_lowercase() == target_name.to_string_lossy().to_lowercase();
        #[cfg(not(windows))]
        let same_name = name == target_name;
        if same_name {
            return true;
        }
    }
    false
}

fn index_to_tree(index: &[u8], target: &str) -> Option<Vec<u8>> {
    let record = index.strip_suffix(&[0])?;
    let tab = record.iter().position(|b| *b == b'\t')?;
    if &record[tab + 1..] != target.as_bytes() {
        return None;
    }
    let fields = record[..tab].split(|b| *b == b' ').collect::<Vec<_>>();
    if fields.len() != 3 || fields[2] != b"0" {
        return None;
    }
    Some(
        [
            fields[0],
            b" blob ",
            fields[1],
            b"\t",
            target.as_bytes(),
            &[0],
        ]
        .concat(),
    )
}
fn delete_once(path: &Path) -> Result<(), std::io::Error> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        let wide = path
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect::<Vec<_>>();
        let ok = unsafe { windows_sys::Win32::Storage::FileSystem::DeleteFileW(wide.as_ptr()) };
        if ok == 0 {
            Err(std::io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
    #[cfg(not(windows))]
    {
        fs::remove_file(path)
    }
}
fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn result(status: &str, path: Option<&Path>, uncertain: bool) -> ToolOutput {
    let mut value = json!({"status":status,"uncertain":uncertain});
    if let Some(path) = path {
        value["path"] = Value::String(path.to_string_lossy().replace('\\', "/"));
    }
    ToolOutput {
        content: vec![ToolContent::Json(value)],
        is_error: status != "deleted_verified",
    }
}
fn policy_error(message: impl Into<String>) -> ToolError {
    ToolError::Execution {
        message: format!(
            "repository file deletion policy rejected capability: {}",
            message.into()
        ),
    }
}
fn fs_error(error: impl std::fmt::Display) -> ToolError {
    policy_error(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        process::Command,
        time::{SystemTime, UNIX_EPOCH},
    };

    struct Fixture {
        root: PathBuf,
        git: PathBuf,
    }
    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "rah-delete-{}",
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir_all(&root).unwrap();
            let git = git_path();
            for args in [
                vec!["init", "--quiet"],
                vec!["config", "user.name", "RAH Test"],
                vec!["config", "user.email", "rah@example.invalid"],
            ] {
                run(&git, &root, &args);
            }
            fs::write(root.join("target.txt"), b"protected\n").unwrap();
            fs::write(root.join("other.txt"), b"untouched\n").unwrap();
            run(&git, &root, &["add", "."]);
            run(&git, &root, &["commit", "--quiet", "-m", "base"]);
            Self { root, git }
        }
        fn request(&self) -> Value {
            json!({"path":"target.txt","expected_file_sha256":sha256(b"protected\n"),"expected_file_byte_length":10})
        }
        fn index(&self) -> Vec<u8> {
            fs::read(self.root.join(".git/index")).unwrap()
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
        fs::canonicalize(
            String::from_utf8(
                Command::new(command.0)
                    .arg(command.1)
                    .output()
                    .unwrap()
                    .stdout,
            )
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
    fn execute(tool: &RepositoryFileDeletionTool, value: Value) -> Value {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let output = runtime
            .block_on(tool.execute(ToolInput(value), ToolContext::default()))
            .unwrap();
        let [ToolContent::Json(value)] = output.content.as_slice() else {
            panic!("JSON result required")
        };
        value.clone()
    }

    #[test]
    fn deletes_one_clean_tracked_file_without_staging_or_collateral_effect() {
        let fixture = Fixture::new();
        let tool = RepositoryFileDeletionTool::new(&fixture.git, &fixture.root).unwrap();
        let index = fixture.index();
        assert_eq!(
            execute(&tool, fixture.request())["status"],
            "deleted_verified"
        );
        assert!(!fixture.root.join("target.txt").exists());
        assert_eq!(fixture.index(), index);
        assert_eq!(
            fs::read(fixture.root.join("other.txt")).unwrap(),
            b"untouched\n"
        );
        assert_eq!(
            tool.policy
                .delete_attempts
                .load(std::sync::atomic::Ordering::SeqCst),
            1
        );
    }

    #[test]
    fn execute_permission_and_other_states_do_not_imply_deletion() {
        let fixture = Fixture::new();
        let tool = RepositoryFileDeletionTool::new(&fixture.git, &fixture.root).unwrap();
        assert_eq!(tool.definition().permission, PermissionLevel::Execute);
        let mut request = fixture.request();
        request["expected_file_sha256"] = Value::String("0".repeat(64));
        assert_eq!(execute(&tool, request)["status"], "precondition_failed");
        assert!(fixture.root.join("target.txt").exists());
        for path in ["missing.txt", "other.txt", ".git"] {
            let value = json!({"path":path,"expected_file_sha256":sha256(b"protected\n"),"expected_file_byte_length":10});
            assert!(matches!(
                execute(&tool, value)["status"].as_str(),
                Some("precondition_failed") | Some("invalid_input")
            ));
        }
    }

    #[test]
    fn stale_preimage_is_refused_before_native_attempt() {
        let fixture = Fixture::new();
        let tool = RepositoryFileDeletionTool::new(&fixture.git, &fixture.root).unwrap();
        tool.policy
            .test_modify_before_delete
            .store(true, std::sync::atomic::Ordering::SeqCst);
        assert_eq!(
            execute(&tool, fixture.request())["status"],
            "precondition_failed"
        );
        assert_eq!(
            tool.policy
                .delete_attempts
                .load(std::sync::atomic::Ordering::SeqCst),
            0
        );
        assert_eq!(
            fs::read(fixture.root.join("target.txt")).unwrap(),
            b"changed externally"
        );
    }

    #[test]
    fn dirty_and_staged_targets_fail_closed() {
        let fixture = Fixture::new();
        let tool = RepositoryFileDeletionTool::new(&fixture.git, &fixture.root).unwrap();
        fs::write(fixture.root.join("target.txt"), b"new work\n").unwrap();
        assert_eq!(
            execute(&tool, fixture.request())["status"],
            "precondition_failed"
        );

        let fixture = Fixture::new();
        let tool = RepositoryFileDeletionTool::new(&fixture.git, &fixture.root).unwrap();
        fs::write(fixture.root.join("target.txt"), b"staged work\n").unwrap();
        run(&fixture.git, &fixture.root, &["add", "target.txt"]);
        assert_eq!(
            execute(&tool, fixture.request())["status"],
            "precondition_failed"
        );
        assert!(fixture.root.join("target.txt").exists());
    }

    #[cfg(unix)]
    #[test]
    fn symlink_target_is_rejected_without_following_it() {
        let fixture = Fixture::new();
        fs::remove_file(fixture.root.join("target.txt")).unwrap();
        std::os::unix::fs::symlink("other.txt", fixture.root.join("target.txt")).unwrap();
        let tool = RepositoryFileDeletionTool::new(&fixture.git, &fixture.root).unwrap();
        assert_eq!(
            execute(&tool, fixture.request())["status"],
            "precondition_failed"
        );
        assert_eq!(
            fs::read(fixture.root.join("other.txt")).unwrap(),
            b"untouched\n"
        );
    }

    #[test]
    fn post_attempt_uncertainty_is_not_replayed() {
        let fixture = Fixture::new();
        let tool = RepositoryFileDeletionTool::new(&fixture.git, &fixture.root).unwrap();
        tool.policy
            .force_uncertain
            .store(true, std::sync::atomic::Ordering::SeqCst);
        assert_eq!(execute(&tool, fixture.request())["status"], "uncertain");
        assert!(!fixture.root.join("target.txt").exists());
        assert_eq!(
            tool.policy
                .delete_attempts
                .load(std::sync::atomic::Ordering::SeqCst),
            1
        );
    }

    #[test]
    fn closed_path_schema_rejects_escape_aliases_and_extra_targets() {
        for value in [
            json!({"path":"../target.txt","expected_file_sha256":"0","expected_file_byte_length":0}),
            json!({"path":"C:/target.txt","expected_file_sha256":"0","expected_file_byte_length":0}),
            json!({"path":"target.txt","expected_file_sha256":"0","expected_file_byte_length":0,"force":true}),
        ] {
            assert!(DeleteRequest::parse(&ToolInput(value)).is_err());
        }
    }

    #[test]
    fn prepared_state_size_accounting_rejects_oversized_retained_state() {
        let fixture = Fixture::new();
        let preparer = RepositoryDeleteFilePreparer::new(&fixture.git, &fixture.root).unwrap();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let mut preparation = runtime
            .block_on(preparer.prepare(RepositoryDeleteFilePreparationRequest {
                path: "target.txt".to_owned(),
            }))
            .unwrap();
        preparation
            .pre
            .bytes
            .resize(MAX_PREPARED_REPRESENTATION_BYTES, 0);
        assert!(serialized_preparation_size(&preparation) > MAX_PREPARED_REPRESENTATION_BYTES);
    }
}
