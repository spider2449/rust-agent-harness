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
    #[error("invalid chunk JSON")]
    ChunkJson,
    #[error("unexpected Chat Completions protocol state")]
    Protocol,
    #[error("unsupported tool definition")]
    ToolSchema,
    #[error("the server rejected authentication")]
    Authentication,
    #[error("the server is unreachable")]
    Unreachable,
    #[error("no model is configured and the server does not serve exactly one")]
    NoModel,
    #[error("the response hit the output token limit")]
    Truncated,
    #[error("the provider filtered the response")]
    Filtered,
}

impl From<E> for ModelError {
    fn from(error: E) -> Self {
        match error {
            E::Transport | E::Unreachable => ModelError::Transport(error.to_string()),
            E::HttpStatus(_)
            | E::Api
            | E::Authentication
            | E::NoModel
            | E::Truncated
            | E::Filtered => ModelError::Rejected(error.to_string()),
            E::Sse | E::ChunkJson | E::Protocol | E::ToolSchema => {
                ModelError::Protocol(error.to_string())
            }
        }
    }
}
