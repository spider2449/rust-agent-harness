//! Host-owned effective composition of already hardened external providers.

use rah_tools::{
    EffectiveCapability, EffectiveProfile, ProfileError, ToolRegistry, TrustedStaticProfile,
};
use rah_tools_mcp::{McpAdapter, McpServerConfig};
use rah_tools_plugin::{PluginAdapter, PluginConfig};

/// Owns every provider whose immutable tools are registered in the profile.
pub struct EffectiveProfileComposition {
    registry: ToolRegistry,
    effective: EffectiveProfile,
    mcp_adapters: Vec<McpAdapter>,
    plugin_adapters: Vec<PluginAdapter>,
}

impl EffectiveProfileComposition {
    #[must_use]
    pub fn registry(&self) -> &ToolRegistry {
        &self.registry
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
        registry,
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
