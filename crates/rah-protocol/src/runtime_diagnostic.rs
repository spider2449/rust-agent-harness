//! Closed, provider-neutral diagnostic snapshots; no private error data.

use serde::{Deserialize, Serialize};

/// Runtime operation during which a failure was observed.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeOperation {
    /// Runtime creation or connection.
    Connection,
    /// Observing the current advertised model catalog, without inference.
    ModelDiscovery,
    /// Session creation and initial request submission.
    SessionStart,
    /// Resuming a session.
    SessionResume,
    /// Consuming a running turn.
    Turn,
    /// Cancelling a request.
    Cancellation,
    /// Shutting down the runtime.
    Shutdown,
}

/// Broad classification independent of any provider's wire contract.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeFailureKind {
    /// Invalid host configuration.
    InvalidConfiguration,
    /// Runtime version or schema is incompatible.
    IncompatibleRuntime,
    /// Provider rejected a request.
    ProviderRejection,
    /// Invalid protocol data or semantics.
    Protocol,
    /// Transport failed.
    Transport,
    /// Runtime process is unavailable.
    Unavailable,
    /// Otherwise unclassified operation failure.
    Operation,
}

/// Sanitized snapshot suitable for IPC. All text comes from closed templates.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeDiagnostic {
    /// Operation whose failure was observed; this does not imply rollback.
    pub operation: RuntimeOperation,
    /// Broad failure classification; this does not authorize retries.
    pub kind: RuntimeFailureKind,
    /// Structured RPC rejection code, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rpc_code: Option<i64>,
}

impl std::fmt::Display for RuntimeDiagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "runtime {:?} failed ({:?})", self.operation, self.kind)
    }
}
