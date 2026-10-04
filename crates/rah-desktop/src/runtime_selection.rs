//! Private static selection; provider features may coexist. Future selection is host-owned.
//! Codex remains the default; no implicit fallback or discovery without its feature.
use crate::FrontendError;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RuntimeArtifactSource {
    Override,
    CertifiedBaseline,
    Path,
    Native,
}
pub(crate) fn configured_version() -> Option<&'static str> {
    #[cfg(feature = "provider-codex")]
    {
        Some(rah_runtime_codex::PREFERRED_CURRENT_CODEX_VERSION)
    }
    #[cfg(not(feature = "provider-codex"))]
    {
        None
    }
}
pub(crate) fn frontend_failure(error: &rah_runtime::RuntimeFailure) -> FrontendError {
    #[cfg(feature = "provider-openai")]
    {
        use std::error::Error;
        if error
            .source()
            .and_then(|s| s.downcast_ref::<rah_runtime_openai::OpenAiAdapterError>())
            .is_some()
        {
            return FrontendError::RuntimeConnectionFailed;
        }
    }
    #[cfg(feature = "provider-codex")]
    {
        use std::error::Error;
        if let Some(source) = error
            .source()
            .and_then(|s| s.downcast_ref::<rah_runtime_codex::CodexAdapterError>())
        {
            return crate::codex_composition::frontend_error(source);
        }
        FrontendError::CodexConnectionFailed
    }
    #[cfg(not(feature = "provider-codex"))]
    {
        let _ = error;
        FrontendError::RuntimeConnectionFailed
    }
}
pub(crate) fn connect_unavailable(
    state: &crate::DesktopAppState,
) -> Result<crate::ConnectionResult, FrontendError> {
    *state
        .connection
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) =
        crate::ConnectionState::Error(FrontendError::RuntimeAdapterUnavailable);
    Err(FrontendError::RuntimeAdapterUnavailable)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProductionAdapter {
    #[cfg(feature = "provider-codex")]
    Codex,
    #[cfg(feature = "provider-openai")]
    OpenAi,
}
pub(crate) fn selected_adapter() -> Option<ProductionAdapter> {
    select(std::env::var("RAH_RUNTIME_PROVIDER").ok().as_deref())
}
fn select(config: Option<&str>) -> Option<ProductionAdapter> {
    match (
        cfg!(feature = "provider-codex"),
        cfg!(feature = "provider-openai"),
        config,
    ) {
        #[cfg(feature = "provider-codex")]
        (true, false, _) | (true, true, Some("codex")) => Some(ProductionAdapter::Codex),
        #[cfg(feature = "provider-openai")]
        (false, true, _) | (true, true, Some("openai")) => Some(ProductionAdapter::OpenAi),
        _ => None,
    }
}
pub(crate) type FactoryConfiguration = (
    Box<dyn rah_runtime::experimental::ConfiguredRuntimeFactory>,
    rah_runtime::experimental::ModelSelection,
    RuntimeArtifactSource,
);
/// Backend-only configuration. No frontend credential or endpoint channel.
pub(crate) fn configured_factory(
    adapter: ProductionAdapter,
    selection: &crate::DesktopModelSelection,
    workspace: &std::path::Path,
    openai_model: Option<&str>,
) -> Result<FactoryConfiguration, FrontendError> {
    let _ = (selection, workspace, openai_model);
    match adapter {
        #[cfg(feature = "provider-codex")]
        ProductionAdapter::Codex => {
            let config = selection.codex_model_config()?;
            let prepared = crate::codex_composition::prepare_codex_connection(
                crate::resolve_codex_executable,
                config,
            )?;
            let (factory, model) = crate::codex_composition::configured_codex_factory(
                std::path::PathBuf::from(prepared.executable),
                prepared.model_config,
                workspace,
            )
            .map_err(|e| frontend_failure(&e))?;
            #[cfg(test)]
            if crate::startup_counter_tracking() {
                crate::startup_activation_counters()
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .codex_runtime_construction += 1;
            }
            Ok((Box::new(factory), model, prepared.source))
        }
        #[cfg(feature = "provider-openai")]
        ProductionAdapter::OpenAi => {
            let key = std::env::var("OPENAI_API_KEY")
                .map_err(|_| FrontendError::RuntimeConnectionFailed)?;
            let model = openai_model
                .ok_or(FrontendError::ModelConfigurationInvalid)?
                .to_owned();
            configured_openai(key, model)
        }
    }
}
#[cfg(feature = "provider-openai")]
fn configured_openai(key: String, model: String) -> Result<FactoryConfiguration, FrontendError> {
    use rah_runtime::experimental::{ConfiguredRuntimeFactory, ModelSelection};
    if model.trim().is_empty() || model.len() > 256 {
        return Err(FrontendError::ModelConfigurationInvalid);
    }
    let factory = rah_runtime_openai::OpenAiFactory::new(key);
    factory.validate().map_err(|e| frontend_failure(&e))?;
    Ok((
        Box::new(factory),
        ModelSelection::Explicit(model),
        RuntimeArtifactSource::Native,
    ))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn task508_feature_selection_matrix() {
        if cfg!(all(feature = "provider-codex", feature = "provider-openai")) {
            assert_eq!(select(None), None);
            assert_eq!(select(Some("unknown")), None);
        }
        #[cfg(feature = "provider-codex")]
        assert_eq!(select(Some("codex")), Some(ProductionAdapter::Codex));
        #[cfg(feature = "provider-openai")]
        assert_eq!(select(Some("openai")), Some(ProductionAdapter::OpenAi));
        if !cfg!(any(feature = "provider-codex", feature = "provider-openai")) {
            assert_eq!(select(None), None);
        }
    }
    #[cfg(feature = "provider-openai")]
    #[test]
    fn task508_openai_configuration_is_explicit_and_sanitized() {
        assert!(configured_openai("fake".into(), "".into()).is_err());
        assert!(configured_openai("".into(), "fixture-model".into()).is_err());
        let (factory, model, source) =
            configured_openai("SECRET-SENTINEL".into(), "fixture-model".into()).unwrap();
        factory.validate().unwrap();
        assert_eq!(
            model,
            rah_runtime::experimental::ModelSelection::Explicit("fixture-model".into())
        );
        assert_eq!(source, RuntimeArtifactSource::Native);
    }
    #[cfg(feature = "provider-codex")]
    #[test]
    fn task506_default_selects_configured_codex_factory_without_invocation() {
        use rah_runtime::experimental::{ConfiguredRuntimeFactory, ModelSelection};
        assert_eq!(select(Some("codex")), Some(ProductionAdapter::Codex));
        let (factory, model) = crate::codex_composition::configured_codex_factory(
            std::path::PathBuf::from(r"C:\rah-test-fixtures\not-invoked.exe"),
            rah_runtime_codex::CodexModelConfig::Inherit,
            &std::env::temp_dir(),
        )
        .unwrap();
        factory.validate().unwrap();
        assert_eq!(model, ModelSelection::RuntimeDefault);
        assert_eq!(
            configured_version(),
            Some(rah_runtime_codex::PREFERRED_CURRENT_CODEX_VERSION)
        );
    }
    #[cfg(not(any(feature = "provider-codex", feature = "provider-openai")))]
    #[tokio::test]
    async fn task506_no_provider_initializes_and_connect_fails_closed() {
        crate::reset_startup_activation_counters();
        let directory =
            std::env::temp_dir().join(format!("rah-task506-{}", rah_protocol::SessionId::new()));
        let state = crate::DesktopAppState::new(directory.clone());
        assert!(selected_adapter().is_none());
        assert!(configured_version().is_none());
        assert!(matches!(
            *state.connection.lock().unwrap(),
            crate::ConnectionState::NotConnected
        ));
        let result = connect_unavailable(&state);
        assert!(matches!(
            result,
            Err(FrontendError::RuntimeAdapterUnavailable)
        ));
        assert!(matches!(
            *state.connection.lock().unwrap(),
            crate::ConnectionState::Error(FrontendError::RuntimeAdapterUnavailable)
        ));
        let counters = crate::startup_activation_snapshot();
        assert_eq!(counters.codex_resolver, 0);
        assert_eq!(counters.codex_runtime_construction, 0);
        assert_eq!(state.status().runtime_status, "not connected");
        assert_eq!(state.status().codex_version, None);
        let _ = std::fs::remove_dir_all(directory);
    }
}

/// Headless smoke of the disabled production state/Connect path; no inference fixture.
#[cfg(not(feature = "provider-codex"))]
pub(crate) fn smoke() -> std::process::ExitCode {
    let directory = std::env::temp_dir().join(format!(
        "rah-task506-smoke-{}",
        rah_protocol::SessionId::new()
    ));
    let state = crate::DesktopAppState::new(directory);
    #[cfg(feature = "provider-openai")]
    if selected_adapter() == Some(ProductionAdapter::OpenAi) {
        use rah_runtime::experimental::ConfiguredRuntimeFactory;
        // Fake configuration only; construction and discovery perform no HTTP.
        let factory = rah_runtime_openai::OpenAiFactory::new("RAH-SMOKE-FAKE-KEY");
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let passed = runtime.block_on(async {
            let instance = factory.create().await.ok()?;
            let alive = instance.is_alive();
            instance.shutdown().await.ok()?;
            Some(alive)
        }) == Some(true);
        println!(
            "{}",
            serde_json::json!({"adapter":"openai", "codex_compiled":false,"http_requests":0,"passed":passed})
        );
        return if passed {
            std::process::ExitCode::SUCCESS
        } else {
            std::process::ExitCode::FAILURE
        };
    }
    let unavailable = selected_adapter().is_none()
        && matches!(
            connect_unavailable(&state),
            Err(FrontendError::RuntimeAdapterUnavailable)
        );
    println!(
        "{}",
        serde_json::json!({
            "adapter_available": selected_adapter().is_some(),
            "connection_error": "runtime_adapter_unavailable",
            "runtime_status": state.status().runtime_status,
            "passed": unavailable,
        })
    );
    if unavailable {
        std::process::ExitCode::SUCCESS
    } else {
        std::process::ExitCode::FAILURE
    }
}
