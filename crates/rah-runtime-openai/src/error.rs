use rah_protocol::{RuntimeDiagnostic, RuntimeFailureKind, RuntimeOperation};
use rah_runtime::RuntimeFailure;
use thiserror::Error;

/// Closed adapter-local errors. No headers, credentials, payloads or provider IDs
/// are retained, even in private Display/Debug projections.
#[derive(Debug, Error, Eq, PartialEq)]
pub enum OpenAiAdapterError {
    #[error("missing or invalid backend configuration")]
    Configuration,
    #[error("HTTP transport failed")]
    Transport,
    #[error("HTTP response status {0}")]
    HttpStatus(u16),
    #[error("Responses API reported failure")]
    Api,
    #[error("invalid or oversized SSE frame")]
    Sse,
    #[error("invalid event JSON")]
    EventJson,
    #[error("unexpected Responses protocol state")]
    Protocol,
    #[error("unsupported Tool definition")]
    ToolSchema,
    #[error("Tool continuation limit exceeded")]
    ContinuationLimit,
    #[error("operation cancelled locally")]
    Cancelled,
    #[error("runtime or conversation shut down")]
    Shutdown,
}

impl OpenAiAdapterError {
    /// Retains this typed source under ADR 0032, projecting only a closed category.
    pub fn into_runtime_failure(self, operation: RuntimeOperation) -> RuntimeFailure {
        let kind = match self {
            Self::Configuration | Self::ToolSchema => RuntimeFailureKind::InvalidConfiguration,
            Self::Transport => RuntimeFailureKind::Transport,
            Self::HttpStatus(_) | Self::Api => RuntimeFailureKind::ProviderRejection,
            Self::Sse | Self::EventJson | Self::Protocol => RuntimeFailureKind::Protocol,
            Self::Shutdown => RuntimeFailureKind::Unavailable,
            Self::Cancelled | Self::ContinuationLimit => RuntimeFailureKind::Operation,
        };
        RuntimeFailure::new(
            RuntimeDiagnostic {
                operation,
                kind,
                rpc_code: None,
            },
            self,
        )
    }
}
