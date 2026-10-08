//! Separate local-provider configuration; never reads official OpenAI credentials.
use crate::transport_instance;
use async_trait::async_trait;
use futures::StreamExt;
use rah_protocol::{RuntimeDiagnostic, RuntimeFailureKind, RuntimeOperation};
use rah_runtime::{RuntimeFailure, experimental::*};
use std::{fmt, sync::Arc, time::Duration};

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum LlamaCppError {
    #[error("llama.cpp endpoint must be a loopback HTTP(S) origin")]
    InvalidEndpoint,
    #[error("llama.cpp server is unreachable")]
    Unreachable,
    #[error("llama.cpp server is loading its model")]
    Loading,
    #[error("llama.cpp server returned an incompatible response")]
    Malformed,
    #[error("llama.cpp model is unavailable; configure a listed model")]
    ModelUnavailable,
    #[error("llama.cpp server rejected authentication")]
    Authentication,
}
impl LlamaCppError {
    pub(crate) fn failure(self, operation: RuntimeOperation) -> RuntimeFailure {
        let kind = match self {
            Self::InvalidEndpoint => RuntimeFailureKind::InvalidConfiguration,
            Self::Unreachable => RuntimeFailureKind::Transport,
            Self::Loading => RuntimeFailureKind::Unavailable,
            Self::Malformed => RuntimeFailureKind::Protocol,
            Self::ModelUnavailable | Self::Authentication => RuntimeFailureKind::ProviderRejection,
        };
        RuntimeFailure::new(
            RuntimeDiagnostic {
                operation,
                kind,
                rpc_code: None,
            },
            self,
        )
    }
}

/// Loopback-only native Responses factory. The optional key belongs to this server.
/// Official `OpenAiFactory` retains its fixed origin and separate required key.
pub struct LlamaCppFactory {
    endpoint: String,
    key: String,
}
impl fmt::Debug for LlamaCppFactory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LlamaCppFactory").finish_non_exhaustive()
    }
}
impl LlamaCppFactory {
    pub fn new(endpoint: impl Into<String>, server_key: Option<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            key: server_key.unwrap_or_default(),
        }
    }
    fn origin(&self) -> Result<String, LlamaCppError> {
        let url =
            reqwest::Url::parse(&self.endpoint).map_err(|_| LlamaCppError::InvalidEndpoint)?;
        let loopback = url.host_str().is_some_and(|host| {
            host == "localhost"
                || host
                    .trim_matches(['[', ']'])
                    .parse::<std::net::IpAddr>()
                    .is_ok_and(|ip| ip.is_loopback())
        });
        if !loopback
            || !matches!(url.scheme(), "http" | "https")
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || url.path() != "/"
            || url.port() == Some(0)
        {
            return Err(LlamaCppError::InvalidEndpoint);
        }
        Ok(url.as_str().trim_end_matches('/').to_owned())
    }
}
async fn json(
    client: &reqwest::Client,
    url: String,
    key: &str,
) -> Result<serde_json::Value, LlamaCppError> {
    let request = client.get(url);
    let request = if key.is_empty() {
        request
    } else {
        request.bearer_auth(key)
    };
    let response = request
        .send()
        .await
        .map_err(|_| LlamaCppError::Unreachable)?;
    match response.status().as_u16() {
        200 => {}
        503 => return Err(LlamaCppError::Loading),
        401 | 403 => return Err(LlamaCppError::Authentication),
        _ => return Err(LlamaCppError::Malformed),
    }
    let mut stream = response.bytes_stream();
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| LlamaCppError::Unreachable)?;
        if bytes.len() + chunk.len() > 64 * 1024 {
            return Err(LlamaCppError::Malformed);
        }
        bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&bytes).map_err(|_| LlamaCppError::Malformed)
}
#[async_trait]
impl ConfiguredRuntimeFactory for LlamaCppFactory {
    fn validate(&self) -> Result<(), RuntimeFailure> {
        self.origin()
            .map_err(|e| e.failure(RuntimeOperation::Connection))?;
        if !self.key.is_empty()
            && reqwest::header::HeaderValue::from_str(&format!("Bearer {}", self.key)).is_err()
        {
            return Err(LlamaCppError::Authentication.failure(RuntimeOperation::Connection));
        }
        Ok(())
    }
    async fn create(&self) -> Result<Arc<dyn RuntimeInstance>, RuntimeFailure> {
        self.validate()?;
        let origin = self
            .origin()
            .map_err(|e| e.failure(RuntimeOperation::Connection))?;
        let ready = async {
            let client = reqwest::Client::builder()
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .retry(reqwest::retry::never())
                .timeout(Duration::from_secs(5))
                .build()
                .map_err(|_| LlamaCppError::Unreachable)?;
            let health = json(&client, format!("{origin}/health"), "").await?;
            if health["status"] != "ok" {
                return Err(LlamaCppError::Malformed);
            }
            let catalog = json(&client, format!("{origin}/v1/models"), &self.key).await?;
            let data = catalog["data"].as_array().ok_or(LlamaCppError::Malformed)?;
            let mut models = Vec::new();
            for item in data {
                let id = item["id"].as_str().ok_or(LlamaCppError::Malformed)?;
                if id.is_empty() || id.len() > 256 || id.chars().any(char::is_control) {
                    return Err(LlamaCppError::Malformed);
                }
                if !models.iter().any(|m| m == id) {
                    models.push(id.to_owned());
                }
            }
            if models.is_empty() {
                return Err(LlamaCppError::ModelUnavailable);
            }
            Ok(models)
        };
        let models = tokio::time::timeout(Duration::from_secs(10), ready)
            .await
            .map_err(|_| LlamaCppError::Unreachable.failure(RuntimeOperation::Connection))?
            .map_err(|e| e.failure(RuntimeOperation::Connection))?;
        transport_instance(
            format!("{origin}/v1/responses"),
            self.key.clone(),
            Some(models),
        )
    }
}

/// llama-server omits output_index and the separate argument-done notification.
/// Resolve only by the previously observed item ID; the ordinary parser still
/// checks complete arguments, call ID, name, duplicate completion and final output.
/// This normalization never applies to official OpenAI streams.
#[derive(Default)]
pub(crate) struct EventNormalizer {
    calls: std::collections::HashMap<String, (u64, bool)>,
    items: std::collections::HashSet<String>,
    indices: std::collections::HashSet<u64>,
    next_index: u64,
}
impl EventNormalizer {
    pub(crate) fn normalize(
        &mut self,
        mut event: serde_json::Value,
    ) -> Result<Vec<serde_json::Value>, crate::OpenAiAdapterError> {
        use crate::{OpenAiAdapterError as E, protocol::string};
        let kind = string(&event, "type")?.to_owned();
        if kind == "response.output_item.added" {
            let id = string(&event["item"], "id")?.to_owned();
            if self.items.len() >= crate::protocol::MAX_CALLS * 3 || !self.items.insert(id.clone())
            {
                return Err(E::Protocol);
            }
            let index = match event.get("output_index") {
                Some(value) => value.as_u64().ok_or(E::Protocol)?,
                None => self.next_index,
            };
            if index >= crate::protocol::MAX_CALLS as u64 * 3 || !self.indices.insert(index) {
                return Err(E::Protocol);
            }
            self.next_index = self.next_index.max(index + 1);
            if event["item"]["type"] == "function_call" {
                self.calls.insert(id, (index, false));
            }
            event["output_index"] = index.into();
        } else if matches!(
            kind.as_str(),
            "response.function_call_arguments.delta" | "response.function_call_arguments.done"
        ) || kind == "response.output_item.done"
            && event["item"]["type"] == "function_call"
        {
            let id = if kind == "response.output_item.done" {
                string(&event["item"], "id")?
            } else {
                string(&event, "item_id")?
            }
            .to_owned();
            let (index, args_done) = self.calls.get_mut(&id).ok_or(E::Protocol)?;
            if event
                .get("output_index")
                .is_some_and(|v| v.as_u64() != Some(*index))
            {
                return Err(E::Protocol);
            }
            event["output_index"] = (*index).into();
            if kind == "response.function_call_arguments.done" {
                *args_done = true;
            }
            if kind == "response.output_item.done" && !*args_done {
                *args_done = true;
                let arguments = string(&event["item"], "arguments")?;
                let done = serde_json::json!({"type":"response.function_call_arguments.done","item_id":id,"output_index":index,"arguments":arguments});
                return Ok(vec![done, event]);
            }
        }
        Ok(vec![event])
    }
}
