//! The `model` service: one streamed request per call, no agent loop.

use async_trait::async_trait;
use futures::{StreamExt, stream::BoxStream};
use rah_kernel::{KernelHandle, ServiceKey};
use rah_protocol::{ContextItem, ModelEvent, ModelRequest, SessionId};
use tokio_util::sync::CancellationToken;

use crate::session::{SESSION, SessionError, rebuild_context};

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
    /// The provider's transport stopped before the stream completed.
    /// A caller that can discard this attempt's output may send the request again.
    #[error("model stream interrupted: {0}")]
    StreamInterrupted(String),
    /// The provider cannot take the request now and did nothing with it:
    /// rate limited, overloaded or temporarily failing (HTTP 429, 500, 502,
    /// 503, 504 or 529). Opening the request again is safe (ADR 0051).
    /// `retry_after` is what the provider asked for, when it said.
    #[error("the model is temporarily unavailable: {message}")]
    Unavailable {
        message: String,
        retry_after: Option<std::time::Duration>,
    },
    /// The request is larger than the model's context window, so sending
    /// less would work (ADR 0052). The text says so and carries nothing the
    /// provider sent.
    #[error("the request is larger than the model's context window: {0}")]
    ContextOverflow(String),
    #[error("the model rejected the request: {0}")]
    Rejected(String),
    #[error("model protocol violation: {0}")]
    Protocol(String),
}

#[derive(Debug, thiserror::Error)]
pub enum SideQuestionError {
    #[error("usage: /btw <question>")]
    EmptyQuestion,
    #[error("no session service is mounted")]
    NoSession,
    #[error("no model service is mounted")]
    NoModel,
    #[error(transparent)]
    Session(#[from] SessionError),
    #[error(transparent)]
    Model(#[from] ModelError),
}

/// Answers from a snapshot of the session log using a single model request.
/// The request offers no tools, and only text from the model is returned.
pub async fn answer_side_question(
    kernel: &KernelHandle,
    session: &SessionId,
    question: &str,
    cancel: CancellationToken,
) -> Result<String, SideQuestionError> {
    let question = question.trim();
    if question.is_empty() {
        return Err(SideQuestionError::EmptyQuestion);
    }
    let sessions = kernel.get(SESSION).ok_or(SideQuestionError::NoSession)?;
    let model = kernel.get(MODEL).ok_or(SideQuestionError::NoModel)?;
    let request_cancel = cancel.child_token();
    let _cancel_on_drop = request_cancel.clone().drop_guard();
    let answer = async {
        let records = sessions.read(session).await?;
        let mut items = rebuild_context(&records);
        items.insert(
            0,
            ContextItem::System {
                text: "Answer the side question concisely using the conversation so far. \
                       If there isn't enough information, say so."
                    .into(),
            },
        );
        items.push(ContextItem::User {
            text: question.to_owned(),
        });
        let request = ModelRequest {
            items,
            ..ModelRequest::default()
        };
        let mut stream = model.stream(request, request_cancel).await?;
        let mut text = String::new();
        while let Some(event) = stream.next().await {
            match event? {
                ModelEvent::TextDelta { text: delta } => text.push_str(&delta),
                ModelEvent::Completed if text.trim().is_empty() => {
                    return Err(
                        ModelError::Protocol("no text answer to the side question".into()).into(),
                    );
                }
                ModelEvent::Completed => return Ok(text),
                ModelEvent::ToolCall { .. }
                | ModelEvent::ReasoningStarted
                | ModelEvent::Usage { .. } => {}
            }
        }
        Err(
            ModelError::StreamInterrupted("no completion event for the side question".into())
                .into(),
        )
    };
    tokio::select! {
        biased;
        () = cancel.cancelled() => Err(ModelError::Cancelled.into()),
        result = answer => result,
    }
}

/// Whether an HTTP status means the provider cannot serve the request now
/// and did nothing with it ([`ModelError::Unavailable`]).
pub fn is_unavailable_status(status: u16) -> bool {
    matches!(status, 429 | 500 | 502 | 503 | 504 | 529)
}

/// The delay a `Retry-After` header asks for, when it is a number of
/// seconds. A date, or anything else, gives `None`.
pub fn parse_retry_after(value: &str) -> Option<std::time::Duration> {
    let seconds: u64 = value.trim().parse().ok()?;
    Some(std::time::Duration::from_secs(seconds))
}

/// Whether the body of a provider's error response says the request did not
/// fit the model's context window ([`ModelError::ContextOverflow`]). The
/// phrases are the ones OpenAI, Anthropic, llama.cpp and the servers that
/// copy them use; anything else is an ordinary rejection.
pub fn is_context_overflow(body: &str) -> bool {
    const PHRASES: [&str; 8] = [
        "context_length_exceeded",
        "maximum context length",
        "exceed_context_size_error",
        "exceeds the available context size",
        "exceeds the context window",
        "prompt is too long",
        "input is too long for",
        "reduce the length of the messages",
    ];
    let body = body.to_ascii_lowercase();
    PHRASES.iter().any(|phrase| body.contains(phrase))
}

/// The start of an error response body, at most `limit` bytes, as text. A
/// body that cannot be read gives an empty string: this only helps tell
/// kinds of rejection apart, so a failure here changes nothing.
pub async fn error_body<S, B, E>(chunks: S, limit: usize) -> String
where
    S: futures::Stream<Item = Result<B, E>>,
    B: AsRef<[u8]>,
{
    use futures::StreamExt;
    let mut chunks = std::pin::pin!(chunks);
    let mut bytes: Vec<u8> = Vec::new();
    while bytes.len() < limit {
        match chunks.next().await {
            Some(Ok(chunk)) => bytes.extend_from_slice(chunk.as_ref()),
            _ => break,
        }
    }
    bytes.truncate(limit);
    String::from_utf8_lossy(&bytes).into_owned()
}
