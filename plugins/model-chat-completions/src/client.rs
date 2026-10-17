use std::time::Duration;

use futures::{Stream, StreamExt};
use rah_api::{
    CancellationToken, async_trait,
    model::{ModelError, ModelHealth, ModelProvider, ModelStream},
};
use rah_protocol::{ModelEvent, ModelRequest, ToolName};
use reqwest::header::{ACCEPT, CONTENT_TYPE};
use serde_json::{Map, Value, json};
use tokio::sync::Mutex;

use crate::{
    error::E,
    sse::{Frame, Sse},
    wire::{self, MAX_BYTES, Response, ToolNames, Update},
};

const MAX_CHUNKS: usize = 65_536;
const DISCOVERY_LIMIT: usize = 256 * 1024;
const HEALTH_TIMEOUT: Duration = Duration::from_secs(3);

/// Request settings that do not depend on the conversation.
pub(crate) struct Settings {
    /// `<base_url>`, without a trailing slash.
    pub base_url: String,
    /// The model to ask; `None` uses the one model the server serves.
    pub model: Option<String>,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f64>,
    /// Whether the server is on this machine: plain HTTP is allowed and no
    /// proxy is used.
    pub loopback: bool,
}

/// An OpenAI-compatible Chat Completions model. Credentials are never exposed
/// through Debug, Display or errors.
pub struct ChatCompletionsModel {
    client: reqwest::Client,
    key: String,
    settings: Settings,
    /// The served model, discovered lazily when none is configured and
    /// dropped on transport errors.
    discovered: Mutex<Option<String>>,
}

impl ChatCompletionsModel {
    pub(crate) fn new(key: String, settings: Settings) -> Result<Self, String> {
        let mut builder = reqwest::Client::builder()
            .https_only(!settings.loopback)
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .connect_timeout(Duration::from_secs(30))
            // A local server may take minutes to read a long prompt before it
            // sends the first byte.
            .read_timeout(Duration::from_secs(600))
            .timeout(Duration::from_secs(1800));
        if settings.loopback {
            builder = builder.no_proxy();
        }
        let client = builder
            .build()
            .map_err(|_| "cannot build the HTTP client".to_owned())?;
        Ok(Self {
            client,
            key,
            settings,
            discovered: Mutex::new(None),
        })
    }

    fn authorized(&self, request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if self.key.is_empty() {
            request
        } else {
            request.bearer_auth(&self.key)
        }
    }

    /// The model to ask: the request's, the configured one, or the one the
    /// server serves.
    async fn model(&self, requested: Option<String>) -> Result<String, E> {
        if let Some(model) = requested.or_else(|| self.settings.model.clone()) {
            return Ok(model);
        }
        let mut discovered = self.discovered.lock().await;
        if let Some(model) = discovered.as_ref() {
            return Ok(model.clone());
        }
        let model = tokio::time::timeout(Duration::from_secs(10), self.discover())
            .await
            .map_err(|_| E::Unreachable)??;
        *discovered = Some(model.clone());
        Ok(model)
    }

    /// `GET <base_url>/models`: the sole served model.
    async fn discover(&self) -> Result<String, E> {
        let url = format!("{}/models", self.settings.base_url);
        let response = self
            .authorized(self.client.get(url))
            .send()
            .await
            .map_err(|_| E::Unreachable)?;
        match response.status().as_u16() {
            200 => {}
            401 | 403 => return Err(E::Authentication),
            status => return Err(E::HttpStatus(status)),
        }
        let mut stream = response.bytes_stream();
        let mut bytes = Vec::new();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|_| E::Unreachable)?;
            if bytes.len() + chunk.len() > DISCOVERY_LIMIT {
                return Err(E::Protocol);
            }
            bytes.extend_from_slice(&chunk);
        }
        let catalog: Value = serde_json::from_slice(&bytes).map_err(|_| E::ChunkJson)?;
        let mut ids: Vec<&str> = Vec::new();
        for item in catalog["data"].as_array().ok_or(E::Protocol)? {
            let id = item["id"].as_str().ok_or(E::Protocol)?;
            if id.is_empty() || id.len() > 256 || id.chars().any(char::is_control) {
                return Err(E::Protocol);
            }
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
        match ids.as_slice() {
            [id] => Ok((*id).to_owned()),
            _ => Err(E::NoModel),
        }
    }

    async fn forget_discovery(&self) {
        *self.discovered.lock().await = None;
    }

    fn body(&self, model: String, request: &ModelRequest, names: &ToolNames) -> Result<Value, E> {
        let mut body = Map::new();
        body.insert("model".into(), model.into());
        body.insert(
            "messages".into(),
            wire::messages(&request.items, names)?.into(),
        );
        let tools = wire::tools(&request.tools, names)?;
        // Some servers reject an empty list.
        if !tools.is_empty() {
            body.insert("tools".into(), tools.into());
        }
        body.insert("stream".into(), true.into());
        body.insert("stream_options".into(), json!({"include_usage": true}));
        if let Some(max_tokens) = self.settings.max_tokens {
            body.insert("max_tokens".into(), max_tokens.into());
        }
        if let Some(temperature) = self.settings.temperature {
            body.insert("temperature".into(), temperature.into());
        }
        Ok(body.into())
    }
}

#[async_trait]
impl ModelProvider for ChatCompletionsModel {
    /// `GET <base_url>/models`. Any answer means the server is up, except a
    /// refused key, which every request would meet too. Gateways without
    /// `/models` answer 404 and still count as reachable.
    async fn health(&self, cancel: CancellationToken) -> ModelHealth {
        let url = format!("{}/models", self.settings.base_url);
        let send = self.authorized(self.client.get(url)).send();
        let response = tokio::select! {
            biased;
            () = cancel.cancelled() => return ModelHealth::Unavailable("check cancelled".into()),
            response = tokio::time::timeout(HEALTH_TIMEOUT, send) => response,
        };
        match response {
            Ok(Ok(response)) if matches!(response.status().as_u16(), 401 | 403) => {
                ModelHealth::Unavailable(E::Authentication.to_string())
            }
            Ok(Ok(_)) => ModelHealth::Reachable,
            Ok(Err(_)) | Err(_) => ModelHealth::Unavailable(E::Unreachable.to_string()),
        }
    }

    async fn stream(
        &self,
        request: ModelRequest,
        cancel: CancellationToken,
    ) -> Result<ModelStream, ModelError> {
        let model = tokio::select! {
            biased;
            () = cancel.cancelled() => return Err(ModelError::Cancelled),
            model = self.model(request.model.clone()) => model,
        };
        let model = match model {
            Ok(model) => model,
            Err(error) => {
                self.forget_discovery().await;
                return Err(error.into());
            }
        };
        let names = ToolNames::new(&request.tools);
        let body = self.body(model, &request, &names)?;
        let post = self
            .authorized(
                self.client
                    .post(format!("{}/chat/completions", self.settings.base_url)),
            )
            .header(CONTENT_TYPE, "application/json")
            .header(ACCEPT, "text/event-stream")
            .body(body.to_string());
        let response = tokio::select! {
            biased;
            () = cancel.cancelled() => return Err(ModelError::Cancelled),
            response = post.send() => response,
        };
        let Ok(response) = response else {
            self.forget_discovery().await;
            return Err(E::Transport.into());
        };
        match response.status().as_u16() {
            200 => {}
            401 | 403 => return Err(E::Authentication.into()),
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
        let (mut reasoning, mut done) = (false, false);
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
            if total > MAX_BYTES * 4 {
                Err::<(), _>(ModelError::from(E::Sse))?;
            }
            for frame in sse.push(&chunk).map_err(ModelError::from)? {
                let chunk = match frame {
                    Frame::Done => {
                        done = true;
                        break 'read;
                    }
                    Frame::Chunk(chunk) => chunk,
                };
                count += 1;
                if count > MAX_CHUNKS {
                    Err::<(), _>(ModelError::from(E::Protocol))?;
                }
                for update in parsed.chunk(&chunk).map_err(ModelError::from)? {
                    match update {
                        Update::Text(text) => yield ModelEvent::TextDelta { text },
                        Update::Reasoning if !reasoning => {
                            reasoning = true;
                            yield ModelEvent::ReasoningStarted;
                        }
                        Update::Reasoning => {}
                    }
                }
            }
        }
        if !done {
            sse.finish().map_err(ModelError::from)?;
        }
        // A finish reason is required; `[DONE]` alone does not complete a
        // response.
        for (call_id, wire_name, arguments) in parsed.finish().map_err(ModelError::from)? {
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
