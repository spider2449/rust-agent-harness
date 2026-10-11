use std::time::Duration;

use futures::{Stream, StreamExt};
use rah_api::{
    CancellationToken, async_trait,
    model::{ModelError, ModelProvider, ModelStream},
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

enum Flavor {
    OpenAi {
        model: String,
    },
    LlamaCpp {
        origin: String,
        model: Option<String>,
        /// Served model ids, discovered lazily and dropped on transport errors.
        served: Mutex<Option<Vec<String>>>,
    },
}

/// A Responses API model. Credentials are never exposed through Debug,
/// Display or errors.
pub struct ResponsesModel {
    client: reqwest::Client,
    endpoint: String,
    key: String,
    flavor: Flavor,
}

fn build_client(https_only: bool) -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .https_only(https_only)
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .no_proxy()
        .connect_timeout(Duration::from_secs(30))
        .read_timeout(Duration::from_secs(120))
        .timeout(Duration::from_secs(600))
        .build()
        .map_err(|_| "cannot build the HTTP client".to_owned())
}

impl ResponsesModel {
    pub(crate) fn openai(endpoint: String, key: String, model: String) -> Result<Self, String> {
        Ok(Self {
            client: build_client(endpoint == OPENAI_ENDPOINT)?,
            endpoint,
            key,
            flavor: Flavor::OpenAi { model },
        })
    }

    pub(crate) fn llamacpp(
        origin: String,
        key: String,
        model: Option<String>,
    ) -> Result<Self, String> {
        Ok(Self {
            client: build_client(false)?,
            endpoint: format!("{origin}/v1/responses"),
            key,
            flavor: Flavor::LlamaCpp {
                origin,
                model,
                served: Mutex::new(None),
            },
        })
    }

    fn is_llamacpp(&self) -> bool {
        matches!(self.flavor, Flavor::LlamaCpp { .. })
    }

    async fn model(&self, requested: Option<String>) -> Result<String, E> {
        match &self.flavor {
            Flavor::OpenAi { model } => Ok(requested.unwrap_or_else(|| model.clone())),
            Flavor::LlamaCpp {
                origin,
                model,
                served,
            } => {
                let mut cache = served.lock().await;
                if cache.is_none() {
                    *cache = Some(self.discover(origin).await?);
                }
                let served = cache.as_deref().unwrap_or_default();
                match requested.or_else(|| model.clone()) {
                    Some(model) if served.contains(&model) => Ok(model),
                    Some(_) => Err(E::ModelUnavailable),
                    None if served.len() == 1 => Ok(served[0].clone()),
                    None => Err(E::ModelUnavailable),
                }
            }
        }
    }

    async fn forget_discovery(&self) {
        if let Flavor::LlamaCpp { served, .. } = &self.flavor {
            *served.lock().await = None;
        }
    }

    async fn get_json(&self, url: String, authenticated: bool) -> Result<Value, E> {
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
            if bytes.len() + chunk.len() > DISCOVERY_LIMIT {
                return Err(E::Protocol);
            }
            bytes.extend_from_slice(&chunk);
        }
        serde_json::from_slice(&bytes).map_err(|_| E::EventJson)
    }

    /// llama-server readiness (`/health`) and served models (`/v1/models`).
    async fn discover(&self, origin: &str) -> Result<Vec<String>, E> {
        let discovery = async {
            let health = self.get_json(format!("{origin}/health"), false).await?;
            if health["status"] != "ok" {
                return Err(E::Loading);
            }
            let catalog = self.get_json(format!("{origin}/v1/models"), true).await?;
            let mut models: Vec<String> = Vec::new();
            for item in catalog["data"].as_array().ok_or(E::Protocol)? {
                let id = item["id"].as_str().ok_or(E::Protocol)?;
                if id.is_empty() || id.len() > 256 || id.chars().any(char::is_control) {
                    return Err(E::Protocol);
                }
                if !models.iter().any(|m| m == id) {
                    models.push(id.to_owned());
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
            "model": model,
            "input": wire::input(&request.items, &names, llamacpp)?,
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
        Ok(Box::pin(events(response, names, llamacpp, cancel)))
    }
}

fn events(
    response: reqwest::Response,
    names: ToolNames,
    llamacpp: bool,
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
                Some(Err(Some(_))) => Err(ModelError::from(E::Transport))?,
                Some(Ok(chunk)) => chunk,
            };
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
        yield ModelEvent::Completed;
    }
}
