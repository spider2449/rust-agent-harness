use std::time::Duration;

use futures::{Stream, StreamExt};
use rah_api::{
    CancellationToken, async_trait,
    model::{ModelError, ModelProvider, ModelStream},
};
use rah_protocol::{ModelEvent, ModelRequest, ToolName};
use reqwest::header::{ACCEPT, CONTENT_TYPE};
use serde_json::json;

use crate::{
    error::E,
    sse::Sse,
    wire::{self, MAX_BYTES, Response, ToolNames, Update},
};

pub(crate) const ANTHROPIC_ENDPOINT: &str = "https://api.anthropic.com/v1/messages";
const API_VERSION: &str = "2023-06-01";
const FALLBACK_BETA: &str = "server-side-fallback-2026-07-01";
/// Models that accept the server-side refusal fallback in its `"default"`
/// form.
const FALLBACK_MODELS: [&str; 4] = [
    "claude-fable-5-1",
    "claude-opus-5-5",
    "claude-opus-5",
    "claude-sonnet-5-5",
];
const MAX_EVENTS: usize = 32_768;

/// Request settings that do not depend on the conversation.
pub(crate) struct Settings {
    pub model: String,
    pub max_tokens: u32,
    pub effort: Option<String>,
    pub refusal_fallback: bool,
}

/// An Anthropic Messages API model. Credentials are never exposed through
/// Debug, Display or errors.
pub struct AnthropicModel {
    client: reqwest::Client,
    endpoint: String,
    key: String,
    settings: Settings,
}

impl AnthropicModel {
    pub(crate) fn new(endpoint: String, key: String, settings: Settings) -> Result<Self, String> {
        let client = reqwest::Client::builder()
            .https_only(endpoint == ANTHROPIC_ENDPOINT)
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .no_proxy()
            .connect_timeout(Duration::from_secs(30))
            .read_timeout(Duration::from_secs(300))
            .timeout(Duration::from_secs(1800))
            .build()
            .map_err(|_| "cannot build the HTTP client".to_owned())?;
        Ok(Self {
            client,
            endpoint,
            key,
            settings,
        })
    }
}

#[async_trait]
impl ModelProvider for AnthropicModel {
    async fn stream(
        &self,
        request: ModelRequest,
        cancel: CancellationToken,
    ) -> Result<ModelStream, ModelError> {
        let model = request
            .model
            .clone()
            .unwrap_or_else(|| self.settings.model.clone());
        let names = ToolNames::new(&request.tools);
        let (system, messages) = wire::conversation(&request.items, &names)?;
        let tools = wire::tools(&request.tools, &names)?;
        // Top-level automatic caching: the history is append-only between
        // turns, so the cached prefix is reused across a tool loop.
        let mut body = json!({
            "model": model,
            "max_tokens": self.settings.max_tokens,
            "messages": messages,
            "stream": true,
            "cache_control": {"type": "ephemeral"},
        });
        if let Some(system) = system {
            body["system"] = system.into();
        }
        if !tools.is_empty() {
            body["tools"] = tools.into();
        }
        if let Some(effort) = &self.settings.effort {
            body["output_config"] = json!({"effort": effort});
        }
        let fallback = self.settings.refusal_fallback && FALLBACK_MODELS.contains(&model.as_str());
        if fallback {
            body["fallbacks"] = "default".into();
        }
        let mut post = self
            .client
            .post(&self.endpoint)
            .header(CONTENT_TYPE, "application/json")
            .header(ACCEPT, "text/event-stream")
            .header("anthropic-version", API_VERSION)
            .header("x-api-key", &self.key)
            .body(body.to_string());
        if fallback {
            post = post.header("anthropic-beta", FALLBACK_BETA);
        }
        let response = tokio::select! {
            biased;
            () = cancel.cancelled() => return Err(ModelError::Cancelled),
            response = post.send() => response,
        };
        let response = response.map_err(|_| ModelError::from(E::Transport))?;
        match response.status().as_u16() {
            200 => {}
            401 | 403 => return Err(E::Authentication.into()),
            status if rah_api::model::is_unavailable_status(status) => {
                return Err(E::unavailable(status, response.headers()).into());
            }
            status @ (400 | 413 | 422) => {
                let body = rah_api::model::error_body(response.bytes_stream(), 8192).await;
                return Err(if rah_api::model::is_context_overflow(&body) {
                    E::ContextOverflow
                } else {
                    E::HttpStatus(status)
                }
                .into());
            }
            status => return Err(E::HttpStatus(status).into()),
        }
        let event_stream = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.split(';').next())
            .is_some_and(|value| value.trim() == "text/event-stream");
        if !event_stream {
            return Err(E::Protocol.into());
        }
        Ok(Box::pin(events(response, names, cancel)))
    }
}

fn events(
    response: reqwest::Response,
    names: ToolNames,
    cancel: CancellationToken,
) -> impl Stream<Item = Result<ModelEvent, ModelError>> + Send + 'static {
    async_stream::try_stream! {
        let mut bytes = response.bytes_stream();
        let mut sse = Sse::default();
        let mut parsed = Response::default();
        let (mut total, mut count) = (0_usize, 0_usize);
        let (mut reasoning, mut completed) = (false, false);
        'read: loop {
            let next = tokio::select! {
                biased;
                () = cancel.cancelled() => Some(Err(None)),
                chunk = bytes.next() => chunk.map(|c| c.map_err(Some)),
            };
            let chunk = match next {
                None => break 'read,
                Some(Err(None)) => Err(ModelError::Cancelled)?,
                Some(Err(Some(_))) => Err(ModelError::from(E::Transport))?,
                Some(Ok(chunk)) => chunk,
            };
            total += chunk.len();
            if total > MAX_BYTES * 2 {
                Err::<(), _>(ModelError::from(E::Sse))?;
            }
            for event in sse.push(&chunk).map_err(ModelError::from)? {
                count += 1;
                if count > MAX_EVENTS {
                    Err::<(), _>(ModelError::from(E::Protocol))?;
                }
                match parsed.event(event).map_err(ModelError::from)? {
                    Update::Text(text) => yield ModelEvent::TextDelta { text },
                    Update::Reasoning if !reasoning => {
                        reasoning = true;
                        yield ModelEvent::ReasoningStarted;
                    }
                    Update::Reasoning | Update::None => {}
                    Update::Complete => {
                        completed = true;
                        break 'read;
                    }
                }
            }
        }
        if !completed {
            sse.finish().map_err(ModelError::from)?;
            Err::<(), _>(ModelError::from(E::Protocol))?;
        }
        for (call_id, wire_name, arguments) in parsed.calls().map_err(ModelError::from)? {
            let name = names
                .original(&wire_name)
                .ok_or_else(|| ModelError::from(E::Protocol))?;
            yield ModelEvent::ToolCall {
                call_id,
                name: ToolName::new(name),
                arguments,
            };
        }
        if let Some(usage) = parsed.usage() {
            yield ModelEvent::Usage { usage };
        }
        yield ModelEvent::Completed;
    }
}
