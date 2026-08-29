use std::{io, path::PathBuf, process::ExitStatus};

use thiserror::Error;

/// Typed failures produced by the private Codex app-server adapter.
#[derive(Debug, Error)]
pub enum CodexAdapterError {
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
    /// The installed CLI is not the exact version supported by the adapter.
    #[error("unsupported Codex version: expected `{expected}`, found `{actual}`")]
    VersionMismatch {
        /// Adapter-supported version.
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
