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
