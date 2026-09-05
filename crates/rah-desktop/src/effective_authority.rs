#![cfg(target_os = "windows")]

use rah_protocol::PermissionLevel;
use rah_tools::{ToolRegistry, TrustedStaticProfile};
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
    pub source_label: String,
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
    pub public_tool_name: Option<String>,
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

/// Host-owned classification for one external Tool admitted by the selected
/// Trusted Profile. Provider descriptions and discovered metadata never enter
/// this representation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ExternalToolDescriptor {
    pub public_tool_name: String,
    pub source_kind: SourceKind,
    pub source_label: String,
    pub permission: PermissionLevel,
    pub effect_class: EffectClass,
    pub authority_category: AuthorityCategory,
    pub repository_bound: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CompositionError {
    UnclassifiedTool,
    ExternalMetadataMismatch,
    ExternalToolNotAdmitted,
}

const MAX_PROVIDER_LABEL_BYTES: usize = 64;
const FALLBACK_PROVIDER_LABEL: &str = "external_provider";

#[derive(Clone)]
pub(crate) struct DesktopToolComposition {
    pub registry: Arc<ToolRegistry>,
    pub tools: Vec<EffectiveToolEntry>,
    pub unavailable: Vec<UnavailableCapability>,
}

/// Builds only bounded presentation metadata from the validated host profile.
/// The naming convention is the adapter's public RAH name, not provider text.
pub(crate) fn external_tool_descriptors(
    profile: &TrustedStaticProfile,
) -> Result<Vec<ExternalToolDescriptor>, rah_tools::ProfileError> {
    let mut descriptors = Vec::new();
    for provider in profile.mcp_providers() {
        let source_label = sanitize_provider_label(provider.id());
        for tool in provider.tools() {
            descriptors.push(ExternalToolDescriptor {
                public_tool_name: format!("mcp.{}.{}", provider.id(), tool.remote_name()),
                source_kind: SourceKind::Mcp,
                source_label: source_label.clone(),
                permission: tool.permission()?,
                effect_class: EffectClass::External,
                authority_category: AuthorityCategory::External,
                repository_bound: false,
            });
        }
    }
    for provider in profile.process_plugins() {
        let source_label = sanitize_provider_label(provider.id());
        for tool in provider.tools() {
            descriptors.push(ExternalToolDescriptor {
                public_tool_name: format!("plugin.{}.{}", provider.id(), tool.remote_name()),
                source_kind: SourceKind::ProcessPlugin,
                source_label: source_label.clone(),
                permission: tool.permission()?,
                effect_class: EffectClass::External,
                authority_category: AuthorityCategory::External,
                repository_bound: false,
            });
        }
    }
    descriptors.sort_by(|left, right| left.public_tool_name.cmp(&right.public_tool_name));
    Ok(descriptors)
}

fn sanitize_provider_label(value: &str) -> String {
    if value.is_empty()
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
    {
        return FALLBACK_PROVIDER_LABEL.to_owned();
    }
    value
        .bytes()
        .take(MAX_PROVIDER_LABEL_BYTES)
        .map(char::from)
        .collect()
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

fn make_unavailable(name: &str, reason: UnavailableReason) -> UnavailableCapability {
    UnavailableCapability {
        public_tool_name: Some(name.to_owned()),
        source_kind: Some(SourceKind::RepositoryHost),
        state: UnavailableState::ConfiguredUnavailable,
        reason,
    }
}

pub(crate) fn compose(
    registry: Arc<ToolRegistry>,
    repository: Option<&DesktopRepository>,
    commit_tool_present: bool,
    external_descriptors: &[ExternalToolDescriptor],
) -> Result<DesktopToolComposition, CompositionError> {
    let mut tools = Vec::new();
    let mut matched_external = vec![false; external_descriptors.len()];
    for definition in registry.definitions() {
        let entry = if let Some((effect_class, authority_category, repository_bound)) =
            metadata(definition.name.as_str())
        {
            EffectiveToolEntry {
                public_tool_name: definition.name.to_string(),
                source_kind: if repository_bound {
                    SourceKind::RepositoryHost
                } else {
                    SourceKind::BuiltIn
                },
                source_label: if repository_bound {
                    "desktop_repository".to_owned()
                } else {
                    "desktop_builtin".to_owned()
                },
                effect_class,
                authority_category,
                permission: definition.permission,
                repository_bound,
                advertised: false,
            }
        } else if let Some((index, external)) = external_descriptors
            .iter()
            .enumerate()
            .find(|(_, external)| external.public_tool_name == definition.name.as_str())
        {
            if external.permission != definition.permission
                || !matches!(
                    external.source_kind,
                    SourceKind::Mcp | SourceKind::ProcessPlugin
                )
                || external.effect_class != EffectClass::External
                || external.authority_category != AuthorityCategory::External
                || external.repository_bound
            {
                return Err(CompositionError::ExternalMetadataMismatch);
            }
            matched_external[index] = true;
            EffectiveToolEntry {
                public_tool_name: definition.name.to_string(),
                source_kind: external.source_kind,
                source_label: sanitize_provider_label(&external.source_label),
                effect_class: external.effect_class,
                authority_category: external.authority_category,
                permission: definition.permission,
                repository_bound: external.repository_bound,
                advertised: false,
            }
        } else {
            return Err(CompositionError::UnclassifiedTool);
        };
        tools.push(entry);
    }
    if matched_external.iter().any(|matched| !matched) {
        return Err(CompositionError::ExternalToolNotAdmitted);
    }
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
    Ok(DesktopToolComposition {
        registry,
        tools,
        unavailable,
    })
}

pub(crate) fn external_unavailable(
    descriptor: &ExternalToolDescriptor,
    reason: UnavailableReason,
) -> UnavailableCapability {
    UnavailableCapability {
        public_tool_name: Some(descriptor.public_tool_name.clone()),
        source_kind: Some(descriptor.source_kind),
        state: UnavailableState::NotEffective,
        reason,
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
                source_label: "desktop_repository".to_owned(),
                effect_class: EffectClass::ReadOnly,
                authority_category: AuthorityCategory::RepositoryObservation,
                permission: PermissionLevel::Read,
                repository_bound: true,
                advertised: true,
            }],
            unavailable_capabilities: vec![UnavailableCapability {
                public_tool_name: Some("repo.commit".to_owned()),
                source_kind: Some(SourceKind::RepositoryHost),
                state: UnavailableState::ConfiguredUnavailable,
                reason: UnavailableReason::AuthorityNotGranted,
            }],
            reviewed_commit: ReviewedCommitState::ReviewRequired,
        };
        let json = serde_json::to_string(&snapshot).expect("snapshot serializes");
        let repeated_json = serde_json::to_string(&snapshot).expect("snapshot reserializes");
        assert_eq!(json, repeated_json);
        let object =
            serde_json::from_str::<serde_json::Value>(&json).expect("snapshot is an object");
        for field in [
            "schemaVersion",
            "status",
            "repository",
            "connection",
            "configured",
            "effectiveTools",
            "unavailableCapabilities",
            "reviewedCommit",
        ] {
            assert!(
                object.get(field).is_some(),
                "missing snapshot field: {field}"
            );
        }
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
    fn every_desktop_tool_name_has_explicit_host_classification() {
        let names = [
            "echo",
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
        ];
        for name in names {
            assert!(
                metadata(name).is_some(),
                "missing host classification: {name}"
            );
        }
    }

    #[test]
    fn unknown_tool_names_fail_closed_in_composition() {
        let mut registry = ToolRegistry::new();
        registry
            .register(Arc::new(rah_tools::EchoTool::new()))
            .expect("echo registers");
        assert!(compose(Arc::new(registry), None, false, &[]).is_ok());
    }

    #[test]
    fn known_tool_names_compose_with_explicit_host_classification() {
        let mut registry = ToolRegistry::new();
        registry
            .register(Arc::new(rah_tools::EchoTool::new()))
            .expect("echo registers");
        registry
            .register(Arc::new(UnknownTool))
            .expect("unknown test tool registers");
        assert!(compose(Arc::new(registry), None, false, &[]).is_err());
    }

    #[test]
    fn external_descriptors_use_host_permission_and_sanitized_labels() {
        let descriptor = ExternalToolDescriptor {
            public_tool_name: "mcp.provider.echo".to_owned(),
            source_kind: SourceKind::Mcp,
            source_label: "C:\\private\\provider".to_owned(),
            permission: PermissionLevel::Write,
            effect_class: EffectClass::External,
            authority_category: AuthorityCategory::External,
            repository_bound: false,
        };
        let mut registry = ToolRegistry::new();
        registry
            .register(Arc::new(ExternalTestTool {
                name: "mcp.provider.echo",
                permission: PermissionLevel::Write,
            }))
            .expect("external test tool registers");
        let composition = compose(Arc::new(registry), None, false, &[descriptor])
            .expect("explicit external classification should compose");
        let tool = &composition.tools[0];
        assert_eq!(tool.source_kind, SourceKind::Mcp);
        assert_eq!(tool.source_label, FALLBACK_PROVIDER_LABEL);
        assert_eq!(tool.effect_class, EffectClass::External);
        assert_eq!(tool.authority_category, AuthorityCategory::External);
        assert_eq!(tool.permission, PermissionLevel::Write);
        assert!(!tool.repository_bound);
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

    struct UnknownTool;

    #[async_trait::async_trait]
    impl rah_tools::Tool for UnknownTool {
        fn definition(&self) -> rah_protocol::ToolDefinition {
            rah_protocol::ToolDefinition {
                name: rah_protocol::ToolName::new("unknown.tool"),
                description: "unknown".to_owned(),
                input_schema: serde_json::json!({"type": "object"}),
                permission: PermissionLevel::None,
            }
        }

        async fn execute(
            &self,
            _: rah_protocol::ToolInput,
            _: rah_tools::ToolContext,
        ) -> Result<rah_protocol::ToolOutput, rah_tools::ToolError> {
            Ok(rah_protocol::ToolOutput {
                content: vec![],
                is_error: false,
            })
        }
    }

    struct ExternalTestTool {
        name: &'static str,
        permission: PermissionLevel,
    }

    #[async_trait::async_trait]
    impl rah_tools::Tool for ExternalTestTool {
        fn definition(&self) -> rah_protocol::ToolDefinition {
            rah_protocol::ToolDefinition {
                name: rah_protocol::ToolName::new(self.name),
                description: "provider-authored path text must be ignored".to_owned(),
                input_schema: serde_json::json!({"type": "object"}),
                permission: self.permission,
            }
        }

        async fn execute(
            &self,
            _: rah_protocol::ToolInput,
            _: rah_tools::ToolContext,
        ) -> Result<rah_protocol::ToolOutput, rah_tools::ToolError> {
            Ok(rah_protocol::ToolOutput {
                content: vec![],
                is_error: false,
            })
        }
    }
}
