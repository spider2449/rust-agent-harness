//! Private preparation for the future bounded multi-file edit capability.
//!
//! This module deliberately has no `Tool` implementation and no native target
//! replacement primitive.  Task 094B keeps the native commit loop here so it
//! can consume the retained plan without exposing a new tool surface.

use std::{
    collections::HashSet,
    fs::{self, OpenOptions},
    io::Write,
    path::{Component, Path, PathBuf},
    sync::Arc,
};

#[cfg(windows)]
use std::fs::File;

use futures::lock::{Mutex as AsyncMutex, MutexGuard};
use rah_protocol::ToolInput;
use serde::Serialize;
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    git_stage::repository_lease,
    host_execute::{is_beneath, paths_equivalent},
    repository_boundary::RepositoryNestedBoundaryPolicy,
    repository_worktree_patch::escape_review_text,
};

const MAX_SERIALIZED_REQUEST_BYTES: usize = 256 * 1024;
const MAX_TARGETS: usize = 4;
const MAX_PATH_BYTES: usize = 1024;
const MAX_REPLACEMENTS_PER_TARGET: usize = 16;
const MAX_AGGREGATE_REPLACEMENTS: usize = 64;
const MAX_TEXT_BYTES: usize = 64 * 1024;
const MAX_FILE_BYTES: usize = 1024 * 1024;
const MAX_AGGREGATE_BYTES: usize = 4 * 1024 * 1024;
const MAX_ESCAPED_CHANGED_MATERIAL_BYTES: usize = 768 * 1024;
const MAX_UNCHANGED_CONTEXT_PER_TARGET_BYTES: usize = 32 * 1024;
const MAX_UNCHANGED_CONTEXT_BYTES: usize = 128 * 1024;
const MAX_SERIALIZED_REVIEW_BYTES: usize = 1024 * 1024;
const MAX_PREPARED_STATE_BYTES: usize = 2 * 1024 * 1024;

/// Stable name of the existing multi-file edit capability.
pub const REPOSITORY_MULTI_FILE_EDIT_PREPARATION_OPERATION: &str = "repo.edit-files";

/// Closed, host-owned human request for a bounded multi-file preparation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepositoryMultiFileEditPreparationRequest {
    /// Logical repository-relative targets.
    pub targets: Vec<RepositoryMultiFileEditPreparationTarget>,
}

/// One target in a multi-file preparation request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepositoryMultiFileEditPreparationTarget {
    /// Logical repository-relative path.
    pub path: String,
    /// Exact literal replacements, all resolved against one original snapshot.
    pub replacements: Vec<RepositoryMultiFileEditTextReplacement>,
}

/// One exact literal replacement in a preparation request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepositoryMultiFileEditTextReplacement {
    /// Nonempty literal text expected in the original snapshot.
    pub expected_old_text: String,
    /// Literal replacement text; empty text removes the match.
    pub replacement_text: String,
}

/// Sanitized failure classes for non-effectful multi-file preparation.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum RepositoryMultiFileEditPreparationError {
    /// The closed preparation request violated a bound or input rule.
    #[error("invalid multi-file edit preparation input: {reason}")]
    InvalidInput { reason: &'static str },
    /// A target cannot be admitted safely for this capability.
    #[error("multi-file edit preparation has an invalid target: {reason}")]
    InvalidTarget { reason: &'static str },
    /// Current repository state no longer satisfies the reviewed precondition.
    #[error("multi-file edit preparation precondition changed: {reason}")]
    PreconditionChanged { reason: &'static str },
    /// The earlier prepared change-set no longer binds to this preparer.
    #[error("multi-file edit preparation is stale")]
    Stale,
    /// Complete changed/review material exceeded the accepted bound.
    #[error("multi-file edit review is too large")]
    ReviewTooLarge,
}

/// One half-open byte range and complete changed material in a multi-file review.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RepositoryMultiFileEditChangedRange {
    start: usize,
    end: usize,
    length: usize,
    expected_old_text_escaped: String,
    replacement_text_escaped: String,
}

impl RepositoryMultiFileEditChangedRange {
    /// Start offset in the original raw file.
    #[must_use]
    pub fn start(&self) -> usize {
        self.start
    }

    /// Exclusive end offset in the original raw file.
    #[must_use]
    pub fn end(&self) -> usize {
        self.end
    }

    /// Matched old-text byte length.
    #[must_use]
    pub fn length(&self) -> usize {
        self.length
    }

    /// Complete escaped old text.
    #[must_use]
    pub fn expected_old_text_escaped(&self) -> &str {
        &self.expected_old_text_escaped
    }

    /// Complete escaped replacement text.
    #[must_use]
    pub fn replacement_text_escaped(&self) -> &str {
        &self.replacement_text_escaped
    }
}

/// Complete review of one host-ordered target.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RepositoryMultiFileEditTargetReview {
    ordinal: usize,
    path: String,
    target_identity: String,
    replacement_count: usize,
    changed_ranges: Vec<RepositoryMultiFileEditChangedRange>,
    preimage_sha256: String,
    preimage_byte_length: usize,
    postimage_sha256: String,
    postimage_byte_length: usize,
}

impl RepositoryMultiFileEditTargetReview {
    /// Host-owned zero-based deterministic target ordinal.
    #[must_use]
    pub fn ordinal(&self) -> usize {
        self.ordinal
    }

    /// Canonical logical repository-relative path.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Stable redacted identity digest for the target and parent binding.
    #[must_use]
    pub fn target_identity(&self) -> &str {
        &self.target_identity
    }

    /// Number of exact replacements for this target.
    #[must_use]
    pub fn replacement_count(&self) -> usize {
        self.replacement_count
    }

    /// Every mutation-relevant changed range.
    #[must_use]
    pub fn changed_ranges(&self) -> &[RepositoryMultiFileEditChangedRange] {
        &self.changed_ranges
    }

    /// Exact preimage SHA-256.
    #[must_use]
    pub fn preimage_sha256(&self) -> &str {
        &self.preimage_sha256
    }

    /// Exact preimage byte length.
    #[must_use]
    pub fn preimage_byte_length(&self) -> usize {
        self.preimage_byte_length
    }

    /// Exact postimage SHA-256.
    #[must_use]
    pub fn postimage_sha256(&self) -> &str {
        &self.postimage_sha256
    }

    /// Exact postimage byte length.
    #[must_use]
    pub fn postimage_byte_length(&self) -> usize {
        self.postimage_byte_length
    }
}

/// Complete bounded review for one `repo.edit-files` change-set.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RepositoryMultiFileEditReview {
    operation: &'static str,
    target_count: usize,
    replacement_count: usize,
    targets: Vec<RepositoryMultiFileEditTargetReview>,
    matching: &'static str,
    unchanged_context: &'static str,
    intended_effect: &'static str,
    non_effects: Vec<&'static str>,
    non_atomic_warning: &'static str,
}

impl RepositoryMultiFileEditReview {
    /// Reviewed operation name.
    #[must_use]
    pub fn operation(&self) -> &str {
        self.operation
    }

    /// Number of reviewed targets.
    #[must_use]
    pub fn target_count(&self) -> usize {
        self.target_count
    }

    /// Total number of reviewed replacements.
    #[must_use]
    pub fn replacement_count(&self) -> usize {
        self.replacement_count
    }

    /// Targets in deterministic host order.
    #[must_use]
    pub fn targets(&self) -> &[RepositoryMultiFileEditTargetReview] {
        &self.targets
    }

    /// Matching semantics represented by this review.
    #[must_use]
    pub fn matching(&self) -> &str {
        self.matching
    }

    /// Statement of unchanged-context handling.
    #[must_use]
    pub fn unchanged_context(&self) -> &str {
        self.unchanged_context
    }

    /// Intended bounded worktree effect.
    #[must_use]
    pub fn intended_effect(&self) -> &str {
        self.intended_effect
    }

    /// Explicit protected non-effects.
    #[must_use]
    pub fn non_effects(&self) -> &[&'static str] {
        &self.non_effects
    }

    /// Required non-atomic warning.
    #[must_use]
    pub fn non_atomic_warning(&self) -> &str {
        self.non_atomic_warning
    }
}

/// Opaque, immutable non-effectful multi-file preparation.
pub struct RepositoryMultiFileEditPreparation {
    preparer_identity: Uuid,
    tool_input: ToolInput,
    review: RepositoryMultiFileEditReview,
    review_identity: String,
    plan: PreparedMultiFilePlan,
}

impl RepositoryMultiFileEditPreparation {
    /// Exact canonical closed `repo.edit-files` input for future binding.
    #[must_use]
    pub fn tool_input(&self) -> &ToolInput {
        &self.tool_input
    }

    /// Complete backend-derived review.
    #[must_use]
    pub fn review(&self) -> &RepositoryMultiFileEditReview {
        &self.review
    }

    /// Stable identity of the complete prepared change-set.
    #[must_use]
    pub fn review_identity(&self) -> &str {
        &self.review_identity
    }

    /// Exact target postimage bytes retained for later same-plan validation.
    #[must_use]
    pub fn postimage(&self, ordinal: usize) -> Option<&[u8]> {
        self.plan
            .targets
            .get(ordinal)
            .map(|target| target.postimage.as_slice())
    }
}

impl std::fmt::Debug for RepositoryMultiFileEditPreparation {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RepositoryMultiFileEditPreparation")
            .field("review_identity", &self.review_identity)
            .field("target_count", &self.plan.targets.len())
            .finish_non_exhaustive()
    }
}

/// Host-bound non-effectful preparation and revalidation for `repo.edit-files`.
pub struct RepositoryMultiFileEditPreparer {
    identity: Uuid,
    policy: RepositoryMultiFileMutationPolicy,
}

impl RepositoryMultiFileEditPreparer {
    /// Creates a preparer bound to one trusted non-bare repository.
    pub fn new(
        git_executable: impl AsRef<Path>,
        repository_root: impl AsRef<Path>,
    ) -> Result<Self, crate::ToolError> {
        RepositoryMultiFileMutationPolicy::new(git_executable.as_ref(), repository_root.as_ref())
            .map(|policy| Self {
                identity: Uuid::new_v4(),
                policy,
            })
            .map_err(|_| crate::ToolError::Execution {
                message: "repository multi-file edit policy rejected host authority".to_owned(),
            })
    }

    /// Captures one exact change-set without Tool execution, replacement, or
    /// repository-adjacent temporary-file creation.
    pub async fn prepare(
        &self,
        request: RepositoryMultiFileEditPreparationRequest,
    ) -> Result<RepositoryMultiFileEditPreparation, RepositoryMultiFileEditPreparationError> {
        let request = HumanMultiFileRequest::validate(request)?;
        let _lease = self.policy.acquire_lease().await;
        let plan = self
            .policy
            .prepare_human_locked(request)
            .map_err(human_preparation_error)?;
        let tool_input = canonical_tool_input_for_plan(&plan);
        let review = build_multi_file_review(&plan)?;
        let review_identity =
            compute_multi_file_review_identity(&self.policy, &plan, &tool_input, &review);
        let preparation = RepositoryMultiFileEditPreparation {
            preparer_identity: self.identity,
            tool_input,
            review,
            review_identity,
            plan,
        };
        if serialized_multi_file_preparation_size(&preparation) > MAX_PREPARED_STATE_BYTES {
            return Err(RepositoryMultiFileEditPreparationError::ReviewTooLarge);
        }
        Ok(preparation)
    }

    /// Revalidates the exact earlier change-set without any effect.
    pub async fn revalidate(
        &self,
        preparation: &RepositoryMultiFileEditPreparation,
    ) -> Result<(), RepositoryMultiFileEditPreparationError> {
        let _lease = self.policy.acquire_lease().await;
        if preparation.preparer_identity != self.identity {
            return Err(RepositoryMultiFileEditPreparationError::Stale);
        }
        let request = MultiFileRequest::parse(&preparation.tool_input)
            .map_err(|_| RepositoryMultiFileEditPreparationError::Stale)?;
        let current = self
            .policy
            .prepare_tool_locked(request)
            .map_err(revalidation_error)?;
        if !plans_match(&preparation.plan, &current) {
            return Err(classify_plan_drift(&preparation.plan, &current));
        }
        let tool_input = canonical_tool_input_for_plan(&current);
        if tool_input != preparation.tool_input {
            return Err(RepositoryMultiFileEditPreparationError::Stale);
        }
        let review = build_multi_file_review(&current)
            .map_err(|_| RepositoryMultiFileEditPreparationError::Stale)?;
        if review != preparation.review {
            return Err(RepositoryMultiFileEditPreparationError::Stale);
        }
        let identity =
            compute_multi_file_review_identity(&self.policy, &current, &tool_input, &review);
        if identity != preparation.review_identity {
            return Err(RepositoryMultiFileEditPreparationError::Stale);
        }
        Ok(())
    }
}

struct HumanMultiFileRequest {
    targets: Vec<RequestTarget>,
}

impl HumanMultiFileRequest {
    fn validate(
        request: RepositoryMultiFileEditPreparationRequest,
    ) -> Result<Self, RepositoryMultiFileEditPreparationError> {
        if request.targets.is_empty() || request.targets.len() > MAX_TARGETS {
            return Err(RepositoryMultiFileEditPreparationError::InvalidInput {
                reason: "target_count",
            });
        }
        let mut total_replacements = 0usize;
        let mut serialized_targets = Vec::with_capacity(request.targets.len());
        let mut targets = Vec::with_capacity(request.targets.len());
        for target in request.targets {
            if target.path.len() > MAX_PATH_BYTES || validate_logical_path(&target.path).is_err() {
                return Err(RepositoryMultiFileEditPreparationError::InvalidInput {
                    reason: "path",
                });
            }
            if target.replacements.is_empty()
                || target.replacements.len() > MAX_REPLACEMENTS_PER_TARGET
            {
                return Err(RepositoryMultiFileEditPreparationError::InvalidInput {
                    reason: "replacement_count",
                });
            }
            total_replacements = total_replacements
                .checked_add(target.replacements.len())
                .ok_or(RepositoryMultiFileEditPreparationError::InvalidInput {
                    reason: "replacement_count",
                })?;
            if total_replacements > MAX_AGGREGATE_REPLACEMENTS {
                return Err(RepositoryMultiFileEditPreparationError::InvalidInput {
                    reason: "replacement_count",
                });
            }
            let mut serialized_replacements = Vec::with_capacity(target.replacements.len());
            let mut replacements = Vec::with_capacity(target.replacements.len());
            for replacement in target.replacements {
                if replacement.expected_old_text.is_empty()
                    || replacement.expected_old_text.len() > MAX_TEXT_BYTES
                    || replacement.replacement_text.len() > MAX_TEXT_BYTES
                    || replacement.expected_old_text.contains('\0')
                    || replacement.replacement_text.contains('\0')
                {
                    return Err(RepositoryMultiFileEditPreparationError::InvalidInput {
                        reason: "text",
                    });
                }
                serialized_replacements.push(json!({
                    "expected_old_text": replacement.expected_old_text,
                    "replacement_text": replacement.replacement_text,
                }));
                replacements.push(Replacement {
                    old: replacement.expected_old_text,
                    new: replacement.replacement_text,
                });
            }
            serialized_targets.push(json!({
                "path": target.path,
                "replacements": serialized_replacements,
            }));
            targets.push(RequestTarget {
                path: target.path,
                expected_sha256: None,
                expected_length: None,
                replacements,
            });
        }
        let serialized =
            serde_json::to_vec(&json!({ "targets": serialized_targets })).map_err(|_| {
                RepositoryMultiFileEditPreparationError::InvalidInput {
                    reason: "request_serialization",
                }
            })?;
        if serialized.len() > MAX_SERIALIZED_REQUEST_BYTES {
            return Err(RepositoryMultiFileEditPreparationError::InvalidInput {
                reason: "request_size",
            });
        }
        Ok(Self { targets })
    }
}

fn human_preparation_error(error: PreflightError) -> RepositoryMultiFileEditPreparationError {
    match error {
        PreflightError::InvalidTarget(reason) => {
            RepositoryMultiFileEditPreparationError::InvalidTarget { reason }
        }
        PreflightError::Precondition(reason) if is_target_admission_reason(reason) => {
            RepositoryMultiFileEditPreparationError::InvalidTarget { reason }
        }
        PreflightError::Precondition(reason) => {
            RepositoryMultiFileEditPreparationError::PreconditionChanged { reason }
        }
    }
}

fn revalidation_error(error: PreflightError) -> RepositoryMultiFileEditPreparationError {
    match error {
        PreflightError::InvalidTarget(reason) => {
            RepositoryMultiFileEditPreparationError::InvalidTarget { reason }
        }
        PreflightError::Precondition(reason)
            if reason.contains("identity changed")
                && (reason.contains("repository") || reason.contains("Git executable")) =>
        {
            RepositoryMultiFileEditPreparationError::Stale
        }
        PreflightError::Precondition(reason) if is_target_admission_reason(reason) => {
            RepositoryMultiFileEditPreparationError::InvalidTarget { reason }
        }
        PreflightError::Precondition(reason) => {
            RepositoryMultiFileEditPreparationError::PreconditionChanged { reason }
        }
    }
}

fn is_target_admission_reason(reason: &str) -> bool {
    reason.contains("regular")
        || reason.contains("hard-linked")
        || reason.contains("identity changed")
        || reason.contains("UTF-8")
        || reason.contains("NUL")
        || reason.contains("symbolic")
        || reason.contains("reparse")
        || reason.contains("attributes")
        || reason.contains("target aliases")
        || reason.contains("path")
}

fn plans_match(left: &PreparedMultiFilePlan, right: &PreparedMultiFilePlan) -> bool {
    left.repository == right.repository
        && left.targets.len() == right.targets.len()
        && left
            .targets
            .iter()
            .zip(&right.targets)
            .all(|(left, right)| {
                left.canonical_logical == right.canonical_logical
                    && paths_equivalent(&left.target.path, &right.target.path)
                    && left.target.identity == right.target.identity
                    && left.target.parent_identity == right.target.parent_identity
                    && target_mode(&left.target) == target_mode(&right.target)
                    && left.original == right.original
                    && left.postimage == right.postimage
                    && left.replacements == right.replacements
                    && left.ranges == right.ranges
            })
}

fn classify_plan_drift(
    old: &PreparedMultiFilePlan,
    current: &PreparedMultiFilePlan,
) -> RepositoryMultiFileEditPreparationError {
    if old.targets.len() != current.targets.len()
        || old
            .targets
            .iter()
            .zip(&current.targets)
            .any(|(old, current)| old.canonical_logical != current.canonical_logical)
    {
        return RepositoryMultiFileEditPreparationError::Stale;
    }
    if old
        .targets
        .iter()
        .zip(&current.targets)
        .any(|(old, current)| {
            old.target.identity != current.target.identity
                || old.target.parent_identity != current.target.parent_identity
                || target_mode(&old.target) != target_mode(&current.target)
        })
    {
        return RepositoryMultiFileEditPreparationError::InvalidTarget {
            reason: "target identity or mode changed",
        };
    }
    if old.repository != current.repository {
        return RepositoryMultiFileEditPreparationError::PreconditionChanged {
            reason: "protected repository state changed",
        };
    }
    RepositoryMultiFileEditPreparationError::PreconditionChanged {
        reason: "prepared change-set changed",
    }
}

fn target_mode(target: &SafeTarget) -> u32 {
    #[cfg(unix)]
    {
        target.mode & 0o7777
    }
    #[cfg(not(unix))]
    {
        let _ = target;
        0
    }
}

fn canonical_tool_input_for_plan(plan: &PreparedMultiFilePlan) -> ToolInput {
    ToolInput(json!({
        "targets": plan.targets.iter().map(|target| {
            let mut order = (0..target.replacements.len()).collect::<Vec<_>>();
            order.sort_unstable_by_key(|index| target.ranges[*index].start);
            json!({
                "path": target.canonical_logical,
                "expected_file_sha256": sha256(&target.original),
                "expected_file_byte_length": target.original.len(),
                "replacements": order.into_iter().map(|index| json!({
                    "expected_old_text": target.replacements[target.ranges[index].replacement_index].old,
                    "replacement_text": target.replacements[target.ranges[index].replacement_index].new,
                })).collect::<Vec<_>>(),
            })
        }).collect::<Vec<_>>(),
    }))
}

fn build_multi_file_review(
    plan: &PreparedMultiFilePlan,
) -> Result<RepositoryMultiFileEditReview, RepositoryMultiFileEditPreparationError> {
    let mut total_replacements = 0usize;
    let mut escaped_changed = 0usize;
    let targets = plan
        .targets
        .iter()
        .enumerate()
        .map(|(ordinal, target)| {
            total_replacements = total_replacements.saturating_add(target.replacements.len());
            let changed_ranges = target
                .ranges
                .iter()
                .map(|range| {
                    let replacement = &target.replacements[range.replacement_index];
                    let old = escape_review_text(&replacement.old);
                    let new = escape_review_text(&replacement.new);
                    escaped_changed = escaped_changed
                        .saturating_add(old.len())
                        .saturating_add(new.len());
                    RepositoryMultiFileEditChangedRange {
                        start: range.start,
                        end: range.end,
                        length: range.end - range.start,
                        expected_old_text_escaped: old,
                        replacement_text_escaped: new,
                    }
                })
                .collect::<Vec<_>>();
            RepositoryMultiFileEditTargetReview {
                ordinal,
                path: target.canonical_logical.clone(),
                target_identity: target_identity_digest(&target.target),
                replacement_count: target.replacements.len(),
                changed_ranges,
                preimage_sha256: sha256(&target.original),
                preimage_byte_length: target.original.len(),
                postimage_sha256: sha256(&target.postimage),
                postimage_byte_length: target.postimage.len(),
            }
        })
        .collect::<Vec<_>>();
    if escaped_changed > MAX_ESCAPED_CHANGED_MATERIAL_BYTES {
        return Err(RepositoryMultiFileEditPreparationError::ReviewTooLarge);
    }
    let unchanged_context_per_target = 0usize;
    let unchanged_context_total = 0usize;
    if unchanged_context_per_target > MAX_UNCHANGED_CONTEXT_PER_TARGET_BYTES
        || unchanged_context_total > MAX_UNCHANGED_CONTEXT_BYTES
    {
        return Err(RepositoryMultiFileEditPreparationError::ReviewTooLarge);
    }
    let review = RepositoryMultiFileEditReview {
        operation: REPOSITORY_MULTI_FILE_EDIT_PREPARATION_OPERATION,
        target_count: targets.len(),
        replacement_count: total_replacements,
        targets,
        matching: "all literal matches resolve exactly once against the same original snapshot; duplicate, overlap, and no-op replacements are rejected",
        unchanged_context: "unchanged surrounding context omitted; complete changed material is retained",
        intended_effect: "replace complete postimages of the reviewed clean tracked files in host order",
        non_effects: vec![
            "preparation performs no Tool execution or native replacement",
            "the worktree, index, HEAD, refs, and history remain unchanged",
            "Stage, Unstage, and Commit remain separate operations",
            "no provider, model, runtime, shell, process, or network action occurs",
        ],
        non_atomic_warning: "repo.edit-files is non-atomic; targets have independent native commit points",
    };
    if serde_json::to_vec(&review)
        .map(|serialized| serialized.len() > MAX_SERIALIZED_REVIEW_BYTES)
        .unwrap_or(true)
    {
        return Err(RepositoryMultiFileEditPreparationError::ReviewTooLarge);
    }
    Ok(review)
}

fn target_identity_digest(target: &SafeTarget) -> String {
    let mut digest = Sha256::new();
    update_identity(&mut digest, &target.identity);
    update_identity(&mut digest, &target.parent_identity);
    digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn compute_multi_file_review_identity(
    policy: &RepositoryMultiFileMutationPolicy,
    plan: &PreparedMultiFilePlan,
    tool_input: &ToolInput,
    review: &RepositoryMultiFileEditReview,
) -> String {
    let mut digest = Sha256::new();
    digest.update(b"rah-repository-multi-file-edit-preparation-v1\0");
    update_serialized(&mut digest, tool_input);
    update_serialized(&mut digest, review);
    update_identity(&mut digest, &policy.root_identity);
    update_identity(&mut digest, &policy.dot_git_identity);
    update_identity(&mut digest, &policy.git_identity);
    digest.update(&plan.repository.index);
    digest.update(&plan.repository.head);
    digest.update(&plan.repository.refs);
    for target in &plan.targets {
        digest.update(target.canonical_logical.as_bytes());
        update_identity(&mut digest, &target.target.identity);
        update_identity(&mut digest, &target.target.parent_identity);
        digest.update(&target.original);
        digest.update(&target.postimage);
    }
    digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn update_serialized<T: Serialize>(digest: &mut Sha256, value: &T) {
    if let Ok(serialized) = serde_json::to_vec(value) {
        digest.update(serialized);
    }
}

fn update_identity(digest: &mut Sha256, identity: &Identity) {
    #[cfg(unix)]
    {
        digest.update(identity.device.to_le_bytes());
        digest.update(identity.inode.to_le_bytes());
    }
    #[cfg(windows)]
    {
        digest.update(identity.volume_serial.to_le_bytes());
        digest.update(identity.file_index.to_le_bytes());
    }
    digest.update(identity.link_count.to_le_bytes());
}

fn serialized_multi_file_preparation_size(
    preparation: &RepositoryMultiFileEditPreparation,
) -> usize {
    let metadata = serde_json::to_vec(&json!({
        "tool_input": &preparation.tool_input,
        "review": &preparation.review,
        "review_identity": &preparation.review_identity,
    }))
    .map(|bytes| bytes.len())
    .unwrap_or(usize::MAX);
    preparation
        .plan
        .targets
        .iter()
        .map(|target| target.original.len().saturating_add(target.postimage.len()))
        .try_fold(metadata, |total, bytes| total.checked_add(bytes))
        .unwrap_or(usize::MAX)
}

/// Private host-owned authority. It is intentionally not a tool and cannot be
/// registered until a later task supplies the separate commit semantics.
pub(crate) struct RepositoryMultiFileMutationPolicy {
    git: PathBuf,
    git_identity: Identity,
    root: PathBuf,
    root_identity: Identity,
    dot_git: PathBuf,
    dot_git_identity: Identity,
    boundary: RepositoryNestedBoundaryPolicy,
    lease: Arc<AsyncMutex<()>>,
}

impl RepositoryMultiFileMutationPolicy {
    pub(crate) fn new(git: &Path, root: &Path) -> Result<Self, PreflightError> {
        if !root.is_absolute() {
            return Err(PreflightError::Precondition(
                "repository root must be absolute",
            ));
        }
        reject_reparse_ancestry(root, "repository root")?;
        let root = canonical_directory(root, "repository root")?;
        let dot_git = root.join(".git");
        reject_link_or_reparse(&dot_git, "repository metadata")?;
        if !fs::metadata(&dot_git).map_err(fs_error)?.is_dir() {
            return Err(PreflightError::Precondition(
                "linked worktrees are unsupported",
            ));
        }
        let git = canonical_file(git, "Git executable")?;
        Ok(Self {
            git_identity: Identity::capture(&git)?,
            git,
            root_identity: Identity::capture(&root)?,
            dot_git_identity: Identity::capture(&dot_git)?,
            dot_git,
            boundary: RepositoryNestedBoundaryPolicy::new(&root),
            lease: repository_lease(&root),
            root,
        })
    }

    pub(crate) async fn acquire_lease(&self) -> MutexGuard<'_, ()> {
        self.lease.lock().await
    }

    /// Fully preflights and constructs all postimages.  It never replaces a
    /// target; owned temporaries are checked then removed before return.
    #[allow(dead_code)]
    pub(crate) async fn prepare(
        &self,
        input: &ToolInput,
    ) -> Result<PreparedMultiFilePlan, PreflightError> {
        let plan = self.prepare_retained_inner(input).await?;
        plan.cleanup_temporaries();
        Ok(plan.without_temporaries())
    }

    /// Runs the private, one-attempt-per-target native commit loop.  This is
    /// intentionally crate-private until the separate policy integration task.
    pub(crate) async fn commit(
        &self,
        input: &ToolInput,
    ) -> Result<MultiFileEditOutcome, PreflightError> {
        let _lease = self.acquire_lease().await;
        let plan = self.prepare_retained_locked(input)?;
        Ok(self.commit_prepared(&plan))
    }

    #[allow(dead_code)]
    async fn prepare_retained_inner(
        &self,
        input: &ToolInput,
    ) -> Result<PreparedMultiFilePlan, PreflightError> {
        let _lease = self.acquire_lease().await;
        self.prepare_retained_locked(input)
    }

    fn prepare_retained_locked(
        &self,
        input: &ToolInput,
    ) -> Result<PreparedMultiFilePlan, PreflightError> {
        let request = MultiFileRequest::parse(input)?;
        let mut plan = self.prepare_tool_locked(request)?;
        let targets = std::mem::take(&mut plan.targets);
        let mut temporaries = Vec::with_capacity(targets.len());
        for (_index, prepared) in targets.iter().enumerate() {
            #[cfg(not(test))]
            let _ = _index;
            #[cfg(test)]
            test_hook::check(&self.root, TestPhase::BeforeTemporaryCreate, _index)?;
            match OwnedTemporary::create(&prepared.target, &prepared.postimage) {
                Ok(temporary) => {
                    temporaries.push(temporary);
                    #[cfg(test)]
                    if let Err(error) =
                        test_hook::check(&self.root, TestPhase::AfterTemporaryCreate, _index)
                    {
                        cleanup_owned_temporaries(&temporaries);
                        return Err(error);
                    }
                    #[cfg(test)]
                    if let Err(error) =
                        test_hook::check(&self.root, TestPhase::AfterTemporaryWrite, _index)
                    {
                        cleanup_owned_temporaries(&temporaries);
                        return Err(error);
                    }
                    #[cfg(test)]
                    if let Err(error) =
                        test_hook::check(&self.root, TestPhase::AfterTemporaryVerify, _index)
                    {
                        cleanup_owned_temporaries(&temporaries);
                        return Err(error);
                    }
                }
                Err(error) => {
                    cleanup_owned_temporaries(&temporaries);
                    return Err(error);
                }
            }
        }
        let _target_count = targets.len();
        plan.targets = targets;
        plan.temporaries = temporaries;
        #[cfg(test)]
        test_hook::check(
            &self.root,
            TestPhase::BeforeGlobalRevalidation,
            _target_count,
        )?;
        if let Err(error) = self.revalidate_pre_commit(&plan) {
            plan.cleanup_temporaries();
            return Err(error);
        }
        Ok(plan)
    }

    fn prepare_tool_locked(
        &self,
        request: MultiFileRequest,
    ) -> Result<PreparedMultiFilePlan, PreflightError> {
        self.revalidate_repository()?;
        let repository = self.repository_observation()?;
        let targets = self.capture_prepared_targets(request.targets)?;
        Ok(PreparedMultiFilePlan {
            repository,
            targets,
            temporaries: Vec::new(),
        })
    }

    fn prepare_human_locked(
        &self,
        request: HumanMultiFileRequest,
    ) -> Result<PreparedMultiFilePlan, PreflightError> {
        self.revalidate_repository()?;
        let repository = self.repository_observation()?;
        let mut seen_logical = HashSet::new();
        let mut targets = Vec::with_capacity(request.targets.len());
        for request_target in request.targets {
            let logical = request_target.path;
            if !seen_logical.insert(logical.clone()) {
                return Err(PreflightError::InvalidTarget("duplicate logical target"));
            }
            targets.push(RequestTarget {
                path: logical,
                expected_sha256: None,
                expected_length: None,
                replacements: request_target.replacements,
            });
        }
        self.capture_prepared_targets(targets).map(|mut targets| {
            targets.sort_by(|a, b| {
                a.canonical_logical
                    .as_bytes()
                    .cmp(b.canonical_logical.as_bytes())
            });
            PreparedMultiFilePlan {
                repository,
                targets,
                temporaries: Vec::new(),
            }
        })
    }

    fn capture_prepared_targets(
        &self,
        request_targets: Vec<RequestTarget>,
    ) -> Result<Vec<PreparedTarget>, PreflightError> {
        let mut seen_canonical = HashSet::new();
        let mut seen_identity = HashSet::new();
        let mut total_image_material = 0usize;
        let mut total_replacements = 0usize;
        let mut targets = Vec::with_capacity(request_targets.len());
        for request_target in request_targets {
            let target = SafeTarget::capture(&self.root, &request_target.path)?;
            self.boundary
                .validate_existing(&target.path)
                .map_err(|_| PreflightError::Precondition("nested repository boundary"))?;
            if !seen_canonical.insert(target.canonical_logical.clone()) {
                return Err(PreflightError::InvalidTarget("duplicate canonical target"));
            }
            if !seen_identity.insert(target.identity.clone()) {
                return Err(PreflightError::InvalidTarget(
                    "duplicate underlying file identity",
                ));
            }
            self.validate_git_target(&target)?;
            self.validate_worktree_clean(&target)?;
            let original = read_bounded(&target.path, MAX_FILE_BYTES)
                .map_err(|_| PreflightError::Precondition("could not read bounded target"))?;
            validate_snapshot(&original, &request_target)?;
            total_image_material = checked_total(
                total_image_material,
                original.len(),
                MAX_AGGREGATE_BYTES,
                "aggregate original/postimage bytes",
            )?;
            total_replacements = checked_total(
                total_replacements,
                request_target.replacements.len(),
                MAX_AGGREGATE_REPLACEMENTS,
                "aggregate replacements",
            )?;
            let postimage = build_postimage(&original, &request_target.replacements)?;
            total_image_material = checked_total(
                total_image_material,
                postimage.len(),
                MAX_AGGREGATE_BYTES,
                "aggregate original/postimage bytes",
            )?;
            let ranges = resolve_replacement_ranges(&original, &request_target.replacements)?;
            let mut prepared = PreparedTarget {
                canonical_logical: target.canonical_logical.clone(),
                target,
                original,
                postimage,
                replacements: request_target.replacements,
                ranges,
            };
            canonicalize_prepared_target(&mut prepared);
            targets.push(prepared);
        }
        targets.sort_by(|a, b| {
            a.canonical_logical
                .as_bytes()
                .cmp(b.canonical_logical.as_bytes())
        });
        Ok(targets)
    }

    fn commit_prepared(&self, plan: &PreparedMultiFilePlan) -> MultiFileEditOutcome {
        let mut effects = plan
            .targets
            .iter()
            .map(|target| MultiFileEffect {
                logical_path: target.canonical_logical.clone(),
                state: MultiFileEffectState::NotAttempted,
            })
            .collect::<Vec<_>>();
        for index in 0..plan.targets.len() {
            let committed = index;
            if self.revalidate_commit_state(plan, committed).is_err() {
                return self.classify_stop(plan, &mut effects, committed, index, false);
            }
            #[cfg(test)]
            if test_commit_hook::take(&self.root, CommitTestPhase::BeforeNativeReplacement, index) {
                return self.classify_stop(plan, &mut effects, committed, index, false);
            }
            #[cfg(test)]
            test_commit_hook::attempt(&self.root, index);
            #[cfg(feature = "live-test-support")]
            live_test_multi_file_native_attempts::record(&self.root, index);
            #[cfg(test)]
            if test_commit_hook::take(&self.root, CommitTestPhase::KnownNoEffectFailure, index) {
                effects[index].state = MultiFileEffectState::UnchangedVerified;
                return self.classify_stop(plan, &mut effects, committed, index, true);
            }

            let target = &plan.targets[index];
            let temporary = &plan.temporaries[index];
            let native = replace_once(&temporary.path, &target.target.path);
            #[cfg(test)]
            if test_commit_hook::take(&self.root, CommitTestPhase::UncertainNativeOutcome, index) {
                return self.uncertain(effects);
            }
            if native.is_err() {
                // A failed native call is not proof of no effect.  Only a full
                // bounded re-observation may classify it as known unchanged.
                if self.target_matches(target, &target.original).is_ok()
                    && temporary.revalidate().is_ok()
                {
                    effects[index].state = MultiFileEffectState::UnchangedVerified;
                    return self.classify_stop(plan, &mut effects, committed, index, true);
                }
                return self.uncertain(effects);
            }
            #[cfg(test)]
            if test_commit_hook::take(
                &self.root,
                CommitTestPhase::AfterNativeBeforeCertification,
                index,
            ) {
                return self.uncertain(effects);
            }
            if self.target_matches(target, &target.postimage).is_err()
                || self.revalidate_repository().is_err()
                || self.repository_observation().ok().as_ref() != Some(&plan.repository)
            {
                return self.uncertain(effects);
            }
            effects[index].state = MultiFileEffectState::CommittedVerified;
            #[cfg(test)]
            if test_commit_hook::take(&self.root, CommitTestPhase::AfterCertification, index) {
                return self.classify_stop(plan, &mut effects, index + 1, index + 1, false);
            }
        }
        MultiFileEditOutcome {
            status: MultiFileEditStatus::Ok,
            effects,
        }
    }

    fn revalidate_commit_state(
        &self,
        plan: &PreparedMultiFilePlan,
        committed: usize,
    ) -> Result<(), PreflightError> {
        self.revalidate_repository()?;
        if self.repository_observation()? != plan.repository {
            return Err(PreflightError::Precondition(
                "repository/index/HEAD/refs changed",
            ));
        }
        for (index, prepared) in plan.targets.iter().enumerate() {
            prepared.target.revalidate_parent(&self.root)?;
            let expected = if index < committed {
                &prepared.postimage
            } else {
                &prepared.original
            };
            self.target_matches(prepared, expected)?;
            if index >= committed {
                self.validate_git_target(&prepared.target)?;
                self.validate_worktree_clean(&prepared.target)?;
                plan.temporaries[index].revalidate()?;
            }
        }
        Ok(())
    }

    fn target_matches(
        &self,
        prepared: &PreparedTarget,
        expected: &[u8],
    ) -> Result<(), PreflightError> {
        prepared.target.revalidate_parent(&self.root)?;
        self.boundary
            .validate_existing(&prepared.target.path)
            .map_err(|_| PreflightError::Precondition("nested repository boundary"))?;
        reject_link_or_reparse(&prepared.target.path, "target")?;
        let metadata = fs::metadata(&prepared.target.path).map_err(fs_error)?;
        if !metadata.is_file() || Identity::capture(&prepared.target.path)?.link_count != 1 {
            return Err(PreflightError::Precondition(
                "target identity is no longer trusted",
            ));
        }
        reject_unsupported_file_attributes(&metadata)?;
        #[cfg(unix)]
        if unix_mode(&metadata) != prepared.target.mode & 0o7777 {
            return Err(PreflightError::Precondition("target mode changed"));
        }
        let current = read_bounded(&prepared.target.path, MAX_FILE_BYTES)
            .map_err(|_| PreflightError::Precondition("could not reread bounded target"))?;
        if current != expected {
            return Err(PreflightError::Precondition("target bytes changed"));
        }
        Ok(())
    }

    fn classify_stop(
        &self,
        plan: &PreparedMultiFilePlan,
        effects: &mut [MultiFileEffect],
        committed: usize,
        stopped: usize,
        native_failed: bool,
    ) -> MultiFileEditOutcome {
        if !self.prove_inventory(plan, effects, committed, stopped) {
            return self.uncertain(effects.to_vec());
        }
        if !cleanup_uncommitted_temporaries(plan, committed) {
            return self.uncertain(effects.to_vec());
        }
        let status = if committed == 0 && native_failed {
            MultiFileEditStatus::FailedKnownNoEffect
        } else if committed == 0 {
            MultiFileEditStatus::PreconditionFailed
        } else {
            MultiFileEditStatus::PartialEffect
        };
        MultiFileEditOutcome {
            status,
            effects: effects.to_vec(),
        }
    }

    fn prove_inventory(
        &self,
        plan: &PreparedMultiFilePlan,
        effects: &mut [MultiFileEffect],
        committed: usize,
        stopped: usize,
    ) -> bool {
        if self.revalidate_repository().is_err()
            || self.repository_observation().ok().as_ref() != Some(&plan.repository)
        {
            return false;
        }
        for (index, prepared) in plan.targets.iter().enumerate() {
            let expected = if index < committed {
                &prepared.postimage
            } else {
                &prepared.original
            };
            if self.target_matches(prepared, expected).is_err() {
                return false;
            }
            if index >= committed && index <= stopped {
                effects[index].state = MultiFileEffectState::UnchangedVerified;
            }
        }
        true
    }

    fn uncertain(&self, mut effects: Vec<MultiFileEffect>) -> MultiFileEditOutcome {
        for effect in &mut effects {
            if effect.state == MultiFileEffectState::NotAttempted {
                effect.state = MultiFileEffectState::Uncertain;
                break;
            }
        }
        MultiFileEditOutcome {
            status: MultiFileEditStatus::Uncertain,
            effects,
        }
    }

    #[cfg(test)]
    async fn prepare_retained_for_test(
        &self,
        input: &ToolInput,
    ) -> Result<PreparedMultiFilePlan, PreflightError> {
        self.prepare_retained_inner(input).await
    }

    #[cfg(any(test, feature = "live-test-support"))]
    pub(crate) fn test_root(&self) -> &Path {
        &self.root
    }

    /// The Task 094B commit loop calls this before every native commit. It has
    /// no replacement operation and is exercised by `prepare` above.
    pub(crate) fn revalidate_pre_commit(
        &self,
        plan: &PreparedMultiFilePlan,
    ) -> Result<(), PreflightError> {
        self.revalidate_repository()?;
        if self.repository_observation()? != plan.repository {
            return Err(PreflightError::Precondition(
                "repository/index/HEAD/refs changed",
            ));
        }
        for prepared in &plan.targets {
            prepared.target.revalidate(&self.root)?;
            let current = read_bounded(&prepared.target.path, MAX_FILE_BYTES)
                .map_err(|_| PreflightError::Precondition("could not reread bounded target"))?;
            if current != prepared.original {
                return Err(PreflightError::Precondition("target preimage changed"));
            }
            self.validate_git_target(&prepared.target)?;
            self.validate_worktree_clean(&prepared.target)?;
        }
        for temporary in &plan.temporaries {
            temporary.revalidate()?;
        }
        Ok(())
    }

    fn revalidate_repository(&self) -> Result<(), PreflightError> {
        reject_link_or_reparse(&self.root, "repository root")?;
        let root = canonical_directory(&self.root, "repository root")?;
        if !paths_equivalent(&root, &self.root) || Identity::capture(&root)? != self.root_identity {
            return Err(PreflightError::Precondition(
                "repository root identity changed",
            ));
        }
        reject_link_or_reparse(&self.dot_git, "repository metadata")?;
        if Identity::capture(&self.dot_git)? != self.dot_git_identity {
            return Err(PreflightError::Precondition(
                "repository metadata identity changed",
            ));
        }
        let git = canonical_file(&self.git, "Git executable")?;
        if !paths_equivalent(&git, &self.git) || Identity::capture(&git)? != self.git_identity {
            return Err(PreflightError::Precondition(
                "Git executable identity changed",
            ));
        }
        Ok(())
    }

    fn repository_observation(&self) -> Result<RepositoryObservation, PreflightError> {
        let index = fs::read(self.dot_git.join("index"))
            .map_err(|_| PreflightError::Precondition("could not observe raw index"))?;
        let head = fs::read(self.dot_git.join("HEAD"))
            .map_err(|_| PreflightError::Precondition("could not observe HEAD"))?;
        let refs = self.git_output(&["for-each-ref", "--format=%(refname)%00%(objectname)%00"])?;
        Ok(RepositoryObservation { index, head, refs })
    }

    fn validate_git_target(&self, target: &SafeTarget) -> Result<(), PreflightError> {
        let path = target.canonical_logical.as_str();
        let head = self.git_output(&["ls-tree", "-z", "HEAD", "--", path])?;
        let index =
            self.git_output(&["--literal-pathspecs", "ls-files", "-s", "-z", "--", path])?;
        let tag = self.git_output(&["--literal-pathspecs", "ls-files", "-v", "-z", "--", path])?;
        let head_entry = parse_head_entry(&head, path.as_bytes())?;
        let index_entry = parse_index_entry(&index, path.as_bytes())?;
        if head_entry != index_entry {
            return Err(PreflightError::Precondition(
                "target has staged or index divergence",
            ));
        }
        if !is_supported_index_tag(&tag) {
            return Err(PreflightError::Precondition(
                "target has unsupported sparse or index flags",
            ));
        }
        Ok(())
    }

    /// Uses a fixed host-owned Git command. `git diff-files --quiet` returns
    /// one for a dirty worktree target, which fails closed through `git_output`.
    fn validate_worktree_clean(&self, target: &SafeTarget) -> Result<(), PreflightError> {
        self.git_output(&[
            "--literal-pathspecs",
            "diff-files",
            "--quiet",
            "--",
            target.canonical_logical.as_str(),
        ])
        .map(|_| ())
    }

    fn git_output(&self, args: &[&str]) -> Result<Vec<u8>, PreflightError> {
        self.revalidate_repository()?;
        let output = std::process::Command::new(&self.git)
            .args(args)
            .current_dir(&self.root)
            .env_clear()
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_TERMINAL_PROMPT", "0")
            .output()
            .map_err(|_| PreflightError::Precondition("Git observation failed"))?;
        if !output.status.success() {
            return Err(PreflightError::Precondition("Git observation failed"));
        }
        Ok(output.stdout)
    }
}

#[cfg(feature = "live-test-support")]
pub mod live_test_multi_file_native_attempts {
    use std::{
        collections::HashMap,
        path::{Path, PathBuf},
        sync::{Mutex, OnceLock},
    };

    static ATTEMPTS: OnceLock<Mutex<HashMap<(PathBuf, usize), usize>>> = OnceLock::new();

    fn key(root: &Path) -> PathBuf {
        std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf())
    }

    pub fn record(root: &Path, index: usize) {
        let mut attempts = ATTEMPTS
            .get_or_init(|| Mutex::new(HashMap::new()))
            .lock()
            .unwrap();
        *attempts.entry((key(root), index)).or_default() += 1;
    }

    pub fn count(root: &Path, index: usize) -> usize {
        ATTEMPTS
            .get_or_init(|| Mutex::new(HashMap::new()))
            .lock()
            .unwrap()
            .get(&(key(root), index))
            .copied()
            .unwrap_or(0)
    }

    pub fn clear(root: &Path) {
        ATTEMPTS
            .get_or_init(|| Mutex::new(HashMap::new()))
            .lock()
            .unwrap()
            .retain(|(path, _), _| path != &key(root));
    }
}

fn cleanup_uncommitted_temporaries(plan: &PreparedMultiFilePlan, committed: usize) -> bool {
    plan.temporaries[committed..]
        .iter()
        .all(|temporary| temporary.remove().is_ok())
}

#[cfg(unix)]
fn unix_mode(metadata: &fs::Metadata) -> u32 {
    use std::os::unix::fs::MetadataExt;
    metadata.mode() & 0o7777
}

fn is_supported_index_tag(tag: &[u8]) -> bool {
    tag.starts_with(b"H ")
}

#[derive(Debug)]
pub(crate) enum PreflightError {
    InvalidTarget(&'static str),
    Precondition(&'static str),
}

impl PreflightError {
    pub(crate) fn public_status(self) -> &'static str {
        match self {
            Self::InvalidTarget(reason) => {
                let _ = reason;
                "invalid_target"
            }
            Self::Precondition(reason) => {
                let _ = reason;
                "precondition_failed"
            }
        }
    }
}

/// Private result carried to the later policy integration layer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MultiFileEditOutcome {
    pub(crate) status: MultiFileEditStatus,
    pub(crate) effects: Vec<MultiFileEffect>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MultiFileEditStatus {
    Ok,
    #[allow(dead_code)]
    InvalidTarget,
    #[allow(dead_code)]
    PreconditionFailed,
    FailedKnownNoEffect,
    PartialEffect,
    Uncertain,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MultiFileEffect {
    pub(crate) logical_path: String,
    pub(crate) state: MultiFileEffectState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MultiFileEffectState {
    CommittedVerified,
    UnchangedVerified,
    NotAttempted,
    Uncertain,
}
struct MultiFileRequest {
    targets: Vec<RequestTarget>,
}
struct RequestTarget {
    path: String,
    expected_sha256: Option<String>,
    expected_length: Option<usize>,
    replacements: Vec<Replacement>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
struct Replacement {
    old: String,
    new: String,
}

impl MultiFileRequest {
    fn parse(input: &ToolInput) -> Result<Self, PreflightError> {
        let serialized = serde_json::to_vec(&input.0)
            .map_err(|_| PreflightError::InvalidTarget("request serialization failed"))?;
        if serialized.len() > MAX_SERIALIZED_REQUEST_BYTES {
            return Err(PreflightError::InvalidTarget(
                "serialized request exceeds limit",
            ));
        }
        let object = input
            .0
            .as_object()
            .ok_or(PreflightError::InvalidTarget("request must be object"))?;
        exact_fields(object, &["targets"])?;
        let values = object
            .get("targets")
            .and_then(Value::as_array)
            .ok_or(PreflightError::InvalidTarget("targets must be array"))?;
        if values.is_empty() || values.len() > MAX_TARGETS {
            return Err(PreflightError::InvalidTarget(
                "targets must contain one through four items",
            ));
        }
        let targets = values
            .iter()
            .map(parse_target)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self { targets })
    }
}

fn parse_target(value: &Value) -> Result<RequestTarget, PreflightError> {
    let object = value
        .as_object()
        .ok_or(PreflightError::InvalidTarget("target must be object"))?;
    exact_fields(
        object,
        &[
            "path",
            "expected_file_sha256",
            "expected_file_byte_length",
            "replacements",
        ],
    )?;
    let path = required_string(object, "path")?.to_owned();
    validate_logical_path(&path)?;
    let expected_sha256 = required_string(object, "expected_file_sha256")?.to_owned();
    if !lower_sha256(&expected_sha256) {
        return Err(PreflightError::InvalidTarget(
            "expected SHA must be lowercase hexadecimal",
        ));
    }
    let expected_length = object
        .get("expected_file_byte_length")
        .and_then(Value::as_u64)
        .and_then(|n| usize::try_from(n).ok())
        .ok_or(PreflightError::InvalidTarget(
            "expected length must be integer",
        ))?;
    if expected_length > MAX_FILE_BYTES {
        return Err(PreflightError::InvalidTarget(
            "expected length exceeds file limit",
        ));
    }
    let values = object
        .get("replacements")
        .and_then(Value::as_array)
        .ok_or(PreflightError::InvalidTarget("replacements must be array"))?;
    if values.is_empty() || values.len() > MAX_REPLACEMENTS_PER_TARGET {
        return Err(PreflightError::InvalidTarget(
            "replacement count exceeds limit",
        ));
    }
    let replacements = values
        .iter()
        .map(parse_replacement)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(RequestTarget {
        path,
        expected_sha256: Some(expected_sha256),
        expected_length: Some(expected_length),
        replacements,
    })
}

fn parse_replacement(value: &Value) -> Result<Replacement, PreflightError> {
    let object = value
        .as_object()
        .ok_or(PreflightError::InvalidTarget("replacement must be object"))?;
    exact_fields(object, &["expected_old_text", "replacement_text"])?;
    let old = required_string(object, "expected_old_text")?.to_owned();
    let new = required_string(object, "replacement_text")?.to_owned();
    if old.is_empty()
        || old.len() > MAX_TEXT_BYTES
        || new.len() > MAX_TEXT_BYTES
        || old.contains('\0')
        || new.contains('\0')
    {
        return Err(PreflightError::InvalidTarget(
            "replacement text exceeds limits or contains NUL",
        ));
    }
    Ok(Replacement { old, new })
}

fn exact_fields(object: &Map<String, Value>, expected: &[&str]) -> Result<(), PreflightError> {
    if object.len() != expected.len()
        || object
            .keys()
            .any(|field| !expected.contains(&field.as_str()))
    {
        return Err(PreflightError::InvalidTarget("unexpected or missing field"));
    }
    Ok(())
}
fn required_string<'a>(
    object: &'a Map<String, Value>,
    name: &str,
) -> Result<&'a str, PreflightError> {
    object
        .get(name)
        .and_then(Value::as_str)
        .ok_or(PreflightError::InvalidTarget(
            "required field must be string",
        ))
}
fn validate_logical_path(path: &str) -> Result<(), PreflightError> {
    if path.is_empty()
        || path.len() > MAX_PATH_BYTES
        || path.contains('\0')
        || path.contains('\\')
        || path.contains(':')
        || path.starts_with('/')
        || path.starts_with("//")
        || Path::new(path).is_absolute()
    {
        return Err(PreflightError::InvalidTarget(
            "path is not a logical relative path",
        ));
    }
    if path.split('/').any(|part| {
        part.is_empty() || matches!(part, "." | "..") || part.eq_ignore_ascii_case(".git")
    }) {
        return Err(PreflightError::InvalidTarget(
            "path has unsupported component",
        ));
    }
    Ok(())
}
fn lower_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || matches!(b, b'a'..=b'f'))
}
fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn checked_total(
    total: usize,
    add: usize,
    limit: usize,
    name: &'static str,
) -> Result<usize, PreflightError> {
    let value = total
        .checked_add(add)
        .ok_or(PreflightError::InvalidTarget(name))?;
    if value > limit {
        Err(PreflightError::InvalidTarget(name))
    } else {
        Ok(value)
    }
}

#[derive(Clone, Eq, PartialEq)]
struct GitEntry {
    mode: Vec<u8>,
    object: Vec<u8>,
}
fn parse_head_entry(bytes: &[u8], path: &[u8]) -> Result<GitEntry, PreflightError> {
    let record = only_record(bytes)?;
    let tab = record
        .iter()
        .position(|b| *b == b'\t')
        .ok_or(PreflightError::Precondition("malformed HEAD entry"))?;
    if &record[tab + 1..] != path {
        return Err(PreflightError::Precondition("unexpected HEAD target"));
    }
    let parts = record[..tab].split(|b| *b == b' ').collect::<Vec<_>>();
    if parts.len() != 3 || parts[1] != b"blob" || !matches!(parts[0], b"100644" | b"100755") {
        return Err(PreflightError::Precondition(
            "target is not regular HEAD blob",
        ));
    }
    Ok(GitEntry {
        mode: parts[0].to_vec(),
        object: parts[2].to_vec(),
    })
}
fn parse_index_entry(bytes: &[u8], path: &[u8]) -> Result<GitEntry, PreflightError> {
    let record = only_record(bytes)?;
    let tab = record
        .iter()
        .position(|b| *b == b'\t')
        .ok_or(PreflightError::Precondition("malformed index entry"))?;
    if &record[tab + 1..] != path {
        return Err(PreflightError::Precondition("unexpected index target"));
    }
    let parts = record[..tab].split(|b| *b == b' ').collect::<Vec<_>>();
    if parts.len() != 3 || parts[2] != b"0" || !matches!(parts[0], b"100644" | b"100755") {
        return Err(PreflightError::Precondition(
            "target is not regular stage-0 index entry",
        ));
    }
    Ok(GitEntry {
        mode: parts[0].to_vec(),
        object: parts[1].to_vec(),
    })
}
fn only_record(bytes: &[u8]) -> Result<&[u8], PreflightError> {
    if !bytes.ends_with(&[0]) {
        return Err(PreflightError::Precondition("malformed Git observation"));
    }
    let records = bytes[..bytes.len() - 1]
        .split(|b| *b == 0)
        .collect::<Vec<_>>();
    if records.len() != 1 {
        Err(PreflightError::Precondition(
            "expected exactly one Git entry",
        ))
    } else {
        Ok(records[0])
    }
}

struct SafeTarget {
    path: PathBuf,
    parent: PathBuf,
    canonical_logical: String,
    identity: Identity,
    parent_identity: Identity,
    #[cfg(unix)]
    mode: u32,
}
impl SafeTarget {
    fn capture(root: &Path, logical: &str) -> Result<Self, PreflightError> {
        let relative = Path::new(logical);
        let mut current = root.to_path_buf();
        for component in relative.components() {
            let Component::Normal(component) = component else {
                return Err(PreflightError::InvalidTarget("non-normal path component"));
            };
            current.push(component);
            reject_link_or_reparse(&current, "target path component")?;
        }
        let path = fs::canonicalize(&current)
            .map_err(|_| PreflightError::InvalidTarget("target missing or cannot be resolved"))?;
        if !is_beneath(&path, root) || !paths_equivalent(&path, &current) {
            return Err(PreflightError::InvalidTarget(
                "target aliases or escapes repository",
            ));
        }
        let metadata = fs::metadata(&path)
            .map_err(|_| PreflightError::InvalidTarget("target metadata unavailable"))?;
        if !metadata.is_file() {
            return Err(PreflightError::Precondition("target is not regular file"));
        }
        reject_unsupported_file_attributes(&metadata)?;
        let identity = Identity::capture(&path)?;
        if identity.link_count > 1 {
            return Err(PreflightError::Precondition(
                "hard-linked targets unsupported",
            ));
        }
        let parent = path
            .parent()
            .ok_or(PreflightError::Precondition("target has no parent"))?
            .to_path_buf();
        Ok(Self {
            canonical_logical: logical.replace('\\', "/"),
            parent_identity: Identity::capture(&parent)?,
            #[cfg(unix)]
            mode: {
                use std::os::unix::fs::MetadataExt;
                metadata.mode()
            },
            parent,
            identity,
            path,
        })
    }
    fn revalidate(&self, root: &Path) -> Result<(), PreflightError> {
        let current = Self::capture(root, &self.canonical_logical)?;
        if !paths_equivalent(&current.path, &self.path)
            || current.identity != self.identity
            || current.parent_identity != self.parent_identity
        {
            Err(PreflightError::Precondition(
                "target or parent identity changed",
            ))
        } else {
            Ok(())
        }
    }

    fn revalidate_parent(&self, root: &Path) -> Result<(), PreflightError> {
        reject_reparse_ancestry(&self.parent, "target parent ancestry")?;
        let parent = canonical_directory(&self.parent, "target parent")?;
        if !is_beneath(root, &parent)
            || !paths_equivalent(&parent, &self.parent)
            || Identity::capture(&parent)? != self.parent_identity
        {
            return Err(PreflightError::Precondition(
                "target parent identity changed",
            ));
        }
        Ok(())
    }
}

fn validate_snapshot(bytes: &[u8], request: &RequestTarget) -> Result<(), PreflightError> {
    if let (Some(expected_length), Some(expected_sha256)) =
        (&request.expected_length, &request.expected_sha256)
        && (bytes.len() != *expected_length || sha256(bytes) != *expected_sha256)
    {
        return Err(PreflightError::Precondition(
            "target SHA or length mismatch",
        ));
    }
    if bytes.contains(&0) || std::str::from_utf8(bytes).is_err() {
        return Err(PreflightError::Precondition(
            "target is not NUL-free strict UTF-8",
        ));
    }
    Ok(())
}
fn build_postimage(
    original: &[u8],
    replacements: &[Replacement],
) -> Result<Vec<u8>, PreflightError> {
    let text = std::str::from_utf8(original)
        .map_err(|_| PreflightError::Precondition("target is not UTF-8"))?;
    let mut definitions = HashSet::new();
    let mut ranges = Vec::with_capacity(replacements.len());
    for replacement in replacements {
        if replacement.old == replacement.new {
            return Err(PreflightError::Precondition("replacement is a no-op"));
        }
        if !definitions.insert((replacement.old.as_str(), replacement.new.as_str())) {
            return Err(PreflightError::Precondition("duplicate replacement"));
        }
        let matches = text.match_indices(&replacement.old).collect::<Vec<_>>();
        if matches.len() != 1 {
            return Err(PreflightError::Precondition(
                "replacement must match exactly once",
            ));
        }
        let (start, found) = matches[0];
        ranges.push((start, start + found.len(), replacement.new.as_bytes()));
    }
    ranges.sort_by_key(|range| range.0);
    if ranges.windows(2).any(|pair| pair[1].0 < pair[0].1) {
        return Err(PreflightError::Precondition("replacement ranges overlap"));
    }
    let mut output = Vec::new();
    let mut cursor = 0;
    for (start, end, new) in ranges {
        output.extend_from_slice(&original[cursor..start]);
        output.extend_from_slice(new);
        cursor = end;
        if output.len() > MAX_FILE_BYTES {
            return Err(PreflightError::Precondition("postimage exceeds file limit"));
        }
    }
    output.extend_from_slice(&original[cursor..]);
    if output.len() > MAX_FILE_BYTES {
        Err(PreflightError::Precondition("postimage exceeds file limit"))
    } else {
        Ok(output)
    }
}

fn resolve_replacement_ranges(
    original: &[u8],
    replacements: &[Replacement],
) -> Result<Vec<ResolvedRange>, PreflightError> {
    let text = std::str::from_utf8(original)
        .map_err(|_| PreflightError::Precondition("target is not UTF-8"))?;
    let mut ranges = Vec::with_capacity(replacements.len());
    for (replacement_index, replacement) in replacements.iter().enumerate() {
        let matches = text.match_indices(&replacement.old).collect::<Vec<_>>();
        if matches.len() != 1 {
            return Err(PreflightError::Precondition(
                "replacement must match exactly once",
            ));
        }
        let (start, found) = matches[0];
        ranges.push(ResolvedRange {
            start,
            end: start + found.len(),
            replacement_index,
        });
    }
    ranges.sort_unstable_by_key(|range| range.start);
    if ranges.windows(2).any(|pair| pair[1].start < pair[0].end) {
        return Err(PreflightError::Precondition("replacement ranges overlap"));
    }
    Ok(ranges)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ResolvedRange {
    start: usize,
    end: usize,
    replacement_index: usize,
}

fn canonicalize_prepared_target(target: &mut PreparedTarget) {
    let replacements = target
        .ranges
        .iter()
        .map(|range| target.replacements[range.replacement_index].clone())
        .collect::<Vec<_>>();
    target.ranges = target
        .ranges
        .iter()
        .enumerate()
        .map(|(replacement_index, range)| ResolvedRange {
            start: range.start,
            end: range.end,
            replacement_index,
        })
        .collect();
    target.replacements = replacements;
}

#[derive(Eq, PartialEq)]
struct RepositoryObservation {
    index: Vec<u8>,
    head: Vec<u8>,
    refs: Vec<u8>,
}
struct PreparedTarget {
    canonical_logical: String,
    target: SafeTarget,
    original: Vec<u8>,
    postimage: Vec<u8>,
    #[allow(dead_code)]
    replacements: Vec<Replacement>,
    ranges: Vec<ResolvedRange>,
}
pub(crate) struct PreparedMultiFilePlan {
    repository: RepositoryObservation,
    targets: Vec<PreparedTarget>,
    temporaries: Vec<OwnedTemporary>,
}
impl PreparedMultiFilePlan {
    fn cleanup_temporaries(&self) {
        for temp in &self.temporaries {
            let _ = temp.remove();
        }
    }
    #[allow(dead_code)]
    fn without_temporaries(mut self) -> Self {
        self.temporaries.clear();
        self
    }
    #[cfg(test)]
    fn paths(&self) -> Vec<&str> {
        self.targets
            .iter()
            .map(|t| t.canonical_logical.as_str())
            .collect()
    }
    #[cfg(test)]
    fn temporary_paths(&self) -> Vec<PathBuf> {
        self.temporaries
            .iter()
            .map(|temp| temp.path.clone())
            .collect()
    }
}

fn cleanup_owned_temporaries(temporaries: &[OwnedTemporary]) {
    for temporary in temporaries {
        let _ = temporary.remove();
    }
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TestPhase {
    BeforeTemporaryCreate,
    AfterTemporaryCreate,
    AfterTemporaryWrite,
    AfterTemporaryVerify,
    BeforeGlobalRevalidation,
}

#[cfg(test)]
mod test_hook {
    use std::{
        collections::HashSet,
        fs,
        sync::{Mutex, OnceLock},
    };

    use super::{PreflightError, TestPhase};

    static HOOKS: OnceLock<Mutex<Vec<(std::path::PathBuf, TestPhase, usize)>>> = OnceLock::new();
    static GLOBAL_TARGET_MUTATIONS: OnceLock<Mutex<HashSet<std::path::PathBuf>>> = OnceLock::new();

    pub(super) fn install(root: &std::path::Path, phase: TestPhase, index: usize) {
        HOOKS
            .get_or_init(|| Mutex::new(Vec::new()))
            .lock()
            .unwrap()
            .push((root.to_path_buf(), phase, index));
    }

    pub(super) fn clear(root: &std::path::Path) {
        HOOKS
            .get_or_init(|| Mutex::new(Vec::new()))
            .lock()
            .unwrap()
            .retain(|(hook_root, _, _)| hook_root != root);
        GLOBAL_TARGET_MUTATIONS
            .get_or_init(|| Mutex::new(HashSet::new()))
            .lock()
            .unwrap()
            .remove(root);
    }

    pub(super) fn install_global_target_mutation(root: &std::path::Path) {
        GLOBAL_TARGET_MUTATIONS
            .get_or_init(|| Mutex::new(HashSet::new()))
            .lock()
            .unwrap()
            .insert(root.to_path_buf());
    }

    pub(super) fn check(
        root: &std::path::Path,
        phase: TestPhase,
        index: usize,
    ) -> Result<(), PreflightError> {
        if phase == TestPhase::BeforeGlobalRevalidation
            && GLOBAL_TARGET_MUTATIONS
                .get_or_init(|| Mutex::new(HashSet::new()))
                .lock()
                .unwrap()
                .remove(root)
        {
            fs::write(root.join("a.txt"), b"test global revalidation mutation\n")
                .map_err(|_| PreflightError::Precondition("test global mutation failed"))?;
        }
        if HOOKS
            .get_or_init(|| Mutex::new(Vec::new()))
            .lock()
            .unwrap()
            .contains(&(root.to_path_buf(), phase, index))
        {
            return Err(PreflightError::Precondition(
                "test temporary preparation failure",
            ));
        }
        Ok(())
    }
}

struct OwnedTemporary {
    path: PathBuf,
    identity: Identity,
    expected: Vec<u8>,
}
impl OwnedTemporary {
    fn create(target: &SafeTarget, bytes: &[u8]) -> Result<Self, PreflightError> {
        for _ in 0..32 {
            let path = target
                .parent
                .join(format!(".rah-repo-edit-files-{}.tmp", Uuid::new_v4()));
            let mut file = match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(f) => f,
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(_) => return Err(PreflightError::Precondition("temporary creation failed")),
            };
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&path, fs::Permissions::from_mode(target.mode & 0o7777))
                    .map_err(|_| PreflightError::Precondition("temporary mode setup failed"))?;
            }
            file.write_all(bytes)
                .map_err(|_| PreflightError::Precondition("temporary write failed"))?;
            file.flush()
                .map_err(|_| PreflightError::Precondition("temporary flush failed"))?;
            drop(file);
            let identity = Identity::capture(&path)?;
            let temp = Self {
                path,
                identity,
                expected: bytes.to_vec(),
            };
            if let Err(error) = temp.revalidate() {
                let _ = temp.remove();
                return Err(error);
            }
            return Ok(temp);
        }
        Err(PreflightError::Precondition(
            "temporary name collision limit",
        ))
    }
    fn revalidate(&self) -> Result<(), PreflightError> {
        let metadata = fs::metadata(&self.path)
            .map_err(|_| PreflightError::Precondition("temporary disappeared"))?;
        if !metadata.is_file()
            || Identity::capture(&self.path)? != self.identity
            || read_bounded(&self.path, MAX_FILE_BYTES)
                .map_err(|_| PreflightError::Precondition("temporary unreadable"))?
                != self.expected
        {
            Err(PreflightError::Precondition(
                "temporary identity or content changed",
            ))
        } else {
            Ok(())
        }
    }
    fn remove(&self) -> Result<(), PreflightError> {
        self.revalidate()?;
        fs::remove_file(&self.path)
            .map_err(|_| PreflightError::Precondition("temporary safe cleanup failed"))
    }
}

/// Performs precisely one per-target native replacement. It is deliberately
/// private so no other capability can gain generic filesystem-write authority.
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
    // This native invocation is the commit point and is never retried.
    if unsafe {
        MoveFileExW(
            temporary.as_ptr(),
            target.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    } == 0
    {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CommitTestPhase {
    BeforeNativeReplacement,
    KnownNoEffectFailure,
    UncertainNativeOutcome,
    AfterNativeBeforeCertification,
    AfterCertification,
}

#[cfg(test)]
pub(crate) mod test_commit_hook {
    use super::CommitTestPhase;
    use std::{
        collections::HashMap,
        path::{Path, PathBuf},
        sync::{Mutex, OnceLock},
    };

    static HOOKS: OnceLock<Mutex<Vec<(PathBuf, CommitTestPhase, usize)>>> = OnceLock::new();
    static ATTEMPTS: OnceLock<Mutex<HashMap<(PathBuf, usize), usize>>> = OnceLock::new();

    pub(crate) fn install(root: &Path, phase: CommitTestPhase, index: usize) {
        HOOKS
            .get_or_init(|| Mutex::new(Vec::new()))
            .lock()
            .unwrap()
            .push((root.to_path_buf(), phase, index));
    }
    pub(super) fn take(root: &Path, phase: CommitTestPhase, index: usize) -> bool {
        let mut hooks = HOOKS.get_or_init(|| Mutex::new(Vec::new())).lock().unwrap();
        if let Some(position) = hooks
            .iter()
            .position(|hook| hook == &(root.to_path_buf(), phase, index))
        {
            hooks.remove(position);
            true
        } else {
            false
        }
    }
    pub(super) fn attempt(root: &Path, index: usize) {
        *ATTEMPTS
            .get_or_init(|| Mutex::new(HashMap::new()))
            .lock()
            .unwrap()
            .entry((root.to_path_buf(), index))
            .or_default() += 1;
    }
    pub(crate) fn attempts(root: &Path, index: usize) -> usize {
        *ATTEMPTS
            .get_or_init(|| Mutex::new(HashMap::new()))
            .lock()
            .unwrap()
            .get(&(root.to_path_buf(), index))
            .unwrap_or(&0)
    }
    pub(crate) fn clear(root: &Path) {
        HOOKS
            .get_or_init(|| Mutex::new(Vec::new()))
            .lock()
            .unwrap()
            .retain(|(path, _, _)| path != root);
        ATTEMPTS
            .get_or_init(|| Mutex::new(HashMap::new()))
            .lock()
            .unwrap()
            .retain(|(path, _), _| path != root);
    }
}

#[derive(Clone, Eq, Hash, PartialEq)]
struct Identity {
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
impl Identity {
    fn capture(path: &Path) -> Result<Self, PreflightError> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let meta = fs::metadata(path).map_err(fs_error)?;
            Ok(Self {
                device: meta.dev(),
                inode: meta.ino(),
                link_count: u32::try_from(meta.nlink()).unwrap_or(u32::MAX),
            })
        }
        #[cfg(windows)]
        {
            capture_windows_identity(path)
        }
        #[cfg(not(any(unix, windows)))]
        {
            let _ = path;
            Ok(Self { link_count: 1 })
        }
    }
}

#[cfg(windows)]
fn capture_windows_identity(path: &Path) -> Result<Identity, PreflightError> {
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
    if unsafe { GetFileInformationByHandle(handle, information.as_mut_ptr()) } == 0 {
        return Err(fs_error(std::io::Error::last_os_error()));
    }
    let information = unsafe { information.assume_init() };
    Ok(Identity {
        volume_serial: information.dwVolumeSerialNumber,
        file_index: (u64::from(information.nFileIndexHigh) << 32)
            | u64::from(information.nFileIndexLow),
        link_count: information.nNumberOfLinks,
    })
}
fn read_bounded(path: &Path, max: usize) -> Result<Vec<u8>, std::io::Error> {
    if fs::metadata(path)?.len() > u64::try_from(max).unwrap_or(u64::MAX) {
        return Err(std::io::Error::other("oversize"));
    }
    let bytes = fs::read(path)?;
    if bytes.len() > max {
        Err(std::io::Error::other("oversize"))
    } else {
        Ok(bytes)
    }
}
fn canonical_directory(path: &Path, _label: &str) -> Result<PathBuf, PreflightError> {
    let p = fs::canonicalize(path).map_err(fs_error)?;
    if !p.is_dir() {
        Err(PreflightError::Precondition("expected directory"))
    } else {
        Ok(p)
    }
}
fn canonical_file(path: &Path, label: &str) -> Result<PathBuf, PreflightError> {
    if !path.is_absolute() {
        return Err(PreflightError::Precondition(
            "configured file must be absolute",
        ));
    }
    reject_link_or_reparse(path, label)?;
    let p = fs::canonicalize(path).map_err(fs_error)?;
    if !fs::metadata(&p).map_err(fs_error)?.is_file() {
        Err(PreflightError::Precondition("expected regular file"))
    } else {
        Ok(p)
    }
}
fn reject_reparse_ancestry(path: &Path, label: &str) -> Result<(), PreflightError> {
    for a in path.ancestors() {
        if a.exists() {
            reject_link_or_reparse(a, label)?
        }
    }
    Ok(())
}
fn reject_link_or_reparse(path: &Path, _label: &str) -> Result<(), PreflightError> {
    let meta = fs::symlink_metadata(path).map_err(fs_error)?;
    if meta.file_type().is_symlink() {
        return Err(PreflightError::Precondition(
            "symbolic links are unsupported",
        ));
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes() & 0x400 != 0 {
            return Err(PreflightError::Precondition(
                "reparse points are unsupported",
            ));
        }
    }
    Ok(())
}

fn reject_unsupported_file_attributes(metadata: &fs::Metadata) -> Result<(), PreflightError> {
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
            return Err(PreflightError::Precondition(
                "target has unsupported file attributes",
            ));
        }
    }
    #[cfg(not(windows))]
    let _ = metadata;
    Ok(())
}

fn fs_error(_: std::io::Error) -> PreflightError {
    PreflightError::Precondition("filesystem validation failed")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::{
        process::Command,
        sync::atomic::{AtomicU64, Ordering},
    };

    static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    struct TestDirectory(PathBuf);
    impl TestDirectory {
        fn repository() -> (Self, PathBuf, PathBuf) {
            let id = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let base = std::env::temp_dir()
                .join(format!("rah-multi-preflight-{}-{id}", std::process::id()));
            let _ = fs::remove_dir_all(&base);
            fs::create_dir(&base).unwrap();
            let root = base.join("repository");
            fs::create_dir(&root).unwrap();
            let git_path = git_executable();
            git(&git_path, &root, &["init", "--quiet"]);
            for (name, bytes) in [
                ("a.txt", b"A old\n".as_slice()),
                ("m.txt", b"M old\n".as_slice()),
                ("z.txt", b"Z old\n".as_slice()),
                ("sentinel.txt", b"sentinel\n".as_slice()),
            ] {
                fs::write(root.join(name), bytes).unwrap();
            }
            git(&git_path, &root, &["add", "."]);
            git(
                &git_path,
                &root,
                &[
                    "-c",
                    "user.name=RAH",
                    "-c",
                    "user.email=rah@example.invalid",
                    "commit",
                    "--quiet",
                    "-m",
                    "initial",
                ],
            );
            (Self(base), git_path, root)
        }
    }
    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn git_executable() -> PathBuf {
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
    fn git(git: &Path, root: &Path, args: &[&str]) {
        let output = git_output(git, root, args);
        assert!(
            output.status.success(),
            "{:?}: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
    }
    fn git_output(git: &Path, root: &Path, args: &[&str]) -> std::process::Output {
        Command::new(git)
            .args(args)
            .current_dir(root)
            .output()
            .unwrap()
    }
    fn state(root: &Path) -> (Vec<Vec<u8>>, Vec<u8>, Vec<u8>, Vec<u8>) {
        (
            ["a.txt", "m.txt", "z.txt", "sentinel.txt"]
                .iter()
                .map(|p| fs::read(root.join(p)).unwrap())
                .collect(),
            fs::read(root.join(".git/index")).unwrap(),
            fs::read(root.join(".git/HEAD")).unwrap(),
            Command::new(git_executable())
                .args(["for-each-ref", "--format=%(refname)%00%(objectname)%00"])
                .current_dir(root)
                .output()
                .unwrap()
                .stdout,
        )
    }
    fn multi(root: &Path, paths: &[&str]) -> ToolInput {
        request(
            paths
                .iter()
                .map(|path| {
                    let bytes = fs::read(root.join(path)).unwrap();
                    target(path, &bytes, vec![replacement("old", "new")])
                })
                .collect(),
        )
    }
    fn replacement(old: &str, new: &str) -> Value {
        json!({"expected_old_text":old,"replacement_text":new})
    }
    fn target(path: &str, bytes: &[u8], replacements: Vec<Value>) -> Value {
        json!({"path":path,"expected_file_sha256":sha256(bytes),"expected_file_byte_length":bytes.len(),"replacements":replacements})
    }
    fn request(targets: Vec<Value>) -> ToolInput {
        ToolInput(json!({"targets":targets}))
    }
    #[test]
    fn strict_shape_and_bounds() {
        assert!(
            MultiFileRequest::parse(&request(vec![target(
                "a",
                b"a",
                vec![replacement("a", "b")]
            )]))
            .is_ok()
        );
        assert!(MultiFileRequest::parse(&request(vec![])).is_err());
        let five = (0..5)
            .map(|n| target(&format!("{n}"), b"a", vec![replacement("a", "b")]))
            .collect();
        assert!(MultiFileRequest::parse(&request(five)).is_err());
        assert!(MultiFileRequest::parse(&ToolInput(json!({"targets":[],"extra":true}))).is_err());
        assert!(MultiFileRequest::parse(&ToolInput(json!({"targets":[{"path":"a","expected_file_sha256":"0".repeat(64),"expected_file_byte_length":1,"replacements":[],"extra":true}]}))).is_err());
    }
    #[test]
    fn parser_enforces_all_fixed_bounds_and_legacy_rejection() {
        let one = target("a", b"a", vec![replacement("a", "b")]);
        assert!(
            MultiFileRequest::parse(&request(vec![
                one.clone(),
                one.clone(),
                one.clone(),
                one.clone()
            ]))
            .is_ok()
        );
        assert!(MultiFileRequest::parse(&ToolInput(json!({"targets":[{"path":"a","expected_file_sha256":"0".repeat(64),"expected_file_byte_length":1,"expected_old_text":"a","replacement_text":"b"}]}))).is_err());
        assert!(MultiFileRequest::parse(&ToolInput(json!({"targets":[{"path":"a","expected_file_sha256":"0".repeat(64),"expected_file_byte_length":1,"replacements":[{"expected_old_text":"a","replacement_text":"b","x":1}]}]}))).is_err());
        let long = "a".repeat(MAX_PATH_BYTES + 1);
        assert!(
            MultiFileRequest::parse(&request(vec![target(
                &long,
                b"a",
                vec![replacement("a", "b")]
            )]))
            .is_err()
        );
        assert!(MultiFileRequest::parse(&request(vec![target("a", b"a", vec![])])).is_err());
        assert!(
            MultiFileRequest::parse(&request(vec![target(
                "a",
                b"a",
                (0..17).map(|_| replacement("a", "b")).collect()
            )]))
            .is_err()
        );
        let old = "a".repeat(MAX_TEXT_BYTES + 1);
        assert!(
            MultiFileRequest::parse(&request(vec![target(
                "a",
                b"a",
                vec![replacement(&old, "b")]
            )]))
            .is_err()
        );
        let new = "b".repeat(MAX_TEXT_BYTES + 1);
        assert!(
            MultiFileRequest::parse(&request(vec![target(
                "a",
                b"a",
                vec![replacement("a", &new)]
            )]))
            .is_err()
        );
        assert!(checked_total(usize::MAX, 1, MAX_AGGREGATE_BYTES, "x").is_err());
        let oversized = ToolInput(
            json!({"targets":[{"path":"a","expected_file_sha256":"0".repeat(64),"expected_file_byte_length":1,"replacements":[{"expected_old_text":"a","replacement_text":"x".repeat(MAX_SERIALIZED_REQUEST_BYTES)}]}]}),
        );
        assert!(MultiFileRequest::parse(&oversized).is_err());
    }
    #[test]
    fn paths_and_replacements_are_closed() {
        for path in [
            "", "/a", "//a", ".", "..", "a/../b", "a//b", "a/", "a\\b", "a:b", "a:stream",
            ".git/x", ".GIT/x", "a\0b",
        ] {
            assert!(validate_logical_path(path).is_err(), "{path}");
        }
        assert_eq!(
            build_postimage(
                b"one two three",
                &[
                    Replacement {
                        old: "one".into(),
                        new: "1".into()
                    },
                    Replacement {
                        old: "three".into(),
                        new: "3".into()
                    }
                ]
            )
            .unwrap(),
            b"1 two 3"
        );
        assert!(
            build_postimage(
                b"xx",
                &[Replacement {
                    old: "x".into(),
                    new: "y".into()
                }]
            )
            .is_err()
        );
        assert!(
            build_postimage(
                b"abc",
                &[Replacement {
                    old: "a".into(),
                    new: "a".into()
                }]
            )
            .is_err()
        );
    }
    #[test]
    fn sparse_and_nonstandard_index_tags_fail_closed() {
        assert!(is_supported_index_tag(b"H a.txt\0"));
        for tag in [b"S a.txt\0".as_slice(), b"h a.txt\0", b"? a.txt\0", b""] {
            assert!(!is_supported_index_tag(tag));
        }
    }
    #[test]
    fn replacement_planning_is_original_snapshot_exact_and_bounded() {
        let replacements = vec![
            Replacement {
                old: "A".into(),
                new: "B".into(),
            },
            Replacement {
                old: "B".into(),
                new: "C".into(),
            },
        ];
        assert!(
            build_postimage(b"A", &replacements).is_err(),
            "new match must not be created sequentially"
        );
        assert_eq!(
            build_postimage(
                b"A B C",
                &[
                    Replacement {
                        old: "A".into(),
                        new: "X".into()
                    },
                    Replacement {
                        old: "C".into(),
                        new: "Z".into()
                    }
                ]
            )
            .unwrap(),
            b"X B Z"
        );
        assert_eq!(
            build_postimage(
                b"abcd",
                &[
                    Replacement {
                        old: "ab".into(),
                        new: "X".into()
                    },
                    Replacement {
                        old: "cd".into(),
                        new: "Y".into()
                    }
                ]
            )
            .unwrap(),
            b"XY"
        );
        assert!(
            build_postimage(
                b"abcdef",
                &[
                    Replacement {
                        old: "abc".into(),
                        new: "X".into()
                    },
                    Replacement {
                        old: "cde".into(),
                        new: "Y".into()
                    }
                ]
            )
            .is_err()
        );
        assert!(
            build_postimage(
                b"a",
                &[
                    Replacement {
                        old: "a".into(),
                        new: "x".into()
                    },
                    Replacement {
                        old: "a".into(),
                        new: "x".into()
                    }
                ]
            )
            .is_err()
        );
        let sixteen = (0..16)
            .map(|n| Replacement {
                old: format!("<{n}>"),
                new: format!("[{n}]"),
            })
            .collect::<Vec<_>>();
        let input = (0..16).map(|n| format!("<{n}>")).collect::<String>();
        assert!(build_postimage(input.as_bytes(), &sixteen).is_ok());
        assert!(
            build_postimage(
                &vec![b'a'; MAX_FILE_BYTES],
                &[Replacement {
                    old: "a".into(),
                    new: "bb".into()
                }]
            )
            .is_err()
        );
    }
    #[test]
    fn canonical_order_is_host_owned() {
        let plan = PreparedMultiFilePlan {
            repository: RepositoryObservation {
                index: vec![],
                head: vec![],
                refs: vec![],
            },
            targets: vec![
                PreparedTarget {
                    canonical_logical: "src/a.rs".into(),
                    target: unsafe_target(),
                    original: vec![],
                    postimage: vec![],
                    replacements: vec![],
                    ranges: vec![],
                },
                PreparedTarget {
                    canonical_logical: "src/m.rs".into(),
                    target: unsafe_target(),
                    original: vec![],
                    postimage: vec![],
                    replacements: vec![],
                    ranges: vec![],
                },
                PreparedTarget {
                    canonical_logical: "src/z.rs".into(),
                    target: unsafe_target(),
                    original: vec![],
                    postimage: vec![],
                    replacements: vec![],
                    ranges: vec![],
                },
            ],
            temporaries: vec![],
        };
        assert_eq!(plan.paths(), vec!["src/a.rs", "src/m.rs", "src/z.rs"]);
    }

    fn human_request(
        paths: &[&str],
        replacements: &[(&str, &str)],
    ) -> RepositoryMultiFileEditPreparationRequest {
        RepositoryMultiFileEditPreparationRequest {
            targets: paths
                .iter()
                .map(|path| RepositoryMultiFileEditPreparationTarget {
                    path: (*path).to_owned(),
                    replacements: replacements
                        .iter()
                        .map(|(old, new)| RepositoryMultiFileEditTextReplacement {
                            expected_old_text: (*old).to_owned(),
                            replacement_text: (*new).to_owned(),
                        })
                        .collect(),
                })
                .collect(),
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn shared_preparation_is_exact_and_host_ordered() {
        let (_base, git_path, root) = TestDirectory::repository();
        let preparer = RepositoryMultiFileEditPreparer::new(&git_path, &root).unwrap();
        let preparation = preparer
            .prepare(human_request(
                &["z.txt", "a.txt", "m.txt"],
                &[("old", "new")],
            ))
            .await
            .unwrap();
        assert_eq!(
            preparation
                .review()
                .targets()
                .iter()
                .map(RepositoryMultiFileEditTargetReview::path)
                .collect::<Vec<_>>(),
            vec!["a.txt", "m.txt", "z.txt"]
        );
        assert_eq!(preparation.review().target_count(), 3);
        assert_eq!(preparation.review().replacement_count(), 3);
        assert_eq!(preparation.review().targets()[0].preimage_byte_length(), 6);
        assert_eq!(preparation.review().targets()[0].postimage_byte_length(), 6);
        assert_eq!(preparation.postimage(0), Some(b"A new\n".as_slice()));
        assert_eq!(preparation.postimage(2), Some(b"Z new\n".as_slice()));
        let targets = preparation.tool_input().0["targets"].as_array().unwrap();
        assert_eq!(targets[0]["path"], "a.txt");
        assert_eq!(targets[2]["path"], "z.txt");
        assert_eq!(targets[0]["expected_file_byte_length"], 6);
        assert_eq!(targets[0]["replacements"][0]["expected_old_text"], "old");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn shared_preparation_has_zero_effect_and_no_repository_temporary() {
        let (_base, git_path, root) = TestDirectory::repository();
        let before = state(&root);
        let before_names = fs::read_dir(&root)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect::<Vec<_>>();
        let preparer = RepositoryMultiFileEditPreparer::new(&git_path, &root).unwrap();
        preparer
            .prepare(human_request(&["z.txt", "a.txt"], &[("old", "new")]))
            .await
            .unwrap();
        assert_eq!(state(&root), before);
        assert_eq!(
            crate::repository_multi_file_edit::test_execute_hook::count(&root),
            0
        );
        for index in 0..4 {
            assert_eq!(test_commit_hook::attempts(&root, index), 0);
        }
        let after_names = fs::read_dir(&root)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect::<Vec<_>>();
        assert_eq!(after_names, before_names);
        assert_eq!(
            after_names
                .iter()
                .filter(|name| name.to_string_lossy().contains("rah-repo-edit-files"))
                .count(),
            0
        );
        crate::repository_multi_file_edit::test_execute_hook::clear(&root);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn successful_prepare_is_zero_effect_for_one_and_four_targets_with_multiple_replacements()
    {
        for (paths, replacements) in [
            (vec!["a.txt"], vec![vec![("A", "a"), ("old", "new")]]),
            (
                vec!["a.txt", "m.txt", "sentinel.txt", "z.txt"],
                vec![
                    vec![("old", "new"), ("\n", "\r\n")],
                    vec![("old", "new"), ("\n", "\r\n")],
                    vec![("sentinel", "SENTINEL"), ("\n", "\r\n")],
                    vec![("old", "new"), ("\n", "\r\n")],
                ],
            ),
        ] {
            let (_base, git_path, root) = TestDirectory::repository();
            let before = state(&root);
            let before_entries = fs::read_dir(&root)
                .unwrap()
                .map(|entry| {
                    let entry = entry.unwrap();
                    (entry.file_name(), fs::read(entry.path()).ok())
                })
                .collect::<Vec<_>>();
            let preparer = RepositoryMultiFileEditPreparer::new(&git_path, &root).unwrap();
            let preparation = preparer
                .prepare(RepositoryMultiFileEditPreparationRequest {
                    targets: paths
                        .iter()
                        .zip(&replacements)
                        .map(
                            |(path, replacements)| RepositoryMultiFileEditPreparationTarget {
                                path: (*path).to_owned(),
                                replacements: replacements
                                    .iter()
                                    .map(|(old, new)| RepositoryMultiFileEditTextReplacement {
                                        expected_old_text: (*old).to_owned(),
                                        replacement_text: (*new).to_owned(),
                                    })
                                    .collect(),
                            },
                        )
                        .collect(),
                })
                .await
                .unwrap();
            assert_eq!(preparation.review().target_count(), paths.len());
            assert_eq!(
                preparation.review().replacement_count(),
                replacements.iter().map(Vec::len).sum::<usize>()
            );
            assert_eq!(state(&root), before, "Prepare changed repository state");
            let after_entries = fs::read_dir(&root)
                .unwrap()
                .map(|entry| {
                    let entry = entry.unwrap();
                    (entry.file_name(), fs::read(entry.path()).ok())
                })
                .collect::<Vec<_>>();
            assert_eq!(
                after_entries, before_entries,
                "Prepare changed worktree entries"
            );
            assert_eq!(
                crate::repository_multi_file_edit::test_execute_hook::count(&root),
                0
            );
            for index in 0..4 {
                assert_eq!(test_commit_hook::attempts(&root, index), 0);
            }
            crate::repository_multi_file_edit::test_execute_hook::clear(&root);
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn shared_preparation_uses_one_original_snapshot_and_rejects_literal_errors() {
        let (_base, git_path, root) = TestDirectory::repository();
        let preparer = RepositoryMultiFileEditPreparer::new(&git_path, &root).unwrap();
        let preparation = preparer
            .prepare(human_request(&["a.txt"], &[("A", "B"), ("old", "new")]))
            .await
            .unwrap();
        assert_eq!(preparation.postimage(0), Some(b"B new\n".as_slice()));
        for replacements in [
            vec![("missing", "new")],
            vec![("old", "new"), ("old", "other")],
            vec![("A old", "x"), ("old", "y")],
            vec![("old", "old")],
        ] {
            assert!(matches!(
                preparer
                    .prepare(human_request(&["a.txt"], &replacements))
                    .await,
                Err(RepositoryMultiFileEditPreparationError::PreconditionChanged { .. })
            ));
        }
        assert!(
            preparer
                .prepare(human_request(&["a.txt"], &[("old", "")]))
                .await
                .is_ok()
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn shared_preparation_enforces_closed_counts_and_material_bounds() {
        let (_base, git_path, root) = TestDirectory::repository();
        let preparer = RepositoryMultiFileEditPreparer::new(&git_path, &root).unwrap();
        assert!(matches!(
            preparer
                .prepare(human_request(&[], &[("old", "new")]))
                .await,
            Err(RepositoryMultiFileEditPreparationError::InvalidInput {
                reason: "target_count"
            })
        ));
        let too_many = (0..17)
            .map(|n| (format!("{n}"), format!("x{n}")))
            .collect::<Vec<_>>();
        let too_many = too_many
            .iter()
            .map(|(old, new)| (old.as_str(), new.as_str()))
            .collect::<Vec<_>>();
        assert!(matches!(
            preparer.prepare(human_request(&["a.txt"], &too_many)).await,
            Err(RepositoryMultiFileEditPreparationError::InvalidInput {
                reason: "replacement_count"
            })
        ));
        assert!(matches!(
            preparer
                .prepare(human_request(
                    &["a.txt"],
                    &[("x", &"x".repeat(MAX_TEXT_BYTES + 1))]
                ))
                .await,
            Err(RepositoryMultiFileEditPreparationError::InvalidInput { reason: "text" })
        ));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn shared_revalidation_rejects_bytes_identity_and_git_drift_without_effect() {
        let (_base, git_path, root) = TestDirectory::repository();
        let preparer = RepositoryMultiFileEditPreparer::new(&git_path, &root).unwrap();
        let preparation = preparer
            .prepare(human_request(&["a.txt"], &[("old", "new")]))
            .await
            .unwrap();
        assert!(preparer.revalidate(&preparation).await.is_ok());
        fs::write(root.join("a.txt"), b"B old\n").unwrap();
        assert!(matches!(
            preparer.revalidate(&preparation).await,
            Err(RepositoryMultiFileEditPreparationError::PreconditionChanged { .. })
        ));
        assert_eq!(fs::metadata(root.join("a.txt")).unwrap().len(), 6);
        let (_base, git_path, root) = TestDirectory::repository();
        let preparer = RepositoryMultiFileEditPreparer::new(&git_path, &root).unwrap();
        let preparation = preparer
            .prepare(human_request(&["a.txt"], &[("old", "new")]))
            .await
            .unwrap();
        if fs::hard_link(root.join("a.txt"), root.join("a-alias.txt")).is_ok() {
            let result = preparer.revalidate(&preparation).await;
            assert!(
                matches!(
                    &result,
                    Err(RepositoryMultiFileEditPreparationError::InvalidTarget { .. })
                ),
                "hard-link identity drift must fail closed: {result:?}"
            );
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn shared_revalidation_rejects_protected_head_ref_and_index_drift() {
        let (_base, git_path, root) = TestDirectory::repository();
        let preparer = RepositoryMultiFileEditPreparer::new(&git_path, &root).unwrap();
        let preparation = preparer
            .prepare(human_request(&["a.txt"], &[("old", "new")]))
            .await
            .unwrap();
        git(
            &git_path,
            &root,
            &["update-ref", "refs/heads/task-259-test", "HEAD"],
        );
        assert!(matches!(
            preparer.revalidate(&preparation).await,
            Err(RepositoryMultiFileEditPreparationError::PreconditionChanged { .. })
        ));

        let (_base, git_path, root) = TestDirectory::repository();
        let preparer = RepositoryMultiFileEditPreparer::new(&git_path, &root).unwrap();
        let preparation = preparer
            .prepare(human_request(&["a.txt"], &[("old", "new")]))
            .await
            .unwrap();
        let head = fs::read(root.join(".git/HEAD")).unwrap();
        fs::write(root.join(".git/HEAD"), b"ref: refs/heads/other\n").unwrap();
        assert!(matches!(
            preparer.revalidate(&preparation).await,
            Err(RepositoryMultiFileEditPreparationError::PreconditionChanged { .. })
        ));
        fs::write(root.join(".git/HEAD"), head).unwrap();

        let (_base, git_path, root) = TestDirectory::repository();
        let preparer = RepositoryMultiFileEditPreparer::new(&git_path, &root).unwrap();
        let preparation = preparer
            .prepare(human_request(&["a.txt"], &[("old", "new")]))
            .await
            .unwrap();
        fs::write(root.join("a.txt"), b"A staged\n").unwrap();
        git(&git_path, &root, &["add", "--", "a.txt"]);
        assert!(matches!(
            preparer.revalidate(&preparation).await,
            Err(RepositoryMultiFileEditPreparationError::PreconditionChanged { .. })
        ));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn real_git_preflight_sorts_and_has_zero_target_effect_on_failure() {
        let (_base, git_path, root) = TestDirectory::repository();
        let policy = RepositoryMultiFileMutationPolicy::new(&git_path, &root).unwrap();
        let plan = policy
            .prepare(&multi(&root, &["z.txt", "a.txt", "m.txt"]))
            .await
            .unwrap();
        assert_eq!(plan.paths(), vec!["a.txt", "m.txt", "z.txt"]);
        let before = state(&root);
        for path in [
            ["missing.txt", "a.txt", "m.txt"],
            ["a.txt", "missing.txt", "m.txt"],
            ["a.txt", "m.txt", "missing.txt"],
        ] {
            let input = request(
                path.iter()
                    .map(|name| {
                        if *name == "missing.txt" {
                            target(name, b"old", vec![replacement("old", "new")])
                        } else {
                            let bytes = fs::read(root.join(name)).unwrap();
                            target(name, &bytes, vec![replacement("old", "new")])
                        }
                    })
                    .collect(),
            );
            assert!(policy.prepare(&input).await.is_err());
            assert_eq!(state(&root), before);
        }
        for (index, path) in [
            ["a.txt", "m.txt", "z.txt"],
            ["a.txt", "m.txt", "z.txt"],
            ["a.txt", "m.txt", "z.txt"],
        ]
        .into_iter()
        .enumerate()
        {
            let mut input = multi(&root, &path);
            let bad = match &mut input.0 {
                Value::Object(o) => o["targets"].as_array_mut().unwrap(),
                _ => unreachable!(),
            };
            bad[index]["expected_file_sha256"] = json!("0".repeat(64));
            assert!(policy.prepare(&input).await.is_err());
            assert_eq!(state(&root), before);
        }
    }
    #[tokio::test(flavor = "current_thread")]
    async fn real_git_state_and_snapshot_preconditions_fail_closed() {
        let (_base, git_path, root) = TestDirectory::repository();
        let policy = RepositoryMultiFileMutationPolicy::new(&git_path, &root).unwrap();
        let before = state(&root);
        let mut bad = multi(&root, &["a.txt"]);
        bad.0["targets"][0]["expected_file_byte_length"] = json!(999);
        assert!(policy.prepare(&bad).await.is_err());
        assert_eq!(state(&root), before);
        fs::write(root.join("untracked.txt"), b"old\n").unwrap();
        assert!(
            policy
                .prepare(&multi(&root, &["untracked.txt"]))
                .await
                .is_err()
        );
        assert_eq!(state(&root).0, before.0);
        fs::write(root.join(".gitignore"), "ignored.txt\n").unwrap();
        fs::write(root.join("ignored.txt"), b"old\n").unwrap();
        assert!(
            policy
                .prepare(&multi(&root, &["ignored.txt"]))
                .await
                .is_err(),
            "an ignored, untracked target is not admissible"
        );
        assert_eq!(fs::read(root.join("ignored.txt")).unwrap(), b"old\n");
        fs::write(root.join("a.txt"), b"changed\n").unwrap();
        git(&git_path, &root, &["add", "--", "a.txt"]);
        assert!(policy.prepare(&multi(&root, &["a.txt"])).await.is_err());
    }
    #[tokio::test(flavor = "current_thread")]
    async fn aliases_and_non_regular_targets_are_rejected_without_effect() {
        let (_base, git_path, root) = TestDirectory::repository();
        let policy = RepositoryMultiFileMutationPolicy::new(&git_path, &root).unwrap();
        let before = state(&root);
        if fs::hard_link(root.join("a.txt"), root.join("alias.txt")).is_ok() {
            git(&git_path, &root, &["add", "--", "alias.txt"]);
            let input = request(vec![
                target("a.txt", b"A old\n", vec![replacement("old", "new")]),
                target("alias.txt", b"A old\n", vec![replacement("old", "new")]),
            ]);
            assert!(policy.prepare(&input).await.is_err());
        }
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink("a.txt", root.join("link.txt")).unwrap();
            assert!(
                policy
                    .prepare(&request(vec![target(
                        "link.txt",
                        b"A old\n",
                        vec![replacement("old", "new")]
                    )]))
                    .await
                    .is_err()
            );
        }
        assert_eq!(state(&root).0[..4], before.0[..4]);
    }
    #[tokio::test(flavor = "current_thread")]
    async fn temporary_preparation_failures_have_zero_target_effect_in_host_order() {
        for (phase, index) in [
            (TestPhase::BeforeTemporaryCreate, 0),
            (TestPhase::AfterTemporaryWrite, 1),
            (TestPhase::AfterTemporaryVerify, 2),
        ] {
            let (_base, git_path, root) = TestDirectory::repository();
            let policy = RepositoryMultiFileMutationPolicy::new(&git_path, &root).unwrap();
            let before = state(&root);
            test_hook::install(&policy.root, phase, index);
            assert!(
                policy
                    .prepare(&multi(&root, &["a.txt", "m.txt", "z.txt"]))
                    .await
                    .is_err()
            );
            test_hook::clear(&policy.root);
            assert_eq!(state(&root), before, "{phase:?} at {index}");
            let leftovers = fs::read_dir(&root)
                .unwrap()
                .filter_map(Result::ok)
                .filter(|entry| {
                    entry
                        .file_name()
                        .to_string_lossy()
                        .starts_with(".rah-repo-edit-files-")
                })
                .count();
            assert_eq!(leftovers, 0, "owned temporaries must be safely cleaned");
        }
    }
    #[tokio::test(flavor = "current_thread")]
    async fn retained_plan_revalidation_rejects_target_and_repository_races() {
        let (_base, git_path, root) = TestDirectory::repository();
        let policy = RepositoryMultiFileMutationPolicy::new(&git_path, &root).unwrap();
        let before = state(&root);
        let plan = policy
            .prepare_retained_for_test(&multi(&root, &["a.txt", "m.txt", "z.txt"]))
            .await
            .unwrap();
        fs::write(root.join("m.txt"), b"external mutation\n").unwrap();
        assert!(policy.revalidate_pre_commit(&plan).is_err());
        assert_eq!(fs::read(root.join("a.txt")).unwrap(), before.0[0]);
        assert_eq!(fs::read(root.join("z.txt")).unwrap(), before.0[2]);
        plan.cleanup_temporaries();
        fs::write(root.join("m.txt"), &before.0[1]).unwrap();
        let plan = policy
            .prepare_retained_for_test(&multi(&root, &["a.txt"]))
            .await
            .unwrap();
        fs::write(root.join("identity-source.txt"), b"A old\n").unwrap();
        fs::remove_file(root.join("a.txt")).unwrap();
        fs::rename(root.join("identity-source.txt"), root.join("a.txt")).unwrap();
        assert!(policy.revalidate_pre_commit(&plan).is_err());
        plan.cleanup_temporaries();
    }
    #[tokio::test(flavor = "current_thread")]
    async fn retained_plan_revalidation_rejects_index_head_and_ref_races() {
        let (_base, git_path, root) = TestDirectory::repository();
        let policy = RepositoryMultiFileMutationPolicy::new(&git_path, &root).unwrap();
        for args in [
            vec!["add", "--", "sentinel.txt"],
            vec!["update-ref", "refs/rah/test", "HEAD"],
        ] {
            let plan = policy
                .prepare_retained_for_test(&multi(&root, &["a.txt"]))
                .await
                .unwrap();
            if args[0] == "add" {
                fs::write(root.join("sentinel.txt"), b"index mutation\n").unwrap();
            }
            git(&git_path, &root, &args);
            assert!(policy.revalidate_pre_commit(&plan).is_err());
            assert_eq!(fs::read(root.join("a.txt")).unwrap(), b"A old\n");
            plan.cleanup_temporaries();
            git(
                &git_path,
                &root,
                &["reset", "--quiet", "HEAD", "--", "sentinel.txt"],
            );
            fs::write(root.join("sentinel.txt"), b"sentinel\n").unwrap();
            git(&git_path, &root, &["update-ref", "-d", "refs/rah/test"]);
        }
        let plan = policy
            .prepare_retained_for_test(&multi(&root, &["a.txt"]))
            .await
            .unwrap();
        fs::write(root.join("sentinel.txt"), b"head mutation\n").unwrap();
        git(&git_path, &root, &["add", "--", "sentinel.txt"]);
        git(
            &git_path,
            &root,
            &[
                "-c",
                "user.name=RAH",
                "-c",
                "user.email=rah@example.invalid",
                "commit",
                "--quiet",
                "-m",
                "head mutation",
            ],
        );
        assert!(policy.revalidate_pre_commit(&plan).is_err());
        assert_eq!(fs::read(root.join("a.txt")).unwrap(), b"A old\n");
        plan.cleanup_temporaries();
    }
    #[tokio::test(flavor = "current_thread")]
    async fn retained_plan_revalidation_rejects_temporary_content_and_identity_races() {
        let (_base, git_path, root) = TestDirectory::repository();
        let policy = RepositoryMultiFileMutationPolicy::new(&git_path, &root).unwrap();
        let plan = policy
            .prepare_retained_for_test(&multi(&root, &["a.txt"]))
            .await
            .unwrap();
        let temporary = plan.temporary_paths().pop().unwrap();
        fs::write(&temporary, b"foreign content").unwrap();
        assert!(policy.revalidate_pre_commit(&plan).is_err());
        plan.cleanup_temporaries();
        assert!(
            temporary.exists(),
            "changed temporary must not be blindly removed"
        );
        fs::remove_file(&temporary).unwrap();

        let plan = policy
            .prepare_retained_for_test(&multi(&root, &["a.txt"]))
            .await
            .unwrap();
        let temporary = plan.temporary_paths().pop().unwrap();
        fs::remove_file(&temporary).unwrap();
        fs::write(&temporary, b"replacement artifact").unwrap();
        assert!(policy.revalidate_pre_commit(&plan).is_err());
        plan.cleanup_temporaries();
        assert_eq!(fs::read(&temporary).unwrap(), b"replacement artifact");
        fs::remove_file(&temporary).unwrap();
        assert_eq!(fs::read(root.join("a.txt")).unwrap(), b"A old\n");
    }
    #[tokio::test(flavor = "current_thread")]
    async fn preparation_waits_for_the_shared_repository_lease() {
        let (_base, git_path, root) = TestDirectory::repository();
        let policy = Arc::new(RepositoryMultiFileMutationPolicy::new(&git_path, &root).unwrap());
        let lease = repository_lease(&policy.root);
        let guard = lease.lock().await;
        test_hook::install(&policy.root, TestPhase::BeforeTemporaryCreate, 0);
        let task = tokio::spawn({
            let policy = policy.clone();
            let input = multi(&root, &["a.txt"]);
            async move { policy.prepare(&input).await }
        });
        tokio::task::yield_now().await;
        assert!(
            !task.is_finished(),
            "preparation must not enter its lease-protected phase while the shared guard is held"
        );
        drop(guard);
        assert!(
            task.await.unwrap().is_err(),
            "the released task reaches the injected phase"
        );
        test_hook::clear(&policy.root);
    }
    #[tokio::test(flavor = "current_thread")]
    async fn global_revalidation_path_rejects_a_hooked_target_race() {
        let (_base, git_path, root) = TestDirectory::repository();
        let policy = RepositoryMultiFileMutationPolicy::new(&git_path, &root).unwrap();
        let before = state(&root);
        test_hook::install_global_target_mutation(&policy.root);
        assert!(
            policy
                .prepare(&multi(&root, &["a.txt", "m.txt", "z.txt"]))
                .await
                .is_err()
        );
        test_hook::clear(&policy.root);
        assert_eq!(fs::read(root.join("m.txt")).unwrap(), before.0[1]);
        assert_eq!(fs::read(root.join("z.txt")).unwrap(), before.0[2]);
        let after = state(&root);
        assert_eq!(after.1, before.1);
        assert_eq!(after.2, before.2);
        assert_eq!(after.3, before.3);
    }
    #[tokio::test(flavor = "current_thread")]
    async fn parent_identity_replacement_is_rejected_before_any_commit() {
        let (_base, git_path, root) = TestDirectory::repository();
        fs::create_dir(root.join("nested")).unwrap();
        fs::write(root.join("nested/target.txt"), b"nested old\n").unwrap();
        git(&git_path, &root, &["add", "--", "nested/target.txt"]);
        git(
            &git_path,
            &root,
            &[
                "-c",
                "user.name=RAH",
                "-c",
                "user.email=rah@example.invalid",
                "commit",
                "--quiet",
                "-m",
                "nested",
            ],
        );
        let policy = RepositoryMultiFileMutationPolicy::new(&git_path, &root).unwrap();
        let plan = policy
            .prepare_retained_for_test(&multi(&root, &["nested/target.txt"]))
            .await
            .unwrap();
        fs::rename(root.join("nested"), root.join("nested-old")).unwrap();
        fs::create_dir(root.join("nested")).unwrap();
        fs::write(root.join("nested/target.txt"), b"nested old\n").unwrap();
        assert!(policy.revalidate_pre_commit(&plan).is_err());
        assert_eq!(
            fs::read(root.join("nested/target.txt")).unwrap(),
            b"nested old\n"
        );
    }
    #[tokio::test(flavor = "current_thread")]
    async fn real_git_unmerged_and_gitlink_targets_are_rejected() {
        let (_base, git_path, root) = TestDirectory::repository();
        git(
            &git_path,
            &root,
            &["checkout", "--quiet", "-b", "conflict-a"],
        );
        fs::write(root.join("a.txt"), b"branch A\n").unwrap();
        git(&git_path, &root, &["add", "--", "a.txt"]);
        git(
            &git_path,
            &root,
            &[
                "-c",
                "user.name=RAH",
                "-c",
                "user.email=rah@example.invalid",
                "commit",
                "--quiet",
                "-m",
                "branch A",
            ],
        );
        git(&git_path, &root, &["checkout", "--quiet", "master"]);
        fs::write(root.join("a.txt"), b"branch master\n").unwrap();
        git(&git_path, &root, &["add", "--", "a.txt"]);
        git(
            &git_path,
            &root,
            &[
                "-c",
                "user.name=RAH",
                "-c",
                "user.email=rah@example.invalid",
                "commit",
                "--quiet",
                "-m",
                "branch master",
            ],
        );
        let merge = git_output(
            &git_path,
            &root,
            &[
                "-c",
                "user.name=RAH Test",
                "-c",
                "user.email=rah-test@example.invalid",
                "merge",
                "conflict-a",
            ],
        );
        assert!(
            !merge.status.success(),
            "merge unexpectedly succeeded: stdout={} stderr={}",
            String::from_utf8_lossy(&merge.stdout),
            String::from_utf8_lossy(&merge.stderr)
        );
        let unmerged = git_output(&git_path, &root, &["ls-files", "-u", "--", "a.txt"]);
        assert!(
            unmerged.status.success(),
            "git ls-files -u failed: {}",
            String::from_utf8_lossy(&unmerged.stderr)
        );
        let unmerged = String::from_utf8(unmerged.stdout).unwrap();
        assert_eq!(
            unmerged.lines().count(),
            3,
            "expected three unmerged index stages for a.txt, got: {unmerged}"
        );
        for stage in 1..=3 {
            assert!(
                unmerged.contains(&format!(" {stage}\ta.txt")),
                "missing unmerged stage {stage} for a.txt: {unmerged}"
            );
        }
        let policy = RepositoryMultiFileMutationPolicy::new(&git_path, &root).unwrap();
        let before = fs::read(root.join("a.txt")).unwrap();
        assert!(policy.prepare(&multi(&root, &["a.txt"])).await.is_err());
        assert_eq!(fs::read(root.join("a.txt")).unwrap(), before);
        git(&git_path, &root, &["merge", "--abort"]);

        fs::write(root.join("gitlink"), b"placeholder\n").unwrap();
        let object = Command::new(&git_path)
            .args(["rev-parse", "HEAD"])
            .current_dir(&root)
            .output()
            .unwrap();
        let object = String::from_utf8(object.stdout).unwrap();
        let entry = format!("160000,{},{}", object.trim(), "gitlink");
        git(
            &git_path,
            &root,
            &["update-index", "--add", "--cacheinfo", &entry],
        );
        assert!(policy.prepare(&multi(&root, &["gitlink"])).await.is_err());
    }
    #[cfg(windows)]
    #[tokio::test(flavor = "current_thread")]
    async fn windows_junction_ancestry_is_rejected_before_target_admission() {
        let (_base, git_path, root) = TestDirectory::repository();
        let real_parent = root.join("real-parent");
        fs::create_dir(&real_parent).unwrap();
        fs::write(real_parent.join("target.txt"), b"old\n").unwrap();
        let junction = root.join("junction");
        assert!(
            Command::new("cmd.exe")
                .args([
                    "/c",
                    "mklink",
                    "/J",
                    junction.to_str().unwrap(),
                    real_parent.to_str().unwrap()
                ])
                .status()
                .unwrap()
                .success()
        );
        let policy = RepositoryMultiFileMutationPolicy::new(&git_path, &root).unwrap();
        assert!(
            policy
                .prepare(&request(vec![target(
                    "junction/target.txt",
                    b"old\n",
                    vec![replacement("old", "new")]
                )]))
                .await
                .is_err()
        );
        assert_eq!(fs::read(real_parent.join("target.txt")).unwrap(), b"old\n");
    }
    #[cfg(unix)]
    #[tokio::test(flavor = "current_thread")]
    async fn unix_fifo_is_rejected_and_prepared_temporary_preserves_mode() {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};

        let (_base, git_path, root) = TestDirectory::repository();
        let fifo = root.join("fifo");
        assert!(
            Command::new("mkfifo")
                .arg(&fifo)
                .status()
                .unwrap()
                .success()
        );
        let policy = RepositoryMultiFileMutationPolicy::new(&git_path, &root).unwrap();
        assert!(
            policy
                .prepare(&request(vec![target(
                    "fifo",
                    b"old",
                    vec![replacement("old", "new")]
                )]))
                .await
                .is_err()
        );
        fs::set_permissions(root.join("a.txt"), fs::Permissions::from_mode(0o754)).unwrap();
        git(&git_path, &root, &["update-index", "--chmod=+x", "a.txt"]);
        git(
            &git_path,
            &root,
            &[
                "-c",
                "user.name=RAH",
                "-c",
                "user.email=rah@example.invalid",
                "commit",
                "--quiet",
                "-m",
                "mode",
            ],
        );
        let plan = policy
            .prepare_retained_for_test(&multi(&root, &["a.txt"]))
            .await
            .unwrap();
        let temporary = plan.temporary_paths().pop().unwrap();
        assert_eq!(fs::metadata(temporary).unwrap().mode() & 0o777, 0o754);
        plan.cleanup_temporaries();
    }
    #[tokio::test(flavor = "current_thread")]
    async fn linked_worktree_git_file_form_is_rejected() {
        let (_base, git_path, root) = TestDirectory::repository();
        let worktree = root.parent().unwrap().join("linked-worktree");
        git(
            &git_path,
            &root,
            &["worktree", "add", "--quiet", worktree.to_str().unwrap()],
        );
        assert!(
            RepositoryMultiFileMutationPolicy::new(&git_path, &worktree).is_err(),
            "the inherited directory-only .git metadata contract rejects linked worktrees"
        );
    }
    #[test]
    fn temporary_is_same_parent_exact_and_never_blindly_removed() {
        let base = std::env::temp_dir().join(format!(
            "rah-multi-temp-{}",
            NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&base).unwrap();
        let target_path = base.join("target.txt");
        fs::write(&target_path, b"old").unwrap();
        let target = SafeTarget {
            path: target_path.clone(),
            parent: base.clone(),
            canonical_logical: "target.txt".into(),
            identity: Identity::capture(&target_path).unwrap(),
            parent_identity: Identity::capture(&base).unwrap(),
            #[cfg(unix)]
            mode: {
                use std::os::unix::fs::MetadataExt;
                fs::metadata(&target_path).unwrap().mode()
            },
        };
        let temporary = OwnedTemporary::create(&target, b"new").unwrap();
        assert_eq!(temporary.path.parent(), Some(base.as_path()));
        assert_eq!(fs::read(&temporary.path).unwrap(), b"new");
        temporary.revalidate().unwrap();
        fs::write(&temporary.path, b"foreign").unwrap();
        assert!(temporary.remove().is_err());
        assert!(temporary.path.exists());
        let _ = fs::remove_file(&temporary.path);
        let _ = fs::remove_file(target_path);
        let _ = fs::remove_dir(base);
    }
    fn unsafe_target() -> SafeTarget {
        let path = std::env::temp_dir();
        SafeTarget {
            path: path.clone(),
            parent: path,
            canonical_logical: String::new(),
            identity: Identity {
                #[cfg(unix)]
                device: 0,
                #[cfg(unix)]
                inode: 0,
                #[cfg(windows)]
                volume_serial: 0,
                #[cfg(windows)]
                file_index: 0,
                link_count: 1,
            },
            parent_identity: Identity {
                #[cfg(unix)]
                device: 0,
                #[cfg(unix)]
                inode: 0,
                #[cfg(windows)]
                volume_serial: 0,
                #[cfg(windows)]
                file_index: 0,
                link_count: 1,
            },
            #[cfg(unix)]
            mode: 0o100644,
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn native_commit_is_ordered_one_attempt_and_preserves_repository_metadata() {
        let (_base, git_path, root) = TestDirectory::repository();
        let policy = RepositoryMultiFileMutationPolicy::new(&git_path, &root).unwrap();
        let before = state(&root);
        let outcome = policy
            .commit(&multi(&root, &["z.txt", "a.txt", "m.txt"]))
            .await
            .unwrap();
        assert_eq!(outcome.status, MultiFileEditStatus::Ok);
        assert_eq!(
            outcome
                .effects
                .iter()
                .map(|effect| effect.logical_path.as_str())
                .collect::<Vec<_>>(),
            vec!["a.txt", "m.txt", "z.txt"]
        );
        assert!(
            outcome
                .effects
                .iter()
                .all(|effect| effect.state == MultiFileEffectState::CommittedVerified)
        );
        for index in 0..3 {
            assert_eq!(test_commit_hook::attempts(&policy.root, index), 1);
        }
        assert_eq!(fs::read(root.join("a.txt")).unwrap(), b"A new\n");
        assert_eq!(fs::read(root.join("m.txt")).unwrap(), b"M new\n");
        assert_eq!(fs::read(root.join("z.txt")).unwrap(), b"Z new\n");
        let after = state(&root);
        assert_eq!(after.1, before.1, "raw index must remain unchanged");
        assert_eq!(after.2, before.2, "HEAD must remain unchanged");
        assert_eq!(after.3, before.3, "refs must remain unchanged");
        test_commit_hook::clear(&policy.root);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn native_commit_supports_one_two_and_four_targets() {
        for paths in [vec!["a.txt"], vec!["a.txt", "m.txt"]] {
            let (_base, git_path, root) = TestDirectory::repository();
            let policy = RepositoryMultiFileMutationPolicy::new(&git_path, &root).unwrap();
            assert_eq!(
                policy.commit(&multi(&root, &paths)).await.unwrap().status,
                MultiFileEditStatus::Ok
            );
        }
        let (_base, git_path, root) = TestDirectory::repository();
        fs::write(root.join("b.txt"), b"B old\n").unwrap();
        git(&git_path, &root, &["add", "--", "b.txt"]);
        git(
            &git_path,
            &root,
            &[
                "-c",
                "user.name=RAH",
                "-c",
                "user.email=rah@example.invalid",
                "commit",
                "--quiet",
                "-m",
                "four target fixture",
            ],
        );
        let policy = RepositoryMultiFileMutationPolicy::new(&git_path, &root).unwrap();
        let outcome = policy
            .commit(&multi(&root, &["z.txt", "b.txt", "a.txt", "m.txt"]))
            .await
            .unwrap();
        assert_eq!(outcome.status, MultiFileEditStatus::Ok);
        assert_eq!(
            outcome
                .effects
                .iter()
                .map(|effect| effect.logical_path.as_str())
                .collect::<Vec<_>>(),
            vec!["a.txt", "b.txt", "m.txt", "z.txt"]
        );
        for index in 0..4 {
            assert_eq!(test_commit_hook::attempts(&policy.root, index), 1);
        }
        test_commit_hook::clear(&policy.root);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn known_no_effect_failures_produce_verified_prefix_without_rollback() {
        for (failed, expected_status) in [
            (0, MultiFileEditStatus::FailedKnownNoEffect),
            (1, MultiFileEditStatus::PartialEffect),
            (2, MultiFileEditStatus::PartialEffect),
        ] {
            let (_base, git_path, root) = TestDirectory::repository();
            let policy = RepositoryMultiFileMutationPolicy::new(&git_path, &root).unwrap();
            test_commit_hook::install(&policy.root, CommitTestPhase::KnownNoEffectFailure, failed);
            let outcome = policy
                .commit(&multi(&root, &["a.txt", "m.txt", "z.txt"]))
                .await
                .unwrap();
            assert_eq!(outcome.status, expected_status);
            for index in 0..failed {
                assert_eq!(
                    outcome.effects[index].state,
                    MultiFileEffectState::CommittedVerified
                );
                assert_eq!(test_commit_hook::attempts(&policy.root, index), 1);
            }
            assert_eq!(
                outcome.effects[failed].state,
                MultiFileEffectState::UnchangedVerified
            );
            assert_eq!(
                test_commit_hook::attempts(&policy.root, failed),
                1,
                "known native failure has one attempt"
            );
            for index in failed + 1..3 {
                assert_eq!(
                    outcome.effects[index].state,
                    MultiFileEffectState::NotAttempted
                );
                assert_eq!(test_commit_hook::attempts(&policy.root, index), 0);
            }
            assert_eq!(
                fs::read(root.join("a.txt")).unwrap(),
                if failed > 0 { b"A new\n" } else { b"A old\n" }
            );
            test_commit_hook::clear(&policy.root);
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn uncertain_native_and_lost_certification_stop_without_retry_or_rollback() {
        for phase in [
            CommitTestPhase::UncertainNativeOutcome,
            CommitTestPhase::AfterNativeBeforeCertification,
        ] {
            for failed in 0..3 {
                let (_base, git_path, root) = TestDirectory::repository();
                let policy = RepositoryMultiFileMutationPolicy::new(&git_path, &root).unwrap();
                test_commit_hook::install(&policy.root, phase, failed);
                let outcome = policy
                    .commit(&multi(&root, &["a.txt", "m.txt", "z.txt"]))
                    .await
                    .unwrap();
                assert_eq!(outcome.status, MultiFileEditStatus::Uncertain);
                for index in 0..=failed {
                    assert_eq!(test_commit_hook::attempts(&policy.root, index), 1);
                }
                for index in failed + 1..3 {
                    assert_eq!(test_commit_hook::attempts(&policy.root, index), 0);
                }
                for index in 0..failed {
                    assert_eq!(
                        fs::read(root.join(["a.txt", "m.txt", "z.txt"][index])).unwrap()[2..5],
                        *b"new"
                    );
                }
                test_commit_hook::clear(&policy.root);
            }
        }
    }
}
