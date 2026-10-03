//! Codex-only admission and production connection composition.
use super::*;
use crate::codex_baseline::CodexExecutableSource;
pub(super) use rah_runtime_codex::{
    CodexAdapterError, CodexLlamaCppProvider, CodexModelConfig, CodexModelProvider,
    CodexModelSelection,
};

#[cfg(target_os = "windows")]
pub(super) fn frontend_error(error: &CodexAdapterError) -> FrontendError {
    match error {
        CodexAdapterError::CatalogDeadline => FrontendError::ModelCatalogUnavailable,
        CodexAdapterError::WorkspaceContext { .. }
        | CodexAdapterError::InvalidModelProviderConfig { .. } => {
            FrontendError::CodexConnectionFailed
        }
        CodexAdapterError::ExecutableDiscovery { .. } => FrontendError::CodexNotFound,
        CodexAdapterError::VersionMismatch { .. } => FrontendError::UnsupportedCodexVersion,
        CodexAdapterError::SchemaInspection { .. } | CodexAdapterError::SchemaMismatch { .. } => {
            FrontendError::CodexSchemaIncompatible
        }
        CodexAdapterError::ProcessStartup { .. } => FrontendError::CodexStartFailed,
        CodexAdapterError::ProcessExited { .. }
        | CodexAdapterError::MalformedFraming { .. }
        | CodexAdapterError::JsonRpc { .. }
        | CodexAdapterError::SharedFailure { .. }
        | CodexAdapterError::TurnFailed { .. }
        | CodexAdapterError::ProtocolViolation { .. }
        | CodexAdapterError::Transport { .. } => FrontendError::CodexConnectionFailed,
    }
}

#[cfg(target_os = "windows")]
#[derive(Debug)]
pub(super) struct PreparedCodexConnection {
    pub(super) executable: std::ffi::OsString,
    pub(super) model_config: CodexModelConfig,
    pub(super) source: CodexExecutableSource,
}

#[cfg(target_os = "windows")]
pub(super) fn prepare_codex_connection<R>(
    resolver: R,
    model_config: CodexModelConfig,
) -> Result<PreparedCodexConnection, FrontendError>
where
    R: FnOnce() -> Result<codex_baseline::CodexExecutableSelection, BaselineError>,
{
    #[cfg(test)]
    if startup_counter_tracking() {
        startup_activation_counters()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .codex_resolver += 1;
    }
    let selection = match resolver() {
        Ok(selection) => selection,
        Err(BaselineError::Invalid) => return Err(FrontendError::CodexBaselineInvalid),
        Err(BaselineError::UnsupportedHost) => return Err(FrontendError::CodexHostUnsupported),
    };
    Ok(PreparedCodexConnection {
        executable: selection.executable,
        model_config,
        source: selection.source,
    })
}

/// Passes one prepared host-owned connection value through the sole Desktop
/// runtime-construction seam.
#[cfg(target_os = "windows")]
pub(super) async fn connect_prepared_codex<T, F, Fut>(
    prepared: PreparedCodexConnection,
    runtime_factory: F,
) -> Result<(T, CodexExecutableSource), FrontendError>
where
    F: FnOnce(PreparedCodexConnection) -> Fut,
    Fut: Future<Output = Result<T, FrontendError>>,
{
    #[cfg(test)]
    if startup_counter_tracking() {
        startup_activation_counters()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .codex_runtime_construction += 1;
    }
    let source = prepared.source;
    let runtime = runtime_factory(prepared).await?;
    Ok((runtime, source))
}

#[cfg(target_os = "windows")]
pub(super) async fn resolve_prepare_and_connect_codex<T, R, F, Fut>(
    resolver: R,
    model_config: CodexModelConfig,
    runtime_factory: F,
) -> Result<(T, CodexExecutableSource), FrontendError>
where
    R: FnOnce() -> Result<codex_baseline::CodexExecutableSelection, BaselineError>,
    F: FnOnce(PreparedCodexConnection) -> Fut,
    Fut: Future<Output = Result<T, FrontendError>>,
{
    let prepared = prepare_codex_connection(resolver, model_config)?;
    connect_prepared_codex(prepared, runtime_factory).await
}

#[cfg(target_os = "windows")]
pub(super) async fn connect_selected(
    state: State<'_, DesktopAppState>,
) -> Result<ConnectionResult, FrontendError> {
    match begin_connect(state.inner())? {
        ConnectRequest::AlreadyConnected => return Ok(ConnectionResult::connected()),
        ConnectRequest::InProgress => return Ok(ConnectionResult::connecting()),
        ConnectRequest::Start => {}
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
        .map(|(tool, control)| DesktopCommitCapability {
            repository_generation,
            model_generation,
            identity_generation,
            _tool: Arc::new(tool),
            control: Arc::new(control),
        }),
        _ => None,
    };
    let commit_tool = commit_capability
        .as_ref()
        .map(|capability| Arc::clone(&capability._tool));
    let first_party_registry =
        match desktop_tool_registry(repository.as_deref(), commit_tool.clone()) {
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
        provider_activation
            .as_ref()
            .map(DesktopProviderActivation::registry),
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
    let retained_allowed_permissions = allowed_permissions.clone();
    let external_descriptors = provider_activation
        .as_ref()
        .map_or_else(Vec::new, |activation| activation.external_tools().to_vec());
    let composition = match desktop_tool_composition_from_registry(
        Arc::clone(&registry),
        repository.as_deref(),
        commit_tool.is_some(),
        &external_descriptors,
    ) {
        Ok(composition) => composition,
        Err(_) => {
            tracing::warn!("Desktop authority classification failed closed");
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
    let repository_fingerprint = repository
        .as_ref()
        .map(|value| repository_context_fingerprint(&value.root));
    let connection_repository_fingerprint = repository_fingerprint.clone();
    let selected_model = match &model_config {
        CodexModelConfig::Explicit(selection) => Some(selection.model().to_owned()),
        CodexModelConfig::Inherit => None,
    };
    let preflight_state = state.inner();
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
                "identity_generation": identity_generation,
                "selected_repository": repository.is_some(),
                "selected_profile": selected_profile,
                "deletion_authority_present": repository
                    .as_ref()
                    .is_some_and(|value| value.deletion_authority.is_some()),
                "rename_authority_present": repository
                    .as_ref()
                    .is_some_and(|value| value.rename_authority.is_some()),
                "branch_creation_authority_present": repository
                    .as_ref()
                    .is_some_and(|value| value.branch_creation_authority.is_some()),
                "bridge_enabled": true,
            }));
            let workspace = if let Some(repository) = repository.as_deref() {
                repository.root.as_path()
            } else {
                neutral_workspace.as_deref().ok_or(FrontendError::CodexConnectionFailed)?
            };
            let (factory, model) = configured_codex_factory(
                PathBuf::from(prepared.executable), prepared.model_config, workspace,
            ).map_err(|error| runtime_frontend_error(&error))?;
            let (instance, preflight) = runtime_composition::create_and_preflight(&factory, &model)
                .await.map_err(|error| runtime_frontend_error(&error))?;
            let (presentation, gate) = model_preflight::present(selected_model, preflight);
            append_live_evidence(serde_json::json!({
                "event": "model_preflight",
                "connection_generation": connection_generation,
                "model_generation": model_generation,
                "observation": presentation.presentation,
            }));
            if let Some(failure) = &presentation.failure {
                tracing::warn!(failure = ?failure, "model catalog observation failed");
            }
            *preflight_state.model_preflight.lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) =
                Some((connection_generation, model_generation, presentation));
            if let Err(error) = gate {
                if let Err(shutdown_error) = instance.shutdown().await {
                    tracing::warn!(failure = ?shutdown_error, "preflight-rejected runtime shutdown failed");
                }
                return Err(error);
            }
            runtime_composition::bind_conversation(instance, model, registry, allowed_permissions)
                .await.map_err(|error| runtime_frontend_error(&error))
        },
    )
    .await
    {
        Ok((runtime, source)) => {
            let runtime = Arc::new(runtime);
            connect_pre_publication_barrier(state.inner());
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
            let current_identity_generation = *state
                .commit_identity_generation
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let stale = !connection_publication_is_current(
                ConnectionPublicationCurrentness {
                    repository_generation,
                    model_generation,
                    profile_generation,
                    connection_generation,
                    identity_generation,
                },
                ConnectionPublicationCurrentness {
                    repository_generation: current_repository_generation,
                    model_generation: current_model_generation,
                    profile_generation: current_profile_generation,
                    connection_generation: current_connection_generation,
                    identity_generation: current_identity_generation,
                },
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
                    "captured_identity_generation": identity_generation,
                    "current_identity_generation": current_identity_generation,
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

            let pending = PendingConnectedPublication {
                runtime,
                activation: provider_activation.take(),
                source,
                repository_generation,
                model_generation,
                profile_generation,
                connection_generation,
                identity_generation,
                repository_fingerprint,
                composition: Arc::clone(&composition),
                allowed_permissions: retained_allowed_permissions,
                commit_capability,
            };
            if let Err(rejected) = publish_connected_provider_state(state.inner(), pending) {
                let RejectedProviderPublication {
                    runtime,
                    activation,
                    reason,
                } = *rejected;
                if let Err(error) = runtime.shutdown().await {
                    tracing::warn!(error = %error, "rejected Codex runtime shutdown failed");
                }
                if let Some(activation) = activation {
                    activation.shutdown().await;
                }
                return match reason {
                    ProviderPublicationRejectionReason::Superseded
                    | ProviderPublicationRejectionReason::Stale => {
                        let mut connection = state
                            .connection
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner);
                        if matches!(*connection, ConnectionState::Connecting) {
                            *connection = ConnectionState::NotConnected;
                        }
                        Err(FrontendError::CodexReconnectRequired)
                    }
                    ProviderPublicationRejectionReason::IndexEffectActive => {
                        let mut connection = state
                            .connection
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner);
                        if matches!(*connection, ConnectionState::Connecting) {
                            *connection = ConnectionState::NotConnected;
                        }
                        Err(FrontendError::RepositoryBusy)
                    }
                    ProviderPublicationRejectionReason::DuplicateOwner => {
                        state.shutdown_provider_activation().await;
                        let frontend_error = FrontendError::ProfileActivationFailed;
                        let mut connection = state
                            .connection
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner);
                        if matches!(*connection, ConnectionState::Connecting) {
                            *connection = ConnectionState::Error(frontend_error);
                        }
                        Err(frontend_error)
                    }
                };
            }
            append_live_evidence(serde_json::json!({
                "event": "connection_published",
                "repository_generation": repository_generation,
                "repository_fingerprint": published_fingerprint,
                "model_generation": model_generation,
                "profile_generation": profile_generation,
                "connection_generation": connection_generation,
                "profile_active": selected_profile,
            }));
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

use rah_runtime::{RuntimeFailure, experimental::ModelSelection};
use rah_runtime_codex::experimental::CodexFactory;
/// Adapter configuration is confined to the production composition root.
pub(crate) fn configured_codex_factory(
    executable: std::path::PathBuf,
    config: CodexModelConfig,
    workspace: &std::path::Path,
) -> Result<(CodexFactory, ModelSelection), RuntimeFailure> {
    let (provider, model) = match config {
        CodexModelConfig::Inherit => (CodexModelProvider::OpenAi, ModelSelection::RuntimeDefault),
        CodexModelConfig::Explicit(selection) => (
            selection.provider().clone(),
            ModelSelection::Explicit(selection.model().to_owned()),
        ),
    };
    Ok((
        CodexFactory::new(executable, provider).with_workspace(workspace)?,
        model,
    ))
}

impl DesktopModelSelection {
    pub(super) fn codex_model_config(&self) -> Result<CodexModelConfig, FrontendError> {
        self.validate()?;
        let provider = match self.provider {
            DesktopModelProvider::Inherit => {
                return if self.model.is_none() && self.llama_cpp_endpoint.is_none() {
                    Ok(CodexModelConfig::Inherit)
                } else {
                    Err(FrontendError::ModelConfigurationInvalid)
                };
            }
            DesktopModelProvider::OpenAi if self.llama_cpp_endpoint.is_none() => {
                CodexModelProvider::OpenAi
            }
            DesktopModelProvider::Ollama if self.llama_cpp_endpoint.is_none() => {
                CodexModelProvider::Ollama
            }
            DesktopModelProvider::LmStudio if self.llama_cpp_endpoint.is_none() => {
                CodexModelProvider::LmStudio
            }
            DesktopModelProvider::LlamaCpp => {
                let endpoint = self
                    .llama_cpp_endpoint
                    .as_ref()
                    .ok_or(FrontendError::ModelConfigurationInvalid)?;
                CodexModelProvider::LlamaCpp(
                    CodexLlamaCppProvider::new(endpoint.base_url(), None)
                        .map_err(|_| FrontendError::ModelConfigurationInvalid)?,
                )
            }
            _ => return Err(FrontendError::ModelConfigurationInvalid),
        };
        let model = self
            .model
            .as_deref()
            .ok_or(FrontendError::ModelConfigurationInvalid)?;
        CodexModelSelection::new(model, provider)
            .map(CodexModelConfig::Explicit)
            .map_err(|_| FrontendError::ModelConfigurationInvalid)
    }
}
