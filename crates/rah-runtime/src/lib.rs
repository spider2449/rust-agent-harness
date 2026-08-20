//! Runtime abstractions for RAH.

use std::pin::Pin;

use async_trait::async_trait;
use futures::Stream;
use rah_protocol::{AgentEvent, AgentRequest, SessionId};
use thiserror::Error;

mod minimal;

pub use minimal::MinimalTestRuntime;

/// Error returned by an agent runtime operation.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum AgentError {
    /// The requested session does not exist.
    #[error("session `{session_id}` was not found")]
    SessionNotFound {
        /// Missing session identifier.
        session_id: SessionId,
    },
    /// The runtime rejected an invalid request.
    #[error("invalid agent request: {message}")]
    InvalidRequest {
        /// Validation failure detail.
        message: String,
    },
    /// The runtime failed to start or manage the operation.
    #[error("agent runtime failed: {message}")]
    Runtime {
        /// Runtime failure detail.
        message: String,
    },
}

/// Asynchronous event stream exposed to runtime consumers.
pub type AgentEventStream = Pin<Box<dyn Stream<Item = AgentEvent> + Send>>;

/// Owned session identity and event stream returned by a runtime.
pub struct AgentHandle {
    session_id: SessionId,
    events: AgentEventStream,
}

impl AgentHandle {
    /// Creates a handle from a RAH session ID and event stream.
    #[must_use]
    pub fn new(session_id: SessionId, events: AgentEventStream) -> Self {
        Self { session_id, events }
    }

    /// Returns the session associated with the operation.
    #[must_use]
    pub fn session_id(&self) -> &SessionId {
        &self.session_id
    }

    /// Consumes the handle and returns its event stream.
    #[must_use]
    pub fn into_events(self) -> AgentEventStream {
        self.events
    }
}

/// Stable RAH-owned interface for agent runtime implementations.
#[async_trait]
pub trait AgentRuntime: Send + Sync {
    /// Starts a new agent operation.
    async fn start(&self, request: AgentRequest) -> Result<AgentHandle, AgentError>;

    /// Resumes an existing session.
    async fn resume(&self, session_id: SessionId) -> Result<AgentHandle, AgentError>;

    /// Cancels an existing session.
    async fn cancel(&self, session_id: SessionId) -> Result<(), AgentError>;
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use futures::{StreamExt, executor::block_on, stream};
    use rah_protocol::{AgentEvent, AgentInput, AgentOptions, AgentRequest, RequestId, SessionId};

    use super::{AgentError, AgentHandle, AgentRuntime};

    struct TestRuntime;

    #[async_trait]
    impl AgentRuntime for TestRuntime {
        async fn start(&self, request: AgentRequest) -> Result<AgentHandle, AgentError> {
            let session_id = SessionId::new();
            let events = vec![AgentEvent::Started {
                session_id: session_id.clone(),
                request_id: request.request_id,
            }];
            Ok(AgentHandle::new(session_id, Box::pin(stream::iter(events))))
        }

        async fn resume(&self, session_id: SessionId) -> Result<AgentHandle, AgentError> {
            Err(AgentError::SessionNotFound { session_id })
        }

        async fn cancel(&self, session_id: SessionId) -> Result<(), AgentError> {
            Err(AgentError::SessionNotFound { session_id })
        }
    }

    #[test]
    fn handle_exposes_session_and_event_stream() {
        block_on(async {
            let request_id = RequestId::new();
            let handle = TestRuntime
                .start(AgentRequest {
                    request_id: request_id.clone(),
                    input: AgentInput {
                        messages: Vec::new(),
                    },
                    options: AgentOptions::default(),
                })
                .await
                .expect("test runtime should start");
            let session_id = handle.session_id().clone();
            let events = handle.into_events().collect::<Vec<_>>().await;

            assert_eq!(
                events,
                vec![AgentEvent::Started {
                    session_id,
                    request_id
                }]
            );
        });
    }
}
