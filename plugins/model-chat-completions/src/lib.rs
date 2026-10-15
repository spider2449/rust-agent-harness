//! OpenAI-compatible Chat Completions model plugin.
//!
//! `model-chat-completions` ([`ChatCompletionsPlugin`]) streams
//! `POST <base_url>/chat/completions`, the format DeepSeek, Ollama, vLLM,
//! llama-server and many gateways serve. Configuration (all optional except
//! that a `base_url` or a `provider` is needed):
//!
//! ```json
//! { "provider": "deepseek", "base_url": "https://api.deepseek.com",
//!   "model": "deepseek-chat", "api_key_env": "DEEPSEEK_API_KEY",
//!   "max_tokens": 8192, "temperature": 0.7 }
//! ```
//!
//! `provider` fills in defaults for a known server, and the other fields
//! override them:
//!
//! | provider   | base_url                    | model           | key                               |
//! |------------|-----------------------------|-----------------|-----------------------------------|
//! | `deepseek` | `https://api.deepseek.com`  | `deepseek-chat` | `DEEPSEEK_API_KEY`, required      |
//! | `ollama`   | `http://127.0.0.1:11434/v1` | the sole served | none                              |
//! | `llamacpp` | `http://127.0.0.1:8080/v1`  | the sole served | `RAH_LLAMA_CPP_API_KEY`, optional |
//! | `vllm`     | `http://127.0.0.1:8000/v1`  | the sole served | none                              |
//!
//! The key is read from the environment variable `api_key_env` names when
//! the plugin starts, and is then required. The mounting configuration is
//! trusted, so `base_url` may name any server; plain HTTP is accepted only
//! for a loopback address, so a key never crosses the network in clear text.
//! Without a model, `<base_url>/models` is asked on the first request and its
//! sole model is used.
//!
//! Requests stream with `stream_options.include_usage` and are never retried.
//! Reasoning (`reasoning_content` or `reasoning`) is reported as a signal and
//! not replayed, because neutral context does not carry it. Images in tool
//! results are sent as a short text: many servers accept only text in a
//! `tool` message.

mod client;
mod error;
mod sse;
mod wire;

use std::{net::IpAddr, sync::Arc};

use rah_api::{Plugin, PluginError, PluginId, PluginManifest, Scope, async_trait, model::MODEL};
use serde::Deserialize;

pub use client::ChatCompletionsModel;
use client::Settings;

const MAX_OUTPUT_TOKENS: u32 = 1_000_000;

/// Defaults for a known server.
struct Provider {
    name: &'static str,
    base_url: &'static str,
    model: Option<&'static str>,
    /// The key variable and whether the key is required.
    key: Option<(&'static str, bool)>,
}

const PROVIDERS: [Provider; 4] = [
    Provider {
        name: "deepseek",
        base_url: "https://api.deepseek.com",
        model: Some("deepseek-chat"),
        key: Some(("DEEPSEEK_API_KEY", true)),
    },
    Provider {
        name: "ollama",
        base_url: "http://127.0.0.1:11434/v1",
        model: None,
        key: None,
    },
    Provider {
        name: "llamacpp",
        base_url: "http://127.0.0.1:8080/v1",
        model: None,
        key: Some(("RAH_LLAMA_CPP_API_KEY", false)),
    },
    Provider {
        name: "vllm",
        base_url: "http://127.0.0.1:8000/v1",
        model: None,
        key: None,
    },
];

fn is_loopback(url: &reqwest::Url) -> bool {
    url.host_str().is_some_and(|host| {
        host == "localhost"
            || host
                .trim_matches(['[', ']'])
                .parse::<IpAddr>()
                .is_ok_and(|ip| ip.is_loopback())
    })
}

/// Validates `base_url`: HTTPS, or plain HTTP to a loopback address; no
/// credentials, query or fragment. Returns it without a trailing slash and
/// whether it is a loopback address.
pub fn base_url(configured: &str) -> Result<(String, bool), String> {
    let invalid = || {
        format!(
            "base_url must be an https URL, or http to a loopback address \
             (such as http://127.0.0.1:8080/v1); got `{configured}`"
        )
    };
    let url = reqwest::Url::parse(configured).map_err(|_| invalid())?;
    let loopback = is_loopback(&url);
    let plain = url.username().is_empty()
        && url.password().is_none()
        && url.query().is_none()
        && url.fragment().is_none()
        && url.port() != Some(0);
    match url.scheme() {
        "https" if plain => {}
        "http" if plain && loopback => {}
        _ => return Err(invalid()),
    }
    Ok((url.as_str().trim_end_matches('/').to_owned(), loopback))
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

pub struct ChatCompletionsPlugin {
    manifest: PluginManifest,
}

impl ChatCompletionsPlugin {
    pub fn new(id: impl Into<PluginId>) -> Self {
        Self {
            manifest: PluginManifest::new(id, env!("CARGO_PKG_VERSION")).provides(["model"]),
        }
    }
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    provider: Option<String>,
    base_url: Option<String>,
    model: Option<String>,
    api_key_env: Option<String>,
    max_tokens: Option<u32>,
    temperature: Option<f64>,
}

impl Config {
    /// The settings and the key variable (with whether it is required).
    fn resolve(self) -> Result<(Settings, Option<(String, bool)>), String> {
        let provider = match self.provider.as_deref().filter(|name| !name.is_empty()) {
            Some(name) => Some(
                PROVIDERS
                    .iter()
                    .find(|provider| provider.name == name)
                    .ok_or_else(|| {
                        let names: Vec<&str> = PROVIDERS.iter().map(|p| p.name).collect();
                        format!("provider must be one of {}", names.join(", "))
                    })?,
            ),
            None => None,
        };
        let configured = self
            .base_url
            .filter(|url| !url.trim().is_empty())
            .or_else(|| provider.map(|provider| provider.base_url.to_owned()))
            .ok_or("configuration needs `base_url` or `provider`")?;
        let (base_url, loopback) = base_url(configured.trim())?;
        let model = self
            .model
            .filter(|model| !model.trim().is_empty())
            .or_else(|| provider.and_then(|provider| provider.model.map(str::to_owned)));
        if let Some(max_tokens) = self.max_tokens
            && !(1..=MAX_OUTPUT_TOKENS).contains(&max_tokens)
        {
            return Err(format!(
                "max_tokens must be between 1 and {MAX_OUTPUT_TOKENS}"
            ));
        }
        if let Some(temperature) = self.temperature
            && !(0.0..=2.0).contains(&temperature)
        {
            return Err("temperature must be between 0 and 2".into());
        }
        let key = match self.api_key_env.filter(|name| !name.trim().is_empty()) {
            Some(variable) => Some((variable, true)),
            None => provider
                .and_then(|provider| provider.key)
                .map(|(variable, required)| (variable.to_owned(), required)),
        };
        let settings = Settings {
            base_url,
            model,
            max_tokens: self.max_tokens,
            temperature: self.temperature,
            loopback,
        };
        Ok((settings, key))
    }
}

#[async_trait]
impl Plugin for ChatCompletionsPlugin {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    fn check_config(&self, config: &serde_json::Value) -> Result<(), PluginError> {
        let config: Config = rah_api::config::parse(config)?;
        config.resolve()?;
        Ok(())
    }

    async fn apply(&self, scope: &mut Scope) -> Result<(), PluginError> {
        let config: Config = rah_api::config::parse(scope.config())?;
        let (settings, key) = config.resolve()?;
        let key = match key {
            Some((variable, required)) => read_key(&variable, required)?,
            None => String::new(),
        };
        let model = ChatCompletionsModel::new(key, settings)?;
        scope.provide(MODEL, Arc::new(model))?;
        Ok(())
    }
}

impl ChatCompletionsPlugin {
    /// An example configuration, mostly the defaults, for reference (the Web
    /// UI shows it in an empty configuration editor). It is never applied.
    pub const CONFIG_EXAMPLE: &str = r#"{
  "provider": "deepseek",
  "base_url": "https://api.deepseek.com",
  "model": "deepseek-chat",
  "api_key_env": "DEEPSEEK_API_KEY"
}"#;
}

#[cfg(test)]
mod tests;

#[cfg(test)]
#[test]
fn the_config_example_is_a_valid_configuration() {
    let parsed: Result<Config, _> = serde_json::from_str(ChatCompletionsPlugin::CONFIG_EXAMPLE);
    assert!(parsed.is_ok(), "{:?}", parsed.err());
}
