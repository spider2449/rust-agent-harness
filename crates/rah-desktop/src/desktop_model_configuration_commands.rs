use super::{
    ConnectionState, DesktopAppState, ModelConfigurationPresentation, ProviderEndpoint,
    ProviderEndpointPresentation, model_configuration_status,
};
use tauri::State;

#[cfg(target_os = "windows")]
#[tauri::command]
pub(super) fn model_configuration(
    state: State<'_, DesktopAppState>,
) -> ModelConfigurationPresentation {
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
