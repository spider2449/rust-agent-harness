//! Host-selected native configuration, writable only across disconnected boundary.
use super::*;

#[derive(Default)]
pub(super) struct Configuration {
    pub adapter: Option<runtime_selection::ProductionAdapter>,
    pub model: Option<String>,
    pub endpoint: Option<String>,
}
impl DesktopAppState {
    pub(super) fn selected_runtime_adapter(&self) -> Option<runtime_selection::ProductionAdapter> {
        self.native_configuration
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .adapter
            .or(self.runtime_adapter)
    }
    pub(super) fn native_model(&self) -> Option<String> {
        let config = self
            .native_configuration
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if config.adapter.is_some() {
            return config.model.clone();
        }
        #[cfg(feature = "provider-llamacpp")]
        if self.runtime_adapter == Some(runtime_selection::ProductionAdapter::LlamaCpp) {
            return std::env::var("RAH_LLAMA_CPP_MODEL")
                .ok()
                .filter(|m| !m.trim().is_empty());
        }
        self.openai_configured_model.clone()
    }
    pub(super) fn llama_endpoint(&self) -> String {
        self.native_configuration
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .endpoint
            .clone()
            .or_else(|| std::env::var("RAH_LLAMA_CPP_ENDPOINT").ok())
            .unwrap_or_else(|| "http://127.0.0.1:8080".into())
    }
}

pub(super) fn configure(
    state: &DesktopAppState,
    provider: &str,
    model: Option<String>,
    endpoint: Option<String>,
) -> Result<(), FrontendError> {
    let _lifecycle = state
        .lifecycle_coordination
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if repository_index_effect_is_active(state) {
        return Err(FrontendError::RepositoryBusy);
    }
    if !matches!(
        *state
            .connection
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
        ConnectionState::NotConnected | ConnectionState::Error(_)
    ) || *state
        .chat
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        != ChatState::Idle
    {
        return Err(FrontendError::ModelConfigurationBusy);
    }
    let adapter = runtime_selection::select(Some(provider))
        .ok_or(FrontendError::RuntimeAdapterUnavailable)?;
    let model = model.filter(|m| !m.trim().is_empty());
    let invalid_model = model.as_deref().is_some_and(|m| {
        #[cfg(feature = "provider-llamacpp")]
        if adapter == runtime_selection::ProductionAdapter::LlamaCpp {
            return m.len() > 256 || m.chars().any(char::is_control);
        }
        validate_model_identifier(m).is_err()
    });
    if invalid_model {
        return Err(FrontendError::ModelConfigurationInvalid);
    }
    #[cfg(feature = "provider-llamacpp")]
    if adapter == runtime_selection::ProductionAdapter::LlamaCpp {
        use rah_runtime::experimental::ConfiguredRuntimeFactory;
        rah_runtime_openai::LlamaCppFactory::new(
            endpoint.as_deref().unwrap_or("http://127.0.0.1:8080"),
            None,
        )
        .validate()
        .map_err(|e| runtime_selection::frontend_failure(&e))?;
    }
    reserve_commit_revocation(state)?;
    *state
        .native_configuration
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Configuration {
        adapter: Some(adapter),
        model,
        endpoint,
    };
    state
        .model
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .generation += 1;
    model_source::configuration_changed(state);
    withdraw_commit_capability_and_workflow(state);
    Ok(())
}

#[tauri::command]
pub(super) fn set_native_configuration(
    state: State<'_, DesktopAppState>,
    provider: String,
    model: Option<String>,
    endpoint: Option<String>,
) -> Result<(), FrontendError> {
    configure(state.inner(), &provider, model, endpoint)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Presentation {
    provider: runtime_model_state::RuntimeAdapterIdentity,
    model: Option<String>,
    endpoint: String,
    credential_configured: bool,
    openai_available: bool,
    llamacpp_available: bool,
    codex_available: bool,
}
#[tauri::command]
pub(super) fn native_configuration(state: State<'_, DesktopAppState>) -> Presentation {
    Presentation {
        provider: runtime_model_state::identity(state.selected_runtime_adapter()),
        model: state.native_model(),
        endpoint: state.llama_endpoint(),
        credential_configured: std::env::var("OPENAI_API_KEY").is_ok_and(|k| !k.trim().is_empty()),
        openai_available: cfg!(feature = "provider-openai"),
        llamacpp_available: cfg!(feature = "provider-llamacpp"),
        codex_available: cfg!(feature = "provider-codex"),
    }
}
