//! The `rah` distribution: a plugin catalog, `rah.toml` parsing, presets and
//! boot. Everything it mounts is a plugin; this crate only composes. It also
//! mounts the `distribution` plugin, whose `admin` service lets frontends
//! manage catalog plugins at runtime.

mod admin;

use std::{collections::BTreeMap, sync::Arc};

use admin::{BuildError, construct};
pub use admin::{DISTRIBUTION_PLUGIN, DistributionPlugin, Registry};

use rah_api::{Kernel, MountOptions, Plugin, PluginId, PluginStatus, kernel::KernelError};
use rah_plugin_context_compact::CompactPlugin;
use rah_plugin_context_default::ContextPlugin;
use rah_plugin_frontend_cli::CliFrontendPlugin;
use rah_plugin_frontend_web::WebFrontendPlugin;
use rah_plugin_loop_plan::PlanLoopPlugin;
use rah_plugin_loop_react::ReactLoopPlugin;
use rah_plugin_model_anthropic::AnthropicPlugin;
use rah_plugin_model_mock::MockModelPlugin;
use rah_plugin_model_responses::{LlamaCppPlugin, OpenAiResponsesPlugin};
use rah_plugin_policy_authority::PolicyAuthorityPlugin;
use rah_plugin_sandbox_process::SandboxProcessPlugin;
use rah_plugin_scheduler_basic::SchedulerPlugin;
use rah_plugin_session_jsonl::JsonlSessionPlugin;
use rah_plugin_session_titles::SessionTitlesPlugin;
use rah_plugin_skills_fs::SkillsPlugin;
use rah_plugin_storage_fs::StorageFsPlugin;
use rah_plugin_telemetry_log::TelemetryPlugin;
use rah_plugin_tools_builtin::BuiltinToolsPlugin;
use rah_plugin_tools_mcp::{McpHttpToolsPlugin, McpToolsPlugin};
use rah_plugin_tools_repo::RepoToolsPlugin;
use rah_plugin_tools_repo_write::RepoWriteToolsPlugin;
use rah_plugin_tools_shell::ShellToolsPlugin;
use rah_plugin_tools_web_search::WebSearchPlugin;
use serde::Deserialize;
use serde_json::Value;

/// Builds a plugin instance with the given id.
pub type Constructor = fn(PluginId) -> Arc<dyn Plugin>;

/// Plugins compiled into this distribution, by `use` name.
pub fn catalog() -> BTreeMap<&'static str, Constructor> {
    let entries: [(&'static str, Constructor); 25] = [
        ("context-compact", |id| Arc::new(CompactPlugin::new(id))),
        ("context-default", |id| Arc::new(ContextPlugin::new(id))),
        ("frontend-cli", |id| Arc::new(CliFrontendPlugin::new(id))),
        ("frontend-web", |id| Arc::new(WebFrontendPlugin::new(id))),
        ("loop-plan", |id| Arc::new(PlanLoopPlugin::new(id))),
        ("loop-react", |id| Arc::new(ReactLoopPlugin::new(id))),
        ("model-anthropic", |id| Arc::new(AnthropicPlugin::new(id))),
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
        ("scheduler-basic", |id| Arc::new(SchedulerPlugin::new(id))),
        ("session-jsonl", |id| Arc::new(JsonlSessionPlugin::new(id))),
        ("session-titles", |id| {
            Arc::new(SessionTitlesPlugin::new(id))
        }),
        ("skills-fs", |id| Arc::new(SkillsPlugin::new(id))),
        ("storage-fs", |id| Arc::new(StorageFsPlugin::new(id))),
        ("telemetry-log", |id| Arc::new(TelemetryPlugin::new(id))),
        ("tools-builtin", |id| Arc::new(BuiltinToolsPlugin::new(id))),
        ("tools-mcp", |id| Arc::new(McpToolsPlugin::new(id))),
        ("tools-mcp-http", |id| Arc::new(McpHttpToolsPlugin::new(id))),
        ("tools-repo", |id| Arc::new(RepoToolsPlugin::new(id))),
        ("tools-repo-write", |id| {
            Arc::new(RepoWriteToolsPlugin::new(id))
        }),
        ("tools-shell", |id| Arc::new(ShellToolsPlugin::new(id))),
        ("tools-web-search", |id| Arc::new(WebSearchPlugin::new(id))),
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

/// Built-in presets. All share `storage-fs` (`.rah/storage`), built-in tools, read-only repository tools
/// over `sandbox-process`, repository write tools (Ask), JSONL sessions, `context-default` (history window
/// and memory notes per workspace in `.rah/memory`), `skills-fs` (`.rah/skills`), `scheduler-basic`
/// (background jobs and `agent.delegate`), the default loop and the terminal
/// frontend. None mounts `tools-shell`. Every preset except `minimal` mounts
/// `policy-authority`; `minimal` runs unguarded and `rah` warns about it.
pub const PRESETS: [Preset; 5] = [
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
    Preset {
        name: "anthropic",
        model: "model-anthropic",
        guarded: true,
    },
];

/// Mounted first, so no tool call can run before the policy is in place.
/// Every preset tool has an explicit assignment; anything else is denied.
const POLICY_TEMPLATE: &str = r#"
[[plugin]]
use = "policy-authority"
config = { allow = ["none", "read"], ask = ["write", "execute"], tools = { echo = "none", "fs.*" = "read", "repo.*" = "read", "repo.create-file" = "write", "repo.edit-files" = "write", "repo.patch" = "write", "memory.*" = "write", "skills.*" = "read", "agent.*" = "read" } }
"#;

const PRESET_TEMPLATE: &str = r#"
[[plugin]]
use = "storage-fs"
config = { dir = ".rah/storage" }

[[plugin]]
use = "session-jsonl"
config = { dir = ".rah/sessions" }

[[plugin]]
use = "session-titles"

[[plugin]]
use = "tools-builtin"
config = { root = "." }

[[plugin]]
use = "sandbox-process"
config = { root = "." }

[[plugin]]
use = "tools-repo"

[[plugin]]
use = "tools-repo-write"
config = { root = "." }

[[plugin]]
use = "context-default"
config = { memory_dir = ".rah/memory" }

[[plugin]]
use = "skills-fs"
config = { dir = ".rah/skills" }

[[plugin]]
use = "{MODEL}"

[[plugin]]
use = "loop-react"

[[plugin]]
use = "scheduler-basic"

[[plugin]]
use = "frontend-cli"
"#;

#[derive(Debug, thiserror::Error)]
pub enum BootError {
    #[error("invalid distribution: {0}")]
    Parse(String),
    #[error("unknown preset `{0}`; available: default, dev, minimal, openai, anthropic")]
    UnknownPreset(String),
    #[error("unknown plugin `{0}`; available: {1}")]
    UnknownPlugin(String, String),
    #[error("{0}")]
    Process(String),
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

    /// Replaces every plugin of kind `from` with a plugin of kind `to` and an
    /// empty configuration. Returns whether any plugin matched.
    pub fn replace(&mut self, from: &str, to: &str) -> bool {
        let mut matched = false;
        for spec in self.plugin.iter_mut().filter(|spec| spec.kind == from) {
            spec.kind = to.to_owned();
            spec.id = None;
            spec.config = Value::Null;
            matched = true;
        }
        matched
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

/// Plugin kinds that work in a workspace directory, through their `root`
/// configuration: what `rah --root` sets at start and what the `admin`
/// service's `set_workspace` changes at runtime. The sandbox comes first, so
/// the tools that depend on it restart with the new root.
pub const WORKSPACE_PLUGINS: [&str; 3] = ["sandbox-process", "tools-builtin", "tools-repo-write"];

/// The `use` name of out-of-process plugins (protocol v3); their `config`
/// says how to start them.
pub const PROCESS_PLUGIN: &str = "process";

/// The `use` name of WebAssembly component plugins; their `config` names the
/// component.
pub const WASM_PLUGIN: &str = "wasm";

/// Mounts the `distribution` plugin, then every plugin in order, and returns
/// the resulting statuses. A configured `policy-authority` that does not
/// become active aborts the boot before any later plugin (such as a
/// frontend) is mounted. A process plugin is started once first to read its
/// manifest.
pub async fn boot(
    kernel: &Kernel,
    distribution: &Distribution,
) -> Result<Vec<(PluginId, PluginStatus)>, BootError> {
    let registry = Arc::new(Registry::default());
    kernel
        .mount(
            Arc::new(DistributionPlugin::new(registry.clone())),
            MountOptions::default(),
        )
        .await?;
    for spec in &distribution.plugin {
        if (spec.kind == PROCESS_PLUGIN || spec.kind == WASM_PLUGIN) && spec.id.is_none() {
            return Err(BootError::Process(format!(
                "a `{}` plugin needs an `id`",
                spec.kind
            )));
        }
        let plugin = construct(&spec.kind, spec.plugin_id(), &spec.config)
            .await
            .map_err(|error| match error {
                BuildError::UnknownKind(kind) => {
                    let names: Vec<&str> = catalog().keys().copied().collect();
                    BootError::UnknownPlugin(
                        kind,
                        format!("{}, {PROCESS_PLUGIN}, {WASM_PLUGIN}", names.join(", ")),
                    )
                }
                BuildError::Process(message) => BootError::Process(message),
            })?;
        let mut options = MountOptions::with_config(spec.config.clone());
        for (service, instance) in &spec.bind {
            options = options.bind(service.as_str(), instance.clone());
        }
        let status = kernel.mount(plugin, options).await?;
        registry.record(
            spec.plugin_id(),
            &spec.kind,
            spec.config.clone(),
            spec.bind.clone(),
        );
        if spec.kind == "policy-authority" && status != PluginStatus::Active {
            return Err(BootError::PolicyInactive(
                spec.plugin_id(),
                format!("{status:?}"),
            ));
        }
    }
    Ok(kernel.plugins())
}
