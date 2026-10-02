use std::{error::Error, fmt, sync::Arc};

use rah_protocol::{AgentErrorCode, AgentEvent, RuntimeDiagnostic, SessionId};

/// Process-local failure. Its source is deliberately neither serializable nor comparable.
#[derive(Clone)]
pub struct RuntimeFailure {
    diagnostic: RuntimeDiagnostic,
    source: Option<Arc<dyn Error + Send + Sync>>,
}

impl RuntimeFailure {
    /// Retains an owned, thread-safe typed cause without requiring `Clone` on it.
    pub fn new(diagnostic: RuntimeDiagnostic, source: impl Error + Send + Sync + 'static) -> Self {
        Self {
            diagnostic,
            source: Some(Arc::new(source)),
        }
    }

    /// Constructs a local failure for which no underlying cause exists.
    #[must_use]
    pub fn without_source(diagnostic: RuntimeDiagnostic) -> Self {
        Self {
            diagnostic,
            source: None,
        }
    }

    /// Returns the only frontend-safe diagnostic snapshot.
    #[must_use]
    pub fn diagnostic(&self) -> &RuntimeDiagnostic {
        &self.diagnostic
    }

    /// Reclassifies the observing operation while retaining the same source allocation.
    #[must_use]
    pub fn at_operation(mut self, operation: rah_protocol::RuntimeOperation) -> Self {
        self.diagnostic.operation = operation;
        self
    }
}

impl fmt::Display for RuntimeFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.diagnostic.fmt(f)
    }
}

// Default logging must never invoke private source Debug or Display.
impl fmt::Debug for RuntimeFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RuntimeFailure")
            .field("diagnostic", &self.diagnostic)
            .finish_non_exhaustive()
    }
}

impl Error for RuntimeFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source
            .as_deref()
            .map(|source| source as &(dyn Error + 'static))
    }
}

/// Atomic local event plus optional retained runtime failure.
/// Use `into_event` only at the presentation boundary. No serde implementation.
#[derive(Clone, Debug)]
pub struct RuntimeEvent {
    event: AgentEvent,
    failure: Option<RuntimeFailure>,
}

impl RuntimeEvent {
    /// Constructs a terminal failure and its safe protocol projection together.
    #[must_use]
    pub fn failed(session_id: SessionId, code: AgentErrorCode, failure: RuntimeFailure) -> Self {
        Self {
            event: AgentEvent::Failed {
                session_id,
                code,
                message: failure.to_string(),
            },
            failure: Some(failure),
        }
    }

    /// Borrows the protocol projection, excluding the typed source.
    #[must_use]
    pub fn event(&self) -> &AgentEvent {
        &self.event
    }

    /// Borrows the process-local failure while this event remains alive.
    #[must_use]
    pub fn failure(&self) -> Option<&RuntimeFailure> {
        self.failure.as_ref()
    }

    /// Consumes this item into its protocol projection, dropping local source ownership.
    #[must_use]
    pub fn into_event(self) -> AgentEvent {
        self.event
    }
}

impl From<AgentEvent> for RuntimeEvent {
    fn from(event: AgentEvent) -> Self {
        Self {
            event,
            failure: None,
        }
    }
}
