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
    #[error("the provider is overloaded")]
    Overloaded,
    #[error("invalid or oversized SSE frame")]
    Sse,
    #[error("invalid event JSON")]
    EventJson,
    #[error("unexpected Messages protocol state")]
    Protocol,
    #[error("unsupported tool definition")]
    ToolSchema,
    #[error("the server rejected authentication")]
    Authentication,
    #[error("the response hit the output token limit")]
    Truncated,
    #[error("the model declined the request")]
    Refused,
}

impl From<E> for ModelError {
    fn from(error: E) -> Self {
        match error {
            E::Transport | E::Overloaded => ModelError::Transport(error.to_string()),
            E::HttpStatus(_) | E::Api | E::Authentication | E::Truncated | E::Refused => {
                ModelError::Rejected(error.to_string())
            }
            E::Sse | E::EventJson | E::Protocol | E::ToolSchema => {
                ModelError::Protocol(error.to_string())
            }
        }
    }
}
