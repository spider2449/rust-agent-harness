//! Private static selection; provider features may coexist. Future selection is host-owned.
//! Codex remains the default; no implicit fallback or discovery without its feature.
use crate::FrontendError;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RuntimeArtifactSource {
    Override,
    CertifiedBaseline,
    Path,
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
        FrontendError::RuntimeAdapterUnavailable
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

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ProductionAdapter {
    #[cfg(feature = "provider-codex")]
    Codex,
}
pub(crate) fn selected_adapter() -> Option<ProductionAdapter> {
    #[cfg(feature = "provider-codex")]
    {
        Some(ProductionAdapter::Codex)
    }
    #[cfg(not(feature = "provider-codex"))]
    {
        None
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "provider-codex")]
    #[test]
    fn task506_default_selects_configured_codex_factory_without_invocation() {
        use rah_runtime::experimental::{ConfiguredRuntimeFactory, ModelSelection};
        assert_eq!(selected_adapter(), Some(ProductionAdapter::Codex));
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
    #[cfg(not(feature = "provider-codex"))]
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
