//! The `rah` distribution: a plugin catalog, `rah.toml` parsing, presets and
//! boot. Everything it mounts is a plugin; this crate only composes. It also
//! mounts the `distribution` plugin, whose `admin` service lets frontends
//! manage catalog plugins at runtime.

mod admin;
mod check;
mod data;
mod extend;
mod features;
mod help;

mod show;

pub use check::{ConfigProblem, check_distribution};
pub use extend::Extended;
pub use features::{Applied, FEATURES, Feature, approved_tools, feature, list as list_features};
pub use show::show_config;

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use admin::{BuildError, construct};
pub use admin::{DISTRIBUTION_PLUGIN, DistributionPlugin, Registry};
pub use data::{DATA_HOME_ENV, default_data_home, resolve_data_paths};

use rah_api::{Kernel, MountOptions, Plugin, PluginId, PluginStatus, kernel::KernelError};
use rah_plugin_context_compact::CompactPlugin;
use rah_plugin_context_default::ContextPlugin;
use rah_plugin_context_instructions::InstructionsPlugin;
use rah_plugin_format_on_write::FormatOnWritePlugin;
use rah_plugin_frontend_cli::CliFrontendPlugin;
use rah_plugin_frontend_web::WebFrontendPlugin;
use rah_plugin_loop_plan::PlanLoopPlugin;
use rah_plugin_loop_react::ReactLoopPlugin;
use rah_plugin_lsp_diagnostics::LspDiagnosticsPlugin;
use rah_plugin_model_anthropic::AnthropicPlugin;
use rah_plugin_model_chat_completions::ChatCompletionsPlugin;
use rah_plugin_model_mock::MockModelPlugin;
use rah_plugin_model_responses::{LlamaCppPlugin, OpenAiResponsesPlugin};
use rah_plugin_policy_authority::PolicyAuthorityPlugin;
use rah_plugin_sandbox_process::{SandboxOsPlugin, SandboxProcessPlugin};
use rah_plugin_scheduler_basic::SchedulerPlugin;
use rah_plugin_session_jsonl::JsonlSessionPlugin;
use rah_plugin_session_titles::SessionTitlesPlugin;
use rah_plugin_skills_fs::SkillsPlugin;
use rah_plugin_storage_fs::StorageFsPlugin;
use rah_plugin_telemetry_log::TelemetryPlugin;
use rah_plugin_todo::TodoPlugin;
use rah_plugin_tool_input_repair::ToolInputRepairPlugin;
use rah_plugin_tools_builtin::BuiltinToolsPlugin;
use rah_plugin_tools_http::HttpToolsPlugin;
use rah_plugin_tools_mcp::{McpHttpToolsPlugin, McpToolsPlugin};
use rah_plugin_tools_repo::RepoToolsPlugin;
use rah_plugin_tools_repo_write::RepoWriteToolsPlugin;
use rah_plugin_tools_shell::ShellToolsPlugin;
use rah_plugin_tools_web_search::WebSearchPlugin;
use rah_plugin_ui_layout_config::UiLayoutConfigPlugin;
use serde::Deserialize;
use serde_json::Value;

/// Builds a plugin instance with the given id.
pub type Constructor = fn(PluginId) -> Arc<dyn Plugin>;

/// Plugins compiled into this distribution, by `use` name.
pub fn catalog() -> BTreeMap<&'static str, Constructor> {
    let entries: [(&'static str, Constructor); 34] = [
        ("context-compact", |id| Arc::new(CompactPlugin::new(id))),
        ("context-default", |id| Arc::new(ContextPlugin::new(id))),
        ("context-instructions", |id| {
            Arc::new(InstructionsPlugin::new(id))
        }),
        ("format-on-write", |id| {
            Arc::new(FormatOnWritePlugin::new(id))
        }),
        ("frontend-cli", |id| Arc::new(CliFrontendPlugin::new(id))),
        ("frontend-web", |id| Arc::new(WebFrontendPlugin::new(id))),
        ("loop-plan", |id| Arc::new(PlanLoopPlugin::new(id))),
        ("loop-react", |id| Arc::new(ReactLoopPlugin::new(id))),
        ("lsp-diagnostics", |id| {
            Arc::new(LspDiagnosticsPlugin::new(id))
        }),
        ("model-anthropic", |id| Arc::new(AnthropicPlugin::new(id))),
        ("model-chat-completions", |id| {
            Arc::new(ChatCompletionsPlugin::new(id))
        }),
        ("model-llamacpp", |id| Arc::new(LlamaCppPlugin::new(id))),
        ("model-mock", |id| Arc::new(MockModelPlugin::new(id))),
        ("model-openai", |id| {
            Arc::new(OpenAiResponsesPlugin::new(id))
        }),
        ("policy-authority", |id| {
            Arc::new(PolicyAuthorityPlugin::new(id))
        }),
        ("sandbox-os", |id| Arc::new(SandboxOsPlugin::new(id))),
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
        ("todo", |id| Arc::new(TodoPlugin::new(id))),
        ("tool-input-repair", |id| {
            Arc::new(ToolInputRepairPlugin::new(id))
        }),
        ("tools-builtin", |id| Arc::new(BuiltinToolsPlugin::new(id))),
        ("tools-http", |id| Arc::new(HttpToolsPlugin::new(id))),
        ("tools-mcp", |id| Arc::new(McpToolsPlugin::new(id))),
        ("tools-mcp-http", |id| Arc::new(McpHttpToolsPlugin::new(id))),
        ("tools-repo", |id| Arc::new(RepoToolsPlugin::new(id))),
        ("tools-repo-write", |id| {
            Arc::new(RepoWriteToolsPlugin::new(id))
        }),
        ("tools-shell", |id| Arc::new(ShellToolsPlugin::new(id))),
        ("tools-web-search", |id| Arc::new(WebSearchPlugin::new(id))),
        ("ui-layout-config", |id| {
            Arc::new(UiLayoutConfigPlugin::new(id))
        }),
    ];
    entries.into_iter().collect()
}

/// An example configuration for each mountable kind, mostly its defaults,
/// from the plugin crates (`<Plugin>::CONFIG_EXAMPLE`).
pub fn config_example(kind: &str) -> Option<&'static str> {
    let example = match kind {
        "context-compact" => CompactPlugin::CONFIG_EXAMPLE,
        "context-default" => ContextPlugin::CONFIG_EXAMPLE,
        "context-instructions" => InstructionsPlugin::CONFIG_EXAMPLE,
        "format-on-write" => FormatOnWritePlugin::CONFIG_EXAMPLE,
        "frontend-cli" => CliFrontendPlugin::CONFIG_EXAMPLE,
        "frontend-web" => WebFrontendPlugin::CONFIG_EXAMPLE,
        "loop-plan" => PlanLoopPlugin::CONFIG_EXAMPLE,
        "loop-react" => ReactLoopPlugin::CONFIG_EXAMPLE,
        "lsp-diagnostics" => LspDiagnosticsPlugin::CONFIG_EXAMPLE,
        "model-anthropic" => AnthropicPlugin::CONFIG_EXAMPLE,
        "model-chat-completions" => ChatCompletionsPlugin::CONFIG_EXAMPLE,
        "model-llamacpp" => LlamaCppPlugin::CONFIG_EXAMPLE,
        "model-mock" => MockModelPlugin::CONFIG_EXAMPLE,
        "model-openai" => OpenAiResponsesPlugin::CONFIG_EXAMPLE,
        "policy-authority" => PolicyAuthorityPlugin::CONFIG_EXAMPLE,
        "sandbox-os" => SandboxOsPlugin::CONFIG_EXAMPLE,
        "sandbox-process" => SandboxProcessPlugin::CONFIG_EXAMPLE,
        "scheduler-basic" => SchedulerPlugin::CONFIG_EXAMPLE,
        "session-jsonl" => JsonlSessionPlugin::CONFIG_EXAMPLE,
        "session-titles" => SessionTitlesPlugin::CONFIG_EXAMPLE,
        "skills-fs" => SkillsPlugin::CONFIG_EXAMPLE,
        "storage-fs" => StorageFsPlugin::CONFIG_EXAMPLE,
        "telemetry-log" => TelemetryPlugin::CONFIG_EXAMPLE,
        "todo" => TodoPlugin::CONFIG_EXAMPLE,
        "tool-input-repair" => ToolInputRepairPlugin::CONFIG_EXAMPLE,
        "tools-builtin" => BuiltinToolsPlugin::CONFIG_EXAMPLE,
        "tools-http" => HttpToolsPlugin::CONFIG_EXAMPLE,
        "tools-mcp" => McpToolsPlugin::CONFIG_EXAMPLE,
        "tools-mcp-http" => McpHttpToolsPlugin::CONFIG_EXAMPLE,
        "tools-repo" => RepoToolsPlugin::CONFIG_EXAMPLE,
        "tools-repo-write" => RepoWriteToolsPlugin::CONFIG_EXAMPLE,
        "tools-shell" => ShellToolsPlugin::CONFIG_EXAMPLE,
        "tools-web-search" => WebSearchPlugin::CONFIG_EXAMPLE,
        "ui-layout-config" => UiLayoutConfigPlugin::CONFIG_EXAMPLE,
        PROCESS_PLUGIN => rah_plugin_host_process::ProcessPlugin::CONFIG_EXAMPLE,
        WASM_PLUGIN => rah_plugin_host_wasm::WasmPlugin::CONFIG_EXAMPLE,
        _ => return None,
    };
    Some(example)
}

/// Help for the configuration keys of each kind that has it, as
/// `(key, help)` pairs in display order, from the plugin crates
/// (`<Plugin>::CONFIG_HELP`).
pub fn config_help(kind: &str) -> &'static [(&'static str, &'static str)] {
    match kind {
        "sandbox-os" => SandboxOsPlugin::CONFIG_HELP,
        "sandbox-process" => SandboxProcessPlugin::CONFIG_HELP,
        "tools-shell" => ShellToolsPlugin::CONFIG_HELP,
        _ => help::fallback(kind),
    }
}

/// A built-in preset: a named composition of catalog plugins.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Preset {
    pub name: &'static str,
    /// The model plugin it mounts.
    pub model: &'static str,
    /// The model plugin's configuration, as a TOML inline table, when it
    /// needs one.
    pub model_config: Option<&'static str>,
    /// Whether it mounts `policy-authority`.
    pub guarded: bool,
}

/// Built-in presets. All share `storage-fs`, built-in tools, read-only repository tools
/// over `sandbox-process`, repository write tools (Ask), JSONL sessions, `context-default` (history window
/// and memory notes per workspace), `skills-fs`, `scheduler-basic`
/// (background jobs and `agent.delegate`), `telemetry-log` (turn records and
/// the Web UI's Usage view), the default loop and the terminal
/// frontend. Their data lives under the data home (see [`Distribution::data_home`]).
/// None mounts `tools-shell`. Every preset except `minimal` mounts
/// `policy-authority`; `minimal` runs unguarded and `rah` warns about it.
pub const PRESETS: [Preset; 6] = [
    Preset {
        name: "default",
        model: "model-llamacpp",
        model_config: None,
        guarded: true,
    },
    Preset {
        name: "dev",
        model: "model-mock",
        model_config: None,
        guarded: true,
    },
    Preset {
        name: "minimal",
        model: "model-mock",
        model_config: None,
        guarded: false,
    },
    Preset {
        name: "openai",
        model: "model-openai",
        model_config: None,
        guarded: true,
    },
    Preset {
        name: "anthropic",
        model: "model-anthropic",
        model_config: None,
        guarded: true,
    },
    Preset {
        name: "deepseek",
        model: "model-chat-completions",
        model_config: Some(r#"{ provider = "deepseek" }"#),
        guarded: true,
    },
];

/// Mounted first, so no tool call can run before the policy is in place.
/// Every preset tool has an explicit assignment; anything else is denied.
const POLICY_TEMPLATE: &str = r#"
[[plugin]]
use = "policy-authority"
config = { allow = ["none", "read"], ask = ["write", "execute"], tools = { echo = "none", "fs.*" = "read", "repo.*" = "read", "repo.create-file" = "write", "repo.edit-files" = "write", "repo.patch" = "write", "repo.delete-file" = "write", "repo.move-file" = "write", "repo.commit" = "execute", "memory.*" = "write", "skills.*" = "read", "agent.*" = "read" } }
"#;

const PRESET_TEMPLATE: &str = r#"
[[plugin]]
use = "storage-fs"

[[plugin]]
use = "session-jsonl"

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
config = { memory_dir = "memory" }

[[plugin]]
use = "skills-fs"

[[plugin]]
use = "tool-input-repair"

[[plugin]]
use = "{MODEL}"{MODEL_CONFIG}

[[plugin]]
use = "telemetry-log"

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
    #[error("unknown preset `{0}`; available: default, dev, minimal, openai, anthropic, deepseek")]
    UnknownPreset(String),
    #[error("unknown plugin `{0}`; available: {1}")]
    UnknownPlugin(String, String),
    #[error("{0}")]
    Process(String),
    #[error("policy plugin `{0}` is not active ({1}); refusing to run unguarded")]
    PolicyInactive(PluginId, String),
    /// Configurations the plugins would refuse; nothing was mounted.
    #[error("{}", invalid_config_message(.0))]
    InvalidConfig(Vec<ConfigProblem>),
    #[error(transparent)]
    Kernel(#[from] KernelError),
}

/// The message of [`BootError::InvalidConfig`]: one line per problem.
fn invalid_config_message(problems: &[ConfigProblem]) -> String {
    let mut text = "invalid plugin configuration, nothing was mounted:".to_owned();
    for problem in problems {
        text.push_str(&format!("\n  {problem}"));
    }
    text
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
    /// Opt-in features to apply at start (`"shell=git"`, `"web-search"`; see
    /// [`FEATURES`]). Put the key before the first `[[plugin]]`.
    #[serde(default)]
    pub features: Vec<String>,
    #[serde(default)]
    pub plugin: Vec<PluginSpec>,
    /// The data home. When set, the data paths of the plugins (sessions,
    /// storage, memory, skills, telemetry) resolve against it at mount,
    /// also for plugins mounted later through the `admin` service: a missing
    /// path defaults under it and a relative one is joined to it. When unset,
    /// the plugins resolve relative paths against the current directory.
    #[serde(skip)]
    pub data_home: Option<PathBuf>,
    /// The features applied to this distribution, for `--show-config`.
    #[serde(skip)]
    pub applied: Vec<Applied>,
    /// The preset this distribution extends and what the file changed in
    /// it, when the file has `extends` (ADR 0058), for `--show-config`.
    #[serde(skip)]
    pub extended: Option<Extended>,
}

impl Distribution {
    /// Parses a distribution file. With `extends = "<preset>"`, the file
    /// lists only its changes to that preset (ADR 0058).
    pub fn parse(text: &str) -> Result<Self, BootError> {
        extend::parse(text)
    }

    pub fn preset(name: &str) -> Result<Self, BootError> {
        Self::parse(&preset_text(name)?)
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

    /// Points every workspace plugin ([`WORKSPACE_PLUGINS`]) at `root`.
    /// Returns whether any plugin matched.
    pub fn set_root(&mut self, root: &Path) -> bool {
        let root = Value::String(root.display().to_string());
        let mut matched = false;
        for kind in WORKSPACE_PLUGINS {
            matched |= self.set_config(kind, "root", root.clone());
        }
        matched
    }

    /// Runs `frontend-web` instead of `frontend-cli`. Returns whether a
    /// `frontend-web` is now in the distribution.
    pub fn use_web_frontend(&mut self) -> bool {
        self.replace("frontend-cli", "frontend-web")
            || self.plugin.iter().any(|spec| spec.kind == "frontend-web")
    }
}

/// The user's own distribution file in the data directory.
pub const USER_CONFIG: &str = "rah.toml";

/// Reads a distribution file.
pub fn read_distribution(path: &Path) -> Result<Distribution, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|error| format!("cannot read `{}`: {error}", path.display()))?;
    Distribution::parse(&text).map_err(|error| format!("`{}`: {error}", path.display()))
}

/// The distribution to run, with `data_home` set to `home`: the file
/// `config` when given, else the built-in `preset`, else [`USER_CONFIG`] in
/// `home` when it exists, else the `default` preset. Also returns the user's
/// file when that was the one read.
pub fn select_distribution(
    config: Option<&Path>,
    preset: Option<&str>,
    home: &Path,
) -> Result<(Distribution, Option<PathBuf>), String> {
    let user_config = home.join(USER_CONFIG);
    let (mut distribution, used) = match (config, preset) {
        (Some(path), _) => (read_distribution(path)?, None),
        (None, Some(preset)) => (
            Distribution::preset(preset).map_err(|error| error.to_string())?,
            None,
        ),
        (None, None) if user_config.is_file() => {
            (read_distribution(&user_config)?, Some(user_config))
        }
        (None, None) => (
            Distribution::preset("default").map_err(|error| error.to_string())?,
            None,
        ),
    };
    distribution.data_home = Some(home.to_path_buf());
    distribution.apply_features()?;
    Ok((distribution, used))
}

/// The distribution file of a built-in preset, as `rah.toml` text.
pub fn preset_text(name: &str) -> Result<String, BootError> {
    let preset = PRESETS
        .iter()
        .find(|preset| preset.name == name)
        .ok_or_else(|| BootError::UnknownPreset(name.to_owned()))?;
    let policy = if preset.guarded { POLICY_TEMPLATE } else { "" };
    let model_config = preset
        .model_config
        .map(|config| format!("\nconfig = {config}"))
        .unwrap_or_default();
    Ok(format!(
        "{policy}{}",
        PRESET_TEMPLATE
            .replace("{MODEL_CONFIG}", &model_config)
            .replace("{MODEL}", preset.model)
    ))
}

/// Plugin kinds that work in a workspace directory, through their `root`
/// configuration: what `rah --root` sets at start and what the `admin`
/// service's `set_workspace` changes at runtime. The sandboxes come first, so
/// the tools that depend on them restart with the new root.
pub const WORKSPACE_PLUGINS: [&str; 6] = [
    "sandbox-process",
    "sandbox-os",
    "tools-builtin",
    "tools-repo-write",
    "lsp-diagnostics",
    "format-on-write",
];

/// The `use` name of out-of-process plugins (protocol v3); their `config`
/// says how to start them.
pub const PROCESS_PLUGIN: &str = "process";

/// The `use` name of WebAssembly component plugins; their `config` names the
/// component.
pub const WASM_PLUGIN: &str = "wasm";

/// Checks every plugin configuration first ([`check_distribution`]) and
/// mounts nothing when one would be refused. Then mounts the `distribution`
/// plugin and every plugin in order, and returns the resulting statuses. A
/// configured `policy-authority` that does not
/// become active aborts the boot before any later plugin (such as a
/// frontend) is mounted. A process plugin is started once first to read its
/// manifest.
pub async fn boot(
    kernel: &Kernel,
    distribution: &Distribution,
) -> Result<Vec<(PluginId, PluginStatus)>, BootError> {
    let problems = check_distribution(distribution);
    if !problems.is_empty() {
        return Err(BootError::InvalidConfig(problems));
    }
    let registry = Arc::new(Registry::default());
    kernel
        .mount(
            Arc::new(
                DistributionPlugin::new(registry.clone())
                    .with_data_home(distribution.data_home.clone()),
            ),
            MountOptions::default(),
        )
        .await?;
    for spec in &distribution.plugin {
        let mut config = spec.config.clone();
        if let Some(home) = &distribution.data_home {
            resolve_data_paths(&spec.kind, &mut config, home);
        }
        if (spec.kind == PROCESS_PLUGIN || spec.kind == WASM_PLUGIN) && spec.id.is_none() {
            return Err(BootError::Process(format!(
                "a `{}` plugin needs an `id`",
                spec.kind
            )));
        }
        let plugin = construct(&spec.kind, spec.plugin_id(), &config)
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
        let mut options = MountOptions::with_config(config.clone());
        for (service, instance) in &spec.bind {
            options = options.bind(service.as_str(), instance.clone());
        }
        let status = match kernel.mount(plugin, options).await {
            Err(KernelError::InvalidConfig { reason, .. }) if spec.kind == "policy-authority" => {
                return Err(BootError::PolicyInactive(spec.plugin_id(), reason));
            }
            mounted => mounted?,
        };
        registry.record(spec.plugin_id(), &spec.kind, config, spec.bind.clone());
        if spec.kind == "policy-authority" && status != PluginStatus::Active {
            return Err(BootError::PolicyInactive(
                spec.plugin_id(),
                format!("{status:?}"),
            ));
        }
    }
    Ok(kernel.plugins())
}
