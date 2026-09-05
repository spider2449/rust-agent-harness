#![cfg(target_os = "windows")]

use std::sync::Arc;

use rah_profile_composition::EffectiveProfileComposition;
use rah_protocol::PermissionLevel;
use rah_tools::{ToolError, ToolRegistry, TrustedStaticProfile};

use crate::effective_authority::{ExternalToolDescriptor, external_tool_descriptors};
use crate::trusted_profile_selection::{DesktopTrustedProfileSelection, ProfileSelectionError};

/// Desktop-owned activation wrapper around the shared effective-profile composer.
///
/// This is deliberately not a generic provider manager. One value belongs to one
/// explicit Desktop Connect attempt and owns every admitted provider until that
/// connection is torn down.
pub(crate) struct DesktopProviderActivation {
    composition: EffectiveProfileComposition,
    permissions: Vec<PermissionLevel>,
    external_tools: Vec<ExternalToolDescriptor>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProviderActivationError {
    Profile(ProfileSelectionError),
    ProviderUnavailable,
}

impl DesktopProviderActivation {
    /// Fresh-loads the exact selected source, re-applies the Desktop provider-only
    /// rule, and only then launches/admit providers through the shared composer.
    pub(crate) async fn activate(
        selection: &DesktopTrustedProfileSelection,
    ) -> Result<Self, ProviderActivationError> {
        let profile = selection
            .load_for_activation()
            .map_err(ProviderActivationError::Profile)?;
        let permissions =
            configured_external_permissions(&profile).map_err(ProviderActivationError::Profile)?;
        let external_tools = external_tool_descriptors(&profile)
            .map_err(|_| ProviderActivationError::Profile(ProfileSelectionError::InvalidProfile))?;
        let composition = rah_profile_composition::compose(profile)
            .await
            .map_err(|_| ProviderActivationError::ProviderUnavailable)?;
        Ok(Self {
            composition,
            permissions,
            external_tools,
        })
    }

    #[must_use]
    pub(crate) fn registry(&self) -> &ToolRegistry {
        self.composition.registry()
    }

    #[must_use]
    pub(crate) fn permissions(&self) -> &[PermissionLevel] {
        &self.permissions
    }

    #[must_use]
    pub(crate) fn external_tools(&self) -> &[ExternalToolDescriptor] {
        &self.external_tools
    }

    pub(crate) async fn shutdown(self) {
        self.composition.shutdown().await;
    }
}

fn configured_external_permissions(
    profile: &TrustedStaticProfile,
) -> Result<Vec<PermissionLevel>, ProfileSelectionError> {
    let mut permissions = Vec::new();
    for permission in profile
        .mcp_providers()
        .iter()
        .flat_map(|provider| provider.tools())
        .map(|tool| tool.permission())
        .chain(
            profile
                .process_plugins()
                .iter()
                .flat_map(|provider| provider.tools())
                .map(|tool| tool.permission()),
        )
    {
        let permission = permission.map_err(|_| ProfileSelectionError::InvalidProfile)?;
        if !permissions.contains(&permission) {
            permissions.push(permission);
        }
    }
    Ok(permissions)
}

/// Builds one fresh final registry. Duplicate public names fail closed before
/// the registry is published to Codex.
pub(crate) fn merge_tool_registries(
    first_party: &ToolRegistry,
    external: Option<&ToolRegistry>,
) -> Result<Arc<ToolRegistry>, ToolError> {
    let mut merged = ToolRegistry::new();
    for registry in std::iter::once(first_party).chain(external) {
        for definition in registry.definitions() {
            let tool = registry
                .get(&definition.name)
                .ok_or_else(|| ToolError::UnknownTool {
                    name: definition.name.clone(),
                })?;
            merged.register(tool)?;
        }
    }
    Ok(Arc::new(merged))
}

/// Permission levels are host-owned. External levels come only from the exact
/// Trusted Profile admission records, never from provider-authored metadata.
pub(crate) fn desktop_allowed_permissions(
    repository_selected: bool,
    external: &[PermissionLevel],
) -> Vec<PermissionLevel> {
    let mut allowed = if repository_selected {
        vec![
            PermissionLevel::None,
            PermissionLevel::Read,
            PermissionLevel::Execute,
        ]
    } else {
        vec![PermissionLevel::None]
    };
    for permission in external {
        if !allowed.contains(permission) {
            allowed.push(*permission);
        }
    }
    allowed
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        sync::{
            Arc,
            atomic::{AtomicU64, Ordering},
        },
    };

    use rah_protocol::{PermissionLevel, ToolCall, ToolCallId, ToolContent, ToolInput, ToolName};
    use rah_tools::{EchoTool, ToolContext, ToolError, ToolRegistry};
    use serde_json::json;

    use super::{DesktopProviderActivation, desktop_allowed_permissions, merge_tool_registries};
    use crate::trusted_profile_selection::load_provider_only_profile;

    static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(1);

    struct FixtureDirectory(PathBuf);

    impl FixtureDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "rah-task205-provider-composition-{}-{}",
                std::process::id(),
                NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).expect("fixture directory should be created");
            Self(path)
        }
    }

    impl Drop for FixtureDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn fixture_program(name: &str) -> PathBuf {
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let workspace = manifest
            .parent()
            .and_then(Path::parent)
            .expect("Desktop crate should be nested below workspace");
        let target = std::env::var_os("RAH_TEST_TARGET_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| workspace.join("target"));
        let path = target
            .join("debug")
            .join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
        assert!(
            path.is_file(),
            "provider fixture must be built before the focused Desktop test: {}",
            path.display()
        );
        path
    }

    fn copy_provider(directory: &Path, binary: &str, name: &str) -> PathBuf {
        let path = directory.join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
        fs::copy(fixture_program(binary), &path).expect("provider fixture should be copied");
        path
    }

    fn call(name: &str, input: serde_json::Value) -> ToolCall {
        ToolCall {
            id: ToolCallId::new(),
            name: ToolName::new(name),
            input: ToolInput(input),
        }
    }

    #[test]
    fn duplicate_public_tool_name_fails_closed_during_final_merge() {
        let mut first_party = ToolRegistry::new();
        first_party
            .register(std::sync::Arc::new(EchoTool::new()))
            .expect("first-party echo should register");
        let mut external = ToolRegistry::new();
        external
            .register(std::sync::Arc::new(EchoTool::new()))
            .expect("isolated external fixture should register");
        let error = match merge_tool_registries(&first_party, Some(&external)) {
            Ok(_) => panic!("duplicate public name must reject the whole final registry"),
            Err(error) => error,
        };
        assert!(matches!(error, ToolError::DuplicateTool { .. }));
    }

    #[test]
    fn external_permissions_extend_only_the_runtime_permission_set() {
        assert_eq!(
            desktop_allowed_permissions(
                false,
                &[
                    PermissionLevel::Write,
                    PermissionLevel::Read,
                    PermissionLevel::Write,
                ],
            ),
            [
                PermissionLevel::None,
                PermissionLevel::Write,
                PermissionLevel::Read,
            ]
        );
        assert_eq!(
            desktop_allowed_permissions(true, &[PermissionLevel::Write]),
            [
                PermissionLevel::None,
                PermissionLevel::Read,
                PermissionLevel::Execute,
                PermissionLevel::Write,
            ]
        );
    }

    #[tokio::test]
    async fn mixed_external_providers_are_fresh_composed_merged_usable_and_reaped() {
        let fixture = FixtureDirectory::new();
        let mcp = copy_provider(&fixture.0, "rah-mcp-echo-server", "mcp");
        let plugin = copy_provider(&fixture.0, "rah-plugin-echo", "plugin");
        let profile_path = fixture.0.join("trusted-profile.json");
        let document = json!({
            "profile_version": 1,
            "profile_id": "desktop-task205",
            "resources": {
                "executables": {
                    "mcp": {"path": mcp, "kind": "native"},
                    "plugin": {"path": plugin, "kind": "native"}
                },
                "repositories": {}
            },
            "capabilities": [],
            "mcp_providers": [{
                "id": "mcp-a",
                "executable": "mcp",
                "tools": [{
                    "remote_name": "echo",
                    "permission": "Read",
                    "input_schema": {
                        "type": "object",
                        "properties": {"text": {"type": "string"}},
                        "required": ["text"],
                        "additionalProperties": false
                    }
                }]
            }],
            "process_plugins": [{
                "id": "plugin-b",
                "executable": "plugin",
                "tools": [{
                    "remote_name": "echo",
                    "permission": "Execute",
                    "input_schema": {
                        "type": "object",
                        "properties": {"value": {}},
                        "required": ["value"],
                        "additionalProperties": false
                    }
                }]
            }]
        });
        fs::write(
            &profile_path,
            serde_json::to_vec(&document).expect("profile should encode"),
        )
        .expect("profile should be written");
        let selection = load_provider_only_profile(profile_path)
            .expect("provider-only selection should remain inert and valid");
        let activation = DesktopProviderActivation::activate(&selection)
            .await
            .expect("Connect-time activation should admit both providers");
        assert_eq!(
            activation.permissions(),
            [PermissionLevel::Read, PermissionLevel::Execute]
        );
        let external_descriptors = activation.external_tools().to_vec();

        let mut first_party = ToolRegistry::new();
        first_party
            .register(std::sync::Arc::new(EchoTool::new()))
            .expect("first-party echo should register");
        let merged = merge_tool_registries(&first_party, Some(activation.registry()))
            .expect("disjoint provider tools should merge");
        let names = merged
            .definitions()
            .into_iter()
            .map(|definition| definition.name.to_string())
            .collect::<Vec<_>>();
        assert_eq!(names, ["echo", "mcp.mcp-a.echo", "plugin.plugin-b.echo"]);
        let authority = crate::effective_authority::compose(
            Arc::clone(&merged),
            None,
            false,
            &external_descriptors,
        )
        .expect("all admitted external Tools must be classified");
        let mcp_authority = authority
            .tools
            .iter()
            .find(|tool| tool.public_tool_name == "mcp.mcp-a.echo")
            .expect("MCP authority entry");
        assert_eq!(
            mcp_authority.source_kind,
            crate::effective_authority::SourceKind::Mcp
        );
        assert_eq!(mcp_authority.source_label, "mcp-a");
        assert_eq!(
            mcp_authority.effect_class,
            crate::effective_authority::EffectClass::External
        );
        assert_eq!(
            mcp_authority.authority_category,
            crate::effective_authority::AuthorityCategory::External
        );
        assert_eq!(mcp_authority.permission, PermissionLevel::Read);
        assert!(!mcp_authority.repository_bound);
        let plugin_authority = authority
            .tools
            .iter()
            .find(|tool| tool.public_tool_name == "plugin.plugin-b.echo")
            .expect("Process Plugin authority entry");
        assert_eq!(
            plugin_authority.source_kind,
            crate::effective_authority::SourceKind::ProcessPlugin
        );
        assert_eq!(plugin_authority.source_label, "plugin-b");
        assert_eq!(
            plugin_authority.effect_class,
            crate::effective_authority::EffectClass::External
        );
        assert_eq!(
            plugin_authority.authority_category,
            crate::effective_authority::AuthorityCategory::External
        );
        assert_eq!(plugin_authority.permission, PermissionLevel::Execute);
        assert!(!plugin_authority.repository_bound);
        let mcp_output = merged
            .execute(
                call("mcp.mcp-a.echo", json!({"text": "mcp-ok"})),
                ToolContext::default(),
            )
            .await
            .expect("MCP proxy should dispatch while owner is alive");
        assert_eq!(mcp_output.content, [ToolContent::Text("mcp-ok".to_owned())]);
        let plugin_output = merged
            .execute(
                call("plugin.plugin-b.echo", json!({"value": "plugin-ok"})),
                ToolContext::default(),
            )
            .await
            .expect("plugin proxy should dispatch while owner is alive");
        assert_eq!(
            plugin_output.content,
            [ToolContent::Text("plugin-ok".to_owned())]
        );

        drop(merged);
        activation.shutdown().await;
        for executable in [mcp, plugin] {
            let moved = executable.with_extension("released");
            fs::rename(&executable, &moved)
                .expect("provider executable should be unlocked after Desktop owner shutdown");
        }
    }

    fn plugin_profile(executable: &Path, provider_id: &str) -> serde_json::Value {
        json!({
            "profile_version": 1,
            "profile_id": format!("profile-{provider_id}"),
            "resources": {
                "executables": {
                    "plugin": {"path": executable, "kind": "native"}
                },
                "repositories": {}
            },
            "capabilities": [],
            "mcp_providers": [],
            "process_plugins": [{
                "id": provider_id,
                "executable": "plugin",
                "tools": [{
                    "remote_name": "echo",
                    "permission": "Execute",
                    "input_schema": {
                        "type": "object",
                        "properties": {"value": {}},
                        "required": ["value"],
                        "additionalProperties": false
                    }
                }]
            }]
        })
    }

    #[tokio::test]
    async fn activation_rereads_current_source_and_keeps_admitted_snapshot_stable() {
        let fixture = FixtureDirectory::new();
        let plugin = copy_provider(&fixture.0, "rah-plugin-echo", "plugin-fresh");
        let profile_path = fixture.0.join("trusted-profile.json");
        fs::write(
            &profile_path,
            serde_json::to_vec(&plugin_profile(&plugin, "plugin-a")).unwrap(),
        )
        .unwrap();
        let selection = load_provider_only_profile(profile_path.clone()).unwrap();

        // Restore/Choose selected A, but Connect must admit the current B source.
        fs::write(
            &profile_path,
            serde_json::to_vec(&plugin_profile(&plugin, "plugin-b")).unwrap(),
        )
        .unwrap();
        let activation = DesktopProviderActivation::activate(&selection)
            .await
            .expect("Connect must load the current source");
        assert_eq!(
            activation.external_tools()[0].public_tool_name,
            "plugin.plugin-b.echo"
        );

        // A post-Connect edit cannot hot reload the admitted composition.
        fs::write(
            &profile_path,
            serde_json::to_vec(&plugin_profile(&plugin, "plugin-c")).unwrap(),
        )
        .unwrap();
        assert_eq!(
            activation.external_tools()[0].public_tool_name,
            "plugin.plugin-b.echo"
        );
        activation.shutdown().await;

        // An explicit later activation fresh-loads C and owns a new provider.
        let fresh = DesktopProviderActivation::activate(&selection)
            .await
            .expect("reconnect must load the changed source");
        assert_eq!(
            fresh.external_tools()[0].public_tool_name,
            "plugin.plugin-c.echo"
        );
        fresh.shutdown().await;
        let released = plugin.with_extension("released");
        fs::rename(&plugin, &released)
            .expect("provider executable should be unlocked after shutdown");
    }

    #[tokio::test]
    async fn changed_source_invalid_or_missing_fails_before_provider_publication() {
        let fixture = FixtureDirectory::new();
        let plugin = copy_provider(&fixture.0, "rah-plugin-echo", "plugin-failure");
        let profile_path = fixture.0.join("trusted-profile.json");
        fs::write(
            &profile_path,
            serde_json::to_vec(&plugin_profile(&plugin, "plugin-failure")).unwrap(),
        )
        .unwrap();
        let selection = load_provider_only_profile(profile_path.clone()).unwrap();

        fs::write(&profile_path, b"{").unwrap();
        assert!(matches!(
            DesktopProviderActivation::activate(&selection).await,
            Err(super::ProviderActivationError::Profile(_))
        ));
        let after_invalid = plugin.with_extension("after-invalid");
        fs::rename(&plugin, &after_invalid)
            .expect("invalid activation must not retain a provider child");

        let plugin = copy_provider(&fixture.0, "rah-plugin-echo", "plugin-missing");
        let missing_profile = fixture.0.join("missing-profile.json");
        fs::write(
            &missing_profile,
            serde_json::to_vec(&plugin_profile(&plugin, "plugin-missing")).unwrap(),
        )
        .unwrap();
        let missing_selection = load_provider_only_profile(missing_profile.clone()).unwrap();
        fs::remove_file(&missing_profile).unwrap();
        assert!(matches!(
            DesktopProviderActivation::activate(&missing_selection).await,
            Err(super::ProviderActivationError::Profile(_))
        ));
        let after_missing = plugin.with_extension("after-missing");
        fs::rename(&plugin, &after_missing)
            .expect("missing activation must not retain a provider child");
    }
}
