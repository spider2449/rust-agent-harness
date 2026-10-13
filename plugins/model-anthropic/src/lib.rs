//! Anthropic Messages API model plugin.
//!
//! `model-anthropic` ([`AnthropicPlugin`]) calls the Messages API with an API
//! key, billed as API usage. Configuration (all optional):
//!
//! ```json
//! { "model": "claude-opus-5-5", "api_key_env": "ANTHROPIC_API_KEY",
//!   "endpoint": "<url>", "max_tokens": 64000, "effort": "high",
//!   "refusal_fallback": true }
//! ```
//!
//! The key is read from the named environment variable when the plugin
//! starts. `endpoint` may only be the official endpoint or a loopback URL, so
//! credentials never go elsewhere. `effort` is sent as `output_config.effort`
//! when set; otherwise the model's default applies. `refusal_fallback` (on by
//! default) asks the API to rerun a declined request on a fallback model, for
//! the models that support it.
//!
//! Requests stream, use automatic prompt caching and are never retried.
//! Thinking blocks are not replayed, because neutral context does not carry
//! them; a turn that follows tool calls is answered without the reasoning
//! that led to them.

mod client;
mod error;
mod sse;
mod wire;

use std::{net::IpAddr, sync::Arc};

use rah_api::{Plugin, PluginError, PluginId, PluginManifest, Scope, async_trait, model::MODEL};
use serde::Deserialize;

pub use client::AnthropicModel;
use client::Settings;

pub const DEFAULT_MODEL: &str = "claude-opus-5-5";
pub const DEFAULT_MAX_TOKENS: u32 = 64_000;
const MAX_OUTPUT_TOKENS: u32 = 128_000;
const EFFORTS: [&str; 5] = ["low", "medium", "high", "xhigh", "max"];

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

/// The Messages endpoint `model-anthropic` may send credentials to: the
/// official endpoint (default) or a loopback URL (local gateways, fixtures).
pub fn anthropic_endpoint(configured: Option<&str>) -> Result<String, String> {
    let Some(configured) = configured else {
        return Ok(client::ANTHROPIC_ENDPOINT.to_owned());
    };
    if configured == client::ANTHROPIC_ENDPOINT {
        return Ok(configured.to_owned());
    }
    match reqwest::Url::parse(configured) {
        Ok(url) if plain_http(&url) && is_loopback(&url) => Ok(configured.to_owned()),
        _ => Err(format!(
            "endpoint must be {} or a loopback URL",
            client::ANTHROPIC_ENDPOINT
        )),
    }
}

fn read_key(variable: &str) -> Result<String, String> {
    let key = std::env::var(variable).unwrap_or_default();
    if key.trim().is_empty() {
        return Err(format!(
            "{variable} is not set; configure the API key in the backend environment"
        ));
    }
    if reqwest::header::HeaderValue::from_str(&key).is_err() {
        return Err(format!("{variable} is not a valid API key"));
    }
    Ok(key)
}

pub struct AnthropicPlugin {
    manifest: PluginManifest,
}

impl AnthropicPlugin {
    pub fn new(id: impl Into<PluginId>) -> Self {
        Self {
            manifest: PluginManifest::new(id, env!("CARGO_PKG_VERSION")).provides(["model"]),
        }
    }
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    model: Option<String>,
    api_key_env: Option<String>,
    endpoint: Option<String>,
    max_tokens: Option<u32>,
    effort: Option<String>,
    refusal_fallback: Option<bool>,
}

impl Config {
    fn settings(self) -> Result<Settings, String> {
        let model = self
            .model
            .filter(|model| !model.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_MODEL.to_owned());
        let max_tokens = self.max_tokens.unwrap_or(DEFAULT_MAX_TOKENS);
        if !(1..=MAX_OUTPUT_TOKENS).contains(&max_tokens) {
            return Err(format!(
                "max_tokens must be between 1 and {MAX_OUTPUT_TOKENS}"
            ));
        }
        let effort = self.effort.filter(|effort| !effort.is_empty());
        if let Some(effort) = &effort
            && !EFFORTS.contains(&effort.as_str())
        {
            return Err(format!("effort must be one of {}", EFFORTS.join(", ")));
        }
        Ok(Settings {
            model,
            max_tokens,
            effort,
            refusal_fallback: self.refusal_fallback.unwrap_or(true),
        })
    }
}

#[async_trait]
impl Plugin for AnthropicPlugin {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    async fn apply(&self, scope: &mut Scope) -> Result<(), PluginError> {
        let config: Config = if scope.config().is_null() {
            Config::default()
        } else {
            scope.config_as()?
        };
        let variable = config
            .api_key_env
            .clone()
            .unwrap_or_else(|| "ANTHROPIC_API_KEY".to_owned());
        let key = read_key(&variable)?;
        let endpoint = anthropic_endpoint(config.endpoint.as_deref())?;
        let model = AnthropicModel::new(endpoint, key, config.settings()?)?;
        scope.provide(MODEL, Arc::new(model))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
