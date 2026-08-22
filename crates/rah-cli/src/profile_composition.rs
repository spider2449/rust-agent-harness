//! Host-owned effective composition of already hardened external providers.

use std::sync::Arc;

use rah_tools::{
    EffectiveCapability, EffectiveProfile, ProfileError, ToolRegistry, TrustedStaticProfile,
};
use rah_tools_mcp::{McpAdapter, McpServerConfig};
use rah_tools_plugin::{PluginAdapter, PluginConfig};

/// Owns every provider whose immutable tools are registered in the profile.
pub struct EffectiveProfileComposition {
    registry: Arc<ToolRegistry>,
    effective: EffectiveProfile,
    mcp_adapters: Vec<McpAdapter>,
    plugin_adapters: Vec<PluginAdapter>,
}

impl EffectiveProfileComposition {
    #[must_use]
    pub fn registry(&self) -> &ToolRegistry {
        self.registry.as_ref()
    }

    /// Returns the fresh registry while this composition retains provider ownership.
    #[must_use]
    pub fn registry_handle(&self) -> Arc<ToolRegistry> {
        Arc::clone(&self.registry)
    }

    #[must_use]
    pub fn effective_profile(&self) -> &EffectiveProfile {
        &self.effective
    }

    /// Shuts down every owned provider after the registry is no longer used.
    pub async fn shutdown(self) {
        for adapter in self.plugin_adapters.into_iter().rev() {
            let _ = adapter.shutdown().await;
        }
        for adapter in self.mcp_adapters.into_iter().rev() {
            let _ = adapter.shutdown().await;
        }
    }
}

/// Explicitly launches every configured provider. Nothing is published unless all admit.
pub async fn compose(
    profile: TrustedStaticProfile,
) -> Result<EffectiveProfileComposition, ProfileError> {
    let mut registry = ToolRegistry::new();
    for definition in profile.registry().definitions() {
        let tool = profile
            .registry()
            .get(&definition.name)
            .ok_or(ProfileError::ConstructionFailed)?;
        registry
            .register(tool)
            .map_err(|_| ProfileError::DuplicateRegistration)?;
    }

    let mut mcp_adapters = Vec::new();
    let mut plugin_adapters = Vec::new();
    for provider in profile.mcp_providers() {
        let executable = profile
            .executable_resource(provider.executable())
            .map_err(|_| ProfileError::ExternalProviderFailed)?;
        let mut config = McpServerConfig::stdio(provider.id(), executable)
            .map_err(|_| ProfileError::ExternalProviderFailed)?;
        for tool in provider.tools() {
            config = config
                .with_expected_tool(
                    tool.remote_name(),
                    tool.input_schema().clone(),
                    tool.permission()
                        .map_err(|_| ProfileError::ExternalProviderFailed)?,
                )
                .map_err(|_| ProfileError::ExternalProviderFailed)?;
        }
        let adapter = match McpAdapter::connect(config).await {
            Ok(adapter) => adapter,
            Err(_) => {
                return fail(
                    mcp_adapters,
                    plugin_adapters,
                    ProfileError::ExternalProviderFailed,
                )
                .await;
            }
        };
        if register_tools(&mut registry, adapter.tools()).is_err() {
            let _ = adapter.shutdown().await;
            return fail(
                mcp_adapters,
                plugin_adapters,
                ProfileError::DuplicateRegistration,
            )
            .await;
        }
        mcp_adapters.push(adapter);
    }
    for provider in profile.process_plugins() {
        let executable = profile
            .executable_resource(provider.executable())
            .map_err(|_| ProfileError::ExternalProviderFailed)?;
        // The current prototype has a fixed provider version and no profile argv.
        let mut config = PluginConfig::stdio(provider.id(), "0.1.0", executable)
            .map_err(|_| ProfileError::ExternalProviderFailed)?;
        for tool in provider.tools() {
            config = config
                .with_expected_tool(
                    tool.remote_name(),
                    tool.input_schema().clone(),
                    tool.permission()
                        .map_err(|_| ProfileError::ExternalProviderFailed)?,
                )
                .map_err(|_| ProfileError::ExternalProviderFailed)?;
        }
        let adapter = match PluginAdapter::connect(config).await {
            Ok(adapter) => adapter,
            Err(_) => {
                return fail(
                    mcp_adapters,
                    plugin_adapters,
                    ProfileError::ExternalProviderFailed,
                )
                .await;
            }
        };
        if register_tools(&mut registry, adapter.tools()).is_err() {
            let _ = adapter.shutdown().await;
            return fail(
                mcp_adapters,
                plugin_adapters,
                ProfileError::DuplicateRegistration,
            )
            .await;
        }
        plugin_adapters.push(adapter);
    }

    let mut effective = profile.effective_profile().clone();
    for provider in &mut effective.providers {
        provider.status = "validated";
    }
    for definition in registry.definitions() {
        if let Some(provider_id) = provider_id(definition.name.as_str()) {
            effective.capabilities.push(EffectiveCapability {
                capability_id: definition.name.to_string(),
                enabled: true,
                registered: true,
                permission: definition.permission,
                resources: vec![provider_id.to_owned()],
                validation: "validated",
            });
        }
    }
    Ok(EffectiveProfileComposition {
        registry: Arc::new(registry),
        effective,
        mcp_adapters,
        plugin_adapters,
    })
}

fn provider_id(name: &str) -> Option<&str> {
    name.strip_prefix("mcp.")
        .or_else(|| name.strip_prefix("plugin."))?
        .split('.')
        .next()
}

fn register_tools(
    registry: &mut ToolRegistry,
    tools: Vec<std::sync::Arc<dyn rah_tools::Tool>>,
) -> Result<(), ()> {
    for tool in tools {
        registry.register(tool).map_err(|_| ())?;
    }
    Ok(())
}

async fn fail(
    mcp_adapters: Vec<McpAdapter>,
    plugin_adapters: Vec<PluginAdapter>,
    error: ProfileError,
) -> Result<EffectiveProfileComposition, ProfileError> {
    for adapter in plugin_adapters.into_iter().rev() {
        let _ = adapter.shutdown().await;
    }
    for adapter in mcp_adapters.into_iter().rev() {
        let _ = adapter.shutdown().await;
    }
    Err(error)
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        sync::atomic::{AtomicU64, Ordering},
        time::Duration,
    };

    use rah_protocol::{PermissionLevel, ToolCall, ToolCallId, ToolContent, ToolInput, ToolName};
    use rah_tools::{ToolContext, TrustedStaticProfile};
    use serde_json::json;

    use super::compose;

    static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(1);

    struct FixtureDirectory(PathBuf);

    impl FixtureDirectory {
        fn new(label: &str) -> Self {
            let directory = std::env::temp_dir().join(format!(
                "rah-task041-{label}-{}-{}",
                std::process::id(),
                NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&directory).expect("fixture directory should be created");
            Self(directory)
        }
    }

    impl Drop for FixtureDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    struct LifecycleFixture {
        executable: PathBuf,
        audit: PathBuf,
    }

    impl LifecycleFixture {
        fn new(directory: &Path, binary: &str, label: &str) -> Self {
            let extension = std::env::consts::EXE_SUFFIX;
            let executable = directory.join(format!("{label}{extension}"));
            fs::copy(fixture_program(binary), &executable)
                .expect("fixture executable should be copied into the test directory");
            let request = executable.with_extension("lifecycle-request");
            fs::write(&request, b"observe")
                .expect("fixture lifecycle observation should be explicitly enabled");
            Self {
                audit: request.with_extension("lifecycle"),
                executable,
            }
        }

        async fn assert_events(&self, expected: &[&str]) {
            for _ in 0..50 {
                let audit = fs::read_to_string(&self.audit).unwrap_or_default();
                let events = audit.lines().collect::<Vec<_>>();
                if events == expected {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            panic!(
                "fixture lifecycle did not reach {:?}: {:?}",
                expected,
                fs::read_to_string(&self.audit).unwrap_or_default()
            );
        }

        async fn assert_unlocked_after_owner_release(&self) {
            let moved = self.executable.with_extension("released");
            for _ in 0..50 {
                if fs::rename(&self.executable, &moved).is_ok() {
                    fs::rename(&moved, &self.executable)
                        .expect("fixture executable should be restored after liveness check");
                    return;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            panic!("fixture executable remained locked after provider owner release");
        }
    }

    fn fixture_program(name: &str) -> PathBuf {
        let manifest_directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let workspace = manifest_directory
            .parent()
            .and_then(std::path::Path::parent)
            .expect("CLI crate should be nested below workspace");
        let target = std::env::var_os("RAH_TEST_TARGET_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| workspace.join("target"));
        let path = target
            .join("debug")
            .join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
        assert!(
            path.is_file(),
            "fixture must be built before this focused composition test: {}",
            path.display()
        );
        path
    }

    fn echo_schema(field: &str) -> serde_json::Value {
        let mut properties = serde_json::Map::new();
        properties.insert(field.to_owned(), json!({"type":"string"}));
        json!({"type":"object", "properties": properties, "required":[field], "additionalProperties":false})
    }

    fn plugin_echo_schema(field: &str) -> serde_json::Value {
        let mut properties = serde_json::Map::new();
        properties.insert(field.to_owned(), json!({}));
        json!({"type":"object", "properties": properties, "required":[field], "additionalProperties":false})
    }

    fn profile(
        directory: &Path,
        mcps: &[(&str, &LifecycleFixture, serde_json::Value, PermissionLevel)],
        plugins: &[(&str, &LifecycleFixture, serde_json::Value, PermissionLevel)],
    ) -> PathBuf {
        let path = directory.join("trusted-profile.json");
        let mut executables = serde_json::Map::new();
        let mut mcp_providers = Vec::new();
        let mut process_plugins = Vec::new();
        for (id, fixture, schema, permission) in mcps {
            let executable = format!("mcp-{id}");
            executables.insert(
                executable.clone(),
                json!({"path": fixture.executable, "kind":"native"}),
            );
            mcp_providers.push(json!({"id":id, "executable":executable, "tools":[{"remote_name":"echo", "permission":format!("{permission:?}"), "input_schema":schema}]}));
        }
        for (id, fixture, schema, permission) in plugins {
            let executable = format!("plugin-{id}");
            executables.insert(
                executable.clone(),
                json!({"path": fixture.executable, "kind":"native"}),
            );
            process_plugins.push(json!({"id":id, "executable":executable, "tools":[{"remote_name":"echo", "permission":format!("{permission:?}"), "input_schema":schema}]}));
        }
        let document = json!({
            "profile_version": 1,
            "profile_id": "task041-mixed",
            "resources": {
                "executables": executables,
                "repositories": {"workspace": {"path": directory}}
            },
            "capabilities": [{"name":"fs.read", "enabled":true, "permission":"read", "workspace":"workspace", "max_bytes":1024}],
            "mcp_providers": mcp_providers,
            "process_plugins": process_plugins
        });
        fs::write(
            &path,
            serde_json::to_vec(&document).expect("profile JSON should serialize"),
        )
        .expect("profile should be written");
        path
    }

    fn call(name: &str, input: serde_json::Value) -> ToolCall {
        ToolCall {
            id: ToolCallId::new(),
            name: ToolName::new(name),
            input: ToolInput(input),
        }
    }

    #[tokio::test]
    async fn actual_effective_composer_admits_mixed_providers_preserves_permissions_and_owns_lifetime()
     {
        let directory = FixtureDirectory::new("mixed-success");
        let mcp = LifecycleFixture::new(&directory.0, "rah-mcp-echo-server", "mcp");
        let plugin = LifecycleFixture::new(&directory.0, "rah-plugin-echo", "plugin");
        let profile_path = profile(
            &directory.0,
            &[("mcp-echo", &mcp, echo_schema("text"), PermissionLevel::Read)],
            &[(
                "plugin-echo",
                &plugin,
                plugin_echo_schema("value"),
                PermissionLevel::Execute,
            )],
        );
        let composition =
            compose(TrustedStaticProfile::load(&profile_path).expect("static profile should load"))
                .await
                .expect("mixed effective composition should succeed");

        mcp.assert_events(&["spawn"]).await;
        plugin.assert_events(&["spawn"]).await;

        let definitions = composition.registry().definitions();
        assert_eq!(
            definitions.len(),
            3,
            "only built-in plus exactly admitted external tools are published"
        );
        assert_eq!(
            definitions
                .iter()
                .find(|definition| definition.name == ToolName::new("mcp.mcp-echo.echo"))
                .expect("MCP tool should be registered")
                .permission,
            PermissionLevel::Read
        );
        assert_eq!(
            definitions
                .iter()
                .find(|definition| definition.name == ToolName::new("plugin.plugin-echo.echo"))
                .expect("Plugin tool should be registered")
                .permission,
            PermissionLevel::Execute
        );
        assert_eq!(
            composition
                .effective_profile()
                .providers
                .iter()
                .map(|provider| provider.status)
                .collect::<Vec<_>>(),
            ["validated", "validated"]
        );

        let mcp_output = composition
            .registry()
            .execute(
                call("mcp.mcp-echo.echo", json!({"text":"mcp-ok"})),
                ToolContext::default(),
            )
            .await
            .expect("MCP proxy should remain usable");
        assert_eq!(mcp_output.content, [ToolContent::Text("mcp-ok".to_owned())]);
        let plugin_output = composition
            .registry()
            .execute(
                call("plugin.plugin-echo.echo", json!({"value":"plugin-ok"})),
                ToolContext::default(),
            )
            .await
            .expect("plugin proxy should remain usable");
        assert_eq!(
            plugin_output.content,
            [ToolContent::Text("plugin-ok".to_owned())]
        );

        drop(composition);
        plugin.assert_unlocked_after_owner_release().await;
        mcp.assert_unlocked_after_owner_release().await;
    }

    #[tokio::test]
    async fn staged_mcp_and_failed_plugin_are_observably_reaped_without_publication() {
        let directory = FixtureDirectory::new("late-plugin-failure");
        let mcp = LifecycleFixture::new(&directory.0, "rah-mcp-echo-server", "mcp");
        let plugin = LifecycleFixture::new(&directory.0, "rah-plugin-echo", "plugin");
        let profile_path = profile(
            &directory.0,
            &[("mcp-a", &mcp, echo_schema("text"), PermissionLevel::None)],
            &[(
                "plugin-b",
                &plugin,
                plugin_echo_schema("wrong"),
                PermissionLevel::None,
            )],
        );
        let error = match compose(
            TrustedStaticProfile::load(&profile_path).expect("static profile should load"),
        )
        .await
        {
            Ok(composition) => {
                composition.shutdown().await;
                panic!("late plugin schema mismatch must fail the whole composition");
            }
            Err(error) => error,
        };
        assert_eq!(error, rah_tools::ProfileError::ExternalProviderFailed);
        // The success-only aggregate owns both registry and inventory, so an Err
        // cannot return a partial publication.
        mcp.assert_unlocked_after_owner_release().await;
        plugin.assert_unlocked_after_owner_release().await;
    }

    #[tokio::test]
    async fn multiple_staged_providers_and_same_provider_late_failures_are_reaped() {
        let directory = FixtureDirectory::new("multiple-late-failure");
        let mcp_a = LifecycleFixture::new(&directory.0, "rah-mcp-echo-server", "mcp-a");
        let plugin_a = LifecycleFixture::new(&directory.0, "rah-plugin-echo", "plugin-a");
        let plugin_b = LifecycleFixture::new(&directory.0, "rah-plugin-echo", "plugin-b");
        let profile_path = profile(
            &directory.0,
            &[("mcp-a", &mcp_a, echo_schema("text"), PermissionLevel::None)],
            &[
                (
                    "plugin-a",
                    &plugin_a,
                    plugin_echo_schema("value"),
                    PermissionLevel::None,
                ),
                (
                    "plugin-b",
                    &plugin_b,
                    plugin_echo_schema("wrong"),
                    PermissionLevel::None,
                ),
            ],
        );
        let late_plugin_error = match compose(
            TrustedStaticProfile::load(&profile_path).expect("static profile should load"),
        )
        .await
        {
            Ok(composition) => {
                composition.shutdown().await;
                panic!("late plugin B mismatch must fail atomically");
            }
            Err(error) => error,
        };
        assert_eq!(
            late_plugin_error,
            rah_tools::ProfileError::ExternalProviderFailed
        );
        for fixture in [&mcp_a, &plugin_a, &plugin_b] {
            fixture.assert_unlocked_after_owner_release().await;
        }

        let mcp_b = LifecycleFixture::new(&directory.0, "rah-mcp-echo-server", "mcp-b");
        let same_provider_path = profile(
            &directory.0,
            &[
                (
                    "mcp-first",
                    &mcp_a,
                    echo_schema("text"),
                    PermissionLevel::None,
                ),
                (
                    "mcp-second",
                    &mcp_b,
                    echo_schema("wrong"),
                    PermissionLevel::None,
                ),
            ],
            &[],
        );
        let same_kind_error = match compose(
            TrustedStaticProfile::load(&same_provider_path).expect("static profile should load"),
        )
        .await
        {
            Ok(composition) => {
                composition.shutdown().await;
                panic!("same-kind late MCP mismatch must fail atomically");
            }
            Err(error) => error,
        };
        assert_eq!(
            same_kind_error,
            rah_tools::ProfileError::ExternalProviderFailed
        );
        mcp_a.assert_unlocked_after_owner_release().await;
        mcp_b.assert_unlocked_after_owner_release().await;
    }
}
