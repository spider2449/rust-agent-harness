//! Desktop observation of host composition. Never serialized as a user preference.
use crate::{DesktopModelProvider, DesktopModelSelection, runtime_selection::ProductionAdapter};
use serde::Serialize;

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RuntimeAdapterIdentity {
    Codex,
    #[serde(rename = "openai")]
    OpenAi,
    #[serde(rename = "llama_cpp")]
    LlamaCpp,
    None,
}

pub(crate) fn identity(adapter: Option<ProductionAdapter>) -> RuntimeAdapterIdentity {
    match adapter {
        #[cfg(feature = "provider-openai")]
        Some(ProductionAdapter::OpenAi) => RuntimeAdapterIdentity::OpenAi,
        #[cfg(feature = "provider-llamacpp")]
        Some(ProductionAdapter::LlamaCpp) => RuntimeAdapterIdentity::LlamaCpp,
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
        RuntimeAdapterIdentity::LlamaCpp => (
            "llamacpp_configuration",
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
    fn task509a_identity_comes_from_composition() {
        assert_eq!(identity(None), RuntimeAdapterIdentity::None);
        #[cfg(feature = "provider-openai")]
        assert_eq!(
            identity(Some(ProductionAdapter::OpenAi)),
            RuntimeAdapterIdentity::OpenAi
        );
    }
}
