use std::time::{Duration, Instant};

use futures::{Stream, StreamExt};
use rah_api::{
    CancellationToken, async_trait,
    model::{ModelError, ModelHealth, ModelProvider, ModelStream},
};
use rah_protocol::{ModelEvent, ModelRequest, ToolName};
use reqwest::header::{ACCEPT, CONTENT_TYPE};
use serde_json::{Value, json};
use tokio::sync::Mutex;

use crate::{
    error::E,
    sse::Sse,
    wire::{self, EventNormalizer, MAX_BYTES, Response, ToolNames, Update},
};

pub(crate) const OPENAI_ENDPOINT: &str = "https://api.openai.com/v1/responses";
const MAX_EVENTS: usize = 32_768;
const DISCOVERY_LIMIT: usize = 64 * 1024;
// The props response can include the entire chat template.
const PROPS_LIMIT: usize = 1024 * 1024;

enum Flavor {
    OpenAi {
        model: String,
    },
    LlamaCpp {
        origin: String,
        model: Option<String>,
        /// Served models, discovered lazily and dropped on transport errors.
        served: Mutex<Option<Vec<Served>>>,
    },
}

/// A model a llama-server serves.
#[derive(Clone)]
struct Served {
    id: String,
    /// Whether `/v1/models` lists image input for it.
    images: bool,
    context_window_tokens: Option<u64>,
    checked_props: bool,
}

/// A Responses API model. Credentials are never exposed through Debug,
/// Display or errors.
pub struct ResponsesModel {
    client: reqwest::Client,
    endpoint: String,
    key: String,
    flavor: Flavor,
    timeouts: Timeouts,
}

#[derive(Clone, Copy)]
struct Timeouts {
    connect: Duration,
    read: Duration,
    request: Duration,
}

impl From<&crate::LlamaConfig> for Timeouts {
    fn from(config: &crate::LlamaConfig) -> Self {
        let seconds = |value: Option<std::num::NonZeroU32>, default| {
            Duration::from_secs(value.map_or(default, |n| u64::from(n.get())))
        };
        Self {
            connect: seconds(config.connect_timeout_secs, 30),
            read: seconds(config.read_timeout_secs, 120),
            request: seconds(config.request_timeout_secs, 600),
        }
    }
}

#[derive(Clone, Copy)]
struct RequestTiming {
    started: Instant,
    last_received: Option<Instant>,
    limits: Timeouts,
}

impl RequestTiming {
    fn error(self, error: reqwest::Error, interrupted: bool) -> ModelError {
        let failure = if interrupted {
            ModelError::StreamInterrupted
        } else {
            ModelError::Transport
        };
        if !error.is_timeout() {
            return failure(E::Transport.to_string());
        }
        let elapsed = self.started.elapsed();
        let idle = self.last_received.unwrap_or(self.started).elapsed();
        let cause = if elapsed >= self.limits.request {
            "HTTP request total timeout"
        } else if error.is_connect() {
            "HTTP connection timeout"
        } else if idle >= self.limits.read {
            "HTTP read idle timeout"
        } else {
            "HTTP timeout"
        };
        let progress = match self.last_received {
            Some(_) => format!("last response data {} ms ago", idle.as_millis()),
            None => "awaiting response headers".to_owned(),
        };
        failure(format!(
            "{cause} ({} ms elapsed, {progress})",
            elapsed.as_millis()
        ))
    }
}

fn build_client(https_only: bool, limits: Timeouts) -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .https_only(https_only)
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .no_proxy()
        .connect_timeout(limits.connect)
        .read_timeout(limits.read)
        .timeout(limits.request)
        .build()
        .map_err(|_| "cannot build the HTTP client".to_owned())
}

impl ResponsesModel {
    pub(crate) fn openai(endpoint: String, key: String, model: String) -> Result<Self, String> {
        let timeouts = Timeouts::from(&crate::LlamaConfig::default());
        Ok(Self {
            timeouts,
            client: build_client(endpoint == OPENAI_ENDPOINT, timeouts)?,
            endpoint,
            key,
            flavor: Flavor::OpenAi { model },
        })
    }

    pub(crate) fn llamacpp(
        origin: String,
        key: String,
        config: crate::LlamaConfig,
    ) -> Result<Self, String> {
        let timeouts = Timeouts::from(&config);
        Ok(Self {
            timeouts,
            client: build_client(false, timeouts)?,
            endpoint: format!("{origin}/v1/responses"),
            key,
            flavor: Flavor::LlamaCpp {
                origin,
                model: config.model,
                served: Mutex::new(None),
            },
        })
    }

    fn is_llamacpp(&self) -> bool {
        matches!(self.flavor, Flavor::LlamaCpp { .. })
    }

    /// The model to ask, and whether it takes image input. OpenAI models are
    /// assumed to; a llama-server model only when the server says so.
    async fn model(&self, requested: Option<String>) -> Result<Served, E> {
        match &self.flavor {
            Flavor::OpenAi { model } => Ok(Served {
                id: requested.unwrap_or_else(|| model.clone()),
                images: true,
                context_window_tokens: None,
                checked_props: false,
            }),
            Flavor::LlamaCpp {
                origin,
                model,
                served,
            } => {
                let mut cache = served.lock().await;
                if cache.is_none() {
                    *cache = Some(self.discover(origin).await?);
                }
                let served = cache.as_deref_mut().unwrap_or_default();
                let found = match requested.or_else(|| model.clone()) {
                    Some(model) => served.iter_mut().find(|served| served.id == model),
                    None if served.len() == 1 => served.first_mut(),
                    None => None,
                };
                let found = found.ok_or(E::ModelUnavailable)?;
                if found.context_window_tokens.is_none() && !found.checked_props {
                    let url = reqwest::Url::parse_with_params(
                        &format!("{origin}/props"),
                        &[("model", &found.id)],
                    )
                    .map_err(|_| E::Protocol)?;
                    if let Ok(Ok(props)) = tokio::time::timeout(
                        Duration::from_secs(2),
                        self.get_json(url.into(), true, PROPS_LIMIT),
                    )
                    .await
                    {
                        found.context_window_tokens = props["default_generation_settings"]["n_ctx"]
                            .as_u64()
                            .filter(|n| *n > 0);
                    }
                    found.checked_props = true;
                }
                Ok(found.clone())
            }
        }
    }

    async fn forget_discovery(&self) {
        if let Flavor::LlamaCpp { served, .. } = &self.flavor {
            *served.lock().await = None;
        }
    }

    async fn get_json(&self, url: String, authenticated: bool, limit: usize) -> Result<Value, E> {
        let mut request = self.client.get(url);
        if authenticated && !self.key.is_empty() {
            request = request.bearer_auth(&self.key);
        }
        let response = request.send().await.map_err(|_| E::Unreachable)?;
        match response.status().as_u16() {
            200 => {}
            503 => return Err(E::Loading),
            401 | 403 => return Err(E::Authentication),
            status => return Err(E::HttpStatus(status)),
        }
        let mut stream = response.bytes_stream();
        let mut bytes = Vec::new();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|_| E::Unreachable)?;
            if bytes.len() + chunk.len() > limit {
                return Err(E::Protocol);
            }
            bytes.extend_from_slice(&chunk);
        }
        serde_json::from_slice(&bytes).map_err(|_| E::EventJson)
    }

    /// llama-server readiness (`/health`) and served models (`/v1/models`).
    async fn discover(&self, origin: &str) -> Result<Vec<Served>, E> {
        let discovery = async {
            let health = self
                .get_json(format!("{origin}/health"), false, DISCOVERY_LIMIT)
                .await?;
            if health["status"] != "ok" {
                return Err(E::Loading);
            }
            let catalog = self
                .get_json(format!("{origin}/v1/models"), true, DISCOVERY_LIMIT)
                .await?;
            let mut models: Vec<Served> = Vec::new();
            for item in catalog["data"].as_array().ok_or(E::Protocol)? {
                let id = item["id"].as_str().ok_or(E::Protocol)?;
                if id.is_empty() || id.len() > 256 || id.chars().any(char::is_control) {
                    return Err(E::Protocol);
                }
                let images = item["architecture"]["input_modalities"]
                    .as_array()
                    .is_some_and(|modalities| modalities.iter().any(|m| m == "image"));
                if !models.iter().any(|m| m.id == id) {
                    models.push(Served {
                        id: id.to_owned(),
                        images,
                        checked_props: false,
                        context_window_tokens: item["meta"]["n_ctx"].as_u64().filter(|n| *n > 0),
                    });
                }
            }
            if models.is_empty() {
                return Err(E::ModelUnavailable);
            }
            Ok(models)
        };
        tokio::time::timeout(Duration::from_secs(10), discovery)
            .await
            .map_err(|_| E::Unreachable)?
    }
}

#[async_trait]
impl ModelProvider for ResponsesModel {
    /// llama-server: its readiness and served models, as before a request
    /// (`/health`, `/v1/models`); no conversation is sent. OpenAI cannot be
    /// checked without a request.
    async fn health(&self, cancel: CancellationToken) -> ModelHealth {
        let Flavor::LlamaCpp { origin, model, .. } = &self.flavor else {
            return ModelHealth::Unknown;
        };
        let served = tokio::select! {
            biased;
            () = cancel.cancelled() => return ModelHealth::Unavailable("check cancelled".into()),
            served = self.discover(origin) => served,
        };
        match served {
            Err(error) => ModelHealth::Unavailable(error.to_string()),
            Ok(served) => match model {
                Some(model) if !served.iter().any(|served| &served.id == model) => {
                    let ids: Vec<&str> = served.iter().map(|served| served.id.as_str()).collect();
                    ModelHealth::Unavailable(format!(
                        "the server does not serve `{model}`; it serves {}",
                        ids.join(", ")
                    ))
                }
                None if served.len() > 1 => ModelHealth::Unavailable(
                    "the server serves several models; name one in `model`".into(),
                ),
                _ => ModelHealth::Reachable,
            },
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
            model = self.model(request.model.clone()) => model?,
        };
        let llamacpp = self.is_llamacpp();
        let names = ToolNames::new(&request.tools);
        let body = json!({
            "model": model.id,
            "input": wire::input(&request.items, &names, llamacpp, model.images)?,
            "tools": wire::tools(&request.tools, &names)?,
            "stream": true,
            "store": false,
        });
        let mut post = self
            .client
            .post(&self.endpoint)
            .header(CONTENT_TYPE, "application/json")
            .header(ACCEPT, "text/event-stream")
            .body(body.to_string());
        if !self.key.is_empty() {
            post = post.bearer_auth(&self.key);
        }
        let mut timing = RequestTiming {
            started: Instant::now(),
            last_received: None,
            limits: self.timeouts,
        };
        let response = tokio::select! {
            biased;
            () = cancel.cancelled() => return Err(ModelError::Cancelled),
            response = post.send() => response,
        };
        let response = match response {
            Ok(response) => response,
            Err(error) => {
                let failure = timing.error(error, false);
                self.forget_discovery().await;
                return Err(failure);
            }
        };
        timing.last_received = Some(Instant::now());
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
        Ok(Box::pin(events(
            response,
            names,
            llamacpp,
            model.context_window_tokens,
            timing,
            cancel,
        )))
    }
}

fn events(
    response: reqwest::Response,
    names: ToolNames,
    llamacpp: bool,
    context_window_tokens: Option<u64>,
    mut timing: RequestTiming,
    cancel: CancellationToken,
) -> impl Stream<Item = Result<ModelEvent, ModelError>> + Send + 'static {
    async_stream::try_stream! {
        let mut bytes = response.bytes_stream();
        let mut sse = Sse::default();
        let mut parsed = Response::default();
        let mut normalizer = llamacpp.then(EventNormalizer::default);
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
                Some(Err(Some(error))) => Err(timing.error(error, llamacpp))?,
                Some(Ok(chunk)) => chunk,
            };
            timing.last_received = Some(Instant::now());
            total += chunk.len();
            if total > MAX_BYTES * 2 {
                Err::<(), _>(ModelError::from(E::Sse))?;
            }
            for event in sse.push(&chunk).map_err(ModelError::from)? {
                let normalized = match normalizer.as_mut() {
                    Some(normalizer) => normalizer.normalize(event).map_err(ModelError::from)?,
                    None => vec![event],
                };
                for event in normalized {
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
        }
        if !completed {
            if llamacpp {
                Err::<(), _>(ModelError::StreamInterrupted(
                    "the HTTP response ended before the model completed".into(),
                ))?;
            }
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
        if let Some(mut usage) = parsed.usage() {
            usage.context_window_tokens = context_window_tokens;
            yield ModelEvent::Usage { usage };
        }
        yield ModelEvent::Completed;
    }
}
