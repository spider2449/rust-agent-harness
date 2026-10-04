use std::{io, path::PathBuf, process::ExitStatus};

use rah_protocol::{RuntimeDiagnostic, RuntimeFailureKind, RuntimeOperation};
use rah_runtime::RuntimeFailure;
use thiserror::Error;

/// Typed failures produced by the private Codex app-server adapter.
#[derive(Debug, Error)]
pub enum CodexAdapterError {
    /// Process-local verification evidence for explicit certification builds.
    #[cfg(feature = "certification-harness")]
    #[doc(hidden)]
    #[error("certification artifact verification failed")]
    CertificationVerification {
        /// Exact evidence, never projected to public diagnostics.
        #[source]
        source: crate::certification_support::CertificationVerificationError,
    },
    /// A bounded catalog observation exceeded its deadline.
    #[error("model catalog observation exceeded its 5 second deadline")]
    CatalogDeadline,
    /// Shared original failure retained by the connection actor for fanout.
    #[error("{failure}")]
    SharedFailure {
        /// Neutral envelope; its source is the original adapter error.
        #[source]
        failure: RuntimeFailure,
    },
    /// The host-selected workspace context could not be canonicalized.
    #[error("invalid host-selected Codex workspace context: {source}")]
    WorkspaceContext {
        /// Operating-system failure while canonicalizing the host-owned path.
        #[source]
        source: io::Error,
    },
    /// Host-owned model/provider selection failed validation.
    #[error("invalid Codex model/provider configuration: {message}")]
    InvalidModelProviderConfig {
        /// Validation failure detail without credential values.
        message: String,
    },
    /// The configured executable could not be found or invoked.
    #[error("failed to discover Codex executable `{path}`: {source}")]
    ExecutableDiscovery {
        /// Configured executable path.
        path: PathBuf,
        /// Operating-system failure.
        #[source]
        source: io::Error,
    },
    /// The installed CLI is absent from the exact current certified set.
    #[error("unsupported Codex version: current preferred `{expected}`, found `{actual}`")]
    VersionMismatch {
        /// Preferred version from the current certified set.
        expected: &'static str,
        /// Version reported by the executable.
        actual: String,
    },
    /// The installed app-server schema could not be generated or read.
    #[error("failed to inspect Codex app-server schema: {message}")]
    SchemaInspection {
        /// Failure detail.
        message: String,
    },
    /// Required app-server methods or payload fields are absent.
    #[error("incompatible Codex app-server schema: missing {missing}")]
    SchemaMismatch {
        /// Comma-separated missing contract elements.
        missing: String,
    },
    /// The app-server child could not be started.
    #[error("failed to start Codex app-server `{path}`: {source}")]
    ProcessStartup {
        /// Configured executable path.
        path: PathBuf,
        /// Operating-system failure.
        #[source]
        source: io::Error,
    },
    /// The child exited while the adapter still expected protocol traffic.
    #[error("Codex app-server exited with {status}; stderr: {stderr}")]
    ProcessExited {
        /// Child exit status.
        status: ExitStatus,
        /// Retained stderr tail.
        stderr: String,
    },
    /// A stdio line was not a valid JSON-RPC message.
    #[error("malformed Codex app-server framing: {message}")]
    MalformedFraming {
        /// Parse or framing detail.
        message: String,
    },
    /// The peer returned a JSON-RPC error response.
    #[error("Codex app-server JSON-RPC error {code}: {message}")]
    JsonRpc {
        /// JSON-RPC error code.
        code: i64,
        /// JSON-RPC error message.
        message: String,
    },
    /// A terminal turn was rejected by the provider.
    #[error("Codex turn failed: {message}")]
    TurnFailed {
        /// Private provider detail, never projected to IPC.
        message: String,
    },
    /// A well-formed message violated required adapter semantics.
    #[error("Codex app-server protocol violation: {message}")]
    ProtocolViolation {
        /// Violation detail.
        message: String,
    },
    /// An established stdio channel failed.
    #[error("Codex app-server transport failed: {source}")]
    Transport {
        /// Operating-system failure.
        #[source]
        source: io::Error,
    },
}

impl CodexAdapterError {
    /// Converts an adapter error to the neutral boundary without formatting private data.
    /// The returned source remains process-local, including any original IO source chain.
    #[must_use]
    pub fn into_runtime_failure(self, operation: RuntimeOperation) -> RuntimeFailure {
        if let Self::SharedFailure { failure } = self {
            return failure.at_operation(operation);
        }
        let kind = match &self {
            #[cfg(feature = "certification-harness")]
            Self::CertificationVerification { .. } => RuntimeFailureKind::IncompatibleRuntime,
            Self::WorkspaceContext { .. } | Self::InvalidModelProviderConfig { .. } => {
                RuntimeFailureKind::InvalidConfiguration
            }
            Self::VersionMismatch { .. }
            | Self::SchemaInspection { .. }
            | Self::SchemaMismatch { .. } => RuntimeFailureKind::IncompatibleRuntime,
            Self::ExecutableDiscovery { .. }
            | Self::ProcessStartup { .. }
            | Self::ProcessExited { .. } => RuntimeFailureKind::Unavailable,
            Self::MalformedFraming { .. } | Self::ProtocolViolation { .. } => {
                RuntimeFailureKind::Protocol
            }
            Self::JsonRpc { .. } | Self::TurnFailed { .. } => RuntimeFailureKind::ProviderRejection,
            Self::Transport { .. } => RuntimeFailureKind::Transport,
            Self::CatalogDeadline => RuntimeFailureKind::Unavailable,
            Self::SharedFailure { .. } => unreachable!(),
        };
        let rpc_code = match &self {
            Self::JsonRpc { code, .. } => Some(*code),
            _ => None,
        };
        RuntimeFailure::new(
            RuntimeDiagnostic {
                operation,
                kind,
                rpc_code,
            },
            self,
        )
    }
}
