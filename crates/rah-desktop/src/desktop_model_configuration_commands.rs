use super::{
    ConnectionState, DesktopAppState, ModelConfigurationPresentation, ProviderEndpoint,
    ProviderEndpointPresentation, model_configuration_status,
};
use tauri::{AppHandle, Manager};

#[cfg(target_os = "windows")]
#[tauri::command]
pub(super) async fn model_configuration(app: AppHandle) -> ModelConfigurationPresentation {
    // The command future owns the handle; no invoke-borrowed State escapes.
    model_configuration_for_state(app.state::<DesktopAppState>().inner()).await
}

pub(super) async fn model_configuration_for_state(
    state: &DesktopAppState,
) -> ModelConfigurationPresentation {
    super::model_source::refresh(state, false).await;
    // Capture configuration and source together after asynchronous resolution.
    let _lifecycle = state
        .lifecycle_coordination
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let model_source = super::model_source::current(state);
    let (selection, generation, readiness) = {
        let model = state
            .model
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        (model.selection.clone(), model.generation, model.readiness)
    };
    let connection = state
        .connection
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    ModelConfigurationPresentation {
        model_source,
        model_selection_mode: selection.model_selection_mode,
        runtime_selection: super::runtime_model_state::present(
            state.selected_runtime_adapter(),
            &selection,
            state.native_model().as_deref(),
        ),
        provider: selection.provider,
        model: selection.model,
        endpoint: selection
            .llama_cpp_endpoint
            .as_ref()
            .map(ProviderEndpointPresentation::from),
        insecure_transport: selection
            .llama_cpp_endpoint
            .as_ref()
            .is_some_and(ProviderEndpoint::insecure_transport),
        readiness,
        status: model_configuration_status(
            match &*connection {
                ConnectionState::Connected {
                    model_generation, ..
                } => Some(*model_generation),
                _ => None,
            },
            generation,
        ),
    }
}
