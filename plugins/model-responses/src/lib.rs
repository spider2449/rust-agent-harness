//! Responses API model plugins.
//!
//! - `model-openai` ([`OpenAiResponsesPlugin`]): the official OpenAI Responses
//!   API. Configuration: `{ "model": "<id>", "api_key_env": "OPENAI_API_KEY",
//!   "endpoint": "<url>" }`. The key is read from the named environment
//!   variable when the plugin starts. `endpoint` may only be the official
//!   endpoint or a loopback URL, so credentials never go elsewhere.
//! - `model-llamacpp` ([`LlamaCppPlugin`]): a local llama-server.
//!   Configuration: `{ "endpoint": "http://127.0.0.1:8080", "model": "<id>",
//!   "api_key_env": "<VAR>" }`. The endpoint must be a loopback origin.
//!   Readiness and served models are checked lazily on the first request, so
//!   the plugin can start before the server. Without `model`, the sole served
//!   model is used. The optional key defaults to `RAH_LLAMA_CPP_API_KEY` and
//!   never falls back to the OpenAI key.
//!
//! Both translate neutral context to Responses input, encode tool names to
//! the provider-safe alphabet and map them back, and validate every streamed
//! event. Requests use `store: false`; nothing is retried.

mod client;
mod error;
mod sse;
mod wire;

use std::{net::IpAddr, sync::Arc};

use rah_api::{Plugin, PluginError, PluginId, PluginManifest, Scope, async_trait, model::MODEL};
use serde::Deserialize;

pub use client::ResponsesModel;

pub const DEFAULT_LLAMACPP_ENDPOINT: &str = "http://127.0.0.1:8080";

fn is_loopback(url: &reqwest::Url) -> bool {
    url.host_str().is_some_and(|host| {
        host == "localhost"
            || host
                .trim_matches(['[', ']'])
                .parse::<IpAddr>()
                .is_ok_and(|ip| ip.is_loopback())
    })
}

fn plain_http(url: &reqwest::Url) -> bool {
    matches!(url.scheme(), "http" | "https")
        && url.username().is_empty()
        && url.password().is_none()
        && url.query().is_none()
        && url.fragment().is_none()
        && url.port() != Some(0)
}

/// The Responses endpoint `model-openai` may send credentials to: the
/// official endpoint (default) or a loopback URL (local gateways, fixtures).
pub fn openai_endpoint(configured: Option<&str>) -> Result<String, String> {
    let Some(configured) = configured else {
        return Ok(client::OPENAI_ENDPOINT.to_owned());
    };
    if configured == client::OPENAI_ENDPOINT {
        return Ok(configured.to_owned());
    }
    match reqwest::Url::parse(configured) {
        Ok(url) if plain_http(&url) && is_loopback(&url) => Ok(configured.to_owned()),
        _ => Err(format!(
            "endpoint must be {} or a loopback URL",
            client::OPENAI_ENDPOINT
        )),
    }
}

/// Validates a llama-server origin: loopback, http(s), root path only.
pub fn llamacpp_origin(endpoint: &str) -> Result<String, String> {
    match reqwest::Url::parse(endpoint) {
        Ok(url) if plain_http(&url) && is_loopback(&url) && url.path() == "/" => {
            Ok(url.as_str().trim_end_matches('/').to_owned())
        }
        _ => Err("endpoint must be a loopback HTTP(S) origin such as http://127.0.0.1:8080".into()),
    }
}

fn read_key(variable: &str, required: bool) -> Result<String, String> {
    let key = std::env::var(variable).unwrap_or_default();
    if key.trim().is_empty() {
        return if required {
            Err(format!(
                "{variable} is not set; configure the API key in the backend environment"
            ))
        } else {
            Ok(String::new())
        };
    }
    if reqwest::header::HeaderValue::from_str(&format!("Bearer {key}")).is_err() {
        return Err(format!("{variable} is not a valid API key"));
    }
    Ok(key)
}

pub struct OpenAiResponsesPlugin {
    manifest: PluginManifest,
}

impl OpenAiResponsesPlugin {
    pub fn new(id: impl Into<PluginId>) -> Self {
        Self {
            manifest: PluginManifest::new(id, env!("CARGO_PKG_VERSION")).provides(["model"]),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OpenAiConfig {
    model: String,
    api_key_env: Option<String>,
    endpoint: Option<String>,
}

#[async_trait]
impl Plugin for OpenAiResponsesPlugin {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    async fn apply(&self, scope: &mut Scope) -> Result<(), PluginError> {
        let config: OpenAiConfig = scope
            .config_as()
            .map_err(|error| format!("configuration needs `model`: {error}"))?;
        let variable = config.api_key_env.as_deref().unwrap_or("OPENAI_API_KEY");
        let key = read_key(variable, true)?;
        let endpoint = openai_endpoint(config.endpoint.as_deref())?;
        let model = ResponsesModel::openai(endpoint, key, config.model)?;
        scope.provide(MODEL, Arc::new(model))?;
        Ok(())
    }
}

pub struct LlamaCppPlugin {
    manifest: PluginManifest,
}

impl LlamaCppPlugin {
    pub fn new(id: impl Into<PluginId>) -> Self {
        Self {
            manifest: PluginManifest::new(id, env!("CARGO_PKG_VERSION")).provides(["model"]),
        }
    }
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct LlamaConfig {
    endpoint: Option<String>,
    model: Option<String>,
    api_key_env: Option<String>,
}

#[async_trait]
impl Plugin for LlamaCppPlugin {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    async fn apply(&self, scope: &mut Scope) -> Result<(), PluginError> {
        let config: LlamaConfig = if scope.config().is_null() {
            LlamaConfig::default()
        } else {
            scope.config_as()?
        };
        let origin = llamacpp_origin(
            config
                .endpoint
                .as_deref()
                .unwrap_or(DEFAULT_LLAMACPP_ENDPOINT),
        )?;
        let key = match config.api_key_env.as_deref() {
            Some(variable) => read_key(variable, true)?,
            None => read_key("RAH_LLAMA_CPP_API_KEY", false)?,
        };
        let model = ResponsesModel::llamacpp(origin, key, config.model)?;
        scope.provide(MODEL, Arc::new(model))?;
        Ok(())
    }
}

impl LlamaCppPlugin {
    /// An example configuration, mostly the defaults, for reference (the Web
    /// UI shows it in an empty configuration editor). It is never applied.
    pub const CONFIG_EXAMPLE: &str = r#"{
  "endpoint": "http://127.0.0.1:8080",
  "model": "<served model id>",
  "api_key_env": "RAH_LLAMA_CPP_API_KEY"
}"#;
}

impl OpenAiResponsesPlugin {
    /// An example configuration, mostly the defaults, for reference (the Web
    /// UI shows it in an empty configuration editor). It is never applied.
    pub const CONFIG_EXAMPLE: &str = r#"{
  "model": "<model id>",
  "api_key_env": "OPENAI_API_KEY"
}"#;
}

#[cfg(test)]
mod tests;

#[cfg(test)]
#[test]
fn the_config_example_is_a_valid_configuration_llamacpp() {
    let parsed: Result<LlamaConfig, _> = serde_json::from_str(LlamaCppPlugin::CONFIG_EXAMPLE);
    assert!(parsed.is_ok(), "{:?}", parsed.err());
}

#[cfg(test)]
#[test]
fn the_config_example_is_a_valid_configuration_openai() {
    let parsed: Result<OpenAiConfig, _> =
        serde_json::from_str(OpenAiResponsesPlugin::CONFIG_EXAMPLE);
    assert!(parsed.is_ok(), "{:?}", parsed.err());
}
