#![cfg(target_os = "windows")]

use rah_protocol::{PermissionLevel, ToolDefinition};
use rah_tools::ToolRegistry;
use serde::Serialize;
use std::sync::Arc;

use crate::{CodexExecutableSource, CommitAuthorizationPresentation, DesktopRepository};

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SnapshotStatus {
    NoRepository,
    Disconnected,
    Connecting,
    ConnectedCurrent,
    ReconnectRequired,
    Stale,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RepositoryKind {
    SelectedRepository,
    None,
}
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RepositoryIdentity {
    Current,
    NotSelected,
    Stale,
    Unknown,
}
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ConnectionBindingState {
    NotConnected,
    Connecting,
    Connected,
    Disconnecting,
    Error,
}
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)]
pub(crate) enum SourceKind {
    BuiltIn,
    TrustedProfile,
    RepositoryHost,
    Mcp,
    ProcessPlugin,
}
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)]
pub(crate) enum EffectClass {
    ReadOnly,
    RepositoryMutation,
    IndexMutation,
    Commit,
    Execute,
    External,
}
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)]
pub(crate) enum AuthorityCategory {
    RepositoryObservation,
    RepositoryContentMutation,
    RepositoryFileCreation,
    RepositoryFileDeletion,
    RepositoryFileRename,
    RepositoryDirectoryCreation,
    RepositoryIndexMutation,
    RepositoryCommit,
    Read,
    Execute,
    External,
}
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)]
pub(crate) enum UnavailableState {
    ConfiguredUnavailable,
    NotEffective,
}
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)]
pub(crate) enum UnavailableReason {
    NotConfigured,
    AuthorityNotGranted,
    RepositoryRequired,
    ReconnectRequired,
    ProviderNotEffective,
    ProviderUnavailable,
    PermissionNotConfigured,
    ReviewRequired,
    StaleContext,
    Unknown,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ReviewedCommitState {
    NotApplicable,
    IdentityNotConfigured,
    ReviewRequired,
    ReadyToAuthorize,
    AuthorizedPending,
    Stale,
    AuthorizationRevoked,
    Unavailable,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RepositoryBinding {
    pub selected: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    pub kind: RepositoryKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_generation: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub captured_generation: Option<u64>,
    pub identity: RepositoryIdentity,
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ConnectionBinding {
    pub state: ConnectionBindingState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime_kind: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime_source: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub captured_repository_generation: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub captured_model_generation: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub captured_connection_generation: Option<u64>,
    pub advertised: bool,
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ConfiguredSummary {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile_source: Option<SourceKind>,
    pub configured_provider_count: u32,
    pub configured_capability_count: u32,
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EffectiveToolEntry {
    pub public_tool_name: String,
    pub source_kind: SourceKind,
    pub source_label: &'static str,
    pub effect_class: EffectClass,
    pub authority_category: AuthorityCategory,
    pub permission: PermissionLevel,
    pub repository_bound: bool,
    pub advertised: bool,
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UnavailableCapability {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub public_tool_name: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_kind: Option<SourceKind>,
    pub state: UnavailableState,
    pub reason: UnavailableReason,
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EffectiveAuthoritySnapshot {
    pub schema_version: u32,
    pub status: SnapshotStatus,
    pub repository: RepositoryBinding,
    pub connection: ConnectionBinding,
    pub configured: ConfiguredSummary,
    pub effective_tools: Vec<EffectiveToolEntry>,
    pub unavailable_capabilities: Vec<UnavailableCapability>,
    pub reviewed_commit: ReviewedCommitState,
}

#[derive(Clone)]
pub(crate) struct DesktopToolComposition {
    pub registry: Arc<ToolRegistry>,
    pub tools: Vec<EffectiveToolEntry>,
    pub unavailable: Vec<UnavailableCapability>,
}

fn metadata(name: &str) -> Option<(EffectClass, AuthorityCategory, bool)> {
    Some(match name {
        "echo" => (EffectClass::Execute, AuthorityCategory::Execute, false),
        "fs.read" => (EffectClass::ReadOnly, AuthorityCategory::Read, true),
        "repo.file-info" | "repo.status" | "repo.diff" | "repo.diff-staged" => (
            EffectClass::ReadOnly,
            AuthorityCategory::RepositoryObservation,
            true,
        ),
        "repo.patch" | "repo.edit-files" => (
            EffectClass::RepositoryMutation,
            AuthorityCategory::RepositoryContentMutation,
            true,
        ),
        "repo.create-file" => (
            EffectClass::RepositoryMutation,
            AuthorityCategory::RepositoryFileCreation,
            true,
        ),
        "repo.create-directory" => (
            EffectClass::RepositoryMutation,
            AuthorityCategory::RepositoryDirectoryCreation,
            true,
        ),
        "repo.delete-file" => (
            EffectClass::RepositoryMutation,
            AuthorityCategory::RepositoryFileDeletion,
            true,
        ),
        "repo.rename-file" => (
            EffectClass::RepositoryMutation,
            AuthorityCategory::RepositoryFileRename,
            true,
        ),
        "repo.commit" => (
            EffectClass::Commit,
            AuthorityCategory::RepositoryCommit,
            true,
        ),
        _ => return None,
    })
}

fn make_unavailable(name: &'static str, reason: UnavailableReason) -> UnavailableCapability {
    UnavailableCapability {
        public_tool_name: Some(name),
        source_kind: Some(SourceKind::RepositoryHost),
        state: UnavailableState::ConfiguredUnavailable,
        reason,
    }
}

pub(crate) fn compose(
    registry: Arc<ToolRegistry>,
    repository: Option<&DesktopRepository>,
    commit_tool_present: bool,
) -> DesktopToolComposition {
    let mut tools = registry
        .definitions()
        .into_iter()
        .filter_map(|definition: ToolDefinition| {
            let (effect_class, authority_category, repository_bound) =
                metadata(definition.name.as_str())?;
            Some(EffectiveToolEntry {
                public_tool_name: definition.name.to_string(),
                source_kind: if repository_bound {
                    SourceKind::RepositoryHost
                } else {
                    SourceKind::BuiltIn
                },
                source_label: if repository_bound {
                    "desktop_repository"
                } else {
                    "desktop_builtin"
                },
                effect_class,
                authority_category,
                permission: definition.permission,
                repository_bound,
                advertised: false,
            })
        })
        .collect::<Vec<_>>();
    tools.sort_by(|left, right| left.public_tool_name.cmp(&right.public_tool_name));
    let mut unavailable = Vec::new();
    if repository.is_none() {
        for name in [
            "fs.read",
            "repo.file-info",
            "repo.status",
            "repo.diff",
            "repo.diff-staged",
            "repo.patch",
            "repo.edit-files",
            "repo.create-file",
            "repo.create-directory",
            "repo.delete-file",
            "repo.rename-file",
            "repo.commit",
        ] {
            unavailable.push(make_unavailable(
                name,
                UnavailableReason::RepositoryRequired,
            ));
        }
    } else {
        if repository.is_some_and(|value| value.directory_creation_authority.is_none()) {
            unavailable.push(make_unavailable(
                "repo.create-directory",
                UnavailableReason::AuthorityNotGranted,
            ));
        }
        if repository.is_some_and(|value| value.deletion_authority.is_none()) {
            unavailable.push(make_unavailable(
                "repo.delete-file",
                UnavailableReason::AuthorityNotGranted,
            ));
        }
        if repository.is_some_and(|value| value.rename_authority.is_none()) {
            unavailable.push(make_unavailable(
                "repo.rename-file",
                UnavailableReason::AuthorityNotGranted,
            ));
        }
        if !commit_tool_present {
            unavailable.push(make_unavailable(
                "repo.commit",
                UnavailableReason::AuthorityNotGranted,
            ));
        }
    }
    DesktopToolComposition {
        registry,
        tools,
        unavailable,
    }
}

pub(crate) fn source_label(source: CodexExecutableSource) -> &'static str {
    match source {
        CodexExecutableSource::CertifiedBaseline => "certified_side_by_side",
        CodexExecutableSource::Override => "configured_runtime",
        CodexExecutableSource::Path => "resolved_host_binary",
    }
}

pub(crate) fn reviewed_commit(
    state: CommitAuthorizationPresentation,
    selected: bool,
) -> ReviewedCommitState {
    if !selected {
        return ReviewedCommitState::NotApplicable;
    }
    match state {
        CommitAuthorizationPresentation::IdentityNotConfigured => {
            ReviewedCommitState::IdentityNotConfigured
        }
        CommitAuthorizationPresentation::ReviewRequired => ReviewedCommitState::ReviewRequired,
        CommitAuthorizationPresentation::ReadyToAuthorize => ReviewedCommitState::ReadyToAuthorize,
        CommitAuthorizationPresentation::AuthorizedPending => {
            ReviewedCommitState::AuthorizedPending
        }
        CommitAuthorizationPresentation::ReviewStale => ReviewedCommitState::Stale,
        CommitAuthorizationPresentation::AuthorizationRevoked => {
            ReviewedCommitState::AuthorizationRevoked
        }
        CommitAuthorizationPresentation::ConnectionRequired
        | CommitAuthorizationPresentation::AuthorizationFailed => ReviewedCommitState::Unavailable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn complete_snapshot_serialization_is_sanitized_and_closed() {
        let snapshot = EffectiveAuthoritySnapshot {
            schema_version: 1,
            status: SnapshotStatus::ConnectedCurrent,
            repository: RepositoryBinding {
                selected: true,
                display_name: Some("private-repo".to_owned()),
                kind: RepositoryKind::SelectedRepository,
                current_generation: Some(7),
                captured_generation: Some(7),
                identity: RepositoryIdentity::Current,
            },
            connection: ConnectionBinding {
                state: ConnectionBindingState::Connected,
                runtime_kind: Some("codex"),
                runtime_source: Some("certified_side_by_side"),
                captured_repository_generation: Some(7),
                captured_model_generation: Some(3),
                captured_connection_generation: Some(9),
                advertised: true,
            },
            configured: ConfiguredSummary {
                profile_source: Some(SourceKind::BuiltIn),
                configured_provider_count: 0,
                configured_capability_count: 1,
            },
            effective_tools: vec![EffectiveToolEntry {
                public_tool_name: "repo.status".to_owned(),
                source_kind: SourceKind::RepositoryHost,
                source_label: "desktop_repository",
                effect_class: EffectClass::ReadOnly,
                authority_category: AuthorityCategory::RepositoryObservation,
                permission: PermissionLevel::Read,
                repository_bound: true,
                advertised: true,
            }],
            unavailable_capabilities: vec![UnavailableCapability {
                public_tool_name: Some("repo.commit"),
                source_kind: Some(SourceKind::RepositoryHost),
                state: UnavailableState::ConfiguredUnavailable,
                reason: UnavailableReason::AuthorityNotGranted,
            }],
            reviewed_commit: ReviewedCommitState::ReviewRequired,
        };
        let json = serde_json::to_string(&snapshot).expect("snapshot serializes");
        for secret in [
            r#"C:\Users\SECRET_USER\private-repo"#,
            "SUPER_SECRET_TOKEN",
            "https://user:password@example.invalid/mcp",
            "SECRET_STDERR",
            "rah_tool_17",
        ] {
            assert!(!json.contains(secret), "secret leaked: {secret}");
        }
        assert!(json.contains("private-repo"));
        assert!(json.contains("certified_side_by_side"));
        assert!(json.contains("authority_not_granted"));
        assert!(json.contains("connected_current"));
    }

    #[test]
    fn source_mapping_never_serializes_an_executable_path() {
        assert_eq!(
            source_label(CodexExecutableSource::CertifiedBaseline),
            "certified_side_by_side"
        );
        assert_eq!(
            source_label(CodexExecutableSource::Override),
            "configured_runtime"
        );
        assert_eq!(
            source_label(CodexExecutableSource::Path),
            "resolved_host_binary"
        );
    }
}
