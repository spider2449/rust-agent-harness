use rah_api::model::ModelError;

/// Closed transport-local errors. No headers, credentials, payloads or
/// provider identifiers are retained, even in Display/Debug.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub(crate) enum E {
    #[error("HTTP transport failed")]
    Transport,
    #[error("HTTP response status {0}")]
    HttpStatus(u16),
    #[error("the provider says the request does not fit its context window")]
    ContextOverflow,
    #[error("HTTP response status {status}; the provider cannot take the request now")]
    Unavailable {
        status: u16,
        retry_after: Option<std::time::Duration>,
    },
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
            E::ContextOverflow => ModelError::ContextOverflow(error.to_string()),
            E::Unavailable { retry_after, .. } => ModelError::Unavailable {
                message: error.to_string(),
                retry_after,
            },
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

impl E {
    /// A response status the provider uses when it cannot take the request
    /// now (ADR 0051), with the delay its `Retry-After` header asks for.
    pub(crate) fn unavailable(status: u16, headers: &reqwest::header::HeaderMap) -> Self {
        let retry_after = headers
            .get(reqwest::header::RETRY_AFTER)
            .and_then(|value| value.to_str().ok())
            .and_then(rah_api::model::parse_retry_after);
        Self::Unavailable {
            status,
            retry_after,
        }
    }
}
