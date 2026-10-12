//! The `rah` distribution: a plugin catalog, `rah.toml` parsing, presets and
//! boot. Everything it mounts is a plugin; this crate only composes.

use std::{collections::BTreeMap, sync::Arc};

use rah_api::{Kernel, MountOptions, Plugin, PluginId, PluginStatus, kernel::KernelError};
use rah_plugin_frontend_cli::CliFrontendPlugin;
use rah_plugin_loop_react::ReactLoopPlugin;
use rah_plugin_model_mock::MockModelPlugin;
use rah_plugin_model_responses::{LlamaCppPlugin, OpenAiResponsesPlugin};
use rah_plugin_policy_authority::PolicyAuthorityPlugin;
use rah_plugin_sandbox_process::SandboxProcessPlugin;
use rah_plugin_session_jsonl::JsonlSessionPlugin;
use rah_plugin_tools_builtin::BuiltinToolsPlugin;
use rah_plugin_tools_shell::ShellToolsPlugin;
use serde::Deserialize;
use serde_json::Value;

/// Builds a plugin instance with the given id.
pub type Constructor = fn(PluginId) -> Arc<dyn Plugin>;

/// Plugins compiled into this distribution, by `use` name.
pub fn catalog() -> BTreeMap<&'static str, Constructor> {
    let entries: [(&'static str, Constructor); 10] = [
        ("frontend-cli", |id| Arc::new(CliFrontendPlugin::new(id))),
        ("loop-react", |id| Arc::new(ReactLoopPlugin::new(id))),
        ("model-llamacpp", |id| Arc::new(LlamaCppPlugin::new(id))),
        ("model-mock", |id| Arc::new(MockModelPlugin::new(id))),
        ("model-openai", |id| {
            Arc::new(OpenAiResponsesPlugin::new(id))
        }),
        ("policy-authority", |id| {
            Arc::new(PolicyAuthorityPlugin::new(id))
        }),
        ("sandbox-process", |id| {
            Arc::new(SandboxProcessPlugin::new(id))
        }),
        ("session-jsonl", |id| Arc::new(JsonlSessionPlugin::new(id))),
        ("tools-builtin", |id| Arc::new(BuiltinToolsPlugin::new(id))),
        ("tools-shell", |id| Arc::new(ShellToolsPlugin::new(id))),
    ];
    entries.into_iter().collect()
}

/// A built-in preset: a named composition of catalog plugins.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Preset {
    pub name: &'static str,
    /// The model plugin it mounts.
    pub model: &'static str,
    /// Whether it mounts `policy-authority`.
    pub guarded: bool,
}

/// Built-in presets. All share built-in tools, JSONL sessions, the default
/// loop and the terminal frontend. Every preset except `minimal` mounts
/// `policy-authority`; `minimal` runs unguarded and `rah` warns about it.
pub const PRESETS: [Preset; 4] = [
    Preset {
        name: "default",
        model: "model-llamacpp",
        guarded: true,
    },
    Preset {
        name: "dev",
        model: "model-mock",
        guarded: true,
    },
    Preset {
        name: "minimal",
        model: "model-mock",
        guarded: false,
    },
    Preset {
        name: "openai",
        model: "model-openai",
        guarded: true,
    },
];

/// Mounted first, so no tool call can run before the policy is in place.
/// Every built-in tool has an explicit assignment; anything else is denied.
const POLICY_TEMPLATE: &str = r#"
[[plugin]]
use = "policy-authority"
config = { allow = ["none", "read"], ask = ["write", "execute"], tools = { echo = "none", "fs.read" = "read" } }
"#;

const PRESET_TEMPLATE: &str = r#"
[[plugin]]
use = "session-jsonl"
config = { dir = ".rah/sessions" }

[[plugin]]
use = "tools-builtin"
config = { root = "." }

[[plugin]]
use = "{MODEL}"

[[plugin]]
use = "loop-react"

[[plugin]]
use = "frontend-cli"
"#;

#[derive(Debug, thiserror::Error)]
pub enum BootError {
    #[error("invalid distribution: {0}")]
    Parse(String),
    #[error("unknown preset `{0}`; available: default, dev, minimal, openai")]
    UnknownPreset(String),
    #[error("unknown plugin `{0}`; available: {1}")]
    UnknownPlugin(String, String),
    #[error("policy plugin `{0}` is not active ({1}); refusing to run unguarded")]
    PolicyInactive(PluginId, String),
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
        let preset = PRESETS
            .iter()
            .find(|preset| preset.name == name)
            .ok_or_else(|| BootError::UnknownPreset(name.to_owned()))?;
        let policy = if preset.guarded { POLICY_TEMPLATE } else { "" };
        Self::parse(&format!(
            "{policy}{}",
            PRESET_TEMPLATE.replace("{MODEL}", preset.model)
        ))
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

/// Mounts every plugin in order and returns the resulting statuses. A
/// configured `policy-authority` that does not become active aborts the boot
/// before any later plugin (such as a frontend) is mounted.
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
        let status = kernel.mount(constructor(spec.plugin_id()), options).await?;
        if spec.kind == "policy-authority" && status != PluginStatus::Active {
            return Err(BootError::PolicyInactive(
                spec.plugin_id(),
                format!("{status:?}"),
            ));
        }
    }
    Ok(kernel.plugins())
}
