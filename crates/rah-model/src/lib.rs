//! Model backend abstractions for RAH.

use std::pin::Pin;

use async_trait::async_trait;
use futures::Stream;
use rah_protocol::{Message, ModelRequestId, ToolCall, ToolDefinition};
use thiserror::Error;

mod mock;

pub use mock::MockBackend;

/// A provider-neutral request sent to a model backend.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelRequest {
    /// Correlates the request with model events.
    pub id: ModelRequestId,
    /// Conversation context supplied to the model.
    pub messages: Vec<Message>,
    /// Tools available for the model to request.
    pub tools: Vec<ToolDefinition>,
    /// Provider-neutral generation controls.
    pub options: GenerationOptions,
}

/// Provider-neutral generation controls.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct GenerationOptions {}

/// An event emitted by a model backend stream.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ModelEvent {
    /// Incremental UTF-8 model output.
    TextDelta {
        /// Newly available text.
        text: String,
    },
    /// A parsed request for a RAH tool.
    ToolCall {
        /// Provider-neutral tool call.
        call: ToolCall,
    },
    /// Token usage reported by the backend.
    Usage {
        /// Number of input tokens consumed.
        input_tokens: u64,
        /// Number of output tokens produced.
        output_tokens: u64,
    },
    /// The backend completed the request stream.
    Completed,
}

/// Error returned while starting or consuming a model request.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ModelError {
    /// The backend rejected an invalid request.
    #[error("model request was rejected: {message}")]
    InvalidRequest {
        /// Backend-neutral rejection detail.
        message: String,
    },
    /// The configured backend is unavailable.
    #[error("model backend is unavailable: {message}")]
    Unavailable {
        /// Backend-neutral availability detail.
        message: String,
    },
    /// The backend stream failed after starting.
    #[error("model stream failed: {message}")]
    Stream {
        /// Backend-neutral stream failure detail.
        message: String,
    },
}

/// Asynchronous stream returned by a model backend.
pub type ModelStream = Pin<Box<dyn Stream<Item = Result<ModelEvent, ModelError>> + Send>>;

/// Provider-neutral interface for model completion backends.
#[async_trait]
pub trait ModelBackend: Send + Sync {
    /// Starts a model completion and returns its event stream.
    async fn complete(&self, request: ModelRequest) -> Result<ModelStream, ModelError>;
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use futures::{StreamExt, executor::block_on, stream};
    use rah_protocol::{
        Message, MessageRole, ModelRequestId, ToolCall, ToolCallId, ToolInput, ToolName,
    };

    use super::{
        GenerationOptions, ModelBackend, ModelError, ModelEvent, ModelRequest, ModelStream,
    };

    struct FakeBackend {
        events: Vec<Result<ModelEvent, ModelError>>,
    }

    #[async_trait]
    impl ModelBackend for FakeBackend {
        async fn complete(&self, _request: ModelRequest) -> Result<ModelStream, ModelError> {
            Ok(Box::pin(stream::iter(self.events.clone())))
        }
    }

    #[test]
    fn in_memory_backend_streams_model_events_in_order() {
        block_on(async {
            let expected_events = vec![
                Ok(ModelEvent::TextDelta {
                    text: "hello".to_owned(),
                }),
                Ok(ModelEvent::ToolCall {
                    call: ToolCall {
                        id: ToolCallId::new(),
                        name: ToolName::new("example.tool"),
                        input: ToolInput(Default::default()),
                    },
                }),
                Ok(ModelEvent::Usage {
                    input_tokens: 4,
                    output_tokens: 2,
                }),
                Ok(ModelEvent::Completed),
            ];
            let backend = FakeBackend {
                events: expected_events.clone(),
            };
            let request = ModelRequest {
                id: ModelRequestId::new(),
                messages: vec![Message {
                    role: MessageRole::User,
                    content: "hello".to_owned(),
                }],
                tools: Vec::new(),
                options: GenerationOptions::default(),
            };

            let events = backend
                .complete(request)
                .await
                .expect("fake backend should accept the request")
                .collect::<Vec<_>>()
                .await;

            assert_eq!(events, expected_events);
        });
    }
}
