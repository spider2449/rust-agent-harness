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
        #[cfg(feature = "certification-harness")]
        CodexAdapterError::CertificationVerification { .. } => FrontendError::CodexConnectionFailed,
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
#[cfg(test)]
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
#[cfg(test)]
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
