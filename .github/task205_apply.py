from pathlib import Path


def replace_once(path, old, new):
    p = Path(path)
    text = p.read_text(encoding="utf-8")
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"expected one match in {path}, found {count}: {old[:100]!r}")
    p.write_text(text.replace(old, new, 1), encoding="utf-8")


def replace_between(path, start, end, replacement):
    p = Path(path)
    text = p.read_text(encoding="utf-8")
    a = text.find(start)
    if a < 0:
        raise SystemExit(f"start marker not found in {path}: {start!r}")
    b = text.find(end, a)
    if b < 0:
        raise SystemExit(f"end marker not found in {path}: {end!r}")
    p.write_text(text[:a] + replacement + text[b:], encoding="utf-8")

# Desktop gets the shared effective profile composer only at Task 205.
replace_once(
    "crates/rah-desktop/Cargo.toml",
    'futures.workspace = true\nrah-protocol = { path = "../rah-protocol" }\n',
    'futures.workspace = true\nrah-profile-composition = { path = "../rah-profile-composition" }\nrah-protocol = { path = "../rah-protocol" }\n',
)

# Fresh activation must load from the retained explicit source again; Task 204's
# previously parsed object is never reused as live authority.
replace_once(
    "crates/rah-desktop/src/trusted_profile_selection.rs",
    "use std::path::PathBuf;\n",
    "use std::path::{Path, PathBuf};\n",
)
replace_once(
    "crates/rah-desktop/src/trusted_profile_selection.rs",
    '''impl DesktopTrustedProfileSelection {\n    #[must_use]\n    pub(crate) fn presentation(&self) -> TrustedProfilePresentation {\n''',
    '''impl DesktopTrustedProfileSelection {\n    pub(crate) fn load_for_activation(\n        &self,\n    ) -> Result<TrustedStaticProfile, ProfileSelectionError> {\n        load_provider_only_static_profile(&self.source)\n    }\n\n    #[must_use]\n    pub(crate) fn presentation(&self) -> TrustedProfilePresentation {\n''',
)
replace_once(
    "crates/rah-desktop/src/trusted_profile_selection.rs",
    '''pub(crate) fn load_provider_only_profile(\n    source: PathBuf,\n) -> Result<DesktopTrustedProfileSelection, ProfileSelectionError> {\n    let profile =\n        TrustedStaticProfile::load(&source).map_err(|_| ProfileSelectionError::InvalidProfile)?;\n    if !profile.effective_profile().capabilities.is_empty() {\n        return Err(ProfileSelectionError::FirstPartyCapabilities);\n    }\n\n''',
    '''fn load_provider_only_static_profile(\n    source: &Path,\n) -> Result<TrustedStaticProfile, ProfileSelectionError> {\n    let profile =\n        TrustedStaticProfile::load(source).map_err(|_| ProfileSelectionError::InvalidProfile)?;\n    if !profile.effective_profile().capabilities.is_empty() {\n        return Err(ProfileSelectionError::FirstPartyCapabilities);\n    }\n    Ok(profile)\n}\n\npub(crate) fn load_provider_only_profile(\n    source: PathBuf,\n) -> Result<DesktopTrustedProfileSelection, ProfileSelectionError> {\n    let profile = load_provider_only_static_profile(&source)?;\n\n''',
)

main = "crates/rah-desktop/src/main.rs"
replace_once(
    main,
    '''#[cfg(target_os = "windows")]\nmod git_discovery;\n#[cfg(target_os = "windows")]\nmod trusted_profile_selection;\n''',
    '''#[cfg(target_os = "windows")]\nmod git_discovery;\n#[cfg(target_os = "windows")]\nmod provider_composition;\n#[cfg(target_os = "windows")]\nmod trusted_profile_selection;\n''',
)
replace_once(
    main,
    '''#[cfg(target_os = "windows")]\nuse rah_protocol::{\n''',
    '''#[cfg(target_os = "windows")]\nuse provider_composition::{\n    DesktopProviderActivation, ProviderActivationError, desktop_allowed_permissions,\n    merge_tool_registries,\n};\n#[cfg(target_os = "windows")]\nuse rah_protocol::{\n''',
)
replace_once(
    main,
    '''        composition: Arc<DesktopToolComposition>,\n    },\n''',
    '''        composition: Arc<DesktopToolComposition>,\n        profile_generation: u64,\n    },\n''',
)
replace_once(
    main,
    '''    /// Explicit host-selected Trusted Profile intent. Static selection never activates providers.\n    trusted_profile: Mutex<Option<DesktopTrustedProfileSelection>>,\n    /// An app-owned non-project directory used only when no repository is selected.\n''',
    '''    /// Explicit host-selected Trusted Profile intent. Static selection never activates providers.\n    trusted_profile: Mutex<Option<DesktopTrustedProfileSelection>>,\n    trusted_profile_generation: Mutex<u64>,\n    /// One effective provider composition owned by the currently published connection.\n    /// Kept outside `ConnectionState` so hard recovery can asynchronously reap providers\n    /// after synchronously withdrawing the usable runtime state.\n    provider_activation: Mutex<Option<DesktopProviderActivation>>,\n    /// An app-owned non-project directory used only when no repository is selected.\n''',
)
replace_once(
    main,
    '''            trusted_profile: Mutex::new(None),\n            neutral_workspace,\n''',
    '''            trusted_profile: Mutex::new(None),\n            trusted_profile_generation: Mutex::new(0),\n            provider_activation: Mutex::new(None),\n            neutral_workspace,\n''',
)
replace_once(
    main,
    '''    ProfileFirstPartyCapabilitiesUnsupported,\n    ProfileDialogFailed,\n    ProfileBusy,\n''',
    '''    ProfileFirstPartyCapabilitiesUnsupported,\n    ProfileActivationFailed,\n    ProfileDialogFailed,\n    ProfileBusy,\n''',
)

# App status remains a bounded summary: selected != activating != active.
replace_once(
    main,
    '''        let profile_selected = self\n            .trusted_profile\n            .lock()\n            .unwrap_or_else(std::sync::PoisonError::into_inner)\n            .is_some();\n        let mut status = current_app_status(\n            &connection,\n            repository_selected,\n            repository_generation,\n            model_generation,\n        );\n        status.profile_status = if profile_selected {\n            "configured; providers inactive"\n        } else {\n            "not loaded"\n        };\n''',
    '''        let profile_selected = self\n            .trusted_profile\n            .lock()\n            .unwrap_or_else(std::sync::PoisonError::into_inner)\n            .is_some();\n        let provider_active = self\n            .provider_activation\n            .lock()\n            .unwrap_or_else(std::sync::PoisonError::into_inner)\n            .is_some();\n        let mut status = current_app_status(\n            &connection,\n            repository_selected,\n            repository_generation,\n            model_generation,\n        );\n        status.profile_status = match (&*connection, profile_selected, provider_active) {\n            (ConnectionState::Connected { .. }, true, true) => "active",\n            (ConnectionState::Connecting, true, _) => "activating",\n            (_, true, _) => "configured; providers inactive",\n            _ => "not loaded",\n        };\n''',
)

# Exit and hard recovery must reap provider children explicitly and asynchronously.
replace_between(
    main,
    '''    async fn shutdown_for_exit(&self) {\n''',
    '''    fn begin_hard_recovery(&self, runtime: &Arc<CodexRuntime>) -> bool {\n''',
    '''    async fn shutdown_for_exit(&self) {\n        let runtime = {\n            let mut connection = self\n                .connection\n                .lock()\n                .unwrap_or_else(std::sync::PoisonError::into_inner);\n            match std::mem::replace(&mut *connection, ConnectionState::Disconnecting) {\n                ConnectionState::Connected { runtime, .. } => Some(runtime),\n                state => {\n                    *connection = state;\n                    None\n                }\n            }\n        };\n        if let Some(runtime) = runtime\n            && let Err(error) = runtime.shutdown().await\n        {\n            tracing::warn!(error = %error, "failed to shut down Codex during desktop exit");\n        }\n        self.shutdown_provider_activation().await;\n    }\n\n    async fn shutdown_provider_activation(&self) {\n        let activation = self\n            .provider_activation\n            .lock()\n            .unwrap_or_else(std::sync::PoisonError::into_inner)\n            .take();\n        if let Some(activation) = activation {\n            activation.shutdown().await;\n        }\n    }\n\n''',
)

# Selection changes become explicit generations. They remain disconnected-only.
replace_once(
    main,
    '''    *state\n        .trusted_profile\n        .lock()\n        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(selection);\n    Ok(())\n}\n''',
    '''    *state\n        .trusted_profile\n        .lock()\n        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(selection);\n    let mut generation = state\n        .trusted_profile_generation\n        .lock()\n        .unwrap_or_else(std::sync::PoisonError::into_inner);\n    *generation = generation.wrapping_add(1);\n    Ok(())\n}\n''',
)
replace_once(
    main,
    '''    *state\n        .trusted_profile\n        .lock()\n        .unwrap_or_else(std::sync::PoisonError::into_inner) = None;\n    Ok(())\n}\n''',
    '''    let changed = state\n        .trusted_profile\n        .lock()\n        .unwrap_or_else(std::sync::PoisonError::into_inner)\n        .take()\n        .is_some();\n    if changed {\n        let mut generation = state\n            .trusted_profile_generation\n            .lock()\n            .unwrap_or_else(std::sync::PoisonError::into_inner);\n        *generation = generation.wrapping_add(1);\n    }\n    Ok(())\n}\n''',
)

# Separate activation-publication currentness closes the pre-existing model race
# and includes the profile selection generation without changing old helpers.
replace_once(
    main,
    '''fn connection_publication_is_current(\n    captured_repository_generation: u64,\n    current_repository_generation: u64,\n    captured_connection_generation: u64,\n    current_connection_generation: u64,\n) -> bool {\n    captured_repository_generation == current_repository_generation\n        && captured_connection_generation == current_connection_generation\n}\n\n''',
    '''fn connection_publication_is_current(\n    captured_repository_generation: u64,\n    current_repository_generation: u64,\n    captured_connection_generation: u64,\n    current_connection_generation: u64,\n) -> bool {\n    captured_repository_generation == current_repository_generation\n        && captured_connection_generation == current_connection_generation\n}\n\n#[cfg(target_os = "windows")]\nfn connection_activation_publication_is_current(\n    captured_repository_generation: u64,\n    current_repository_generation: u64,\n    captured_model_generation: u64,\n    current_model_generation: u64,\n    captured_profile_generation: u64,\n    current_profile_generation: u64,\n    captured_connection_generation: u64,\n    current_connection_generation: u64,\n) -> bool {\n    captured_repository_generation == current_repository_generation\n        && captured_model_generation == current_model_generation\n        && captured_profile_generation == current_profile_generation\n        && captured_connection_generation == current_connection_generation\n}\n\n''',
)

# Keep the existing no-external helper for old tests; Connect uses the new final-registry path.
replace_between(
    main,
    '''#[cfg(target_os = "windows")]\nfn desktop_tool_composition(\n''',
    '''/// Resolves host executable selection and combines it with the already chosen,\n''',
    '''#[cfg(target_os = "windows")]\nfn desktop_tool_composition_from_registry(\n    registry: Arc<ToolRegistry>,\n    repository: Option<&DesktopRepository>,\n    commit_tool_present: bool,\n) -> Arc<DesktopToolComposition> {\n    Arc::new(effective_authority::compose(\n        registry,\n        repository,\n        commit_tool_present,\n    ))\n}\n\n#[cfg(target_os = "windows")]\nfn desktop_tool_composition(\n    repository: Option<&DesktopRepository>,\n    commit_tool: Option<Arc<RepositoryCommitTool>>,\n) -> Result<Arc<DesktopToolComposition>, ToolError> {\n    let registry = desktop_tool_registry(repository, commit_tool.clone())?;\n    Ok(desktop_tool_composition_from_registry(\n        registry,\n        repository,\n        commit_tool.is_some(),\n    ))\n}\n\n''',
)

connect = r'''#[cfg(target_os = "windows")]
#[tauri::command]
async fn connect_codex(
    state: State<'_, DesktopAppState>,
) -> Result<ConnectionResult, FrontendError> {
    {
        let mut connection = state
            .connection
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match request_connect(&mut connection) {
            ConnectRequest::AlreadyConnected => return Ok(ConnectionResult::connected()),
            ConnectRequest::InProgress => return Ok(ConnectionResult::connecting()),
            ConnectRequest::Start => {}
        }
    }

    let connection_generation = {
        let mut generation = state
            .next_connection_generation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *generation = generation.wrapping_add(1);
        *generation
    };

    let (repository, repository_generation, neutral_workspace) = {
        let repository = state
            .repository
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        let generation = *state
            .repository_generation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        (repository, generation, state.neutral_workspace.clone())
    };
    let (model_config, model_generation) = {
        let model = state
            .model
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match model.selection.codex_model_config() {
            Ok(config) => (config, model.generation),
            Err(error) => {
                let mut connection = state
                    .connection
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                *connection = ConnectionState::Error(error);
                return Err(error);
            }
        }
    };
    let (profile_selection, profile_generation) = {
        let selection = state
            .trusted_profile
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        let generation = *state
            .trusted_profile_generation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        (selection, generation)
    };
    let identity = state
        .commit_identity
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    let identity_generation = *state
        .commit_identity_generation
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let commit_capability = match (repository.as_deref(), identity) {
        (Some(repository), Some(identity)) => RepositoryCommitTool::compose(
            &repository.git_executable,
            &repository.root,
            identity.name,
            identity.email,
        )
        .ok()
        .map(|(tool, control)| (Arc::new(tool), Arc::new(control))),
        _ => None,
    };
    let commit_tool = commit_capability.as_ref().map(|(tool, _)| Arc::clone(tool));
    let first_party_registry = match desktop_tool_registry(repository.as_deref(), commit_tool.clone()) {
        Ok(registry) => registry,
        Err(error) => {
            tracing::error!(error = %error, "failed to construct Desktop first-party registry");
            let frontend_error = FrontendError::ToolRegistryFailed;
            *state
                .connection
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) =
                ConnectionState::Error(frontend_error);
            return Err(frontend_error);
        }
    };

    let mut provider_activation = match profile_selection.as_ref() {
        Some(selection) => match DesktopProviderActivation::activate(selection).await {
            Ok(activation) => Some(activation),
            Err(error) => {
                let frontend_error = match error {
                    ProviderActivationError::Profile(error) => profile_selection_error(error),
                    ProviderActivationError::ProviderUnavailable => {
                        FrontendError::ProfileActivationFailed
                    }
                };
                tracing::warn!(reason = ?error, "Desktop provider activation failed");
                *state
                    .connection
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) =
                    ConnectionState::Error(frontend_error);
                return Err(frontend_error);
            }
        },
        None => None,
    };
    let registry = match merge_tool_registries(
        first_party_registry.as_ref(),
        provider_activation.as_ref().map(DesktopProviderActivation::registry),
    ) {
        Ok(registry) => registry,
        Err(error) => {
            tracing::warn!(error = %error, "Desktop final Tool registry merge failed");
            if let Some(activation) = provider_activation.take() {
                activation.shutdown().await;
            }
            let frontend_error = FrontendError::ToolRegistryFailed;
            *state
                .connection
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) =
                ConnectionState::Error(frontend_error);
            return Err(frontend_error);
        }
    };
    let allowed_permissions = desktop_allowed_permissions(
        repository.is_some(),
        provider_activation
            .as_ref()
            .map_or(&[], DesktopProviderActivation::permissions),
    );
    let composition = desktop_tool_composition_from_registry(
        Arc::clone(&registry),
        repository.as_deref(),
        commit_tool.is_some(),
    );
    let repository_fingerprint = repository
        .as_ref()
        .map(|value| repository_context_fingerprint(&value.root));
    let connection_repository_fingerprint = repository_fingerprint.clone();
    let selected_profile = profile_selection.is_some();
    match resolve_prepare_and_connect_codex(
        resolve_codex_executable,
        model_config,
        |prepared| async move {
            append_live_evidence(serde_json::json!({
                "event": "connection_started",
                "repository_generation": repository_generation,
                "repository_fingerprint": connection_repository_fingerprint,
                "model_generation": model_generation,
                "profile_generation": profile_generation,
                "connection_generation": connection_generation,
                "selected_repository": repository.is_some(),
                "selected_profile": selected_profile,
                "deletion_authority_present": repository
                    .as_ref()
                    .is_some_and(|value| value.deletion_authority.is_some()),
                "rename_authority_present": repository
                    .as_ref()
                    .is_some_and(|value| value.rename_authority.is_some()),
                "bridge_enabled": true,
            }));
            CodexRuntime::connect_tool_bridge_with_model_config_and_workspace(
                prepared.executable,
                registry,
                allowed_permissions,
                prepared.model_config,
                if let Some(repository) = repository.as_deref() {
                    repository.root.as_path()
                } else {
                    neutral_workspace
                        .as_deref()
                        .ok_or(FrontendError::CodexConnectionFailed)?
                },
            )
            .await
            .map_err(|error| {
                tracing::warn!(error = %error, "Codex desktop connection failed");
                frontend_error(&error)
            })
        },
    )
    .await
    {
        Ok((runtime, source)) => {
            let runtime = Arc::new(runtime);
            let published_fingerprint = repository_fingerprint.clone();
            let current_repository_generation = *state
                .repository_generation
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let current_model_generation = state
                .model
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .generation;
            let current_profile_generation = *state
                .trusted_profile_generation
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let current_connection_generation = *state
                .next_connection_generation
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let stale = !connection_activation_publication_is_current(
                repository_generation,
                current_repository_generation,
                model_generation,
                current_model_generation,
                profile_generation,
                current_profile_generation,
                connection_generation,
                current_connection_generation,
            );
            if stale {
                append_live_evidence(serde_json::json!({
                    "event": "connection_publication_rejected_stale",
                    "captured_repository_generation": repository_generation,
                    "current_repository_generation": current_repository_generation,
                    "captured_model_generation": model_generation,
                    "current_model_generation": current_model_generation,
                    "captured_profile_generation": profile_generation,
                    "current_profile_generation": current_profile_generation,
                    "connection_generation": connection_generation,
                }));
                if let Err(error) = runtime.shutdown().await {
                    tracing::warn!(error = %error, "stale Codex desktop runtime shutdown failed");
                }
                if let Some(activation) = provider_activation.take() {
                    activation.shutdown().await;
                }
                let mut connection = state
                    .connection
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                if matches!(*connection, ConnectionState::Connecting) {
                    *connection = ConnectionState::NotConnected;
                }
                return Err(FrontendError::CodexReconnectRequired);
            }

            let mut connection = state
                .connection
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if !matches!(*connection, ConnectionState::Connecting) {
                drop(connection);
                if let Err(error) = runtime.shutdown().await {
                    tracing::warn!(error = %error, "superseded Codex runtime shutdown failed");
                }
                if let Some(activation) = provider_activation.take() {
                    activation.shutdown().await;
                }
                return Err(FrontendError::CodexReconnectRequired);
            }
            let mut published_provider = state
                .provider_activation
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if published_provider.is_some() {
                drop(published_provider);
                drop(connection);
                if let Err(error) = runtime.shutdown().await {
                    tracing::warn!(error = %error, "duplicate provider owner runtime shutdown failed");
                }
                if let Some(activation) = provider_activation.take() {
                    activation.shutdown().await;
                }
                let frontend_error = FrontendError::ProfileActivationFailed;
                *state
                    .connection
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) =
                    ConnectionState::Error(frontend_error);
                return Err(frontend_error);
            }
            *published_provider = provider_activation.take();
            *connection = ConnectionState::Connected {
                runtime,
                source,
                repository_generation,
                model_generation,
                connection_generation,
                repository_fingerprint,
                composition: Arc::clone(&composition),
                profile_generation,
            };
            drop(published_provider);
            drop(connection);
            append_live_evidence(serde_json::json!({
                "event": "connection_published",
                "repository_generation": repository_generation,
                "repository_fingerprint": published_fingerprint,
                "model_generation": model_generation,
                "profile_generation": profile_generation,
                "connection_generation": connection_generation,
                "profile_active": selected_profile,
            }));
            if let Some((tool, control)) = commit_capability {
                *state
                    .commit_capability
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) =
                    Some(DesktopCommitCapability {
                        repository_generation,
                        model_generation,
                        identity_generation,
                        _tool: tool,
                        control,
                    });
            }
            Ok(ConnectionResult::connected())
        }
        Err(frontend_error) => {
            if let Some(activation) = provider_activation.take() {
                activation.shutdown().await;
            }
            let mut connection = state
                .connection
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            *connection = ConnectionState::Error(frontend_error);
            Err(frontend_error)
        }
    }
}

'''
replace_between(
    main,
    '''#[cfg(target_os = "windows")]\n#[tauri::command]\nasync fn connect_codex(\n''',
    '''#[cfg(target_os = "windows")]\n#[tauri::command]\nasync fn disconnect_codex(\n''',
    connect,
)

disconnect = r'''#[cfg(target_os = "windows")]
#[tauri::command]
async fn disconnect_codex(
    state: State<'_, DesktopAppState>,
) -> Result<ConnectionResult, FrontendError> {
    if *state
        .chat
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        != ChatState::Idle
    {
        return Err(FrontendError::ChatAlreadyRunning);
    }
    let runtime = {
        revoke_repository_commit_context(state.inner()).await;
        let mut connection = state
            .connection
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match std::mem::replace(&mut *connection, ConnectionState::Disconnecting) {
            ConnectionState::Connected { runtime, .. } => Some(runtime),
            previous => {
                *connection = previous;
                None
            }
        }
    };

    let activation = state
        .provider_activation
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .take();
    let Some(runtime) = runtime else {
        if let Some(activation) = activation {
            activation.shutdown().await;
        }
        return Ok(ConnectionResult::not_connected());
    };

    let runtime_result = runtime.shutdown().await;
    if let Some(activation) = activation {
        activation.shutdown().await;
    }
    if let Err(error) = runtime_result {
        tracing::warn!(error = %error, "Codex desktop disconnection failed");
        let frontend_error = frontend_error(&error);
        let mut connection = state
            .connection
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *connection = ConnectionState::Error(frontend_error);
        return Err(frontend_error);
    }

    let mut connection = state
        .connection
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    *connection = ConnectionState::NotConnected;
    Ok(ConnectionResult::not_connected())
}

'''
replace_between(
    main,
    '''#[cfg(target_os = "windows")]\n#[tauri::command]\nasync fn disconnect_codex(\n''',
    '''#[cfg(target_os = "windows")]\nfn validate_prompt(prompt: &str) -> Result<(), FrontendError> {\n''',
    disconnect,
)

replace_once(
    main,
    '''        CancelRecoveryOutcome::Hard(hard) => {\n            if hard != HardShutdownOutcome::Completed {\n                tracing::warn!("bounded hard Codex shutdown did not complete successfully");\n            }\n            state.finish_hard_recovery(hard == HardShutdownOutcome::Completed);\n''',
    '''        CancelRecoveryOutcome::Hard(hard) => {\n            if hard != HardShutdownOutcome::Completed {\n                tracing::warn!("bounded hard Codex shutdown did not complete successfully");\n            }\n            state.shutdown_provider_activation().await;\n            state.finish_hard_recovery(hard == HardShutdownOutcome::Completed);\n''',
)

# Add a focused currentness regression test without disturbing legacy helper tests.
replace_once(
    main,
    '''    #[test]\n    fn stale_connection_publication_cannot_match_a_new_repository_generation() {\n        assert!(connection_publication_is_current(4, 4, 9, 9));\n        assert!(!connection_publication_is_current(4, 5, 9, 9));\n        assert!(!connection_publication_is_current(4, 4, 9, 10));\n    }\n\n''',
    '''    #[test]\n    fn stale_connection_publication_cannot_match_a_new_repository_generation() {\n        assert!(connection_publication_is_current(4, 4, 9, 9));\n        assert!(!connection_publication_is_current(4, 5, 9, 9));\n        assert!(!connection_publication_is_current(4, 4, 9, 10));\n    }\n\n    #[test]\n    fn activation_publication_requires_repository_model_profile_and_connection_currentness() {\n        assert!(super::connection_activation_publication_is_current(\n            4, 4, 5, 5, 6, 6, 9, 9\n        ));\n        assert!(!super::connection_activation_publication_is_current(\n            4, 5, 5, 5, 6, 6, 9, 9\n        ));\n        assert!(!super::connection_activation_publication_is_current(\n            4, 4, 5, 7, 6, 6, 9, 9\n        ));\n        assert!(!super::connection_activation_publication_is_current(\n            4, 4, 5, 5, 6, 8, 9, 9\n        ));\n        assert!(!super::connection_activation_publication_is_current(\n            4, 4, 5, 5, 6, 6, 9, 10\n        ));\n    }\n\n''',
)

# Bounded frontend error copy only; no provider paths, stderr, or protocol details.
replace_once(
    "crates/rah-desktop/frontend/status.js",
    '''    profile_first_party_capabilities_unsupported: "Desktop v0.17 accepts provider-only Trusted Profiles; remove first-party capabilities",\n    profile_dialog_failed: "Trusted Profile picker failed",\n''',
    '''    profile_first_party_capabilities_unsupported: "Desktop v0.17 accepts provider-only Trusted Profiles; remove first-party capabilities",\n    profile_activation_failed: "Trusted Profile providers could not be activated",\n    profile_dialog_failed: "Trusted Profile picker failed",\n''',
)
