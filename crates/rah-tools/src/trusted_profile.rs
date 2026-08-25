//! Trusted static host composition for a fixed set of existing tools.
//!
//! This module is intentionally host-facing. It does not expose profile data
//! through model-visible protocol types and it never creates generic process
//! authority.

use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    sync::Arc,
};

use rah_protocol::PermissionLevel;
use serde::Deserialize;
use serde_json::Value;
use thiserror::Error;

use crate::trusted_profile_source;
use crate::{CargoVersionTool, FsReadTool, GitStatusTool, ToolError, ToolRegistry};

const PROFILE_VERSION: u32 = 1;
const MAX_FS_READ_BYTES: u64 = 16 * 1024 * 1024;

/// A successfully constructed registry and its host-only redacted inventory.
pub struct TrustedStaticProfile {
    registry: ToolRegistry,
    effective: EffectiveProfile,
    repository_worktree_patches: Vec<RepositoryWorktreePatchProfile>,
    repository_file_creations: Vec<RepositoryFileCreationProfile>,
    repository_multi_file_edits: Vec<RepositoryMultiFileEditProfile>,
    repository_observers: Vec<RepositoryObserverProfile>,
    mcp_providers: Vec<McpProviderProfile>,
    process_plugins: Vec<ProcessPluginProfile>,
    resources: Resources,
}

impl TrustedStaticProfile {
    /// Loads one explicit, absolute, trusted static profile without search or reload.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, ProfileError> {
        let raw = trusted_profile_source::read(path.as_ref())?;
        let value = parse_unique_json(raw.as_bytes())?;
        let profile: ProfileDocument = serde_json::from_value(value)
            .map_err(|_| ProfileError::InvalidProfile { reason: "schema" })?;
        validate_identifier(&profile.profile_id).map_err(|_| ProfileError::InvalidProfile {
            reason: "profile_id",
        })?;
        if profile.profile_version != PROFILE_VERSION {
            return Err(ProfileError::UnsupportedVersion);
        }
        validate_resources(&profile.resources)?;
        validate_mcp_providers(&profile.mcp_providers, &profile.resources)?;
        validate_process_plugins(&profile.process_plugins, &profile.resources)?;
        let mut external_provider_ids = BTreeSet::new();
        for id in profile
            .mcp_providers
            .iter()
            .map(|provider| &provider.id)
            .chain(profile.process_plugins.iter().map(|provider| &provider.id))
        {
            if !external_provider_ids.insert(id) {
                return Err(ProfileError::InvalidProfile {
                    reason: "duplicate_external_provider",
                });
            }
        }

        let mut names = BTreeSet::new();
        for capability in &profile.capabilities {
            if !names.insert(&capability.name) {
                return Err(ProfileError::DuplicateCapability);
            }
        }

        let mut registry = ToolRegistry::new();
        let mut capabilities = Vec::with_capacity(profile.capabilities.len());
        let mut repository_worktree_patches = Vec::new();
        let mut repository_file_creations = Vec::new();
        let mut repository_multi_file_edits = Vec::new();
        let mut repository_observers = Vec::new();
        for capability in &profile.capabilities {
            let (
                inventory,
                repository_worktree_patch,
                repository_file_creation,
                repository_multi_file_edit,
                repository_observer,
            ) = build_capability(capability, &profile.resources, &mut registry)?;
            capabilities.push(inventory);
            if let Some(repository_worktree_patch) = repository_worktree_patch {
                repository_worktree_patches.push(repository_worktree_patch);
            }
            if let Some(repository_file_creation) = repository_file_creation {
                repository_file_creations.push(repository_file_creation);
            }
            if let Some(repository_multi_file_edit) = repository_multi_file_edit {
                repository_multi_file_edits.push(repository_multi_file_edit);
            }
            if let Some(repository_observer) = repository_observer {
                repository_observers.push(repository_observer);
            }
        }

        Ok(Self {
            registry,
            effective: EffectiveProfile {
                profile_version: PROFILE_VERSION,
                profile_id: profile.profile_id,
                source_class: "trusted_static_file",
                capabilities,
                providers: profile
                    .mcp_providers
                    .iter()
                    .map(|provider| EffectiveProvider {
                        kind: "mcp",
                        provider_id: provider.id.clone(),
                        status: "configured",
                        tool_count: provider.tools.len(),
                    })
                    .chain(
                        profile
                            .process_plugins
                            .iter()
                            .map(|provider| EffectiveProvider {
                                kind: "process_plugin",
                                provider_id: provider.id.clone(),
                                status: "configured",
                                tool_count: provider.tools.len(),
                            }),
                    )
                    .collect(),
            },
            repository_worktree_patches,
            repository_file_creations,
            repository_multi_file_edits,
            repository_observers,
            mcp_providers: profile.mcp_providers,
            process_plugins: profile.process_plugins,
            resources: profile.resources,
        })
    }

    /// Returns the newly constructed registry. No registry is returned on failure.
    #[must_use]
    pub fn registry(&self) -> &ToolRegistry {
        &self.registry
    }

    /// Returns a redacted host inventory, never raw paths or profile contents.
    #[must_use]
    pub fn effective_profile(&self) -> &EffectiveProfile {
        &self.effective
    }

    /// Splits the statically constructed registry from its redacted inventory.
    ///
    /// Capabilities that require effective host construction, such as
    /// `repo.patch`, remain unavailable until the host invokes its effective
    /// composer.
    #[must_use]
    pub fn into_parts(self) -> (ToolRegistry, EffectiveProfile) {
        (self.registry, self.effective)
    }

    /// Returns closed, host-only MCP declarations for explicit effective composition.
    #[must_use]
    pub fn mcp_providers(&self) -> &[McpProviderProfile] {
        &self.mcp_providers
    }

    /// Returns closed, host-only Process Plugin declarations for effective composition.
    #[must_use]
    pub fn process_plugins(&self) -> &[ProcessPluginProfile] {
        &self.process_plugins
    }

    /// Returns closed, host-only `repo.patch` declarations for effective composition.
    #[must_use]
    pub fn repository_worktree_patches(&self) -> &[RepositoryWorktreePatchProfile] {
        &self.repository_worktree_patches
    }

    /// Returns closed, host-only `repo.create-file` declarations for effective composition.
    #[must_use]
    pub fn repository_file_creations(&self) -> &[RepositoryFileCreationProfile] {
        &self.repository_file_creations
    }

    /// Returns closed, host-only `repo.edit-files` declarations for effective composition.
    #[must_use]
    pub fn repository_multi_file_edits(&self) -> &[RepositoryMultiFileEditProfile] {
        &self.repository_multi_file_edits
    }

    /// Returns closed, host-only repository observer declarations for effective composition.
    #[must_use]
    pub fn repository_observers(&self) -> &[RepositoryObserverProfile] {
        &self.repository_observers
    }

    /// Resolves a configured symbolic executable for an external provider.
    pub fn executable_resource(&self, id: &str) -> Result<&Path, ProfileError> {
        executable(&self.resources, Some(id))
    }

    /// Resolves a configured symbolic repository resource for effective composition.
    pub fn repository_resource(&self, id: &str) -> Result<&Path, ProfileError> {
        directory(&self.resources, Some(id))
    }
}

/// Redacted host-side description of the successfully effective profile.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EffectiveProfile {
    /// Exact schema version accepted by the loader.
    pub profile_version: u32,
    /// Host-selected profile identifier.
    pub profile_id: String,
    /// Source category, never the source path.
    pub source_class: &'static str,
    /// Effective states of all declared capabilities.
    pub capabilities: Vec<EffectiveCapability>,
    /// Configured provider identities only; static validation never launches them.
    pub providers: Vec<EffectiveProvider>,
}

/// Redacted provider state for trusted-host inspection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EffectiveProvider {
    pub kind: &'static str,
    pub provider_id: String,
    pub status: &'static str,
    pub tool_count: usize,
}

/// Redacted state for one declared capability.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EffectiveCapability {
    /// Stable known capability identifier.
    pub capability_id: String,
    /// Whether the declaration was enabled.
    pub enabled: bool,
    /// Whether the tool was registered into the fresh registry.
    pub registered: bool,
    /// Fixed RAH permission, not a provider-supplied value.
    pub permission: PermissionLevel,
    /// Symbolic resource identifiers only.
    pub resources: Vec<String>,
    /// Bounded validation state for ordinary operator inspection.
    pub validation: &'static str,
}

/// Bounded, redacted static-profile load failure.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ProfileError {
    #[error("trusted profile source must be an absolute path")]
    InvalidSource,
    #[error("trusted profile source could not be read")]
    UnreadableSource,
    #[error("trusted profile source must be a regular file")]
    NonRegularSource,
    #[error("trusted profile source must not traverse a link or reparse point")]
    LinkedSource,
    #[error("trusted profile source form is unsupported")]
    UnsupportedSource,
    #[error("trusted profile source changed while it was being opened")]
    SourceChanged,
    #[error("trusted profile source exceeds the maximum size")]
    SourceTooLarge,
    #[error("trusted profile source must be valid UTF-8")]
    InvalidEncoding,
    #[error("trusted profile is not valid JSON")]
    InvalidJson,
    #[error("trusted profile contains a duplicate JSON key")]
    DuplicateKey,
    #[error("trusted profile is invalid: {reason}")]
    InvalidProfile { reason: &'static str },
    #[error("trusted profile version is unsupported")]
    UnsupportedVersion,
    #[error("trusted profile declares a capability more than once")]
    DuplicateCapability,
    #[error("trusted profile capability is unsupported")]
    UnsupportedCapability,
    #[error("trusted profile capability permission is invalid")]
    InvalidPermission,
    #[error("trusted profile references an unavailable symbolic resource")]
    UnavailableResource,
    #[error("trusted profile capability could not be constructed")]
    ConstructionFailed,
    #[error("trusted profile contains a duplicate registered tool")]
    DuplicateRegistration,
    #[error("trusted profile external provider could not be composed")]
    ExternalProviderFailed,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProfileDocument {
    profile_version: u32,
    profile_id: String,
    resources: Resources,
    capabilities: Vec<Capability>,
    #[serde(default)]
    mcp_providers: Vec<McpProviderProfile>,
    #[serde(default)]
    process_plugins: Vec<ProcessPluginProfile>,
}

/// Closed host-only declaration for one local MCP provider.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct McpProviderProfile {
    id: String,
    executable: String,
    tools: Vec<McpExpectedToolProfile>,
}

impl McpProviderProfile {
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }
    #[must_use]
    pub fn executable(&self) -> &str {
        &self.executable
    }
    #[must_use]
    pub fn tools(&self) -> &[McpExpectedToolProfile] {
        &self.tools
    }
}

/// Exact host admission record for one remote MCP tool.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct McpExpectedToolProfile {
    remote_name: String,
    permission: String,
    input_schema: Value,
}

impl McpExpectedToolProfile {
    #[must_use]
    pub fn remote_name(&self) -> &str {
        &self.remote_name
    }
    #[must_use]
    pub fn input_schema(&self) -> &Value {
        &self.input_schema
    }
    pub fn permission(&self) -> Result<PermissionLevel, ProfileError> {
        parse_external_permission(&self.permission).ok_or(ProfileError::InvalidPermission)
    }
}

/// Closed host-only declaration for one local Process Plugin provider.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProcessPluginProfile {
    id: String,
    executable: String,
    tools: Vec<ProcessPluginExpectedToolProfile>,
}

impl ProcessPluginProfile {
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }
    #[must_use]
    pub fn executable(&self) -> &str {
        &self.executable
    }
    #[must_use]
    pub fn tools(&self) -> &[ProcessPluginExpectedToolProfile] {
        &self.tools
    }
}

/// Exact host admission record for one remote Process Plugin tool.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProcessPluginExpectedToolProfile {
    remote_name: String,
    permission: String,
    input_schema: Value,
}

impl ProcessPluginExpectedToolProfile {
    #[must_use]
    pub fn remote_name(&self) -> &str {
        &self.remote_name
    }
    #[must_use]
    pub fn input_schema(&self) -> &Value {
        &self.input_schema
    }
    pub fn permission(&self) -> Result<PermissionLevel, ProfileError> {
        parse_external_permission(&self.permission).ok_or(ProfileError::InvalidPermission)
    }
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Resources {
    #[serde(default)]
    executables: BTreeMap<String, ExecutableResource>,
    #[serde(default)]
    repositories: BTreeMap<String, DirectoryResource>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExecutableResource {
    path: PathBuf,
    kind: NativeKind,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum NativeKind {
    Native,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DirectoryResource {
    path: PathBuf,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Capability {
    name: String,
    enabled: bool,
    permission: String,
    #[serde(default)]
    workspace: Option<String>,
    #[serde(default)]
    max_bytes: Option<u64>,
    #[serde(default)]
    executable: Option<String>,
    #[serde(default)]
    cwd_resource: Option<String>,
    #[serde(default)]
    repository: Option<String>,
}

/// Closed host-only binding needed to construct the bounded `repo.patch` tool.
///
/// This contains symbolic resource identities only. The private
/// `RepositoryWorktreeMutationPolicy` remains constructed by the host-side
/// effective composer after resource resolution.
#[derive(Clone, Debug)]
pub struct RepositoryWorktreePatchProfile {
    executable: String,
    repository: String,
}

/// Closed host-only binding needed to construct the bounded `repo.create-file` tool.
///
/// This contains symbolic resource identities only. The private
/// `RepositoryFileCreationPolicy` remains constructed by the host-side effective
/// composer after resource resolution.
#[derive(Clone, Debug)]
pub struct RepositoryFileCreationProfile {
    executable: String,
    repository: String,
}

/// Closed host-only binding needed to construct the bounded `repo.edit-files` tool.
///
/// This contains symbolic resource identities only. The private
/// `RepositoryMultiFileMutationPolicy` remains constructed by the host-side
/// effective composer after resource resolution.
#[derive(Clone, Debug)]
pub struct RepositoryMultiFileEditProfile {
    executable: String,
    repository: String,
}

type CapabilityBuildResult = (
    EffectiveCapability,
    Option<RepositoryWorktreePatchProfile>,
    Option<RepositoryFileCreationProfile>,
    Option<RepositoryMultiFileEditProfile>,
    Option<RepositoryObserverProfile>,
);

/// Closed host-only binding needed to construct one fixed repository observer.
///
/// This carries only the closed capability identity and symbolic resources. The
/// private observer command envelope remains entirely inside the observer tools.
#[derive(Clone, Debug)]
pub struct RepositoryObserverProfile {
    capability_id: String,
    executable: String,
    repository: String,
}

impl RepositoryObserverProfile {
    #[must_use]
    pub fn capability_id(&self) -> &str {
        &self.capability_id
    }

    #[must_use]
    pub fn executable(&self) -> &str {
        &self.executable
    }

    #[must_use]
    pub fn repository(&self) -> &str {
        &self.repository
    }
}

impl RepositoryWorktreePatchProfile {
    #[must_use]
    pub fn executable(&self) -> &str {
        &self.executable
    }

    #[must_use]
    pub fn repository(&self) -> &str {
        &self.repository
    }
}

impl RepositoryFileCreationProfile {
    #[must_use]
    pub fn executable(&self) -> &str {
        &self.executable
    }

    #[must_use]
    pub fn repository(&self) -> &str {
        &self.repository
    }
}

impl RepositoryMultiFileEditProfile {
    #[must_use]
    pub fn executable(&self) -> &str {
        &self.executable
    }

    #[must_use]
    pub fn repository(&self) -> &str {
        &self.repository
    }
}

fn build_capability(
    capability: &Capability,
    resources: &Resources,
    registry: &mut ToolRegistry,
) -> Result<CapabilityBuildResult, ProfileError> {
    let (permission, symbolic_resources) = capability_contract(capability)?;
    if !capability.enabled {
        return Ok((
            EffectiveCapability {
                capability_id: capability.name.clone(),
                enabled: false,
                registered: false,
                permission,
                resources: symbolic_resources,
                validation: "disabled",
            },
            None,
            None,
            None,
            None,
        ));
    }

    if capability.name == "repo.patch" {
        let binding = repository_worktree_patch_binding(capability)?;
        // Static validation resolves only the closed symbolic resource shape.
        // It must not construct the tool, inspect Git, or touch the worktree.
        executable(resources, Some(binding.executable()))?;
        directory(resources, Some(binding.repository()))?;
        return Ok((
            EffectiveCapability {
                capability_id: capability.name.clone(),
                enabled: true,
                registered: false,
                permission,
                resources: symbolic_resources,
                validation: "configured",
            },
            Some(binding),
            None,
            None,
            None,
        ));
    }

    if capability.name == "repo.create-file" {
        let binding = repository_file_creation_binding(capability)?;
        // Static validation resolves only the closed symbolic resource shape.
        // It must not construct the tool, inspect Git, or create a target.
        executable(resources, Some(binding.executable()))?;
        directory(resources, Some(binding.repository()))?;
        return Ok((
            EffectiveCapability {
                capability_id: capability.name.clone(),
                enabled: true,
                registered: false,
                permission,
                resources: symbolic_resources,
                validation: "configured",
            },
            None,
            Some(binding),
            None,
            None,
        ));
    }

    if capability.name == "repo.edit-files" {
        let binding = repository_multi_file_edit_binding(capability)?;
        // Static validation resolves only the closed symbolic resource shape.
        // It must not construct the tool, inspect Git, or touch the worktree.
        executable(resources, Some(binding.executable()))?;
        directory(resources, Some(binding.repository()))?;
        return Ok((
            EffectiveCapability {
                capability_id: capability.name.clone(),
                enabled: true,
                registered: false,
                permission,
                resources: symbolic_resources,
                validation: "configured",
            },
            None,
            None,
            Some(binding),
            None,
        ));
    }

    if is_repository_observer(&capability.name) {
        let binding = repository_observer_binding(capability)?;
        // Static validation resolves only the closed symbolic resource shape.
        // It must not construct an observer, inspect Git, or touch the worktree.
        executable(resources, Some(binding.executable()))?;
        directory(resources, Some(binding.repository()))?;
        return Ok((
            EffectiveCapability {
                capability_id: capability.name.clone(),
                enabled: true,
                registered: false,
                permission,
                resources: symbolic_resources,
                validation: "configured",
            },
            None,
            None,
            None,
            Some(binding),
        ));
    }

    let tool: Arc<dyn crate::Tool> = match capability.name.as_str() {
        "fs.read" => {
            let workspace = directory(resources, capability.workspace.as_deref())?;
            let max_bytes = capability.max_bytes.ok_or(ProfileError::InvalidProfile {
                reason: "capability_binding",
            })?;
            if max_bytes == 0 || max_bytes > MAX_FS_READ_BYTES {
                return Err(ProfileError::InvalidProfile {
                    reason: "max_bytes",
                });
            }
            let max_bytes =
                usize::try_from(max_bytes).map_err(|_| ProfileError::InvalidProfile {
                    reason: "max_bytes",
                })?;
            Arc::new(
                FsReadTool::new(workspace, max_bytes)
                    .map_err(|_| ProfileError::ConstructionFailed)?,
            )
        }
        "host.cargo.version" => {
            let executable = executable(resources, capability.executable.as_deref())?;
            let cwd = directory(resources, capability.cwd_resource.as_deref())?;
            Arc::new(
                CargoVersionTool::new(executable, cwd)
                    .map_err(|_| ProfileError::ConstructionFailed)?,
            )
        }
        "host.git.status" => {
            let executable = executable(resources, capability.executable.as_deref())?;
            let repository = directory(resources, capability.repository.as_deref())?;
            Arc::new(
                GitStatusTool::new(executable, repository)
                    .map_err(|_| ProfileError::ConstructionFailed)?,
            )
        }
        _ => return Err(ProfileError::UnsupportedCapability),
    };
    registry.register(tool).map_err(registration_error)?;
    Ok((
        EffectiveCapability {
            capability_id: capability.name.clone(),
            enabled: true,
            registered: true,
            permission,
            resources: symbolic_resources,
            validation: "validated",
        },
        None,
        None,
        None,
        None,
    ))
}

fn validate_resources(resources: &Resources) -> Result<(), ProfileError> {
    for (id, resource) in &resources.executables {
        validate_identifier(id).map_err(|_| ProfileError::InvalidProfile {
            reason: "resource_id",
        })?;
        if !resource.path.is_absolute() {
            return Err(ProfileError::InvalidProfile {
                reason: "resource_path",
            });
        }
    }
    for (id, resource) in &resources.repositories {
        validate_identifier(id).map_err(|_| ProfileError::InvalidProfile {
            reason: "resource_id",
        })?;
        if !resource.path.is_absolute() {
            return Err(ProfileError::InvalidProfile {
                reason: "resource_path",
            });
        }
    }
    Ok(())
}

fn validate_mcp_providers(
    providers: &[McpProviderProfile],
    resources: &Resources,
) -> Result<(), ProfileError> {
    let mut provider_ids = BTreeSet::new();
    for provider in providers {
        component_identifier(&provider.id).map_err(|_| ProfileError::InvalidProfile {
            reason: "mcp_provider_id",
        })?;
        if !provider_ids.insert(&provider.id) {
            return Err(ProfileError::InvalidProfile {
                reason: "duplicate_mcp_provider",
            });
        }
        executable(resources, Some(&provider.executable))?;
        let mut names = BTreeSet::new();
        for tool in &provider.tools {
            component_identifier(&tool.remote_name).map_err(|_| ProfileError::InvalidProfile {
                reason: "mcp_remote_name",
            })?;
            if !names.insert(&tool.remote_name) {
                return Err(ProfileError::InvalidProfile {
                    reason: "duplicate_mcp_tool",
                });
            }
            tool.permission()?;
            if !tool.input_schema.is_object() {
                return Err(ProfileError::InvalidProfile {
                    reason: "mcp_input_schema",
                });
            }
        }
    }
    Ok(())
}

fn validate_process_plugins(
    providers: &[ProcessPluginProfile],
    resources: &Resources,
) -> Result<(), ProfileError> {
    let mut provider_ids = BTreeSet::new();
    for provider in providers {
        component_identifier(&provider.id).map_err(|_| ProfileError::InvalidProfile {
            reason: "process_plugin_id",
        })?;
        if !provider_ids.insert(&provider.id) {
            return Err(ProfileError::InvalidProfile {
                reason: "duplicate_process_plugin",
            });
        }
        executable(resources, Some(&provider.executable))?;
        let mut names = BTreeSet::new();
        for tool in &provider.tools {
            component_identifier(&tool.remote_name).map_err(|_| ProfileError::InvalidProfile {
                reason: "process_plugin_remote_name",
            })?;
            if !names.insert(&tool.remote_name) {
                return Err(ProfileError::InvalidProfile {
                    reason: "duplicate_process_plugin_tool",
                });
            }
            tool.permission()?;
            if !tool.input_schema.is_object() {
                return Err(ProfileError::InvalidProfile {
                    reason: "process_plugin_input_schema",
                });
            }
        }
    }
    Ok(())
}

fn capability_contract(
    capability: &Capability,
) -> Result<(PermissionLevel, Vec<String>), ProfileError> {
    let expected = match capability.name.as_str() {
        "fs.read" => (
            PermissionLevel::Read,
            capability.workspace.iter().cloned().collect(),
        ),
        "host.cargo.version" => (
            PermissionLevel::Execute,
            [
                capability.executable.as_ref(),
                capability.cwd_resource.as_ref(),
            ]
            .into_iter()
            .flatten()
            .cloned()
            .collect(),
        ),
        "host.git.status" => (
            PermissionLevel::Execute,
            [
                capability.executable.as_ref(),
                capability.repository.as_ref(),
            ]
            .into_iter()
            .flatten()
            .cloned()
            .collect(),
        ),
        "repo.patch" | "repo.create-file" | "repo.edit-files" | "repo.file-info"
        | "repo.status" | "repo.diff" | "repo.diff-staged" => (
            PermissionLevel::Execute,
            [
                capability.executable.as_ref(),
                capability.repository.as_ref(),
            ]
            .into_iter()
            .flatten()
            .cloned()
            .collect(),
        ),
        _ => return Err(ProfileError::UnsupportedCapability),
    };
    if parse_permission(&capability.permission) != Some(expected.0) {
        return Err(ProfileError::InvalidPermission);
    }
    Ok(expected)
}

fn is_repository_observer(name: &str) -> bool {
    matches!(
        name,
        "repo.file-info" | "repo.status" | "repo.diff" | "repo.diff-staged"
    )
}

fn repository_worktree_patch_binding(
    capability: &Capability,
) -> Result<RepositoryWorktreePatchProfile, ProfileError> {
    if capability.workspace.is_some()
        || capability.max_bytes.is_some()
        || capability.cwd_resource.is_some()
    {
        return Err(ProfileError::InvalidProfile {
            reason: "capability_binding",
        });
    }
    let executable = capability
        .executable
        .as_ref()
        .ok_or(ProfileError::InvalidProfile {
            reason: "capability_binding",
        })?;
    let repository = capability
        .repository
        .as_ref()
        .ok_or(ProfileError::InvalidProfile {
            reason: "capability_binding",
        })?;
    validate_identifier(executable).map_err(|_| ProfileError::UnavailableResource)?;
    validate_identifier(repository).map_err(|_| ProfileError::UnavailableResource)?;
    Ok(RepositoryWorktreePatchProfile {
        executable: executable.clone(),
        repository: repository.clone(),
    })
}

fn repository_file_creation_binding(
    capability: &Capability,
) -> Result<RepositoryFileCreationProfile, ProfileError> {
    if capability.workspace.is_some()
        || capability.max_bytes.is_some()
        || capability.cwd_resource.is_some()
    {
        return Err(ProfileError::InvalidProfile {
            reason: "capability_binding",
        });
    }
    let executable = capability
        .executable
        .as_ref()
        .ok_or(ProfileError::InvalidProfile {
            reason: "capability_binding",
        })?;
    let repository = capability
        .repository
        .as_ref()
        .ok_or(ProfileError::InvalidProfile {
            reason: "capability_binding",
        })?;
    validate_identifier(executable).map_err(|_| ProfileError::UnavailableResource)?;
    validate_identifier(repository).map_err(|_| ProfileError::UnavailableResource)?;
    Ok(RepositoryFileCreationProfile {
        executable: executable.clone(),
        repository: repository.clone(),
    })
}

fn repository_multi_file_edit_binding(
    capability: &Capability,
) -> Result<RepositoryMultiFileEditProfile, ProfileError> {
    if capability.workspace.is_some()
        || capability.max_bytes.is_some()
        || capability.cwd_resource.is_some()
    {
        return Err(ProfileError::InvalidProfile {
            reason: "capability_binding",
        });
    }
    let executable = capability
        .executable
        .as_ref()
        .ok_or(ProfileError::InvalidProfile {
            reason: "capability_binding",
        })?;
    let repository = capability
        .repository
        .as_ref()
        .ok_or(ProfileError::InvalidProfile {
            reason: "capability_binding",
        })?;
    validate_identifier(executable).map_err(|_| ProfileError::UnavailableResource)?;
    validate_identifier(repository).map_err(|_| ProfileError::UnavailableResource)?;
    Ok(RepositoryMultiFileEditProfile {
        executable: executable.clone(),
        repository: repository.clone(),
    })
}

fn repository_observer_binding(
    capability: &Capability,
) -> Result<RepositoryObserverProfile, ProfileError> {
    if capability.workspace.is_some()
        || capability.max_bytes.is_some()
        || capability.cwd_resource.is_some()
    {
        return Err(ProfileError::InvalidProfile {
            reason: "capability_binding",
        });
    }
    let executable = capability
        .executable
        .as_ref()
        .ok_or(ProfileError::InvalidProfile {
            reason: "capability_binding",
        })?;
    let repository = capability
        .repository
        .as_ref()
        .ok_or(ProfileError::InvalidProfile {
            reason: "capability_binding",
        })?;
    validate_identifier(executable).map_err(|_| ProfileError::UnavailableResource)?;
    validate_identifier(repository).map_err(|_| ProfileError::UnavailableResource)?;
    Ok(RepositoryObserverProfile {
        capability_id: capability.name.clone(),
        executable: executable.clone(),
        repository: repository.clone(),
    })
}

fn parse_permission(value: &str) -> Option<PermissionLevel> {
    match value {
        "read" => Some(PermissionLevel::Read),
        "execute" => Some(PermissionLevel::Execute),
        _ => None,
    }
}

fn parse_external_permission(value: &str) -> Option<PermissionLevel> {
    match value {
        "None" | "none" => Some(PermissionLevel::None),
        "Read" | "read" => Some(PermissionLevel::Read),
        "Write" | "write" => Some(PermissionLevel::Write),
        "Execute" | "execute" => Some(PermissionLevel::Execute),
        _ => None,
    }
}

fn component_identifier(value: &str) -> Result<(), ()> {
    if value.is_empty()
        || value.len() > 64
        || !value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
        })
    {
        return Err(());
    }
    Ok(())
}

fn executable<'a>(resources: &'a Resources, id: Option<&str>) -> Result<&'a Path, ProfileError> {
    let id = id.ok_or(ProfileError::InvalidProfile {
        reason: "capability_binding",
    })?;
    validate_identifier(id).map_err(|_| ProfileError::UnavailableResource)?;
    let resource = resources
        .executables
        .get(id)
        .ok_or(ProfileError::UnavailableResource)?;
    let NativeKind::Native = resource.kind;
    absolute_path(&resource.path)
}

fn directory<'a>(resources: &'a Resources, id: Option<&str>) -> Result<&'a Path, ProfileError> {
    let id = id.ok_or(ProfileError::InvalidProfile {
        reason: "capability_binding",
    })?;
    validate_identifier(id).map_err(|_| ProfileError::UnavailableResource)?;
    let resource = resources
        .repositories
        .get(id)
        .ok_or(ProfileError::UnavailableResource)?;
    absolute_path(&resource.path)
}

fn absolute_path(path: &Path) -> Result<&Path, ProfileError> {
    if path.is_absolute() {
        Ok(path)
    } else {
        Err(ProfileError::UnavailableResource)
    }
}

fn registration_error(error: ToolError) -> ProfileError {
    match error {
        ToolError::DuplicateTool { .. } => ProfileError::DuplicateRegistration,
        _ => ProfileError::ConstructionFailed,
    }
}

fn validate_identifier(value: &str) -> Result<(), ()> {
    if value.is_empty()
        || value.len() > 96
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
    {
        return Err(());
    }
    Ok(())
}

fn parse_unique_json(raw: &[u8]) -> Result<Value, ProfileError> {
    let mut deserializer = serde_json::Deserializer::from_slice(raw);
    let value = serde::de::Deserializer::deserialize_any(&mut deserializer, UniqueValueVisitor)
        .map_err(|error| {
            if error.to_string().contains("duplicate JSON key") {
                ProfileError::DuplicateKey
            } else {
                ProfileError::InvalidJson
            }
        })?;
    deserializer.end().map_err(|_| ProfileError::InvalidJson)?;
    Ok(value)
}

struct UniqueValueVisitor;

impl<'de> serde::de::Visitor<'de> for UniqueValueVisitor {
    type Value = Value;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("a JSON value")
    }

    fn visit_bool<E: serde::de::Error>(self, value: bool) -> Result<Value, E> {
        Ok(Value::Bool(value))
    }
    fn visit_i64<E: serde::de::Error>(self, value: i64) -> Result<Value, E> {
        Ok(Value::Number(value.into()))
    }
    fn visit_u64<E: serde::de::Error>(self, value: u64) -> Result<Value, E> {
        Ok(Value::Number(value.into()))
    }
    fn visit_f64<E: serde::de::Error>(self, value: f64) -> Result<Value, E> {
        serde_json::Number::from_f64(value)
            .map(Value::Number)
            .ok_or_else(|| E::custom("invalid number"))
    }
    fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Value, E> {
        Ok(Value::String(value.to_owned()))
    }
    fn visit_string<E: serde::de::Error>(self, value: String) -> Result<Value, E> {
        Ok(Value::String(value))
    }
    fn visit_none<E: serde::de::Error>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }
    fn visit_unit<E: serde::de::Error>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }
    fn visit_seq<A: serde::de::SeqAccess<'de>>(self, mut sequence: A) -> Result<Value, A::Error> {
        let mut values = Vec::new();
        while let Some(value) = sequence.next_element_seed(UniqueValueSeed)? {
            values.push(value);
        }
        Ok(Value::Array(values))
    }
    fn visit_map<A: serde::de::MapAccess<'de>>(self, mut map: A) -> Result<Value, A::Error> {
        let mut values = serde_json::Map::new();
        while let Some(key) = map.next_key::<String>()? {
            if values.contains_key(&key) {
                return Err(serde::de::Error::custom("duplicate JSON key"));
            }
            values.insert(key, map.next_value_seed(UniqueValueSeed)?);
        }
        Ok(Value::Object(values))
    }
}

struct UniqueValueSeed;

impl<'de> serde::de::DeserializeSeed<'de> for UniqueValueSeed {
    type Value = Value;
    fn deserialize<D: serde::Deserializer<'de>>(self, deserializer: D) -> Result<Value, D::Error> {
        deserializer.deserialize_any(UniqueValueVisitor)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::{ProfileError, TrustedStaticProfile};
    use crate::trusted_profile_source::MAX_PROFILE_BYTES;
    use serde_json::json;

    static NEXT_TEST_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let stamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock should follow epoch")
                .as_nanos();
            let sequence = NEXT_TEST_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "rah-trusted-profile-{}-{stamp}-{sequence}",
                std::process::id()
            ));
            fs::create_dir(&path).expect("test directory should be created");
            Self(path)
        }

        fn profile(&self, contents: &str) -> PathBuf {
            let path = self.0.join("profile.json");
            fs::write(&path, contents).expect("profile should be written");
            path
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn fs_read_profile(workspace: &str) -> String {
        format!(
            r#"{{
                "profile_version": 1,
                "profile_id": "test-profile",
                "resources": {{"repositories": {{"workspace": {{"path": "{workspace}"}}}}}},
                "capabilities": [{{
                    "name": "fs.read",
                    "enabled": true,
                    "permission": "read",
                    "workspace": "workspace",
                    "max_bytes": 1024
                }}]
            }}"#,
            workspace = workspace.replace('\\', "\\\\")
        )
    }

    #[test]
    fn valid_profile_constructs_fresh_registry_and_redacted_inventory() {
        let directory = TestDirectory::new();
        let path = directory.profile(&fs_read_profile(&directory.0.to_string_lossy()));

        let loaded = TrustedStaticProfile::load(path).expect("profile should load");

        assert_eq!(loaded.registry().definitions().len(), 1);
        let effective = loaded.effective_profile();
        assert_eq!(effective.source_class, "trusted_static_file");
        assert_eq!(effective.capabilities[0].resources, vec!["workspace"]);
        assert!(!format!("{effective:?}").contains(&directory.0.to_string_lossy().to_string()));
    }

    #[test]
    fn repo_patch_profile_is_deferred_and_accepts_only_symbolic_host_resources() {
        let directory = TestDirectory::new();
        let executable = directory
            .0
            .join("git.exe")
            .to_string_lossy()
            .replace('\\', "\\\\");
        let repository = directory.0.to_string_lossy().replace('\\', "\\\\");
        let valid = format!(
            r#"{{"profile_version":1,"profile_id":"repo-patch","resources":{{"executables":{{"git":{{"path":"{executable}","kind":"native"}}}},"repositories":{{"worktree":{{"path":"{repository}"}}}}}},"capabilities":[{{"name":"repo.patch","enabled":true,"permission":"execute","executable":"git","repository":"worktree"}}]}}"#
        );

        let loaded = TrustedStaticProfile::load(directory.profile(&valid))
            .expect("repo.patch static profile should load");
        assert!(loaded.registry().definitions().is_empty());
        assert_eq!(loaded.repository_worktree_patches().len(), 1);
        let capability = &loaded.effective_profile().capabilities[0];
        assert_eq!(capability.capability_id, "repo.patch");
        assert_eq!(
            capability.permission,
            rah_protocol::PermissionLevel::Execute
        );
        assert_eq!(capability.resources, ["git", "worktree"]);
        assert!(!capability.registered);
        assert_eq!(capability.validation, "configured");
        assert!(!format!("{:?}", loaded.effective_profile()).contains(&repository));

        for invalid in [
            valid.replace("\"executable\":\"git\",", ""),
            valid.replace("\"repository\":\"worktree\"", ""),
            valid.replace("\"permission\":\"execute\",", ""),
            valid.replace("\"repository\":\"worktree\"", "\"repository\":\"missing\""),
            valid.replace("\"permission\":\"execute\"", "\"permission\":\"write\""),
            valid.replace(
                "\"repository\":\"worktree\"",
                "\"repository\":\"worktree\",\"repository_root\":\"C:\\\\raw\"",
            ),
            valid.replace(
                "\"repository\":\"worktree\"",
                "\"repository\":\"worktree\",\"workspace\":\"worktree\"",
            ),
        ] {
            assert!(TrustedStaticProfile::load(directory.profile(&invalid)).is_err());
        }

        let wrong_resource_type = format!(
            r#"{{"profile_version":1,"profile_id":"repo-patch","resources":{{"executables":{{"worktree":{{"path":"{executable}","kind":"native"}}}},"repositories":{{"git":{{"path":"{repository}"}}}}}},"capabilities":[{{"name":"repo.patch","enabled":true,"permission":"execute","executable":"git","repository":"worktree"}}]}}"#
        );
        assert!(TrustedStaticProfile::load(directory.profile(&wrong_resource_type)).is_err());

        let mut duplicate: serde_json::Value =
            serde_json::from_str(&valid).expect("valid profile should deserialize for fixture");
        let capability = duplicate["capabilities"][0].clone();
        duplicate["capabilities"]
            .as_array_mut()
            .expect("capabilities should be an array")
            .push(capability);
        assert!(matches!(
            TrustedStaticProfile::load(directory.profile(&duplicate.to_string())),
            Err(ProfileError::DuplicateCapability)
        ));
    }

    #[test]
    fn repo_create_file_profile_is_deferred_and_accepts_only_symbolic_host_resources() {
        let directory = TestDirectory::new();
        let executable = directory
            .0
            .join("git.exe")
            .to_string_lossy()
            .replace('\\', "\\\\");
        let repository = directory.0.to_string_lossy().replace('\\', "\\\\");
        let valid = format!(
            r#"{{"profile_version":1,"profile_id":"repo-create-file","resources":{{"executables":{{"git":{{"path":"{executable}","kind":"native"}}}},"repositories":{{"worktree":{{"path":"{repository}"}}}}}},"capabilities":[{{"name":"repo.create-file","enabled":true,"permission":"execute","executable":"git","repository":"worktree"}}]}}"#
        );

        let loaded = TrustedStaticProfile::load(directory.profile(&valid))
            .expect("repo.create-file static profile should load");
        assert!(loaded.registry().definitions().is_empty());
        assert_eq!(loaded.repository_file_creations().len(), 1);
        let binding = &loaded.repository_file_creations()[0];
        assert_eq!(binding.executable(), "git");
        assert_eq!(binding.repository(), "worktree");
        let capability = &loaded.effective_profile().capabilities[0];
        assert_eq!(capability.capability_id, "repo.create-file");
        assert_eq!(
            capability.permission,
            rah_protocol::PermissionLevel::Execute
        );
        assert_eq!(capability.resources, ["git", "worktree"]);
        assert!(!capability.registered);
        assert_eq!(capability.validation, "configured");
        assert!(!format!("{:?}", loaded.effective_profile()).contains(&repository));

        for invalid in [
            valid.replace("\"executable\":\"git\",", ""),
            valid.replace("\"repository\":\"worktree\"", ""),
            valid.replace("\"permission\":\"execute\",", ""),
            valid.replace("\"repository\":\"worktree\"", "\"repository\":\"missing\""),
            valid.replace("\"permission\":\"execute\"", "\"permission\":\"write\""),
            valid.replace(
                "\"repository\":\"worktree\"",
                "\"repository\":\"worktree\",\"path\":\"target.txt\"",
            ),
            valid.replace(
                "\"repository\":\"worktree\"",
                "\"repository\":\"worktree\",\"content\":\"target bytes\"",
            ),
        ] {
            assert!(TrustedStaticProfile::load(directory.profile(&invalid)).is_err());
        }

        let wrong_resource_type = format!(
            r#"{{"profile_version":1,"profile_id":"repo-create-file","resources":{{"executables":{{"worktree":{{"path":"{executable}","kind":"native"}}}},"repositories":{{"git":{{"path":"{repository}"}}}}}},"capabilities":[{{"name":"repo.create-file","enabled":true,"permission":"execute","executable":"git","repository":"worktree"}}]}}"#
        );
        assert!(TrustedStaticProfile::load(directory.profile(&wrong_resource_type)).is_err());

        let mut duplicate: serde_json::Value =
            serde_json::from_str(&valid).expect("valid profile should deserialize for fixture");
        let capability = duplicate["capabilities"][0].clone();
        duplicate["capabilities"]
            .as_array_mut()
            .expect("capabilities should be an array")
            .push(capability);
        assert!(matches!(
            TrustedStaticProfile::load(directory.profile(&duplicate.to_string())),
            Err(ProfileError::DuplicateCapability)
        ));
    }

    #[test]
    fn repo_edit_files_profile_is_deferred_and_accepts_only_symbolic_host_resources() {
        let directory = TestDirectory::new();
        let executable = directory
            .0
            .join("git.exe")
            .to_string_lossy()
            .replace('\\', "\\\\");
        let repository = directory.0.to_string_lossy().replace('\\', "\\\\");
        let valid = format!(
            r#"{{"profile_version":1,"profile_id":"repo-edit-files","resources":{{"executables":{{"git":{{"path":"{executable}","kind":"native"}}}},"repositories":{{"worktree":{{"path":"{repository}"}}}}}},"capabilities":[{{"name":"repo.edit-files","enabled":true,"permission":"execute","executable":"git","repository":"worktree"}}]}}"#
        );

        let loaded = TrustedStaticProfile::load(directory.profile(&valid))
            .expect("repo.edit-files static profile should load");
        assert!(loaded.registry().definitions().is_empty());
        assert_eq!(loaded.repository_multi_file_edits().len(), 1);
        let binding = &loaded.repository_multi_file_edits()[0];
        assert_eq!(binding.executable(), "git");
        assert_eq!(binding.repository(), "worktree");
        let capability = &loaded.effective_profile().capabilities[0];
        assert_eq!(capability.capability_id, "repo.edit-files");
        assert_eq!(
            capability.permission,
            rah_protocol::PermissionLevel::Execute
        );
        assert_eq!(capability.resources, ["git", "worktree"]);
        assert!(!capability.registered);
        assert_eq!(capability.validation, "configured");
        assert!(!format!("{:?}", loaded.effective_profile()).contains(&repository));

        for invalid in [
            valid.replace("\"executable\":\"git\",", ""),
            valid.replace("\"repository\":\"worktree\"", ""),
            valid.replace("\"permission\":\"execute\"", "\"permission\":\"read\""),
            valid.replace("\"repository\":\"worktree\"", "\"repository\":\"missing\""),
            valid.replace(
                "\"repository\":\"worktree\"",
                "\"repository\":\"worktree\",\"workspace\":\"worktree\"",
            ),
            valid.replace(
                "\"repository\":\"worktree\"",
                "\"repository\":\"worktree\",\"max_bytes\":1",
            ),
            valid.replace(
                "\"repository\":\"worktree\"",
                "\"repository\":\"worktree\",\"cwd_resource\":\"worktree\"",
            ),
            valid.replace(
                "\"repository\":\"worktree\"",
                "\"repository\":\"worktree\",\"root\":\"raw\"",
            ),
        ] {
            assert!(TrustedStaticProfile::load(directory.profile(&invalid)).is_err());
        }

        let disabled = valid.replace("\"enabled\":true", "\"enabled\":false");
        let disabled = TrustedStaticProfile::load(directory.profile(&disabled))
            .expect("disabled repo.edit-files should load");
        assert!(disabled.repository_multi_file_edits().is_empty());
        assert!(!disabled.effective_profile().capabilities[0].registered);
        assert_eq!(
            disabled.effective_profile().capabilities[0].validation,
            "disabled"
        );

        let mut duplicate: serde_json::Value =
            serde_json::from_str(&valid).expect("valid profile should deserialize for fixture");
        let capability = duplicate["capabilities"][0].clone();
        duplicate["capabilities"]
            .as_array_mut()
            .expect("capabilities should be an array")
            .push(capability);
        assert!(matches!(
            TrustedStaticProfile::load(directory.profile(&duplicate.to_string())),
            Err(ProfileError::DuplicateCapability)
        ));
    }

    #[test]
    fn repository_observer_profiles_are_deferred_and_fail_closed() {
        let directory = TestDirectory::new();
        let document = json!({
            "profile_version": 1,
            "profile_id": "repository-observers",
            "resources": {
                "executables": {"git": {"path": directory.0.join("git.exe"), "kind": "native"}},
                "repositories": {"workspace": {"path": directory.0}}
            },
            "capabilities": [
                {"name": "repo.file-info", "enabled": true, "permission": "execute", "executable": "git", "repository": "workspace"},
                {"name": "repo.status", "enabled": true, "permission": "execute", "executable": "git", "repository": "workspace"},
                {"name": "repo.diff", "enabled": true, "permission": "execute", "executable": "git", "repository": "workspace"},
                {"name": "repo.diff-staged", "enabled": true, "permission": "execute", "executable": "git", "repository": "workspace"}
            ]
        });
        let loaded = TrustedStaticProfile::load(directory.profile(&document.to_string()))
            .expect("observer static profile should not construct tools");
        assert!(loaded.registry().definitions().is_empty());
        assert_eq!(loaded.repository_observers().len(), 4);
        for capability in &loaded.effective_profile().capabilities {
            assert!(capability.enabled);
            assert!(!capability.registered);
            assert_eq!(
                capability.permission,
                rah_protocol::PermissionLevel::Execute
            );
            assert_eq!(capability.resources, ["git", "workspace"]);
            assert_eq!(capability.validation, "configured");
        }
        assert!(
            !format!("{:?}", loaded.effective_profile())
                .contains(directory.0.to_string_lossy().as_ref())
        );

        for invalid in [
            json!({"name":"repo.status", "enabled":true, "permission":"execute", "repository":"workspace"}),
            json!({"name":"repo.status", "enabled":true, "permission":"execute", "executable":"git"}),
            json!({"name":"repo.status", "enabled":true, "permission":"execute", "executable":"missing", "repository":"workspace"}),
            json!({"name":"repo.status", "enabled":true, "permission":"execute", "executable":"git", "repository":"missing"}),
            json!({"name":"repo.status", "enabled":true, "permission":"execute", "executable":"git", "repository":"workspace", "workspace":"workspace"}),
            json!({"name":"repo.status", "enabled":true, "permission":"execute", "executable":"git", "repository":"workspace", "max_bytes":1}),
            json!({"name":"repo.status", "enabled":true, "permission":"execute", "executable":"git", "repository":"workspace", "cwd_resource":"workspace"}),
            json!({"name":"repo.status", "enabled":true, "permission":"read", "executable":"git", "repository":"workspace"}),
            json!({"name":"repo.unknown", "enabled":true, "permission":"execute", "executable":"git", "repository":"workspace"}),
        ] {
            let mut invalid_document = document.clone();
            invalid_document["capabilities"] = json!([invalid]);
            assert!(
                TrustedStaticProfile::load(directory.profile(&invalid_document.to_string()))
                    .is_err()
            );
        }

        let mut duplicate = document.clone();
        for name in [
            "repo.file-info",
            "repo.status",
            "repo.diff",
            "repo.diff-staged",
        ] {
            duplicate["capabilities"] = json!([
                {"name":name, "enabled":true, "permission":"execute", "executable":"git", "repository":"workspace"},
                {"name":name, "enabled":true, "permission":"execute", "executable":"git", "repository":"workspace"}
            ]);
            assert!(matches!(
                TrustedStaticProfile::load(directory.profile(&duplicate.to_string())),
                Err(ProfileError::DuplicateCapability)
            ));
        }
    }

    #[test]
    fn mcp_profile_schema_requires_symbolic_resources_unique_names_and_permissions() {
        let directory = TestDirectory::new();
        let executable = directory
            .0
            .join("server.exe")
            .to_string_lossy()
            .replace('\\', "\\\\");
        for contents in [
            format!(
                r#"{{"profile_version":1,"profile_id":"test","resources":{{"executables":{{"server":{{"path":"{executable}","kind":"native"}}}}}},"capabilities":[],"mcp_providers":[{{"id":"same","executable":"missing","tools":[]}}]}}"#
            ),
            format!(
                r#"{{"profile_version":1,"profile_id":"test","resources":{{"executables":{{"server":{{"path":"{executable}","kind":"native"}}}}}},"capabilities":[],"mcp_providers":[{{"id":"same","executable":"server","tools":[{{"remote_name":"echo","permission":"None","input_schema":{{}}}},{{"remote_name":"echo","permission":"None","input_schema":{{}}}}]}}]}}"#
            ),
            format!(
                r#"{{"profile_version":1,"profile_id":"test","resources":{{"executables":{{"server":{{"path":"{executable}","kind":"native"}}}}}},"capabilities":[],"mcp_providers":[{{"id":"same","executable":"server","tools":[{{"remote_name":"echo","input_schema":{{}}}}]}}]}}"#
            ),
            format!(
                r#"{{"profile_version":1,"profile_id":"test","resources":{{"executables":{{"server":{{"path":"{executable}","kind":"native"}}}}}},"capabilities":[],"mcp_providers":[{{"id":"same","executable":"server","tools":[],"cwd":"forbidden"}}]}}"#
            ),
        ] {
            assert!(TrustedStaticProfile::load(directory.profile(&contents)).is_err());
        }
    }

    #[test]
    fn process_plugin_profile_is_closed_and_validated_without_spawning() {
        let directory = TestDirectory::new();
        let executable = directory
            .0
            .join("plugin.exe")
            .to_string_lossy()
            .replace('\\', "\\\\");
        let valid = format!(
            r#"{{"profile_version":1,"profile_id":"test","resources":{{"executables":{{"plugin":{{"path":"{executable}","kind":"native"}}}}}},"capabilities":[],"process_plugins":[{{"id":"local-test","executable":"plugin","tools":[{{"remote_name":"echo","permission":"None","input_schema":{{"type":"object"}}}}]}}]}}"#
        );
        let loaded = TrustedStaticProfile::load(directory.profile(&valid))
            .expect("static profile should parse");
        assert_eq!(loaded.process_plugins().len(), 1);
        assert_eq!(
            loaded.effective_profile().providers[0].kind,
            "process_plugin"
        );
        for invalid in [
            valid.replace("\"local-test\"", "\"INVALID\""),
            valid.replace("\"permission\":\"None\",", ""),
            valid.replace(
                "\"input_schema\":{\"type\":\"object\"}",
                "\"input_schema\":[]",
            ),
            valid.replace("\"tools\":[", "\"cwd\":\"forbidden\",\"tools\":["),
        ] {
            assert!(TrustedStaticProfile::load(directory.profile(&invalid)).is_err());
        }
    }

    #[test]
    fn source_validation_rejects_relative_missing_directory_oversized_and_non_utf8_files() {
        let directory = TestDirectory::new();
        assert!(matches!(
            TrustedStaticProfile::load("profile.json"),
            Err(ProfileError::InvalidSource)
        ));
        assert!(matches!(
            TrustedStaticProfile::load(directory.0.join("missing-secret-profile.json")),
            Err(ProfileError::UnreadableSource)
        ));
        assert!(matches!(
            TrustedStaticProfile::load(&directory.0),
            Err(ProfileError::NonRegularSource)
        ));

        let oversized = directory.0.join("oversized.json");
        fs::write(&oversized, vec![b' '; MAX_PROFILE_BYTES as usize + 1])
            .expect("oversized profile should be written");
        assert!(matches!(
            TrustedStaticProfile::load(oversized),
            Err(ProfileError::SourceTooLarge)
        ));

        let binary = directory.0.join("binary.json");
        fs::write(&binary, [0xff, 0xfe]).expect("binary profile should be written");
        assert!(matches!(
            TrustedStaticProfile::load(binary),
            Err(ProfileError::InvalidEncoding)
        ));
    }

    #[test]
    fn source_validation_rejects_links_without_leaking_source_or_contents() {
        let directory = TestDirectory::new();
        let secret = "TRUSTED_PROFILE_SOURCE_SECRET";
        let target = directory.0.join("target.json");
        fs::write(&target, secret).expect("profile target should be written");
        let link = directory.0.join("linked-profile.json");

        if !create_file_link(&target, &link) {
            return;
        }

        let error = match TrustedStaticProfile::load(&link) {
            Err(error) => error,
            Ok(_) => panic!("link must be rejected"),
        };
        let rendered = error.to_string();
        assert_eq!(error, ProfileError::LinkedSource);
        assert!(!rendered.contains(link.to_string_lossy().as_ref()));
        assert!(!rendered.contains(secret));
    }

    #[test]
    fn lexical_path_aliases_are_rejected_before_source_identity_checks() {
        let directory = TestDirectory::new();
        let path = directory.profile(&fs_read_profile(&directory.0.to_string_lossy()));
        let alias = directory.0.join("unused").join("..").join("profile.json");

        assert!(matches!(
            TrustedStaticProfile::load(alias),
            Err(ProfileError::UnsupportedSource)
        ));
        assert!(path.exists());
    }

    #[cfg(windows)]
    #[test]
    fn windows_rejects_unsupported_source_prefixes_and_ads_forms() {
        for path in [
            r"\\server\share\profile.json",
            r"\\?\C:\profile.json",
            r"C:\profile.json:ads",
        ] {
            assert!(matches!(
                TrustedStaticProfile::load(path),
                Err(ProfileError::UnsupportedSource)
            ));
        }
    }

    #[test]
    fn rejects_unknown_fields_and_versions_before_construction() {
        let directory = TestDirectory::new();
        let unknown = directory.profile(
            r#"{"profile_version":1,"profile_id":"test","resources":{},"capabilities":[],"token":"secret"}"#,
        );
        assert!(matches!(
            TrustedStaticProfile::load(unknown),
            Err(ProfileError::InvalidProfile { reason: "schema" })
        ));

        let version = directory.profile(
            r#"{"profile_version":2,"profile_id":"test","resources":{},"capabilities":[]}"#,
        );
        assert!(matches!(
            TrustedStaticProfile::load(version),
            Err(ProfileError::UnsupportedVersion)
        ));
    }

    #[test]
    fn rejects_duplicate_json_keys_and_capabilities() {
        let directory = TestDirectory::new();
        let duplicate_key = directory.profile(
            r#"{"profile_version":1,"profile_version":1,"profile_id":"test","resources":{},"capabilities":[]}"#,
        );
        assert!(matches!(
            TrustedStaticProfile::load(duplicate_key),
            Err(ProfileError::DuplicateKey)
        ));

        let duplicate_capability = directory.profile(
            r#"{"profile_version":1,"profile_id":"test","resources":{},"capabilities":[{"name":"fs.read","enabled":false,"permission":"read"},{"name":"fs.read","enabled":false,"permission":"read"}]}"#,
        );
        assert!(matches!(
            TrustedStaticProfile::load(duplicate_capability),
            Err(ProfileError::DuplicateCapability)
        ));
    }

    #[test]
    fn rejects_permission_and_symbolic_resource_errors_without_paths_or_secrets() {
        let directory = TestDirectory::new();
        let invalid = directory.profile(
            r#"{"profile_version":1,"profile_id":"test","resources":{},"capabilities":[{"name":"fs.read","enabled":true,"permission":"execute","workspace":"missing","max_bytes":32}]}"#,
        );
        assert!(matches!(
            TrustedStaticProfile::load(invalid),
            Err(ProfileError::InvalidPermission)
        ));

        let missing = directory.profile(
            r#"{"profile_version":1,"profile_id":"test","resources":{},"capabilities":[{"name":"fs.read","enabled":true,"permission":"read","workspace":"missing","max_bytes":32}]}"#,
        );
        let error = match TrustedStaticProfile::load(missing) {
            Ok(_) => panic!("missing resource should fail"),
            Err(error) => error,
        };
        let rendered = error.to_string();
        assert_eq!(error, ProfileError::UnavailableResource);
        assert!(!rendered.contains("missing"));
        assert!(!rendered.contains("secret"));
    }

    #[cfg(unix)]
    fn create_file_link(target: &std::path::Path, link: &std::path::Path) -> bool {
        std::os::unix::fs::symlink(target, link).is_ok()
    }

    #[cfg(windows)]
    fn create_file_link(target: &std::path::Path, link: &std::path::Path) -> bool {
        std::os::windows::fs::symlink_file(target, link).is_ok()
    }

    #[cfg(not(any(unix, windows)))]
    fn create_file_link(_target: &std::path::Path, _link: &std::path::Path) -> bool {
        false
    }
}
