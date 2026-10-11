use rah_api::model::ModelError;

/// Closed transport-local errors. No headers, credentials, payloads or
/// provider identifiers are retained, even in Display/Debug.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub(crate) enum E {
    #[error("HTTP transport failed")]
    Transport,
    #[error("HTTP response status {0}")]
    HttpStatus(u16),
    #[error("the provider reported a failure")]
    Api,
    #[error("invalid or oversized SSE frame")]
    Sse,
    #[error("invalid event JSON")]
    EventJson,
    #[error("unexpected Responses protocol state")]
    Protocol,
    #[error("unsupported tool definition")]
    ToolSchema,
    #[error("the llama.cpp server is unreachable")]
    Unreachable,
    #[error("the llama.cpp server is loading its model")]
    Loading,
    #[error("the configured model is not served")]
    ModelUnavailable,
    #[error("the server rejected authentication")]
    Authentication,
}

impl From<E> for ModelError {
    fn from(error: E) -> Self {
        match error {
            E::Transport | E::Unreachable | E::Loading => ModelError::Transport(error.to_string()),
            E::HttpStatus(_) | E::Api | E::ModelUnavailable | E::Authentication => {
                ModelError::Rejected(error.to_string())
            }
            E::Sse | E::EventJson | E::Protocol | E::ToolSchema => {
                ModelError::Protocol(error.to_string())
            }
        }
    }
}
