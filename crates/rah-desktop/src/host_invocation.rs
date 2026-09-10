#![cfg(target_os = "windows")]

//! Desktop-private explicit host Tool invocation.

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use rah_protocol::{PermissionLevel, ToolCall, ToolDefinition, ToolInput, ToolName};
use rah_tools::{
    RepositoryCreateFilePreparation, RepositoryCreateFilePreparer, RepositoryCreateFileReview,
    RepositoryDeleteFilePreparation, RepositoryDeleteFilePreparer, RepositoryDeleteFileReview,
    RepositoryMultiFileEditPreparation, RepositoryMultiFileEditPreparer,
    RepositoryMultiFileEditReview, RepositoryPatchPreparation, RepositoryPatchPreparer,
    RepositoryPatchReview, ToolRegistry,
};
use serde::{Deserialize, Serialize};

use crate::effective_authority::{EffectiveToolEntry, SourceKind};

pub(crate) const BRANCH_TICKET_TTL: Duration = Duration::from_secs(5 * 60);
pub(crate) const DESKTOP_HOST_PATH_MAX_BYTES: usize = 1024;
pub(crate) const DESKTOP_HOST_BRANCH_NAME_MAX_BYTES: usize = 128;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum HostInvocationKind {
    FsRead,
    RepoFileInfo,
    RepoStatus {},
    RepoDiff {},
    RepoDiffStaged {},
    RepoCreateBranch,
    RepoPatch,
    RepoEditFiles,
    RepoCreateFile,
    RepoDeleteFile,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum HostInvocationUnavailableReason {
    NotSupported,
    NotConnectedCurrent,
    PermissionDenied,
    RepositoryRequired,
    AuthorityNotGranted,
    ModelTurnActive,
    HostInvocationBusy,
    ProviderNotSupported,
    Stale,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HostInvocationDescriptor {
    pub eligible: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<HostInvocationKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unavailable_reason: Option<HostInvocationUnavailableReason>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum HostReadRequest {
    FsRead {
        path: String,
    },
    RepoFileInfo {
        path: String,
    },
    RepoStatus {
        #[serde(flatten)]
        fields: EmptyHostRequest,
    },
    RepoDiff {
        #[serde(flatten)]
        fields: EmptyHostRequest,
    },
    RepoDiffStaged {
        #[serde(flatten)]
        fields: EmptyHostRequest,
    },
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct EmptyHostRequest {}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct HostPrepareBranchRequest {
    pub name: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct HostPreparePatchRequest {
    pub path: String,
    pub expected_old_text: String,
    pub replacement_text: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct HostPrepareMultiFileEditRequest {
    pub targets: Vec<HostPrepareMultiFileEditTarget>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct HostPrepareMultiFileEditTarget {
    pub path: String,
    pub replacements: Vec<HostPrepareMultiFileEditReplacement>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct HostPrepareMultiFileEditReplacement {
    pub expected_old_text: String,
    pub replacement_text: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct HostPrepareCreateFileRequest {
    pub path: String,
    pub content: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct HostPrepareDeleteFileRequest {
    pub path: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct HostConfirmRequest {
    pub ticket_id: String,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HostInvocationResponse {
    pub invocation_id: String,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PreparedBranchResponse {
    pub ticket_id: String,
    pub review: BranchReview,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PreparedPatchResponse {
    pub ticket_id: String,
    pub review: RepositoryPatchReview,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PreparedMultiFileEditResponse {
    pub ticket_id: String,
    pub review: RepositoryMultiFileEditReview,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PreparedCreateFileResponse {
    pub ticket_id: String,
    pub review: RepositoryCreateFileReview,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PreparedDeleteFileResponse {
    pub ticket_id: String,
    pub review: RepositoryDeleteFileReview,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BranchReview {
    pub operation: &'static str,
    pub branch: String,
    pub target: &'static str,
    pub effect: &'static str,
    pub non_effect: &'static str,
    pub permission_category: &'static str,
    pub authority_category: &'static str,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(untagged)]
pub(crate) enum HostInvocationReview {
    Branch(BranchReview),
}

pub(crate) enum PreparedHostPayload {
    Branch {
        review: BranchReview,
    },
    Patch {
        preparation: Box<RepositoryPatchPreparation>,
        preparer: Arc<RepositoryPatchPreparer>,
    },
    MultiFileEdit {
        preparation: Box<RepositoryMultiFileEditPreparation>,
        preparer: Arc<RepositoryMultiFileEditPreparer>,
    },
    CreateFile {
        preparation: Box<RepositoryCreateFilePreparation>,
        preparer: Arc<RepositoryCreateFilePreparer>,
    },
    DeleteFile {
        preparation: Box<RepositoryDeleteFilePreparation>,
        preparer: Arc<RepositoryDeleteFilePreparer>,
    },
}

pub(crate) struct PreparedHostInvocation {
    pub ticket_id: String,
    pub activity_id: String,
    pub kind: HostInvocationKind,
    pub tool_name: ToolName,
    pub expected_definition: ToolDefinition,
    pub call: ToolCall,
    pub registry: Arc<ToolRegistry>,
    pub allowed_permissions: Vec<PermissionLevel>,
    pub generations: [u64; 4],
    pub repository_identity: Option<String>,
    pub composition_identity: usize,
    pub payload: PreparedHostPayload,
    created_at: Instant,
}

impl PreparedHostInvocation {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        ticket_id: String,
        activity_id: String,
        kind: HostInvocationKind,
        tool_name: ToolName,
        expected_definition: ToolDefinition,
        call: ToolCall,
        registry: Arc<ToolRegistry>,
        allowed_permissions: Vec<PermissionLevel>,
        generations: [u64; 4],
        repository_identity: Option<String>,
        composition_identity: usize,
        payload: PreparedHostPayload,
    ) -> Self {
        Self {
            ticket_id,
            activity_id,
            kind,
            tool_name,
            expected_definition,
            call,
            registry,
            allowed_permissions,
            generations,
            repository_identity,
            composition_identity,
            payload,
            created_at: Instant::now(),
        }
    }

    #[must_use]
    pub(crate) fn is_expired(&self, now: Instant) -> bool {
        now.duration_since(self.created_at) >= BRANCH_TICKET_TTL
    }

    #[cfg(test)]
    pub(crate) fn for_test(created_at: Instant) -> Self {
        Self {
            ticket_id: "test-ticket".to_owned(),
            activity_id: "test-activity".to_owned(),
            kind: HostInvocationKind::RepoCreateBranch,
            tool_name: ToolName::new("repo.create-branch"),
            expected_definition: ToolDefinition {
                name: ToolName::new("repo.create-branch"),
                description: String::new(),
                input_schema: serde_json::json!({}),
                permission: PermissionLevel::Execute,
            },
            call: ToolCall {
                id: rah_protocol::ToolCallId::new(),
                name: ToolName::new("repo.create-branch"),
                input: rah_protocol::ToolInput(serde_json::json!({"name": "test"})),
            },
            registry: Arc::new(ToolRegistry::new()),
            allowed_permissions: vec![PermissionLevel::Execute],
            generations: [0; 4],
            repository_identity: None,
            composition_identity: 0,
            payload: PreparedHostPayload::Branch {
                review: BranchReview {
                    operation: "Create local branch",
                    branch: "test".to_owned(),
                    target: "Current committed HEAD",
                    effect: "Creates one new local branch reference.",
                    non_effect: "Does not switch branches or modify HEAD/index/worktree.",
                    permission_category: "execute",
                    authority_category: "repository_local_branch_creation",
                },
            },
            created_at,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum CoordinatorState {
    #[default]
    Idle,
    ModelTurn,
    HostPrepared,
    HostRunning,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CoordinatorError {
    Busy,
    NotPrepared,
}

#[derive(Default)]
pub(crate) struct HostInvocationCoordinator {
    state: CoordinatorState,
    next_id: u64,
    prepared: Option<PreparedHostInvocation>,
    preparing: bool,
}

impl HostInvocationCoordinator {
    pub(crate) fn reap_expired(&mut self, now: Instant) {
        if self.state == CoordinatorState::HostPrepared
            && self
                .prepared
                .as_ref()
                .is_some_and(|ticket| ticket.is_expired(now))
        {
            self.clear_prepared();
        }
    }

    pub(crate) fn state(&self) -> CoordinatorState {
        self.state
    }

    pub(crate) fn next_invocation_id(&mut self) -> String {
        self.next_id = self.next_id.wrapping_add(1);
        format!("host-explicit-{}", self.next_id)
    }

    pub(crate) fn next_ticket_id(&mut self) -> String {
        self.next_id = self.next_id.wrapping_add(1);
        format!("host-ticket-{}", self.next_id)
    }

    pub(crate) fn begin_model(&mut self) -> Result<(), CoordinatorError> {
        if self.state != CoordinatorState::Idle {
            return Err(CoordinatorError::Busy);
        }
        self.state = CoordinatorState::ModelTurn;
        Ok(())
    }

    pub(crate) fn release_model(&mut self) {
        if self.state == CoordinatorState::ModelTurn {
            self.state = CoordinatorState::Idle;
        }
    }

    pub(crate) fn prepare(
        &mut self,
        ticket: PreparedHostInvocation,
    ) -> Result<(), CoordinatorError> {
        if self.state != CoordinatorState::Idle {
            return Err(CoordinatorError::Busy);
        }
        self.prepared = Some(ticket);
        self.preparing = false;
        self.state = CoordinatorState::HostPrepared;
        Ok(())
    }

    pub(crate) fn begin_prepare(&mut self) -> Result<(), CoordinatorError> {
        if self.state != CoordinatorState::Idle {
            return Err(CoordinatorError::Busy);
        }
        self.state = CoordinatorState::HostPrepared;
        self.preparing = true;
        Ok(())
    }

    pub(crate) fn finalize_prepare(
        &mut self,
        ticket: PreparedHostInvocation,
    ) -> Result<(), CoordinatorError> {
        if self.state != CoordinatorState::HostPrepared
            || !self.preparing
            || self.prepared.is_some()
        {
            return Err(CoordinatorError::Busy);
        }
        self.prepared = Some(ticket);
        self.preparing = false;
        Ok(())
    }

    pub(crate) fn abort_prepare(&mut self) {
        if self.state == CoordinatorState::HostPrepared && self.preparing {
            self.preparing = false;
            self.state = CoordinatorState::Idle;
        }
    }

    pub(crate) fn take_prepared(
        &mut self,
        ticket_id: &str,
        now: Instant,
    ) -> Result<PreparedHostInvocation, CoordinatorError> {
        if self.state != CoordinatorState::HostPrepared {
            return Err(CoordinatorError::NotPrepared);
        }
        if self.preparing {
            return Err(CoordinatorError::NotPrepared);
        }
        let Some(ticket) = self.prepared.take() else {
            self.state = CoordinatorState::Idle;
            return Err(CoordinatorError::NotPrepared);
        };
        if ticket.ticket_id != ticket_id || ticket.is_expired(now) {
            self.state = CoordinatorState::Idle;
            return Err(CoordinatorError::NotPrepared);
        }
        self.state = CoordinatorState::HostRunning;
        Ok(ticket)
    }

    pub(crate) fn begin_read(&mut self) -> Result<String, CoordinatorError> {
        if self.state != CoordinatorState::Idle {
            return Err(CoordinatorError::Busy);
        }
        let id = self.next_invocation_id();
        self.state = CoordinatorState::HostRunning;
        Ok(id)
    }

    pub(crate) fn cancel(
        &mut self,
        ticket_id: &str,
    ) -> Result<(HostInvocationKind, String), CoordinatorError> {
        if self.state != CoordinatorState::HostPrepared {
            return Err(CoordinatorError::NotPrepared);
        }
        if self
            .prepared
            .as_ref()
            .is_none_or(|ticket| ticket.ticket_id != ticket_id)
        {
            return Err(CoordinatorError::NotPrepared);
        }
        let kind = self
            .prepared
            .as_ref()
            .map(|ticket| ticket.kind)
            .ok_or(CoordinatorError::NotPrepared)?;
        let activity_id = self
            .prepared
            .as_ref()
            .map(|ticket| ticket.activity_id.clone())
            .ok_or(CoordinatorError::NotPrepared)?;
        self.prepared = None;
        self.preparing = false;
        self.state = CoordinatorState::Idle;
        Ok((kind, activity_id))
    }

    pub(crate) fn finish_host(&mut self) {
        if self.state == CoordinatorState::HostRunning {
            self.state = CoordinatorState::Idle;
        }
    }

    pub(crate) fn clear_prepared(&mut self) {
        if self.state == CoordinatorState::HostPrepared {
            self.prepared = None;
            self.preparing = false;
            self.state = CoordinatorState::Idle;
        }
    }
}

pub(crate) fn host_kind(name: &str) -> Option<HostInvocationKind> {
    Some(match name {
        "fs.read" => HostInvocationKind::FsRead,
        "repo.file-info" => HostInvocationKind::RepoFileInfo,
        "repo.status" => HostInvocationKind::RepoStatus {},
        "repo.diff" => HostInvocationKind::RepoDiff {},
        "repo.diff-staged" => HostInvocationKind::RepoDiffStaged {},
        "repo.create-branch" => HostInvocationKind::RepoCreateBranch,
        "repo.patch" => HostInvocationKind::RepoPatch,
        "repo.edit-files" => HostInvocationKind::RepoEditFiles,
        "repo.create-file" => HostInvocationKind::RepoCreateFile,
        "repo.delete-file" => HostInvocationKind::RepoDeleteFile,
        _ => return None,
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn host_descriptor(
    entry: &EffectiveToolEntry,
    connected_current: bool,
    repository_selected: bool,
    permission_allowed: bool,
    branch_authority_present: bool,
    patch_preparer_present: bool,
    multi_file_edit_preparer_present: bool,
    create_file_preparer_present: bool,
    delete_file_preparer_present: bool,
    coordinator_state: CoordinatorState,
) -> HostInvocationDescriptor {
    let kind = host_kind(&entry.public_tool_name);
    let reason = if (!matches!(
        entry.source_kind,
        SourceKind::BuiltIn | SourceKind::RepositoryHost
    )) || (kind == Some(HostInvocationKind::RepoDeleteFile)
        && !matches!(entry.source_kind, SourceKind::RepositoryHost))
    {
        HostInvocationUnavailableReason::ProviderNotSupported
    } else if kind.is_none() {
        HostInvocationUnavailableReason::NotSupported
    } else if !connected_current {
        HostInvocationUnavailableReason::NotConnectedCurrent
    } else if coordinator_state == CoordinatorState::ModelTurn {
        HostInvocationUnavailableReason::ModelTurnActive
    } else if coordinator_state != CoordinatorState::Idle {
        HostInvocationUnavailableReason::HostInvocationBusy
    } else if entry.repository_bound && !repository_selected {
        HostInvocationUnavailableReason::RepositoryRequired
    } else if !permission_allowed {
        HostInvocationUnavailableReason::PermissionDenied
    } else if (entry.public_tool_name == "repo.create-branch" && !branch_authority_present)
        || (entry.public_tool_name == "repo.patch" && !patch_preparer_present)
        || (entry.public_tool_name == "repo.edit-files" && !multi_file_edit_preparer_present)
        || (entry.public_tool_name == "repo.create-file" && !create_file_preparer_present)
        || (entry.public_tool_name == "repo.delete-file" && !delete_file_preparer_present)
    {
        HostInvocationUnavailableReason::AuthorityNotGranted
    } else {
        return HostInvocationDescriptor {
            eligible: true,
            kind,
            unavailable_reason: None,
        };
    };
    HostInvocationDescriptor {
        eligible: false,
        kind,
        unavailable_reason: Some(reason),
    }
}

pub(crate) fn validate_bounded_string(value: &str, max_bytes: usize) -> bool {
    !value.is_empty() && value.len() <= max_bytes && !value.contains('\0')
}

pub(crate) fn read_request(
    request: HostReadRequest,
) -> Result<(HostInvocationKind, ToolName, ToolInput), ()> {
    match request {
        HostReadRequest::FsRead { path } => {
            if !validate_bounded_string(&path, DESKTOP_HOST_PATH_MAX_BYTES) {
                return Err(());
            }
            Ok((
                HostInvocationKind::FsRead,
                ToolName::new("fs.read"),
                ToolInput(serde_json::json!({"path": path})),
            ))
        }
        HostReadRequest::RepoFileInfo { path } => {
            if !validate_bounded_string(&path, DESKTOP_HOST_PATH_MAX_BYTES) {
                return Err(());
            }
            Ok((
                HostInvocationKind::RepoFileInfo,
                ToolName::new("repo.file-info"),
                ToolInput(serde_json::json!({"path": path})),
            ))
        }
        HostReadRequest::RepoStatus { .. } => Ok((
            HostInvocationKind::RepoStatus {},
            ToolName::new("repo.status"),
            ToolInput(serde_json::json!({})),
        )),
        HostReadRequest::RepoDiff { .. } => Ok((
            HostInvocationKind::RepoDiff {},
            ToolName::new("repo.diff"),
            ToolInput(serde_json::json!({})),
        )),
        HostReadRequest::RepoDiffStaged { .. } => Ok((
            HostInvocationKind::RepoDiffStaged {},
            ToolName::new("repo.diff-staged"),
            ToolInput(serde_json::json!({})),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_allowlist_is_exact() {
        let supported = [
            "fs.read",
            "repo.file-info",
            "repo.status",
            "repo.diff",
            "repo.diff-staged",
            "repo.create-branch",
            "repo.patch",
            "repo.edit-files",
            "repo.create-file",
            "repo.delete-file",
        ];
        for name in supported {
            assert!(host_kind(name).is_some());
        }
        for name in [
            "repo.commit",
            "repo.rename-file",
            "repo.create-directory",
            "echo",
            "fixture",
            "mcp.example.tool",
            "unknown",
        ] {
            assert!(host_kind(name).is_none());
        }
    }

    #[test]
    fn patch_host_descriptor_requires_the_retained_preparer() {
        let entry = EffectiveToolEntry {
            public_tool_name: "repo.patch".to_owned(),
            source_kind: SourceKind::RepositoryHost,
            source_label: "desktop_repository".to_owned(),
            effect_class: crate::effective_authority::EffectClass::RepositoryMutation,
            authority_category:
                crate::effective_authority::AuthorityCategory::RepositoryContentMutation,
            permission: PermissionLevel::Execute,
            repository_bound: true,
            advertised: true,
            host_invocation: HostInvocationDescriptor {
                eligible: false,
                kind: None,
                unavailable_reason: None,
            },
        };
        let unavailable = host_descriptor(
            &entry,
            true,
            true,
            true,
            false,
            false,
            false,
            false,
            false,
            CoordinatorState::Idle,
        );
        assert!(!unavailable.eligible);
        assert_eq!(
            unavailable.unavailable_reason,
            Some(HostInvocationUnavailableReason::AuthorityNotGranted)
        );
        assert!(
            host_descriptor(
                &entry,
                true,
                true,
                true,
                false,
                true,
                false,
                false,
                false,
                CoordinatorState::Idle,
            )
            .eligible
        );
    }

    #[test]
    fn multi_file_host_descriptor_requires_the_retained_preparer() {
        let entry = EffectiveToolEntry {
            public_tool_name: "repo.edit-files".to_owned(),
            source_kind: SourceKind::RepositoryHost,
            source_label: "desktop_repository".to_owned(),
            effect_class: crate::effective_authority::EffectClass::RepositoryMutation,
            authority_category:
                crate::effective_authority::AuthorityCategory::RepositoryContentMutation,
            permission: PermissionLevel::Execute,
            repository_bound: true,
            advertised: true,
            host_invocation: HostInvocationDescriptor {
                eligible: false,
                kind: None,
                unavailable_reason: None,
            },
        };
        assert!(
            !host_descriptor(
                &entry,
                true,
                true,
                true,
                false,
                true,
                false,
                false,
                false,
                CoordinatorState::Idle,
            )
            .eligible
        );
        assert!(
            host_descriptor(
                &entry,
                true,
                true,
                true,
                false,
                true,
                true,
                false,
                false,
                CoordinatorState::Idle,
            )
            .eligible
        );
    }

    #[test]
    fn create_file_host_descriptor_requires_the_retained_preparer() {
        let entry = EffectiveToolEntry {
            public_tool_name: "repo.create-file".to_owned(),
            source_kind: SourceKind::RepositoryHost,
            source_label: "desktop_repository".to_owned(),
            effect_class: crate::effective_authority::EffectClass::RepositoryMutation,
            authority_category:
                crate::effective_authority::AuthorityCategory::RepositoryFileCreation,
            permission: PermissionLevel::Execute,
            repository_bound: true,
            advertised: true,
            host_invocation: HostInvocationDescriptor {
                eligible: false,
                kind: None,
                unavailable_reason: None,
            },
        };
        let unavailable = host_descriptor(
            &entry,
            true,
            true,
            true,
            false,
            false,
            false,
            false,
            false,
            CoordinatorState::Idle,
        );
        assert!(!unavailable.eligible);
        assert_eq!(
            unavailable.unavailable_reason,
            Some(HostInvocationUnavailableReason::AuthorityNotGranted)
        );
        let eligible = host_descriptor(
            &entry,
            true,
            true,
            true,
            false,
            false,
            false,
            true,
            false,
            CoordinatorState::Idle,
        );
        assert_eq!(eligible.kind, Some(HostInvocationKind::RepoCreateFile));
        assert!(eligible.eligible);
    }

    #[test]
    fn deletion_host_descriptor_requires_the_retained_preparer() {
        let entry = EffectiveToolEntry {
            public_tool_name: "repo.delete-file".to_owned(),
            source_kind: SourceKind::RepositoryHost,
            source_label: "desktop_repository".to_owned(),
            effect_class: crate::effective_authority::EffectClass::RepositoryMutation,
            authority_category:
                crate::effective_authority::AuthorityCategory::RepositoryFileDeletion,
            permission: PermissionLevel::Execute,
            repository_bound: true,
            advertised: true,
            host_invocation: HostInvocationDescriptor {
                eligible: false,
                kind: None,
                unavailable_reason: None,
            },
        };
        let unavailable = host_descriptor(
            &entry,
            true,
            true,
            true,
            false,
            false,
            false,
            false,
            false,
            CoordinatorState::Idle,
        );
        assert!(!unavailable.eligible);
        assert_eq!(
            unavailable.unavailable_reason,
            Some(HostInvocationUnavailableReason::AuthorityNotGranted)
        );
        let eligible = host_descriptor(
            &entry,
            true,
            true,
            true,
            false,
            false,
            false,
            false,
            true,
            CoordinatorState::Idle,
        );
        assert_eq!(eligible.kind, Some(HostInvocationKind::RepoDeleteFile));
        assert!(eligible.eligible);
        assert_eq!(
            host_descriptor(
                &entry,
                true,
                true,
                false,
                false,
                false,
                false,
                false,
                true,
                CoordinatorState::Idle,
            )
            .unavailable_reason,
            Some(HostInvocationUnavailableReason::PermissionDenied)
        );
    }

    #[test]
    fn ticket_expiry_is_deterministic_without_sleeping() {
        let now = Instant::now();
        let ticket = PreparedHostInvocation::for_test(now);
        assert!(!ticket.is_expired(now + BRANCH_TICKET_TTL - Duration::from_nanos(1)));
        assert!(ticket.is_expired(now + BRANCH_TICKET_TTL));
    }

    #[test]
    fn coordinator_excludes_model_and_host_work() {
        let mut coordinator = HostInvocationCoordinator::default();
        coordinator
            .begin_model()
            .expect("model owns idle coordinator");
        assert_eq!(coordinator.begin_read(), Err(CoordinatorError::Busy));
        coordinator.release_model();
        let id = coordinator
            .begin_read()
            .expect("host owns idle coordinator");
        assert!(!id.is_empty());
        assert_eq!(coordinator.begin_model(), Err(CoordinatorError::Busy));
        coordinator.finish_host();
        coordinator
            .begin_model()
            .expect("terminal host releases coordinator");
    }

    #[test]
    fn async_prepare_reservation_has_one_finalize_path() {
        let now = Instant::now();
        let mut coordinator = HostInvocationCoordinator::default();

        coordinator
            .begin_prepare()
            .expect("idle coordinator reserves preparation");
        assert_eq!(coordinator.state(), CoordinatorState::HostPrepared);
        assert_eq!(coordinator.begin_prepare(), Err(CoordinatorError::Busy));
        assert_eq!(coordinator.begin_model(), Err(CoordinatorError::Busy));
        assert_eq!(coordinator.begin_read(), Err(CoordinatorError::Busy));
        assert!(matches!(
            coordinator.take_prepared("test-ticket", now),
            Err(CoordinatorError::NotPrepared)
        ));
        assert_eq!(
            coordinator.cancel("test-ticket"),
            Err(CoordinatorError::NotPrepared)
        );

        coordinator
            .finalize_prepare(PreparedHostInvocation::for_test(now))
            .expect("reserved preparation finalizes");
        assert_eq!(coordinator.state(), CoordinatorState::HostPrepared);

        let ticket = coordinator
            .take_prepared("test-ticket", now)
            .expect("finalized ticket is available");
        assert_eq!(ticket.ticket_id, "test-ticket");
        assert!(matches!(
            coordinator.take_prepared("test-ticket", now),
            Err(CoordinatorError::NotPrepared)
        ));
    }

    #[test]
    fn finalize_prepare_requires_the_exact_transient_reservation() {
        let now = Instant::now();
        let mut coordinator = HostInvocationCoordinator::default();

        assert!(matches!(
            coordinator.finalize_prepare(PreparedHostInvocation::for_test(now)),
            Err(CoordinatorError::Busy)
        ));

        coordinator
            .prepare(PreparedHostInvocation::for_test(now))
            .expect("direct branch preparation still works");
        assert!(matches!(
            coordinator.finalize_prepare(PreparedHostInvocation::for_test(now)),
            Err(CoordinatorError::Busy)
        ));
        assert!(coordinator.take_prepared("test-ticket", now).is_ok());
        coordinator.finish_host();
    }

    #[test]
    fn abort_only_releases_a_transient_reservation() {
        let now = Instant::now();
        let mut coordinator = HostInvocationCoordinator::default();

        coordinator
            .begin_prepare()
            .expect("idle coordinator reserves preparation");
        coordinator.abort_prepare();
        assert_eq!(coordinator.state(), CoordinatorState::Idle);

        coordinator
            .begin_prepare()
            .expect("reservation can be retried after abort");
        coordinator
            .finalize_prepare(PreparedHostInvocation::for_test(now))
            .expect("reserved preparation finalizes");
        coordinator.abort_prepare();
        assert_eq!(coordinator.state(), CoordinatorState::HostPrepared);
        assert!(coordinator.take_prepared("test-ticket", now).is_ok());
    }

    #[test]
    fn typed_read_forms_construct_only_the_closed_tool_inputs() {
        let cases = [
            (
                HostReadRequest::FsRead {
                    path: "notes.txt".to_owned(),
                },
                "fs.read",
                serde_json::json!({"path": "notes.txt"}),
            ),
            (
                HostReadRequest::RepoFileInfo {
                    path: "src/lib.rs".to_owned(),
                },
                "repo.file-info",
                serde_json::json!({"path": "src/lib.rs"}),
            ),
            (
                HostReadRequest::RepoStatus {
                    fields: EmptyHostRequest::default(),
                },
                "repo.status",
                serde_json::json!({}),
            ),
            (
                HostReadRequest::RepoDiff {
                    fields: EmptyHostRequest::default(),
                },
                "repo.diff",
                serde_json::json!({}),
            ),
            (
                HostReadRequest::RepoDiffStaged {
                    fields: EmptyHostRequest::default(),
                },
                "repo.diff-staged",
                serde_json::json!({}),
            ),
        ];
        for (request, name, input) in cases {
            let (_, actual_name, actual_input) =
                read_request(request).expect("closed typed request should construct");
            assert_eq!(actual_name.as_str(), name);
            assert_eq!(actual_input.0, input);
        }
        assert!(
            serde_json::from_value::<HostReadRequest>(serde_json::json!({
                "kind": "repo_status",
                "options": {}
            }))
            .is_err()
        );
        assert!(
            serde_json::from_value::<HostPrepareBranchRequest>(serde_json::json!({
                "name": "topic",
                "permission": "execute"
            }))
            .is_err()
        );
        let patch = serde_json::from_value::<HostPreparePatchRequest>(serde_json::json!({
            "path": "src/lib.rs",
            "expectedOldText": "old",
            "replacementText": "new"
        }))
        .expect("typed patch request should deserialize");
        assert_eq!(patch.path, "src/lib.rs");
        assert_eq!(patch.expected_old_text, "old");
        assert_eq!(patch.replacement_text, "new");
        let multi = serde_json::from_value::<HostPrepareMultiFileEditRequest>(serde_json::json!({
            "targets": [{
                "path": "src/lib.rs",
                "replacements": [{
                    "expectedOldText": "old",
                    "replacementText": "new"
                }]
            }]
        }))
        .expect("typed multi-file request should deserialize");
        assert_eq!(multi.targets.len(), 1);
        assert_eq!(multi.targets[0].replacements[0].expected_old_text, "old");
        let create = serde_json::from_value::<HostPrepareCreateFileRequest>(serde_json::json!({
            "path": "src/new.rs",
            "content": "RAH_SECRET_CREATE_FILE_CONTENT_SENTINEL"
        }))
        .expect("typed create-file request should deserialize");
        assert_eq!(create.path, "src/new.rs");
        assert_eq!(create.content, "RAH_SECRET_CREATE_FILE_CONTENT_SENTINEL");
        let delete = serde_json::from_value::<HostPrepareDeleteFileRequest>(serde_json::json!({
            "path": "src/delete.rs"
        }))
        .expect("typed delete-file request should deserialize");
        assert_eq!(delete.path, "src/delete.rs");
        for field in [
            serde_json::json!({"path":"a","expectedOldText":"b","replacementText":"c","hash":"x"}),
            serde_json::json!({"path":"a","expectedOldText":"b","replacementText":"c","toolName":"repo.patch"}),
            serde_json::json!({"path":"a","expectedOldText":"b","replacementText":"c","input":{}}),
        ] {
            assert!(serde_json::from_value::<HostPreparePatchRequest>(field).is_err());
        }
        for field in [
            serde_json::json!({"targets": [], "toolName": "repo.edit-files"}),
            serde_json::json!({"targets": [{"path":"a","replacements":[],"hash":"x"}]}),
            serde_json::json!({"targets": [{"path":"a","replacements":[{"expectedOldText":"b","replacementText":"c","order":1}]}]}),
        ] {
            assert!(serde_json::from_value::<HostPrepareMultiFileEditRequest>(field).is_err());
        }
        for field in [
            serde_json::json!({"path":"a","content":"b","repository":"repo"}),
            serde_json::json!({"path":"a","content":"b","nativePath":"x"}),
            serde_json::json!({"path":"a","content":"b","toolName":"repo.create-file"}),
            serde_json::json!({"path":"a","content":"b","input":{}}),
            serde_json::json!({"path":"a","content":"b","hash":"x"}),
            serde_json::json!({"path":"a","content":"b","length":1}),
            serde_json::json!({"path":"a","content":"b","permission":"execute"}),
            serde_json::json!({"path":"a","content":"b","authority":true}),
            serde_json::json!({"path":"a","content":"b","retry":true}),
            serde_json::json!({"path":"a","content":"b","stage":true}),
            serde_json::json!({"path":"a","content":"b","commit":true}),
            serde_json::json!({"path":"a","content":"b","ticketId":"x"}),
            serde_json::json!({"path":"a","content":"b","activityId":"x"}),
        ] {
            assert!(serde_json::from_value::<HostPrepareCreateFileRequest>(field).is_err());
        }
        for field in [
            serde_json::json!({"path":"a","hash":"x"}),
            serde_json::json!({"path":"a","length":1}),
            serde_json::json!({"path":"a","repository":"repo"}),
            serde_json::json!({"path":"a","nativePath":"x"}),
            serde_json::json!({"path":"a","toolName":"repo.delete-file"}),
            serde_json::json!({"path":"a","input":{}}),
            serde_json::json!({"path":"a","permission":"execute"}),
            serde_json::json!({"path":"a","authority":true}),
            serde_json::json!({"path":"a","retry":true}),
            serde_json::json!({"path":"a","restore":true}),
            serde_json::json!({"path":"a","stage":true}),
            serde_json::json!({"path":"a","commit":true}),
            serde_json::json!({"path":"a","ticketId":"x"}),
            serde_json::json!({"path":"a","activityId":"x"}),
        ] {
            assert!(serde_json::from_value::<HostPrepareDeleteFileRequest>(field).is_err());
        }
    }

    #[test]
    fn invalid_confirmation_consumes_the_prepared_ticket() {
        let now = Instant::now();
        let mut coordinator = HostInvocationCoordinator::default();
        coordinator
            .prepare(PreparedHostInvocation::for_test(now))
            .expect("test ticket prepares");
        assert!(matches!(
            coordinator.take_prepared("wrong-ticket", now),
            Err(CoordinatorError::NotPrepared)
        ));
        assert_eq!(coordinator.state(), CoordinatorState::Idle);
        assert!(matches!(
            coordinator.take_prepared("test-ticket", now),
            Err(CoordinatorError::NotPrepared)
        ));
    }

    #[test]
    fn ticket_matrix_is_single_use_for_valid_expired_cancelled_and_stale_paths() {
        let now = Instant::now();

        let mut valid = HostInvocationCoordinator::default();
        valid
            .prepare(PreparedHostInvocation::for_test(now))
            .expect("valid ticket prepares");
        assert!(valid.take_prepared("test-ticket", now).is_ok());
        valid.finish_host();
        assert_eq!(valid.state(), CoordinatorState::Idle);
        assert!(matches!(
            valid.take_prepared("test-ticket", now),
            Err(CoordinatorError::NotPrepared)
        ));

        let mut expired = HostInvocationCoordinator::default();
        expired
            .prepare(PreparedHostInvocation::for_test(now - BRANCH_TICKET_TTL))
            .expect("expired ticket prepares for deterministic test");
        expired.reap_expired(now);
        assert_eq!(expired.state(), CoordinatorState::Idle);
        assert!(matches!(
            expired.take_prepared("test-ticket", now),
            Err(CoordinatorError::NotPrepared)
        ));

        let mut cancelled = HostInvocationCoordinator::default();
        cancelled
            .prepare(PreparedHostInvocation::for_test(now))
            .expect("cancelled ticket prepares");
        assert_eq!(
            cancelled.cancel("test-ticket"),
            Ok((
                HostInvocationKind::RepoCreateBranch,
                "test-activity".to_owned()
            ))
        );
        assert_eq!(
            cancelled.cancel("test-ticket"),
            Err(CoordinatorError::NotPrepared)
        );
        assert!(matches!(
            cancelled.take_prepared("test-ticket", now),
            Err(CoordinatorError::NotPrepared)
        ));

        let mut stale = HostInvocationCoordinator::default();
        stale
            .prepare(PreparedHostInvocation::for_test(now))
            .expect("stale ticket prepares");
        assert!(matches!(
            stale.take_prepared("different-ticket", now),
            Err(CoordinatorError::NotPrepared)
        ));
        assert_eq!(stale.state(), CoordinatorState::Idle);
        assert!(matches!(
            stale.take_prepared("test-ticket", now),
            Err(CoordinatorError::NotPrepared)
        ));
    }

    #[test]
    fn activity_correlation_id_is_not_a_ticket_for_confirmation_or_cancellation() {
        const TICKET: &str = "RAH_SECRET_AUTHORITY_TICKET_SENTINEL";
        let now = Instant::now();
        let mut coordinator = HostInvocationCoordinator::default();
        let mut prepared = PreparedHostInvocation::for_test(now);
        prepared.ticket_id = TICKET.to_owned();
        prepared.activity_id = "host-explicit-activity-1".to_owned();
        let activity_id = prepared.activity_id.clone();
        coordinator
            .prepare(prepared)
            .expect("separated ticket and activity state prepares");
        assert_eq!(
            coordinator.cancel(&activity_id),
            Err(CoordinatorError::NotPrepared)
        );
        assert_eq!(
            coordinator.cancel(TICKET),
            Ok((HostInvocationKind::RepoCreateBranch, activity_id,))
        );
    }
}
