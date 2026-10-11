//! The `rah` distribution: a plugin catalog, `rah.toml` parsing, presets and
//! boot. Everything it mounts is a plugin; this crate only composes.

use std::{collections::BTreeMap, sync::Arc};

use rah_api::{Kernel, MountOptions, Plugin, PluginId, PluginStatus, kernel::KernelError};
use rah_plugin_frontend_cli::CliFrontendPlugin;
use rah_plugin_loop_react::ReactLoopPlugin;
use rah_plugin_model_mock::MockModelPlugin;
use rah_plugin_session_jsonl::JsonlSessionPlugin;
use rah_plugin_tools_builtin::BuiltinToolsPlugin;
use serde::Deserialize;
use serde_json::Value;

/// Builds a plugin instance with the given id.
pub type Constructor = fn(PluginId) -> Arc<dyn Plugin>;

/// Plugins compiled into this distribution, by `use` name.
pub fn catalog() -> BTreeMap<&'static str, Constructor> {
    let entries: [(&'static str, Constructor); 5] = [
        ("frontend-cli", |id| Arc::new(CliFrontendPlugin::new(id))),
        ("loop-react", |id| Arc::new(ReactLoopPlugin::new(id))),
        ("model-mock", |id| Arc::new(MockModelPlugin::new(id))),
        ("session-jsonl", |id| Arc::new(JsonlSessionPlugin::new(id))),
        ("tools-builtin", |id| Arc::new(BuiltinToolsPlugin::new(id))),
    ];
    entries.into_iter().collect()
}

/// Development preset: mock model, built-in tools, JSONL sessions, the
/// default loop and the terminal frontend. It mounts no policy plugin.
pub const DEV_PRESET: &str = r#"
[[plugin]]
use = "session-jsonl"
config = { dir = ".rah/sessions" }

[[plugin]]
use = "tools-builtin"
config = { root = "." }

[[plugin]]
use = "model-mock"

[[plugin]]
use = "loop-react"

[[plugin]]
use = "frontend-cli"
"#;

#[derive(Debug, thiserror::Error)]
pub enum BootError {
    #[error("invalid distribution: {0}")]
    Parse(String),
    #[error("unknown preset `{0}`; available: dev")]
    UnknownPreset(String),
    #[error("unknown plugin `{0}`; available: {1}")]
    UnknownPlugin(String, String),
    #[error(transparent)]
    Kernel(#[from] KernelError),
}

/// One `[[plugin]]` entry of `rah.toml`.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PluginSpec {
    /// Catalog name of the plugin.
    #[serde(rename = "use")]
    pub kind: String,
    /// Instance id; defaults to the catalog name.
    pub id: Option<String>,
    #[serde(default)]
    pub config: Value,
    /// Selects provider instances for dependencies: `{ service = "instance" }`.
    #[serde(default)]
    pub bind: BTreeMap<String, String>,
}

impl PluginSpec {
    pub fn plugin_id(&self) -> PluginId {
        PluginId::new(self.id.clone().unwrap_or_else(|| self.kind.clone()))
    }
}

/// An ordered list of plugins to mount.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Distribution {
    #[serde(default)]
    pub plugin: Vec<PluginSpec>,
}

impl Distribution {
    pub fn parse(text: &str) -> Result<Self, BootError> {
        toml::from_str(text).map_err(|error| BootError::Parse(error.to_string()))
    }

    pub fn preset(name: &str) -> Result<Self, BootError> {
        match name {
            "dev" => Self::parse(DEV_PRESET),
            other => Err(BootError::UnknownPreset(other.to_owned())),
        }
    }

    /// Sets `key` in the configuration of every plugin of `kind`. Returns
    /// whether any plugin matched.
    pub fn set_config(&mut self, kind: &str, key: &str, value: Value) -> bool {
        let mut matched = false;
        for spec in self.plugin.iter_mut().filter(|spec| spec.kind == kind) {
            if !spec.config.is_object() {
                spec.config = Value::Object(serde_json::Map::new());
            }
            if let Value::Object(map) = &mut spec.config {
                map.insert(key.to_owned(), value.clone());
                matched = true;
            }
        }
        matched
    }
}

/// Mounts every plugin in order and returns the resulting statuses.
pub async fn boot(
    kernel: &Kernel,
    distribution: &Distribution,
) -> Result<Vec<(PluginId, PluginStatus)>, BootError> {
    let catalog = catalog();
    for spec in &distribution.plugin {
        let constructor = catalog.get(spec.kind.as_str()).ok_or_else(|| {
            let names: Vec<&str> = catalog.keys().copied().collect();
            BootError::UnknownPlugin(spec.kind.clone(), names.join(", "))
        })?;
        let mut options = MountOptions::with_config(spec.config.clone());
        for (service, instance) in &spec.bind {
            options = options.bind(service.as_str(), instance.clone());
        }
        kernel.mount(constructor(spec.plugin_id()), options).await?;
    }
    Ok(kernel.plugins())
}
