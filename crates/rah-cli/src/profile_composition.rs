//! Host-owned effective composition of already hardened external providers.

use std::sync::Arc;

use rah_tools::{
    EffectiveCapability, EffectiveProfile, ProfileError, RepositoryCommitControl,
    RepositoryCommitTool, RepositoryDiffStagedTool, RepositoryDiffTool, RepositoryFileCreationTool,
    RepositoryFileDeletionAuthority, RepositoryFileDeletionTool, RepositoryFileInfoTool,
    RepositoryFileRenameAuthority, RepositoryFileRenameTool, RepositoryMultiFileEditTool,
    RepositoryStatusTool, RepositoryWorktreePatchTool, ToolRegistry, TrustedStaticProfile,
};
use rah_tools_mcp::{McpAdapter, McpServerConfig};
use rah_tools_plugin::{PluginAdapter, PluginConfig};

/// Owns every provider whose immutable tools are registered in the profile.
pub struct EffectiveProfileComposition {
    registry: Arc<ToolRegistry>,
    effective: EffectiveProfile,
    mcp_adapters: Vec<McpAdapter>,
    plugin_adapters: Vec<PluginAdapter>,
    repository_commit_control: Option<RepositoryCommitControl>,
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

    /// Returns the host-only control for the sole composed `repo.commit` capability.
    #[must_use]
    pub fn repository_commit_control(&self) -> Option<&RepositoryCommitControl> {
        self.repository_commit_control.as_ref()
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
    compose_with_repository_file_authorities(profile, None, None).await
}

/// Composes a profile using a deletion authority separately constructed by the host.
pub async fn compose_with_repository_file_deletion_authority(
    profile: TrustedStaticProfile,
    deletion_authority: Option<RepositoryFileDeletionAuthority>,
) -> Result<EffectiveProfileComposition, ProfileError> {
    compose_with_repository_file_authorities(profile, deletion_authority, None).await
}

/// Composes a profile using a rename authority separately constructed by the host.
pub async fn compose_with_repository_file_rename_authority(
    profile: TrustedStaticProfile,
    rename_authority: Option<RepositoryFileRenameAuthority>,
) -> Result<EffectiveProfileComposition, ProfileError> {
    compose_with_repository_file_authorities(profile, None, rename_authority).await
}

async fn compose_with_repository_file_authorities(
    profile: TrustedStaticProfile,
    deletion_authority: Option<RepositoryFileDeletionAuthority>,
    rename_authority: Option<RepositoryFileRenameAuthority>,
) -> Result<EffectiveProfileComposition, ProfileError> {
    let mut registry = ToolRegistry::new();
    let mut repository_commit_control = None;
    for definition in profile.registry().definitions() {
        let tool = profile
            .registry()
            .get(&definition.name)
            .ok_or(ProfileError::ConstructionFailed)?;
        registry
            .register(tool)
            .map_err(|_| ProfileError::DuplicateRegistration)?;
    }

    for patch in profile.repository_worktree_patches() {
        let executable = profile
            .executable_resource(patch.executable())
            .map_err(|_| ProfileError::ConstructionFailed)?;
        let repository = profile
            .repository_resource(patch.repository())
            .map_err(|_| ProfileError::ConstructionFailed)?;
        registry
            .register(Arc::new(
                RepositoryWorktreePatchTool::new(executable, repository)
                    .map_err(|_| ProfileError::ConstructionFailed)?,
            ))
            .map_err(|_| ProfileError::DuplicateRegistration)?;
    }

    for creation in profile.repository_file_creations() {
        let executable = profile
            .executable_resource(creation.executable())
            .map_err(|_| ProfileError::ConstructionFailed)?;
        let repository = profile
            .repository_resource(creation.repository())
            .map_err(|_| ProfileError::ConstructionFailed)?;
        registry
            .register(Arc::new(
                RepositoryFileCreationTool::new(executable, repository)
                    .map_err(|_| ProfileError::ConstructionFailed)?,
            ))
            .map_err(|_| ProfileError::DuplicateRegistration)?;
    }

    for deletion in profile.repository_file_deletions() {
        let executable = profile
            .executable_resource(deletion.executable())
            .map_err(|_| ProfileError::ConstructionFailed)?;
        let repository = profile
            .repository_resource(deletion.repository())
            .map_err(|_| ProfileError::ConstructionFailed)?;
        let authority = deletion_authority
            .as_ref()
            .filter(|authority| authority.matches_resources(executable, repository))
            .ok_or(ProfileError::ConstructionFailed)?;
        registry
            .register(Arc::new(RepositoryFileDeletionTool::from_authority(
                authority.clone(),
            )))
            .map_err(|_| ProfileError::DuplicateRegistration)?;
    }

    for rename in profile.repository_file_renames() {
        let executable = profile
            .executable_resource(rename.executable())
            .map_err(|_| ProfileError::ConstructionFailed)?;
        let repository = profile
            .repository_resource(rename.repository())
            .map_err(|_| ProfileError::ConstructionFailed)?;
        let authority = rename_authority
            .as_ref()
            .filter(|authority| authority.matches_resources(executable, repository))
            .ok_or(ProfileError::ConstructionFailed)?;
        registry
            .register(Arc::new(RepositoryFileRenameTool::from_authority(
                authority.clone(),
            )))
            .map_err(|_| ProfileError::DuplicateRegistration)?;
    }

    for commit in profile.repository_commits() {
        if repository_commit_control.is_some() {
            return Err(ProfileError::DuplicateRegistration);
        }
        let executable = profile
            .executable_resource(commit.executable())
            .map_err(|_| ProfileError::ConstructionFailed)?;
        let repository = profile
            .repository_resource(commit.repository())
            .map_err(|_| ProfileError::ConstructionFailed)?;
        let (tool, control) = RepositoryCommitTool::compose(
            executable,
            repository,
            commit.identity_name().to_owned(),
            commit.identity_email().to_owned(),
        )
        .map_err(|_| ProfileError::ConstructionFailed)?;
        registry
            .register(Arc::new(tool))
            .map_err(|_| ProfileError::DuplicateRegistration)?;
        repository_commit_control = Some(control);
    }

    for edit in profile.repository_multi_file_edits() {
        let executable = profile
            .executable_resource(edit.executable())
            .map_err(|_| ProfileError::ConstructionFailed)?;
        let repository = profile
            .repository_resource(edit.repository())
            .map_err(|_| ProfileError::ConstructionFailed)?;
        registry
            .register(Arc::new(
                RepositoryMultiFileEditTool::new(executable, repository)
                    .map_err(|_| ProfileError::ConstructionFailed)?,
            ))
            .map_err(|_| ProfileError::DuplicateRegistration)?;
    }

    for observer in profile.repository_observers() {
        let executable = profile
            .executable_resource(observer.executable())
            .map_err(|_| ProfileError::ConstructionFailed)?;
        let repository = profile
            .repository_resource(observer.repository())
            .map_err(|_| ProfileError::ConstructionFailed)?;
        let tool: Arc<dyn rah_tools::Tool> = match observer.capability_id() {
            "repo.file-info" => Arc::new(
                RepositoryFileInfoTool::new(executable, repository)
                    .map_err(|_| ProfileError::ConstructionFailed)?,
            ),
            "repo.status" => Arc::new(
                RepositoryStatusTool::new(executable, repository)
                    .map_err(|_| ProfileError::ConstructionFailed)?,
            ),
            "repo.diff" => Arc::new(
                RepositoryDiffTool::new(executable, repository)
                    .map_err(|_| ProfileError::ConstructionFailed)?,
            ),
            "repo.diff-staged" => Arc::new(
                RepositoryDiffStagedTool::new(executable, repository)
                    .map_err(|_| ProfileError::ConstructionFailed)?,
            ),
            _ => return Err(ProfileError::ConstructionFailed),
        };
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
    for capability in &mut effective.capabilities {
        if capability.enabled
            && (matches!(
                capability.capability_id.as_str(),
                "repo.patch"
                    | "repo.create-file"
                    | "repo.edit-files"
                    | "repo.commit"
                    | "repo.delete-file"
                    | "repo.rename-file"
            ) || matches!(
                capability.capability_id.as_str(),
                "repo.file-info" | "repo.status" | "repo.diff" | "repo.diff-staged"
            ))
        {
            capability.registered = true;
            capability.validation = "validated";
        }
    }
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
        repository_commit_control,
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
        process::Command,
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
            let status = Command::new(git_executable())
                .args(["init", "--quiet"])
                .current_dir(&directory)
                .status()
                .expect("Git should initialize the owned fixture repository");
            assert!(
                status.success(),
                "Git should initialize the fixture repository"
            );
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

    fn git_executable() -> PathBuf {
        #[cfg(windows)]
        let output = Command::new("where.exe")
            .arg("git.exe")
            .output()
            .expect("where should locate Git");
        #[cfg(not(windows))]
        let output = Command::new("which")
            .arg("git")
            .output()
            .expect("which should locate Git");
        assert!(
            output.status.success(),
            "Git should be available for fixtures"
        );
        PathBuf::from(
            String::from_utf8(output.stdout)
                .expect("Git path should be UTF-8")
                .lines()
                .next()
                .expect("Git path should be present"),
        )
    }

    fn worktree_status(directory: &Path) -> Vec<u8> {
        let output = Command::new(git_executable())
            .args(["status", "--porcelain=v1"])
            .current_dir(directory)
            .output()
            .expect("Git status should run for fixture");
        assert!(output.status.success(), "Git status should succeed");
        output.stdout
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
        executables.insert(
            "git".to_owned(),
            json!({"path": git_executable(), "kind":"native"}),
        );
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
            "capabilities": [
                {"name":"fs.read", "enabled":true, "permission":"read", "workspace":"workspace", "max_bytes":1024},
                {"name":"repo.patch", "enabled":true, "permission":"execute", "executable":"git", "repository":"workspace"},
                {"name":"repo.create-file", "enabled":true, "permission":"execute", "executable":"git", "repository":"workspace"},
                {"name":"repo.edit-files", "enabled":true, "permission":"execute", "executable":"git", "repository":"workspace"},
                {"name":"repo.file-info", "enabled":true, "permission":"execute", "executable":"git", "repository":"workspace"},
                {"name":"repo.status", "enabled":true, "permission":"execute", "executable":"git", "repository":"workspace"},
                {"name":"repo.diff", "enabled":true, "permission":"execute", "executable":"git", "repository":"workspace"},
                {"name":"repo.diff-staged", "enabled":true, "permission":"execute", "executable":"git", "repository":"workspace"}
            ],
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
        let target = directory.0.join("target.txt");
        fs::write(&target, b"repo.patch composition must not execute\n")
            .expect("fixture target should be written");
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
        let target_before = fs::read(&target).expect("fixture target should be readable");
        let composition =
            compose(TrustedStaticProfile::load(&profile_path).expect("static profile should load"))
                .await
                .expect("mixed effective composition should succeed");

        mcp.assert_events(&["spawn"]).await;
        plugin.assert_events(&["spawn"]).await;

        let definitions = composition.registry().definitions();
        assert_eq!(
            definitions.len(),
            10,
            "built-ins, repository tools, and exactly admitted external tools are published"
        );
        for name in [
            "repo.file-info",
            "repo.status",
            "repo.diff",
            "repo.diff-staged",
        ] {
            assert_eq!(
                definitions
                    .iter()
                    .find(|definition| definition.name == ToolName::new(name))
                    .expect("repository observer should be registered")
                    .permission,
                PermissionLevel::Execute
            );
        }
        assert_eq!(
            definitions
                .iter()
                .find(|definition| definition.name == ToolName::new("repo.patch"))
                .expect("repo.patch should be registered through ToolRegistry")
                .permission,
            PermissionLevel::Execute
        );
        assert_eq!(
            definitions
                .iter()
                .find(|definition| definition.name == ToolName::new("repo.create-file"))
                .expect("repo.create-file should be registered through ToolRegistry")
                .permission,
            PermissionLevel::Execute
        );
        assert_eq!(
            definitions
                .iter()
                .find(|definition| definition.name == ToolName::new("repo.edit-files"))
                .expect("repo.edit-files should be registered through ToolRegistry")
                .permission,
            PermissionLevel::Execute
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
        let repo_patch = composition
            .effective_profile()
            .capabilities
            .iter()
            .find(|capability| capability.capability_id == "repo.patch")
            .expect("redacted inventory should include repo.patch");
        assert!(repo_patch.registered);
        assert_eq!(repo_patch.permission, PermissionLevel::Execute);
        assert_eq!(repo_patch.resources, ["git", "workspace"]);
        assert_eq!(repo_patch.validation, "validated");
        let repo_create_file = composition
            .effective_profile()
            .capabilities
            .iter()
            .find(|capability| capability.capability_id == "repo.create-file")
            .expect("redacted inventory should include repo.create-file");
        assert!(repo_create_file.registered);
        assert_eq!(repo_create_file.permission, PermissionLevel::Execute);
        assert_eq!(repo_create_file.resources, ["git", "workspace"]);
        assert_eq!(repo_create_file.validation, "validated");
        let repo_edit_files = composition
            .effective_profile()
            .capabilities
            .iter()
            .find(|capability| capability.capability_id == "repo.edit-files")
            .expect("redacted inventory should include repo.edit-files");
        assert!(repo_edit_files.registered);
        assert_eq!(repo_edit_files.permission, PermissionLevel::Execute);
        assert_eq!(repo_edit_files.resources, ["git", "workspace"]);
        assert_eq!(repo_edit_files.validation, "validated");
        for name in [
            "repo.file-info",
            "repo.status",
            "repo.diff",
            "repo.diff-staged",
        ] {
            let observer = composition
                .effective_profile()
                .capabilities
                .iter()
                .find(|capability| capability.capability_id == name)
                .expect("redacted inventory should include repository observer");
            assert!(observer.registered);
            assert_eq!(observer.permission, PermissionLevel::Execute);
            assert_eq!(observer.resources, ["git", "workspace"]);
            assert_eq!(observer.validation, "validated");
        }
        assert!(
            !format!("{:?}", composition.effective_profile())
                .contains(directory.0.to_string_lossy().as_ref())
        );
        assert_eq!(
            fs::read(&target).expect("composition must not mutate the fixture target"),
            target_before
        );
        assert!(
            !fs::read_dir(&directory.0)
                .expect("fixture directory should remain readable")
                .any(|entry| {
                    entry
                        .expect("fixture directory entry should be readable")
                        .file_name()
                        .to_string_lossy()
                        .starts_with(".rah-repo-patch-")
                }),
            "composition must not leave repo.patch temporary files"
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
    async fn repo_patch_effective_composition_rejects_a_non_worktree_resource_without_mutation() {
        let directory = FixtureDirectory::new("repo-patch-non-root");
        let target = directory.0.join("target.txt");
        fs::write(&target, b"original\n").expect("fixture target should be written");
        let non_root = directory.0.join("nested");
        fs::create_dir(&non_root).expect("non-root fixture directory should be created");
        let profile_path = directory.0.join("trusted-profile.json");
        let document = json!({
            "profile_version": 1,
            "profile_id": "repo-patch-non-root",
            "resources": {
                "executables": {"git": {"path": git_executable(), "kind": "native"}},
                "repositories": {"non_root": {"path": non_root}}
            },
            "capabilities": [{
                "name": "repo.patch",
                "enabled": true,
                "permission": "execute",
                "executable": "git",
                "repository": "non_root"
            }]
        });
        fs::write(
            &profile_path,
            serde_json::to_vec(&document).expect("profile should serialize"),
        )
        .expect("profile should be written");
        let status_before = worktree_status(&directory.0);
        let target_before = fs::read(&target).expect("fixture target should be readable");

        let error = match compose(
            TrustedStaticProfile::load(&profile_path)
                .expect("static validation should not inspect the repository resource"),
        )
        .await
        {
            Ok(composition) => {
                composition.shutdown().await;
                panic!("effective composition must reject a resource outside the worktree root");
            }
            Err(error) => error,
        };

        assert_eq!(error, rah_tools::ProfileError::ConstructionFailed);
        assert_eq!(fs::read(&target).unwrap(), target_before);
        assert_eq!(worktree_status(&directory.0), status_before);
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
