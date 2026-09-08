#![cfg(target_os = "windows")]

//! Desktop-private explicit host Tool invocation.

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use rah_protocol::{PermissionLevel, ToolCall, ToolDefinition, ToolInput, ToolName};
use rah_tools::{
    RepositoryPatchPreparation, RepositoryPatchPreparer, RepositoryPatchReview, ToolRegistry,
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
}

pub(crate) struct PreparedHostInvocation {
    pub ticket_id: String,
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
    ) -> Result<HostInvocationKind, CoordinatorError> {
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
        self.prepared = None;
        self.preparing = false;
        self.state = CoordinatorState::Idle;
        Ok(kind)
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
        _ => return None,
    })
}

pub(crate) fn host_descriptor(
    entry: &EffectiveToolEntry,
    connected_current: bool,
    repository_selected: bool,
    permission_allowed: bool,
    branch_authority_present: bool,
    patch_preparer_present: bool,
    coordinator_state: CoordinatorState,
) -> HostInvocationDescriptor {
    let kind = host_kind(&entry.public_tool_name);
    let reason = if !matches!(
        entry.source_kind,
        SourceKind::BuiltIn | SourceKind::RepositoryHost
    ) {
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
        ];
        for name in supported {
            assert!(host_kind(name).is_some());
        }
        for name in [
            "repo.commit",
            "repo.create-file",
            "repo.edit-files",
            "repo.delete-file",
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
                CoordinatorState::Idle,
            )
            .eligible
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
        for field in [
            serde_json::json!({"path":"a","expectedOldText":"b","replacementText":"c","hash":"x"}),
            serde_json::json!({"path":"a","expectedOldText":"b","replacementText":"c","toolName":"repo.patch"}),
            serde_json::json!({"path":"a","expectedOldText":"b","replacementText":"c","input":{}}),
        ] {
            assert!(serde_json::from_value::<HostPreparePatchRequest>(field).is_err());
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
}
