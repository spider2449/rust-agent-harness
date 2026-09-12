//! Host-authorized, exclusive creation of one new repository worktree file.

use std::{
    fs,
    path::{Component, Path, PathBuf},
    sync::Arc,
};

use async_trait::async_trait;
use futures::lock::{Mutex as AsyncMutex, MutexGuard};
use rah_protocol::{PermissionLevel, ToolContent, ToolDefinition, ToolInput, ToolName, ToolOutput};
use serde::Serialize;
use serde_json::{Map, Value, json};
use sha2::{Digest as _, Sha256};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    HostArgumentPolicy, HostExecutionPolicy, Tool, ToolContext, ToolError,
    git_stage::repository_lease,
    git_support::git_environment,
    native_repository_create::{NativeCreateError, NativeObjectIdentity, NativeParent, create_new},
    repository_boundary::RepositoryNestedBoundaryPolicy,
    repository_worktree_patch::FileIdentity,
};

/// Stable name for the bounded repository file-creation capability.
pub const REPOSITORY_CREATE_FILE_TOOL_NAME: &str = "repo.create-file";
const MAX_CONTENT_BYTES: usize = 256 * 1024;
const MAX_REQUEST_BYTES: usize = 320 * 1024;
const MAX_PATH_BYTES: usize = 1024;
const MAX_SERIALIZED_REVIEW_BYTES: usize = 256 * 1024;
const MAX_PREPARED_REPRESENTATION_BYTES: usize = 512 * 1024;

/// Typed host input for non-effectful reviewed new-file preparation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepositoryCreateFilePreparationRequest {
    /// Validated repository-relative logical path.
    pub path: String,
    /// Exact UTF-8 content to retain and later pass to the Tool.
    pub content: String,
}

/// Sanitized failure classes for non-effectful new-file preparation.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum RepositoryCreateFilePreparationError {
    /// The closed request violated an input bound or rule.
    #[error("invalid repository create-file preparation input: {reason}")]
    InvalidInput { reason: &'static str },
    /// The repository or requested target is not admissible.
    #[error("repository create-file preparation precondition failed: {reason}")]
    PreconditionFailed { reason: &'static str },
    /// The complete review cannot fit the bounded review surface.
    #[error("repository create-file review is too large")]
    ReviewTooLarge,
    /// The retained preparation no longer matches current host state.
    #[error("repository create-file preparation is stale")]
    Stale,
}

/// BOM state of the exact requested content.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RepositoryCreateFileBomState {
    Present,
    Absent,
}

/// Complete, deterministic content visibility facts in the review.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RepositoryCreateFileContentFacts {
    /// Number of CR bytes.
    pub carriage_returns: usize,
    /// Number of LF bytes.
    pub line_feeds: usize,
    /// Number of CRLF pairs.
    pub crlf_pairs: usize,
    /// Whether the exact content ends in CR or LF.
    pub final_eof: &'static str,
    /// Number of escaped control characters.
    pub control_characters: usize,
    /// Number of escaped format, bidi, or zero-width characters.
    pub format_characters: usize,
}

/// Complete bounded review for one exact new-file creation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RepositoryCreateFileReview {
    operation: &'static str,
    target_count: usize,
    path: String,
    parent_path: String,
    existing_parent: &'static str,
    target_worktree: &'static str,
    target_head: &'static str,
    target_index: &'static str,
    expected_effect: &'static str,
    content_escaped: String,
    content_byte_length: usize,
    content_sha256: String,
    bom: RepositoryCreateFileBomState,
    content_facts: RepositoryCreateFileContentFacts,
    file_intent: &'static str,
    creation_semantics: Vec<&'static str>,
    non_effects: Vec<&'static str>,
    warnings: Vec<&'static str>,
}

impl RepositoryCreateFileReview {
    /// Returns the operation name.
    #[must_use]
    pub fn operation(&self) -> &str {
        self.operation
    }
    /// Returns the validated logical path.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }
    /// Returns the logical parent path.
    #[must_use]
    pub fn parent_path(&self) -> &str {
        &self.parent_path
    }
    /// Returns the fixed one-target count.
    #[must_use]
    pub fn target_count(&self) -> usize {
        self.target_count
    }
    /// Returns the safe existing-parent fact.
    #[must_use]
    pub fn existing_parent(&self) -> &str {
        self.existing_parent
    }
    /// Returns the worktree absence fact.
    #[must_use]
    pub fn target_worktree(&self) -> &str {
        self.target_worktree
    }
    /// Returns the HEAD absence fact.
    #[must_use]
    pub fn target_head(&self) -> &str {
        self.target_head
    }
    /// Returns the index absence fact.
    #[must_use]
    pub fn target_index(&self) -> &str {
        self.target_index
    }
    /// Returns the expected post-effect fact.
    #[must_use]
    pub fn expected_effect(&self) -> &str {
        self.expected_effect
    }
    /// Returns the complete deterministically escaped content.
    #[must_use]
    pub fn content_escaped(&self) -> &str {
        &self.content_escaped
    }
    /// Returns the exact content byte length.
    #[must_use]
    pub fn content_byte_length(&self) -> usize {
        self.content_byte_length
    }
    /// Returns the SHA-256 of exact content bytes.
    #[must_use]
    pub fn content_sha256(&self) -> &str {
        &self.content_sha256
    }
    /// Returns the exact content BOM state.
    #[must_use]
    pub fn bom(&self) -> RepositoryCreateFileBomState {
        self.bom
    }
    /// Returns newline, EOF, and invisible-character facts.
    #[must_use]
    pub fn content_facts(&self) -> &RepositoryCreateFileContentFacts {
        &self.content_facts
    }
    /// Returns the regular non-executable file intent.
    #[must_use]
    pub fn file_intent(&self) -> &str {
        self.file_intent
    }
    /// Returns exclusive-create and no-clobber semantics.
    #[must_use]
    pub fn creation_semantics(&self) -> &[&'static str] {
        &self.creation_semantics
    }
    /// Returns effects explicitly excluded by this review.
    #[must_use]
    pub fn non_effects(&self) -> &[&'static str] {
        &self.non_effects
    }
    /// Returns conservative post-acquisition warnings.
    #[must_use]
    pub fn warnings(&self) -> &[&'static str] {
        &self.warnings
    }
}

/// Opaque bounded preparation retained by a trusted host.
pub struct RepositoryCreateFilePreparation {
    preparer_identity: Uuid,
    tool_input: ToolInput,
    review: RepositoryCreateFileReview,
    review_identity: String,
    content_sha256: String,
    content_byte_length: usize,
    root_identity: FileIdentity,
    dot_git_identity: FileIdentity,
    git_identity: FileIdentity,
    pre: PreState,
}

impl RepositoryCreateFilePreparation {
    /// Returns the exact host-reconstructed `{path, content}` ToolInput.
    #[must_use]
    pub fn tool_input(&self) -> &ToolInput {
        &self.tool_input
    }
    /// Returns the complete bounded review.
    #[must_use]
    pub fn review(&self) -> &RepositoryCreateFileReview {
        &self.review
    }
    /// Returns the deterministic identity of this complete preparation.
    #[must_use]
    pub fn review_identity(&self) -> &str {
        &self.review_identity
    }
    /// Returns the exact content SHA-256.
    #[must_use]
    pub fn content_sha256(&self) -> &str {
        &self.content_sha256
    }
    /// Returns the exact content byte length.
    #[must_use]
    pub fn content_byte_length(&self) -> usize {
        self.content_byte_length
    }
}

impl std::fmt::Debug for RepositoryCreateFilePreparation {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RepositoryCreateFilePreparation")
            .field("review_identity", &self.review_identity)
            .field("content_sha256", &self.content_sha256)
            .field("content_byte_length", &self.content_byte_length)
            .finish_non_exhaustive()
    }
}

/// Host-bound, non-effectful preparation and revalidation for new-file creation.
pub struct RepositoryCreateFilePreparer {
    identity: Uuid,
    policy: RepositoryFileCreationPolicy,
}

impl RepositoryCreateFilePreparer {
    /// Creates a preparer bound to one host-selected repository and Git binary.
    pub fn new(
        git_executable: impl AsRef<Path>,
        repository_root: impl AsRef<Path>,
    ) -> Result<Self, ToolError> {
        Ok(Self {
            identity: Uuid::new_v4(),
            policy: RepositoryFileCreationPolicy::new(
                git_executable.as_ref(),
                repository_root.as_ref(),
            )?,
        })
    }

    /// Captures a complete review without executing a Tool or creating anything.
    pub async fn prepare(
        &self,
        request: RepositoryCreateFilePreparationRequest,
    ) -> Result<RepositoryCreateFilePreparation, RepositoryCreateFilePreparationError> {
        let request = CreateRequest::from_preparation(request)?;
        let _lease = self.policy.acquire_lease().await;
        let pre = self.policy.validate(&request).await.map_err(|_| {
            RepositoryCreateFilePreparationError::PreconditionFailed {
                reason: "repository_state",
            }
        })?;
        let tool_input = canonical_tool_input(&request);
        let review = build_create_review(&request)?;
        let content_sha256 = sha256_hex(request.content.as_bytes());
        let review_identity = compute_create_review_identity(
            self.identity,
            &self.policy,
            &request,
            &pre,
            &tool_input,
            &review,
        );
        let preparation = RepositoryCreateFilePreparation {
            preparer_identity: self.identity,
            tool_input,
            review,
            review_identity,
            content_sha256,
            content_byte_length: request.content.len(),
            root_identity: self.policy.root_identity.clone(),
            dot_git_identity: self.policy.dot_git_identity.clone(),
            git_identity: self.policy.git_identity.clone(),
            pre,
        };
        if serialized_preparation_size(&preparation) > MAX_PREPARED_REPRESENTATION_BYTES {
            return Err(RepositoryCreateFilePreparationError::ReviewTooLarge);
        }
        Ok(preparation)
    }

    /// Revalidates retained state without executing a Tool or creating anything.
    pub async fn revalidate(
        &self,
        preparation: &RepositoryCreateFilePreparation,
    ) -> Result<(), RepositoryCreateFilePreparationError> {
        let _lease = self.policy.acquire_lease().await;
        if preparation.preparer_identity != self.identity
            || preparation.root_identity != self.policy.root_identity
            || preparation.dot_git_identity != self.policy.dot_git_identity
            || preparation.git_identity != self.policy.git_identity
        {
            return Err(RepositoryCreateFilePreparationError::Stale);
        }
        let request = CreateRequest::parse_with_trailing_rule(&preparation.tool_input, true)
            .map_err(|_| RepositoryCreateFilePreparationError::Stale)?;
        if canonical_tool_input(&request) != preparation.tool_input {
            return Err(RepositoryCreateFilePreparationError::Stale);
        }
        let current = self
            .policy
            .validate(&request)
            .await
            .map_err(|_| RepositoryCreateFilePreparationError::Stale)?;
        if current != preparation.pre {
            return Err(RepositoryCreateFilePreparationError::Stale);
        }
        let review = build_create_review(&request)
            .map_err(|_| RepositoryCreateFilePreparationError::Stale)?;
        if review != preparation.review
            || sha256_hex(request.content.as_bytes()) != preparation.content_sha256
            || request.content.len() != preparation.content_byte_length
        {
            return Err(RepositoryCreateFilePreparationError::Stale);
        }
        let identity = compute_create_review_identity(
            self.identity,
            &self.policy,
            &request,
            &current,
            &preparation.tool_input,
            &review,
        );
        if identity != preparation.review_identity {
            return Err(RepositoryCreateFilePreparationError::Stale);
        }
        Ok(())
    }
}

/// Host-configured capability for one exclusive create-new operation.
pub struct RepositoryFileCreationTool {
    policy: RepositoryFileCreationPolicy,
    #[cfg(test)]
    test_hook: Arc<TestHook>,
}

impl RepositoryFileCreationTool {
    /// Constructs the tool from host-owned Git executable and repository root.
    pub fn new(
        git_executable: impl AsRef<Path>,
        repository_root: impl AsRef<Path>,
    ) -> Result<Self, ToolError> {
        Ok(Self {
            policy: RepositoryFileCreationPolicy::new(
                git_executable.as_ref(),
                repository_root.as_ref(),
            )?,
            #[cfg(test)]
            test_hook: Arc::new(TestHook::default()),
        })
    }
}

#[async_trait]
impl Tool for RepositoryFileCreationTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: ToolName::new(REPOSITORY_CREATE_FILE_TOOL_NAME),
            description: "Creates one new UTF-8 file at a validated repository-relative path."
                .to_owned(),
            input_schema: json!({"type":"object","additionalProperties":false,"required":["path","content"],"properties":{"path":{"type":"string"},"content":{"type":"string"}}}),
            permission: PermissionLevel::Execute,
        }
    }

    async fn execute(
        &self,
        input: ToolInput,
        _context: ToolContext,
    ) -> Result<ToolOutput, ToolError> {
        #[cfg(feature = "live-test-support")]
        live_test_create_file_tool_executions::record(&self.policy.root);
        let request = match CreateRequest::parse(&input) {
            Ok(value) => value,
            Err(()) => return Ok(output("invalid_target", None, None, None)),
        };
        let _lease = self.policy.acquire_lease().await;
        let pre = match self.policy.validate(&request).await {
            Ok(value) => value,
            Err(()) => return Ok(output("precondition_failed", None, None, None)),
        };
        let parent = match self.policy.revalidate(&request, &pre).await {
            Ok(value) => value,
            Err(()) => return Ok(output("precondition_failed", None, None, None)),
        };
        #[cfg(test)]
        self.test_hook.before_native_create(&pre.path);
        #[cfg(test)]
        self.test_hook.record_native_attempt();
        #[cfg(feature = "live-test-support")]
        live_test_create_file_native_attempts::record(&self.policy.root);
        let fail_after = {
            #[cfg(test)]
            {
                self.test_hook.write_fail_after()
            }
            #[cfg(not(test))]
            {
                None
            }
        };
        match create_new(
            &parent,
            &request.name,
            request.content.as_bytes(),
            fail_after,
        ) {
            Ok(created) => {
                #[cfg(test)]
                if self.test_hook.force_post_verification_failure() {
                    return Ok(output("uncertain", None, None, None));
                }
                #[cfg(test)]
                self.test_hook
                    .replace_target_before_post_verification(&pre.path);
                if self
                    .policy
                    .verify_post(&request, &pre, &created)
                    .await
                    .is_err()
                {
                    return Ok(output("uncertain", None, None, None));
                }
                return Ok(output(
                    "ok",
                    Some(&request.path),
                    Some(request.content.len()),
                    Some(sha256_hex(request.content.as_bytes())),
                ));
            }
            Err(NativeCreateError::AlreadyExists) => {
                let status = if self
                    .policy
                    .prove_no_effect(&request, &pre, true)
                    .await
                    .is_ok()
                {
                    "create_failed_known"
                } else {
                    "uncertain"
                };
                return Ok(output(status, None, None, None));
            }
            Err(NativeCreateError::WriteFailed { created, .. }) => {
                #[cfg(test)]
                self.test_hook
                    .replace_target_before_post_verification(&pre.path);
                let status = if self
                    .policy
                    .verify_known_partial(&request, &pre, &created)
                    .await
                    .is_ok()
                {
                    "write_failed_known"
                } else {
                    "uncertain"
                };
                return Ok(output(status, None, None, None));
            }
            Err(NativeCreateError::Io(_)) => {
                let status = if self
                    .policy
                    .prove_no_effect(&request, &pre, false)
                    .await
                    .is_ok()
                {
                    "create_failed_known"
                } else {
                    "uncertain"
                };
                return Ok(output(status, None, None, None));
            }
            Err(_) => return Ok(output("uncertain", None, None, None)),
        }
    }
}

#[cfg(feature = "live-test-support")]
pub mod live_test_create_file_tool_executions {
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
pub mod live_test_create_file_native_attempts {
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

#[cfg(test)]
#[derive(Default)]
struct TestHook {
    create_target_before_native: std::sync::atomic::AtomicBool,
    write_fail_after: std::sync::atomic::AtomicUsize,
    force_post_verification_failure: std::sync::atomic::AtomicBool,
    replace_target_before_post_verification: std::sync::atomic::AtomicBool,
    native_attempts: std::sync::atomic::AtomicUsize,
}

#[cfg(test)]
impl TestHook {
    fn before_native_create(&self, path: &Path) {
        if self
            .create_target_before_native
            .swap(false, std::sync::atomic::Ordering::SeqCst)
        {
            fs::write(path, b"external target").expect("race fixture should create target");
        }
    }

    fn write_fail_after(&self) -> Option<usize> {
        let value = self
            .write_fail_after
            .load(std::sync::atomic::Ordering::SeqCst);
        if value == 0 { None } else { Some(value - 1) }
    }

    fn record_native_attempt(&self) {
        self.native_attempts
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }

    fn force_post_verification_failure(&self) -> bool {
        self.force_post_verification_failure
            .swap(false, std::sync::atomic::Ordering::SeqCst)
    }

    fn replace_target_before_post_verification(&self, path: &Path) {
        if self
            .replace_target_before_post_verification
            .swap(false, std::sync::atomic::Ordering::SeqCst)
        {
            fs::remove_file(path).expect("replacement fixture should remove target");
            fs::write(path, b"external replacement").expect("replacement fixture should write");
        }
    }
}

/// Private, host-owned policy. Model input cannot supply any authority here.
struct RepositoryFileCreationPolicy {
    root: PathBuf,
    git: PathBuf,
    root_identity: FileIdentity,
    dot_git_identity: FileIdentity,
    git_identity: FileIdentity,
    boundary: RepositoryNestedBoundaryPolicy,
    lease: Arc<AsyncMutex<()>>,
}

impl RepositoryFileCreationPolicy {
    fn new(git: &Path, root: &Path) -> Result<Self, ToolError> {
        if !root.is_absolute() || !git.is_absolute() {
            return Err(config_error("host paths must be absolute"));
        }
        let root = fs::canonicalize(root).map_err(config_error)?;
        let git = fs::canonicalize(git).map_err(config_error)?;
        let dot_git = root.join(".git");
        if !root.is_dir()
            || !git.is_file()
            || is_link_or_reparse(&root)
            || is_link_or_reparse(&git)
            || is_link_or_reparse(&dot_git)
            || !dot_git.is_dir()
        {
            return Err(config_error("repository or Git identity is invalid"));
        }
        let root_identity = FileIdentity::capture(&root)?;
        let dot_git_identity = FileIdentity::capture(&dot_git)?;
        let git_identity = FileIdentity::capture(&git)?;
        Ok(Self {
            lease: repository_lease(&root),
            root: root.clone(),
            git,
            root_identity,
            dot_git_identity,
            git_identity,
            boundary: RepositoryNestedBoundaryPolicy::new(&root),
        })
    }
    async fn acquire_lease(&self) -> MutexGuard<'_, ()> {
        self.lease.lock().await
    }
    async fn validate(&self, request: &CreateRequest) -> Result<PreState, ()> {
        self.revalidate_root()?;
        let path = self.root.join(&request.path);
        let parent = path.parent().ok_or(())?;
        self.boundary.validate_existing(parent).map_err(|_| ())?;
        let parent_relative = parent.strip_prefix(&self.root).map_err(|_| ())?;
        let native_parent = NativeParent::open(&self.root, parent_relative).map_err(|_| ())?;
        match observe_absence(&path) {
            AbsenceObservation::Absent => {}
            AbsenceObservation::Present | AbsenceObservation::Failed => return Err(()),
        }
        self.require_git_absent(request).await?;
        Ok(PreState {
            path,
            parent_chain: native_parent.identities().to_vec(),
            head: self
                .git_output(&["rev-parse", "--verify", "HEAD"])
                .await
                .map_err(|_| ())?,
            refs: self
                .git_output(&["for-each-ref", "--format=%(refname)%00%(objectname)%00"])
                .await
                .map_err(|_| ())?,
            index: fs::read(self.root.join(".git/index")).map_err(|_| ())?,
        })
    }
    async fn revalidate(
        &self,
        request: &CreateRequest,
        expected: &PreState,
    ) -> Result<NativeParent, ()> {
        let current = self.validate(request).await?;
        if current != *expected {
            return Err(());
        }
        let parent = request.path.parent().unwrap_or_else(|| Path::new(""));
        NativeParent::open(&self.root, parent).map_err(|_| ())
    }
    async fn require_git_absent(&self, request: &CreateRequest) -> Result<(), ()> {
        let target = request.path.to_str().ok_or(())?;
        if !self
            .git_output(&["--literal-pathspecs", "ls-tree", "-z", "HEAD", "--", target])
            .await
            .map_err(|_| ())?
            .is_empty()
        {
            return Err(());
        }
        if !self
            .git_output(&["--literal-pathspecs", "ls-files", "-s", "-z", "--", target])
            .await
            .map_err(|_| ())?
            .is_empty()
        {
            return Err(());
        }
        match self
            .git_status(&["check-ignore", "-q", "--", target])
            .await?
        {
            1 => {}
            _ => return Err(()),
        }
        // Task 084's conservative policy: the capability fails closed whenever
        // sparse checkout is active, rather than guessing materialization scope.
        if self
            .git_status(&["config", "--bool", "core.sparseCheckout"])
            .await?
            == 0
        {
            return Err(());
        }
        let mut ancestor = PathBuf::new();
        let components = request.path.components().collect::<Vec<_>>();
        for component in &components[..components.len().saturating_sub(1)] {
            ancestor.push(component.as_os_str());
            let name = ancestor.to_str().ok_or(())?;
            let entry = self
                .git_output(&["--literal-pathspecs", "ls-files", "-s", "-z", "--", name])
                .await
                .map_err(|_| ())?;
            if entry.windows(6).any(|part| part == b"160000") {
                return Err(());
            }
        }
        Ok(())
    }
    async fn verify_post(
        &self,
        request: &CreateRequest,
        pre: &PreState,
        created: &crate::native_repository_create::NativeCreatedObject,
    ) -> Result<(), ()> {
        self.revalidate_root()?;
        let parent = NativeParent::open(
            &self.root,
            request.path.parent().unwrap_or_else(|| Path::new("")),
        )
        .map_err(|_| ())?;
        self.boundary
            .validate_existing(
                &self
                    .root
                    .join(request.path.parent().unwrap_or_else(|| Path::new(""))),
            )
            .map_err(|_| ())?;
        if parent.identities() != pre.parent_chain.as_slice() {
            return Err(());
        }
        let metadata = fs::symlink_metadata(&pre.path).map_err(|_| ())?;
        let target_identity = native_target_identity(&pre.path)?;
        if !metadata.is_file()
            || is_link_or_reparse(&pre.path)
            || !created.same_identity(&target_identity)
            || fs::read(&pre.path).map_err(|_| ())? != request.content.as_bytes()
        {
            return Err(());
        }
        self.require_git_absent(request).await?;
        if self
            .git_output(&["rev-parse", "--verify", "HEAD"])
            .await
            .map_err(|_| ())?
            != pre.head
            || self
                .git_output(&["for-each-ref", "--format=%(refname)%00%(objectname)%00"])
                .await
                .map_err(|_| ())?
                != pre.refs
            || fs::read(self.root.join(".git/index")).map_err(|_| ())? != pre.index
        {
            return Err(());
        }
        Ok(())
    }
    async fn verify_known_partial(
        &self,
        request: &CreateRequest,
        pre: &PreState,
        created: &crate::native_repository_create::NativeCreatedObject,
    ) -> Result<(), ()> {
        self.revalidate_root()?;
        let parent = NativeParent::open(
            &self.root,
            request.path.parent().unwrap_or_else(|| Path::new("")),
        )
        .map_err(|_| ())?;
        self.boundary
            .validate_existing(
                &self
                    .root
                    .join(request.path.parent().unwrap_or_else(|| Path::new(""))),
            )
            .map_err(|_| ())?;
        if parent.identities() != pre.parent_chain.as_slice() {
            return Err(());
        }
        let metadata = fs::symlink_metadata(&pre.path).map_err(|_| ())?;
        let bytes = fs::read(&pre.path).map_err(|_| ())?;
        if !metadata.is_file()
            || is_link_or_reparse(&pre.path)
            || !created.same_identity(&native_target_identity(&pre.path)?)
            || !request.content.as_bytes().starts_with(&bytes)
        {
            return Err(());
        }
        self.require_git_absent(request).await?;
        if self
            .git_output(&["rev-parse", "--verify", "HEAD"])
            .await
            .map_err(|_| ())?
            != pre.head
            || self
                .git_output(&["for-each-ref", "--format=%(refname)%00%(objectname)%00"])
                .await
                .map_err(|_| ())?
                != pre.refs
            || fs::read(self.root.join(".git/index")).map_err(|_| ())? != pre.index
        {
            return Err(());
        }
        Ok(())
    }

    async fn prove_no_effect(
        &self,
        request: &CreateRequest,
        pre: &PreState,
        collision: bool,
    ) -> Result<(), ()> {
        self.revalidate_root()?;
        let parent = NativeParent::open(
            &self.root,
            request.path.parent().unwrap_or_else(|| Path::new("")),
        )
        .map_err(|_| ())?;
        self.boundary
            .validate_existing(
                &self
                    .root
                    .join(request.path.parent().unwrap_or_else(|| Path::new(""))),
            )
            .map_err(|_| ())?;
        if parent.identities() != pre.parent_chain.as_slice() {
            return Err(());
        }
        match observe_absence(&pre.path) {
            AbsenceObservation::Absent => {}
            AbsenceObservation::Present if collision => {}
            AbsenceObservation::Present | AbsenceObservation::Failed => return Err(()),
        }
        self.require_git_absent(request).await?;
        if self
            .git_output(&["rev-parse", "--verify", "HEAD"])
            .await
            .map_err(|_| ())?
            != pre.head
            || self
                .git_output(&["for-each-ref", "--format=%(refname)%00%(objectname)%00"])
                .await
                .map_err(|_| ())?
                != pre.refs
            || fs::read(self.root.join(".git/index")).map_err(|_| ())? != pre.index
        {
            return Err(());
        }
        Ok(())
    }
    fn revalidate_root(&self) -> Result<(), ()> {
        let canonical = fs::canonicalize(&self.root).map_err(|_| ())?;
        let root_identity = FileIdentity::capture(&self.root).map_err(|_| ())?;
        let dot_git_identity = FileIdentity::capture(&self.root.join(".git")).map_err(|_| ())?;
        let git_identity = FileIdentity::capture(&self.git).map_err(|_| ())?;
        if canonical != self.root
            || is_link_or_reparse(&self.root)
            || !self.root.join(".git").is_dir()
            || !root_identity.same_object(&self.root_identity)
            || !dot_git_identity.same_object(&self.dot_git_identity)
            || !git_identity.same_object(&self.git_identity)
        {
            Err(())
        } else {
            Ok(())
        }
    }
    async fn git_output(&self, args: &[&str]) -> Result<Vec<u8>, ToolError> {
        let process = self
            .git_process(args)?
            .execute_process(&ToolInput(json!({})))
            .await?;
        if process.exit_code == Some(0) && !process.timed_out && process.overflow.is_none() {
            Ok(process.stdout)
        } else {
            Err(ToolError::Execution {
                message: "bounded Git observation failed".to_owned(),
            })
        }
    }
    async fn git_status(&self, args: &[&str]) -> Result<i32, ()> {
        let process = self
            .git_process(args)
            .map_err(|_| ())?
            .execute_process(&ToolInput(json!({})))
            .await
            .map_err(|_| ())?;
        process.exit_code.ok_or(())
    }
    fn git_process(&self, args: &[&str]) -> Result<HostExecutionPolicy, ToolError> {
        HostExecutionPolicy::new(
            &self.git,
            HostArgumentPolicy::Exact(args.iter().map(|value| (*value).to_owned()).collect()),
            &self.root,
            ".",
        )?
        .with_environment(git_environment())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PreState {
    path: PathBuf,
    parent_chain: Vec<NativeObjectIdentity>,
    head: Vec<u8>,
    refs: Vec<u8>,
    index: Vec<u8>,
}
struct CreateRequest {
    path: PathBuf,
    logical_path: String,
    name: String,
    content: String,
}
impl CreateRequest {
    fn parse(input: &ToolInput) -> Result<Self, ()> {
        Self::parse_with_trailing_rule(input, false)
    }

    fn parse_with_trailing_rule(input: &ToolInput, reviewed: bool) -> Result<Self, ()> {
        if serde_json::to_vec(&input.0).map_err(|_| ())?.len() > MAX_REQUEST_BYTES {
            return Err(());
        }
        let object = input.0.as_object().ok_or(())?;
        if object.len() != 2 || !object.contains_key("path") || !object.contains_key("content") {
            return Err(());
        }
        let path = object.get("path").and_then(Value::as_str).ok_or(())?;
        let content = object.get("content").and_then(Value::as_str).ok_or(())?;
        if content.len() > MAX_CONTENT_BYTES || content.contains('\0') {
            return Err(());
        }
        let path = parse_path(path, reviewed)?;
        let logical_path = path.to_string_lossy().replace('\\', "/");
        Ok(Self {
            name: path
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or(())?
                .to_owned(),
            path,
            logical_path,
            content: content.to_owned(),
        })
    }

    fn from_preparation(
        request: RepositoryCreateFilePreparationRequest,
    ) -> Result<Self, RepositoryCreateFilePreparationError> {
        let input = ToolInput(json!({
            "path": request.path,
            "content": request.content,
        }));
        Self::parse_with_trailing_rule(&input, true).map_err(|_| {
            RepositoryCreateFilePreparationError::InvalidInput {
                reason: "bounds_or_path",
            }
        })
    }
}
fn parse_path(value: &str, reviewed: bool) -> Result<PathBuf, ()> {
    if value.is_empty()
        || value.len() > MAX_PATH_BYTES
        || value.contains(['\0', '\\', ':'])
        || value.starts_with('/')
        || value.starts_with("//")
        || value.ends_with('/')
        || Path::new(value).is_absolute()
    {
        return Err(());
    }
    for component in value.split('/') {
        if component.is_empty()
            || matches!(component, "." | "..")
            || component.eq_ignore_ascii_case(".git")
            || reserved_windows_name(component)
            || (cfg!(windows) && reviewed && component.ends_with(['.', ' ']))
        {
            return Err(());
        }
    }
    let path = PathBuf::from(value);
    if path
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        Err(())
    } else {
        Ok(path)
    }
}

fn canonical_tool_input(request: &CreateRequest) -> ToolInput {
    ToolInput(json!({
        "path": request.logical_path,
        "content": request.content,
    }))
}

fn build_create_review(
    request: &CreateRequest,
) -> Result<RepositoryCreateFileReview, RepositoryCreateFilePreparationError> {
    let content = &request.content;
    let content_escaped = crate::repository_worktree_patch::escape_review_text(content);
    let content_sha256 = sha256_hex(content.as_bytes());
    let content_facts = content_facts(content);
    let review = RepositoryCreateFileReview {
        operation: REPOSITORY_CREATE_FILE_TOOL_NAME,
        target_count: 1,
        path: request.logical_path.clone(),
        parent_path: request
            .logical_path
            .rsplit_once('/')
            .map_or_else(|| ".".to_owned(), |(parent, _)| parent.to_owned()),
        existing_parent: "safe existing ordinary directory; no parent creation",
        target_worktree: "absent",
        target_head: "absent",
        target_index: "absent from every index stage, including intent-to-add",
        expected_effect: "one new untracked regular non-executable file",
        content_escaped,
        content_byte_length: content.len(),
        content_sha256,
        bom: if content.as_bytes().starts_with(b"\xef\xbb\xbf") {
            RepositoryCreateFileBomState::Present
        } else {
            RepositoryCreateFileBomState::Absent
        },
        content_facts,
        file_intent: "regular non-executable file",
        creation_semantics: vec![
            "exclusive create-new",
            "no clobber",
            "no overwrite",
            "exclusive name acquisition is the commit point",
        ],
        non_effects: vec![
            "no parent creation",
            "no Stage",
            "no Commit",
            "no index, HEAD, ref, or history mutation",
        ],
        warnings: vec![
            "full content creation is not crash-atomic",
            "write failure may retain an empty or partial file",
            "cancel, timeout, or disconnect is not rollback",
            "no automatic cleanup",
            "no retry or replay",
        ],
    };
    if serde_json::to_vec(&review)
        .map(|bytes| bytes.len() <= MAX_SERIALIZED_REVIEW_BYTES)
        .unwrap_or(false)
    {
        Ok(review)
    } else {
        Err(RepositoryCreateFilePreparationError::ReviewTooLarge)
    }
}

fn content_facts(content: &str) -> RepositoryCreateFileContentFacts {
    let bytes = content.as_bytes();
    let carriage_returns = bytes.iter().filter(|byte| **byte == b'\r').count();
    let line_feeds = bytes.iter().filter(|byte| **byte == b'\n').count();
    let crlf_pairs = bytes.windows(2).filter(|pair| pair == b"\r\n").count();
    let mut control_characters = 0;
    let mut format_characters = 0;
    for character in content.chars() {
        if character.is_control() {
            control_characters += 1;
        } else if is_format_character(character) {
            format_characters += 1;
        }
    }
    RepositoryCreateFileContentFacts {
        carriage_returns,
        line_feeds,
        crlf_pairs,
        final_eof: if bytes
            .last()
            .is_some_and(|byte| matches!(byte, b'\r' | b'\n'))
        {
            "final_newline"
        } else {
            "no_final_newline"
        },
        control_characters,
        format_characters,
    }
}

fn is_format_character(character: char) -> bool {
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

fn compute_create_review_identity(
    preparer_identity: Uuid,
    policy: &RepositoryFileCreationPolicy,
    request: &CreateRequest,
    pre: &PreState,
    tool_input: &ToolInput,
    review: &RepositoryCreateFileReview,
) -> String {
    let mut digest = Sha256::new();
    digest.update(b"rah-repository-create-file-preparation-v1\0");
    digest.update(preparer_identity.as_bytes());
    update_serialized(&mut digest, tool_input);
    update_serialized(&mut digest, review);
    digest.update(request.content.as_bytes());
    digest.update(format!("{:?}", policy.root_identity).as_bytes());
    digest.update(format!("{:?}", policy.dot_git_identity).as_bytes());
    digest.update(format!("{:?}", policy.git_identity).as_bytes());
    digest.update(format!("{:?}", pre.parent_chain).as_bytes());
    digest.update(&pre.head);
    digest.update(&pre.refs);
    digest.update(&pre.index);
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

fn serialized_preparation_size(preparation: &RepositoryCreateFilePreparation) -> usize {
    let public_size = serde_json::to_vec(&json!({
        "tool_input": &preparation.tool_input,
        "review": &preparation.review,
        "review_identity": &preparation.review_identity,
        "content_sha256": &preparation.content_sha256,
        "content_byte_length": preparation.content_byte_length,
    }))
    .map(|bytes| bytes.len())
    .unwrap_or(usize::MAX);
    public_size
        .saturating_add(preparation.pre.head.len())
        .saturating_add(preparation.pre.refs.len())
        .saturating_add(preparation.pre.index.len())
        .saturating_add(preparation.pre.parent_chain.len().saturating_mul(32))
        .saturating_add(1024)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AbsenceObservation {
    Absent,
    Present,
    Failed,
}

fn observe_absence(path: &Path) -> AbsenceObservation {
    match fs::symlink_metadata(path) {
        Ok(_) => AbsenceObservation::Present,
        Err(error) => classify_absence_error(error.kind()),
    }
}

fn classify_absence_error(kind: std::io::ErrorKind) -> AbsenceObservation {
    if kind == std::io::ErrorKind::NotFound {
        AbsenceObservation::Absent
    } else {
        AbsenceObservation::Failed
    }
}

fn native_target_identity(path: &Path) -> Result<NativeObjectIdentity, ()> {
    crate::native_repository_create::capture_existing(path).map_err(|_| ())
}
fn reserved_windows_name(component: &str) -> bool {
    let stem = component
        .trim_end_matches(['.', ' '])
        .split('.')
        .next()
        .unwrap_or("");
    matches!(
        stem.to_ascii_uppercase().as_str(),
        "CON" | "PRN" | "AUX" | "NUL"
    ) || stem.len() == 4
        && matches!(&stem[..3].to_ascii_uppercase()[..], "COM" | "LPT")
        && matches!(stem.as_bytes()[3], b'1'..=b'9')
}
fn is_link_or_reparse(path: &Path) -> bool {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return true;
    };
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        false
    }
}
fn config_error(error: impl std::fmt::Display) -> ToolError {
    ToolError::Execution {
        message: format!("host creation authority rejected configuration: {error}"),
    }
}
fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn output(
    status: &str,
    path: Option<&Path>,
    length: Option<usize>,
    sha256: Option<String>,
) -> ToolOutput {
    let mut object = Map::new();
    object.insert("status".to_owned(), Value::String(status.to_owned()));
    if let (Some(path), Some(length), Some(sha256)) = (path, length, sha256) {
        object.insert(
            "path".to_owned(),
            Value::String(path.to_string_lossy().replace('\\', "/")),
        );
        object.insert("length".to_owned(), json!(length));
        object.insert("sha256".to_owned(), Value::String(sha256));
    }
    ToolOutput {
        content: vec![ToolContent::Json(Value::Object(object))],
        is_error: status != "ok",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        process::Command,
        sync::atomic::Ordering,
        time::{SystemTime, UNIX_EPOCH},
    };

    struct Fixture {
        root: PathBuf,
        git: PathBuf,
    }
    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "rah-create-file-fault-{}",
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir_all(root.join("src")).unwrap();
            let git = native_git();
            for args in [
                ["init", "--quiet"].as_slice(),
                ["config", "user.name", "RAH Test"].as_slice(),
                ["config", "user.email", "rah@example.invalid"].as_slice(),
            ] {
                run(&git, &root, args);
            }
            fs::write(root.join("sentinel"), b"unchanged").unwrap();
            run(&git, &root, &["add", "sentinel"]);
            run(&git, &root, &["commit", "--quiet", "-m", "base"]);
            Self { root, git }
        }
        fn snapshot(&self) -> (Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>) {
            (
                fs::read(self.root.join(".git/HEAD")).unwrap(),
                fs::read(self.root.join(".git/index")).unwrap(),
                git_stdout(
                    &self.git,
                    &self.root,
                    &["for-each-ref", "--format=%(refname)%00%(objectname)%00"],
                ),
                fs::read(self.root.join("sentinel")).unwrap(),
            )
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
    fn native_git() -> PathBuf {
        #[cfg(windows)]
        let command = ("where.exe", "git.exe");
        #[cfg(not(windows))]
        let command = ("which", "git");
        let output = Command::new(command.0).arg(command.1).output().unwrap();
        fs::canonicalize(
            String::from_utf8(output.stdout)
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
    fn git_stdout(git: &Path, root: &Path, args: &[&str]) -> Vec<u8> {
        let output = Command::new(git)
            .args(args)
            .current_dir(root)
            .output()
            .unwrap();
        assert!(output.status.success(), "{args:?}");
        output.stdout
    }
    fn execute(tool: &RepositoryFileCreationTool, input: Value) -> Value {
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
    fn assert_snapshot(fixture: &Fixture, snapshot: &(Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>)) {
        assert_eq!(
            fs::read(fixture.root.join(".git/HEAD")).unwrap(),
            snapshot.0
        );
        assert_eq!(
            fs::read(fixture.root.join(".git/index")).unwrap(),
            snapshot.1
        );
        assert_eq!(
            git_stdout(
                &fixture.git,
                &fixture.root,
                &["for-each-ref", "--format=%(refname)%00%(objectname)%00"]
            ),
            snapshot.2
        );
        assert_eq!(fs::read(fixture.root.join("sentinel")).unwrap(), snapshot.3);
    }

    #[test]
    fn request_path_limit_is_measured_in_utf8_bytes() {
        let exact = "\u{03bb}".repeat(512);
        let over = "\u{03bb}".repeat(513);
        assert_eq!(exact.len(), 1024);
        assert!(CreateRequest::parse(&ToolInput(json!({"path":exact,"content":""}))).is_ok());
        assert_eq!(over.len(), 1026);
        assert!(CreateRequest::parse(&ToolInput(json!({"path":over,"content":""}))).is_err());
    }

    #[test]
    fn absence_classifier_fails_closed_for_non_not_found_errors() {
        assert_eq!(
            classify_absence_error(std::io::ErrorKind::NotFound),
            AbsenceObservation::Absent
        );
        assert_eq!(
            classify_absence_error(std::io::ErrorKind::PermissionDenied),
            AbsenceObservation::Failed
        );
        assert_eq!(
            classify_absence_error(std::io::ErrorKind::Other),
            AbsenceObservation::Failed
        );
    }

    #[test]
    fn root_level_create_preserves_git_state_and_remains_untracked() {
        let fixture = Fixture::new();
        let tool = RepositoryFileCreationTool::new(&fixture.git, &fixture.root).unwrap();
        let snapshot = fixture.snapshot();
        let value = execute(&tool, json!({"path":"root.txt","content":"root content"}));

        assert_eq!(value["status"], "ok");
        assert_eq!(
            fs::read(fixture.root.join("root.txt")).unwrap(),
            b"root content"
        );
        assert_eq!(tool.test_hook.native_attempts.load(Ordering::SeqCst), 1);
        assert_snapshot(&fixture, &snapshot);
        assert_eq!(
            git_stdout(
                &fixture.git,
                &fixture.root,
                &["status", "--porcelain=v1", "-z"]
            ),
            b"?? root.txt\0"
        );
        let repeated = execute(&tool, json!({"path":"root.txt","content":"replacement"}));
        assert_eq!(repeated["status"], "precondition_failed");
        assert_eq!(tool.test_hook.native_attempts.load(Ordering::SeqCst), 1);
        assert_eq!(
            fs::read(fixture.root.join("root.txt")).unwrap(),
            b"root content"
        );
        assert_snapshot(&fixture, &snapshot);
    }

    #[test]
    fn nested_repository_parent_is_rejected_before_native_create() {
        let fixture = Fixture::new();
        fs::create_dir(fixture.root.join("nested")).unwrap();
        fs::create_dir(fixture.root.join("nested/.git")).unwrap();
        let tool = RepositoryFileCreationTool::new(&fixture.git, &fixture.root).unwrap();

        let value = execute(&tool, json!({"path":"nested/new.rs","content":"new"}));

        assert_eq!(value["status"], "precondition_failed");
        assert!(!fixture.root.join("nested/new.rs").exists());
        assert_eq!(tool.test_hook.native_attempts.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn target_race_uses_one_exclusive_create_without_overwrite_or_retry() {
        let fixture = Fixture::new();
        let tool = RepositoryFileCreationTool::new(&fixture.git, &fixture.root).unwrap();
        let snapshot = fixture.snapshot();
        tool.test_hook
            .create_target_before_native
            .store(true, Ordering::SeqCst);
        let value = execute(&tool, json!({"path":"src/race.rs","content":"wanted"}));
        assert_eq!(value["status"], "create_failed_known");
        assert_eq!(
            fs::read(fixture.root.join("src/race.rs")).unwrap(),
            b"external target"
        );
        assert_eq!(tool.test_hook.native_attempts.load(Ordering::SeqCst), 1);
        assert_snapshot(&fixture, &snapshot);
    }

    #[test]
    fn partial_write_is_retained_and_reported_once() {
        let fixture = Fixture::new();
        let tool = RepositoryFileCreationTool::new(&fixture.git, &fixture.root).unwrap();
        let snapshot = fixture.snapshot();
        tool.test_hook.write_fail_after.store(3, Ordering::SeqCst);
        let value = execute(&tool, json!({"path":"src/partial.rs","content":"abcdef"}));
        assert_eq!(value["status"], "write_failed_known");
        assert_eq!(
            fs::read(fixture.root.join("src/partial.rs")).unwrap(),
            b"ab"
        );
        assert_eq!(tool.test_hook.native_attempts.load(Ordering::SeqCst), 1);
        assert_snapshot(&fixture, &snapshot);
    }

    #[test]
    fn lost_post_create_certification_is_uncertain_without_replay_or_cleanup() {
        let fixture = Fixture::new();
        let tool = RepositoryFileCreationTool::new(&fixture.git, &fixture.root).unwrap();
        let snapshot = fixture.snapshot();
        tool.test_hook
            .force_post_verification_failure
            .store(true, Ordering::SeqCst);
        let value = execute(
            &tool,
            json!({"path":"src/uncertain.rs","content":"committed"}),
        );
        assert_eq!(value["status"], "uncertain");
        assert_eq!(
            fs::read(fixture.root.join("src/uncertain.rs")).unwrap(),
            b"committed"
        );
        assert_eq!(tool.test_hook.native_attempts.load(Ordering::SeqCst), 1);
        assert_snapshot(&fixture, &snapshot);
    }

    #[test]
    fn replacing_created_target_makes_success_uncertain() {
        let fixture = Fixture::new();
        let tool = RepositoryFileCreationTool::new(&fixture.git, &fixture.root).unwrap();
        tool.test_hook
            .replace_target_before_post_verification
            .store(true, Ordering::SeqCst);
        let value = execute(&tool, json!({"path":"src/replaced.rs","content":"wanted"}));
        assert_eq!(value["status"], "uncertain");
        assert_eq!(
            fs::read(fixture.root.join("src/replaced.rs")).unwrap(),
            b"external replacement"
        );
        assert_eq!(tool.test_hook.native_attempts.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn replacing_created_partial_target_makes_write_failure_uncertain() {
        let fixture = Fixture::new();
        let tool = RepositoryFileCreationTool::new(&fixture.git, &fixture.root).unwrap();
        tool.test_hook.write_fail_after.store(3, Ordering::SeqCst);
        tool.test_hook
            .replace_target_before_post_verification
            .store(true, Ordering::SeqCst);
        let value = execute(
            &tool,
            json!({"path":"src/replaced-partial.rs","content":"abcdef"}),
        );
        assert_eq!(value["status"], "uncertain");
        assert_eq!(
            fs::read(fixture.root.join("src/replaced-partial.rs")).unwrap(),
            b"external replacement"
        );
        assert_eq!(tool.test_hook.native_attempts.load(Ordering::SeqCst), 1);
    }
}
