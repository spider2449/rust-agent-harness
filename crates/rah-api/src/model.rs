//! The `model` service: one streamed request per call, no agent loop.

use async_trait::async_trait;
use futures::stream::BoxStream;
use rah_kernel::ServiceKey;
use rah_protocol::{ModelEvent, ModelRequest};
use tokio_util::sync::CancellationToken;

pub const MODEL: ServiceKey<dyn ModelProvider> = ServiceKey::new("model");

pub type ModelStream = BoxStream<'static, Result<ModelEvent, ModelError>>;

#[async_trait]
pub trait ModelProvider: Send + Sync {
    /// Opens one streamed model request. The stream ends after
    /// `ModelEvent::Completed` or an error, and stops promptly once `cancel`
    /// fires. Provider wire formats stay inside the implementing plugin.
    async fn stream(
        &self,
        request: ModelRequest,
        cancel: CancellationToken,
    ) -> Result<ModelStream, ModelError>;

    /// A quick, side-effect free check that the model can be reached now,
    /// for status displays. It must return promptly, send no conversation
    /// and never spend tokens. Providers that cannot tell keep the default.
    async fn health(&self, _cancel: CancellationToken) -> ModelHealth {
        ModelHealth::Unknown
    }
}

/// The result of [`ModelProvider::health`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModelHealth {
    /// The provider has no cheap way to check.
    Unknown,
    /// The server answered.
    Reachable,
    /// The server cannot serve requests now; the text says why and carries
    /// no credentials.
    Unavailable(String),
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ModelError {
    #[error("the model request was cancelled")]
    Cancelled,
    #[error("model transport failed: {0}")]
    Transport(String),
    #[error("the model rejected the request: {0}")]
    Rejected(String),
    #[error("model protocol violation: {0}")]
    Protocol(String),
}
