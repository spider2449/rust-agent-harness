//! Desktop observation of host composition. Never serialized as a user preference.
use crate::{DesktopModelProvider, DesktopModelSelection, runtime_selection::ProductionAdapter};
use serde::Serialize;

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RuntimeAdapterIdentity {
    Codex,
    #[serde(rename = "openai")]
    OpenAi,
    None,
}

pub(crate) fn identity(adapter: Option<ProductionAdapter>) -> RuntimeAdapterIdentity {
    match adapter {
        #[cfg(feature = "provider-codex")]
        Some(ProductionAdapter::Codex) => RuntimeAdapterIdentity::Codex,
        #[cfg(feature = "provider-openai")]
        Some(ProductionAdapter::OpenAi) => RuntimeAdapterIdentity::OpenAi,
        None => RuntimeAdapterIdentity::None,
    }
}

pub(crate) fn configured_openai_model() -> Option<String> {
    std::env::var("RAH_OPENAI_MODEL")
        .ok()
        .filter(|model| crate::validate_model_identifier(model).is_ok())
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RuntimeSelectionState {
    pub runtime_adapter: RuntimeAdapterIdentity,
    pub runtime_available: bool,
    pub model_source_kind: &'static str,
    pub codex_model_provider: Option<DesktopModelProvider>,
    pub current_model: Option<String>,
    // Desired/configured state is never catalog evidence. Connect revalidates.
    pub model_validated: bool,
}

pub(crate) fn present(
    adapter: Option<ProductionAdapter>,
    codex_preferences: &DesktopModelSelection,
    openai_model: Option<&str>,
) -> RuntimeSelectionState {
    scoped(identity(adapter), codex_preferences, openai_model)
}

fn scoped(
    adapter: RuntimeAdapterIdentity,
    codex_preferences: &DesktopModelSelection,
    openai_model: Option<&str>,
) -> RuntimeSelectionState {
    let (source, provider, model) = match adapter {
        RuntimeAdapterIdentity::Codex => (
            "codex_catalog",
            Some(codex_preferences.provider),
            codex_preferences.model.clone(),
        ),
        RuntimeAdapterIdentity::OpenAi => (
            "openai_configuration",
            None,
            openai_model.map(str::to_owned),
        ),
        RuntimeAdapterIdentity::None => ("none", None, None),
    };
    RuntimeSelectionState {
        runtime_adapter: adapter,
        runtime_available: adapter != RuntimeAdapterIdentity::None,
        model_source_kind: source,
        codex_model_provider: provider,
        current_model: model,
        model_validated: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task509a_scopes_preferences_and_restores_codex_without_validity_transfer() {
        let preferences = DesktopModelSelection {
            provider: DesktopModelProvider::OpenAi,
            model: Some("gpt-6.1-sol".into()),
            model_selection_mode: Some(crate::ModelSelectionMode::Advertised),
            llama_cpp_endpoint: None,
        };
        let codex = scoped(RuntimeAdapterIdentity::Codex, &preferences, None);
        assert_eq!(
            codex.codex_model_provider,
            Some(DesktopModelProvider::OpenAi)
        );
        let native = scoped(
            RuntimeAdapterIdentity::OpenAi,
            &preferences,
            Some("configured-model"),
        );
        assert_eq!(native.current_model.as_deref(), Some("configured-model"));
        assert_eq!(native.codex_model_provider, None);
        assert!(!native.model_validated);
        let missing = scoped(RuntimeAdapterIdentity::OpenAi, &preferences, None);
        assert_eq!(missing.current_model, None);
        let none = scoped(RuntimeAdapterIdentity::None, &preferences, None);
        assert!(!none.runtime_available);
        assert_eq!(none.current_model, None);
        assert_eq!(none.codex_model_provider, None);
        assert_eq!(
            scoped(RuntimeAdapterIdentity::Codex, &preferences, None),
            codex
        );
        assert!(!codex.model_validated);
    }

    #[test]
    #[cfg(all(feature = "provider-codex", feature = "provider-openai"))]
    fn task509a_runtime_change_rejects_old_catalog_and_return_revalidates() {
        use crate::{ConnectionState, FrontendError, model_preflight};
        let directory =
            std::env::temp_dir().join(format!("rah-task509a-{}", rah_protocol::SessionId::new()));
        let mut desktop = crate::DesktopAppState::new(directory.clone());
        desktop.runtime_adapter = Some(ProductionAdapter::Codex);
        let preference = DesktopModelSelection {
            provider: DesktopModelProvider::OpenAi,
            model: Some("gpt-6.1-sol".into()),
            model_selection_mode: Some(crate::ModelSelectionMode::Advertised),
            llama_cpp_endpoint: None,
        };
        desktop.model.lock().unwrap().selection = preference.clone();
        *desktop.connection.lock().unwrap() =
            ConnectionState::Error(FrontendError::ModelNotAdvertised);
        // Representative bounded fixture, not a permanent model/version prohibition.
        let (state, gate) = model_preflight::present(
            preference.model.clone(),
            Ok(rah_runtime::ModelPreflight::NotAdvertised(
                rah_runtime::ModelCatalog {
                    models: vec!["gpt-6-astra".into()],
                },
            )),
        );
        assert_eq!(gate, Err(FrontendError::ModelNotAdvertised));
        *desktop.model_preflight.lock().unwrap() = Some(model_preflight::ScopedModelPreflight {
            adapter: RuntimeAdapterIdentity::Codex,
            connection_generation: 0,
            model_generation: 0,
            state,
        });
        assert!(desktop.status().model_preflight.is_some());
        desktop.runtime_adapter = Some(ProductionAdapter::OpenAi);
        assert!(desktop.status().model_preflight.is_none());
        assert_eq!(
            desktop.status().runtime_adapter,
            RuntimeAdapterIdentity::OpenAi
        );
        desktop.runtime_adapter = None;
        assert!(!desktop.status().runtime_available);
        assert!(matches!(
            crate::runtime_selection::connect_unavailable(&desktop),
            Err(FrontendError::RuntimeAdapterUnavailable)
        ));
        desktop.runtime_adapter = Some(ProductionAdapter::Codex);
        *desktop.connection.lock().unwrap() = ConnectionState::NotConnected;
        assert!(desktop.status().model_preflight.is_none());
        assert_eq!(desktop.model.lock().unwrap().selection, preference);
        // Recomposition must obtain fresh preflight; preference restore isn't validity.
        assert!(!present(desktop.runtime_adapter, &preference, None).model_validated);
        drop(desktop);
        let _ = std::fs::remove_dir_all(directory);
    }

    #[test]
    fn task509a_identity_comes_from_composition() {
        assert_eq!(identity(None), RuntimeAdapterIdentity::None);
        #[cfg(feature = "provider-codex")]
        assert_eq!(
            identity(Some(ProductionAdapter::Codex)),
            RuntimeAdapterIdentity::Codex
        );
        #[cfg(feature = "provider-openai")]
        assert_eq!(
            identity(Some(ProductionAdapter::OpenAi)),
            RuntimeAdapterIdentity::OpenAi
        );
    }
}
