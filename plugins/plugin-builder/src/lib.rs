//! `plugin-builder`: the user describes a plugin, the model writes it.
//!
//! A personal plugin is a process plugin (`docs/PROCESS_PLUGINS.md`) in
//! Python or Node.js, kept in its own directory under `dir`. The plugin
//! provides four tools and one view:
//!
//! - `plugins.guide` (`none`) returns the rules the model follows: the
//!   manifest, the runtime library, and what every plugin must do with
//!   untrusted input.
//! - `plugins.save` (`write`) checks a plugin the model wrote and writes it:
//!   the model's files, `manifest.json`, the runtime library
//!   (`rah_plugin.py` or `rah_plugin.mjs`, which speaks the protocol and
//!   answers `initialize` with `manifest.json`), `plugin-builder.json` and
//!   `mount.toml`, the `rah.toml` entry with the policy assignments its tools
//!   need. A plugin is written in full into a staging directory and renamed
//!   into place, so it is replaced whole or not at all.
//! - `plugins.list` and `plugins.read` (`read`) return the saved plugins, and
//!   one plugin in the form `plugins.save` takes, to change it.
//! - The `plugins` view (ADR 0062, ADR 0073) is where the user manages the
//!   saved plugins; it is announced with `UiChanged` after each change. The
//!   list shows each plugin's state: not enabled, enabled, or enabled with a
//!   later save waiting. A plugin's `details:<name>` button shows its tools
//!   with their permissions, its services and hooks instead of the list,
//!   until the `list` button goes back, with the actions its state allows:
//!   - `enable:<name>` and `reload:<name>` forms, which need a ticked
//!     confirmation, mount the plugin through the `admin` service, assign
//!     the tools the policy lacks and keep both in the configuration file
//!     ([`lifecycle`]); `disable:<name>` undoes it and keeps the files.
//!   - A `delete:<name>` form deletes the plugin's directory once the user
//!     types its name, after disabling it and writing a backup of it.
//!
//!   Under the list, `backup` writes every plugin to a new bundle in
//!   `backups/` and the `restore` form saves the plugins of a chosen one.
//!   The `export` form writes the ticked plugins to one bundle file
//!   (`{"format": "rah-personal-plugins", "version": 1, "enabled": [...],
//!   "plugins": [...]}`, each entry in the form `plugins.save` takes) and
//!   never overwrites a file; the `import` form saves the plugins of a
//!   bundle after every one passed the checks of `plugins.save`. Restoring
//!   and importing enable nothing. All of it runs only on the user's click;
//!   the model has no tool for any of it.
//!
//! Every model request gets one system item ([`HINT`]) after the leading
//! system items: a request for a RAH plugin starts with `plugins.guide`. A
//! plugin for anything else is ordinary workspace work, an unclear request
//! is asked about, and a missing tool is only offered as a plugin, never
//! written unasked.
//!
//! The mount entry, in `mount.toml` too, sets `personal = true`, so the Web
//! UI lists the mounted plugin apart from the distribution's own, and sets
//! the working directory to the plugin directory rather than the plugin's
//! own, so a running plugin can be replaced and deleted on Windows.
//!
//! Saving never mounts or runs a plugin. The model's output is untrusted:
//! the manifest is checked like the process host checks it at `initialize`,
//! tool names must be `<plugin name>.<tool>` (so a plugin cannot take a
//! built-in tool's name or fall under the user's policy entry for one), a
//! `tool.before_invoke` hook needs a priority below the policy's, and file
//! paths stay inside the plugin's directory. Enabling the result is the
//! user's decision, taken in the view or by following `mount.toml`; the host
//! checks the running plugin's manifest again.
//!
//! Configuration: `{ "dir": "plugins", "python": "python", "node":
//! "node", "max_files": 16, "max_file_bytes": 131072 }`. `python` and `node`
//! are the programs `mount.toml` starts. On each save they are looked up on
//! `PATH` (and a Python the configuration does not name also among uv's
//! installs) and written as full paths; one that is not found is written as
//! given, with a warning in `mount.toml` and in the reply. Looking only
//! checks that files exist; nothing is run.

mod lifecycle;
mod manifest;
mod programs;
mod store;
mod views;

use std::{
    collections::BTreeSet,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use rah_api::{
    KernelHandle, Plugin, PluginError, PluginId, PluginManifest, Scope, async_trait,
    hooks::ModelBeforeRequest,
    kernel::Spawner,
    tools::{TOOLS_SERVICE, Tool, ToolContext, ToolError, provide_tools},
    ui::{UI_VIEWS, UI_VIEWS_SERVICE, UiChanged, UiViews},
};
use rah_protocol::{
    ContextItem, PermissionLevel, ToolContent, ToolDefinition, ToolName, ToolOutput,
};
use serde::Deserialize;
use serde_json::{Value, json};

pub use crate::manifest::{HookSpec, Manifest};
use crate::programs::{Found, Search};
pub use crate::store::Runtime;
use crate::store::{About, MOUNT_FILE, Plan, SaveError, Store};

/// The tool that returns the rules.
pub const GUIDE_TOOL: &str = "plugins.guide";
/// The tool that checks and writes a plugin.
pub const SAVE_TOOL: &str = "plugins.save";
/// The tool that lists the saved plugins.
pub const LIST_TOOL: &str = "plugins.list";
/// The tool that returns one saved plugin.
pub const READ_TOOL: &str = "plugins.read";
/// The view that lists the saved plugins.
pub const VIEW: &str = "plugins";
/// The view action that writes ticked plugins to a bundle file.
pub const EXPORT_ACTION: &str = "export";
/// The view action that saves the plugins of a bundle file.
pub const IMPORT_ACTION: &str = "import";
/// The name prefix of the export form's check boxes; the rest is a plugin
/// name.
const EXPORT_PREFIX: &str = "plugin:";
/// The view action that writes every saved plugin to a new backup.
pub const BACKUP_ACTION: &str = "backup";
/// The view action that saves the plugins of a backup.
pub const RESTORE_ACTION: &str = "restore";
/// The prefix of the view action that enables a plugin; the rest is a plugin
/// name. Its form's [`READ_FIELD`] must be ticked.
pub const ENABLE_PREFIX: &str = "enable:";
/// The prefix of the view action that runs the last save of an enabled
/// plugin; confirmed like [`ENABLE_PREFIX`].
pub const RELOAD_PREFIX: &str = "reload:";
/// The prefix of the view action that disables a plugin.
pub const DISABLE_PREFIX: &str = "disable:";
/// The field of the enable and reload forms the user ticks to confirm.
pub const READ_FIELD: &str = "read";
/// The view action that goes back from a plugin's details to the list.
pub const LIST_ACTION: &str = "list";
/// The prefix of the view action that shows a plugin's details; the rest is a
/// plugin name.
pub const DETAILS_PREFIX: &str = "details:";
/// The prefix of the view action that deletes a plugin; the rest is a plugin
/// name, which the form's [`CONFIRM_FIELD`] must repeat.
pub const DELETE_PREFIX: &str = "delete:";
/// The delete form's field, where the user types the plugin's name.
pub const CONFIRM_FIELD: &str = "confirm";
/// The longest plugin description, in characters.
pub const MAX_DESCRIPTION_CHARS: usize = 200;
const GUIDE: &str = include_str!("guide.md");
/// The system item every model request gets, so a request for a RAH plugin
/// reaches these tools, and only such a request.
pub const HINT: &str = "You run inside RAH (Rust Agent Harness), which has a plugin builder: \
     `plugins.guide` and `plugins.save` write a personal RAH plugin, a small program that adds \
     tools to RAH, saved in the user's plugin directory rather than in the project. Use them \
     only when the user asks for a RAH plugin or to add a tool to RAH. A plugin for anything \
     else (the user's project, an editor, a browser, a build tool) is ordinary work in the \
     workspace. If it is unclear which the user means, ask. When a task needs a tool you do \
     not have, say so and offer a personal plugin; do not write one unless the user agrees.";
const DEFAULT_MAX_FILES: usize = 16;
const DEFAULT_MAX_FILE_BYTES: usize = 128 * 1024;
/// The largest `max_file_bytes`: a file must fit in one tool call.
const MAX_FILE_BYTES_LIMIT: usize = 1024 * 1024;
const MAX_FILES_LIMIT: usize = 64;

pub struct PluginBuilderPlugin {
    manifest: PluginManifest,
    /// Where to look for interpreters; the environment when `None`.
    search: Option<Search>,
}

impl PluginBuilderPlugin {
    pub fn new(id: impl Into<PluginId>) -> Self {
        Self {
            manifest: PluginManifest::new(id, env!("CARGO_PKG_VERSION"))
                .provides([TOOLS_SERVICE, UI_VIEWS_SERVICE]),
            search: None,
        }
    }

    /// Looks for interpreters in `search` instead of the environment.
    #[cfg(test)]
    fn with_search(mut self, search: Search) -> Self {
        self.search = Some(search);
        self
    }

    /// An example configuration for reference: the defaults.
    pub const CONFIG_EXAMPLE: &str = r#"{ "dir": "plugins", "python": "python", "node": "node", "max_files": 16, "max_file_bytes": 131072 }"#;
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    dir: Option<PathBuf>,
    python: Option<String>,
    node: Option<String>,
    max_files: Option<usize>,
    max_file_bytes: Option<usize>,
}

impl Config {
    fn check(&self) -> Result<(), String> {
        for (key, program) in [("python", &self.python), ("node", &self.node)] {
            if program.as_ref().is_some_and(|p| p.trim().is_empty()) {
                return Err(format!("`{key}` must name a program"));
            }
        }
        if self
            .max_files
            .is_some_and(|n| n == 0 || n > MAX_FILES_LIMIT)
        {
            return Err(format!("`max_files` must be 1 to {MAX_FILES_LIMIT}"));
        }
        if self
            .max_file_bytes
            .is_some_and(|n| n == 0 || n > MAX_FILE_BYTES_LIMIT)
        {
            return Err(format!(
                "`max_file_bytes` must be 1 to {MAX_FILE_BYTES_LIMIT}"
            ));
        }
        Ok(())
    }
}

/// What the tools and the view share.
struct Builder {
    kernel: KernelHandle,
    instance: String,
    store: Store,
    python: String,
    /// Whether the configuration names the Python; only a default one may
    /// be found among uv's installs.
    python_named: bool,
    node: String,
    max_files: usize,
    max_file_bytes: usize,
    search: Search,
    /// Starts the tasks of view actions, so they end with the scope and not
    /// with the page that asked.
    spawner: Spawner,
    /// One enable, reload, disable or delete at a time.
    lifecycle: tokio::sync::Mutex<()>,
    /// The running plugins that were saved again since they started.
    changed: Mutex<BTreeSet<String>>,
}

impl Builder {
    /// Tells frontends that the view changed.
    fn announce(&self) {
        self.kernel.emit(&UiChanged {
            instance: self.instance.clone(),
            view: VIEW.to_owned(),
        });
    }

    /// The program `mount.toml` starts `runtime` plugins with, looked up
    /// on every save so an interpreter installed later is found.
    fn program(&self, runtime: Runtime) -> Found {
        let configured = match runtime {
            Runtime::Python => &self.python,
            Runtime::Node => &self.node,
        };
        programs::find(configured, runtime, &self.search, !self.python_named)
    }
}

#[async_trait]
impl Plugin for PluginBuilderPlugin {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    fn check_config(&self, config: &Value) -> Result<(), PluginError> {
        let config: Config = rah_api::config::parse(config)?;
        config.check()?;
        Ok(())
    }

    async fn apply(&self, scope: &mut Scope) -> Result<(), PluginError> {
        let config: Config = rah_api::config::parse(scope.config())?;
        config.check()?;
        let default_python = if cfg!(windows) { "python" } else { "python3" };
        let builder = Arc::new(Builder {
            kernel: scope.kernel(),
            instance: scope.id().to_string(),
            store: Store::new(config.dir.unwrap_or_else(|| PathBuf::from("plugins"))),
            python_named: config.python.is_some(),
            python: config.python.unwrap_or_else(|| default_python.to_owned()),
            node: config.node.unwrap_or_else(|| "node".to_owned()),
            max_files: config.max_files.unwrap_or(DEFAULT_MAX_FILES),
            max_file_bytes: config.max_file_bytes.unwrap_or(DEFAULT_MAX_FILE_BYTES),
            search: self.search.clone().unwrap_or_else(Search::from_env),
            spawner: scope.spawner(),
            lifecycle: tokio::sync::Mutex::new(()),
            changed: Mutex::new(BTreeSet::new()),
        });
        // Without this, a model asked for "a plugin" in a project workspace
        // tends to search the project for a plugin system instead.
        scope.hook_waterfall::<ModelBeforeRequest, _, _>(0, |mut request| async move {
            let at = request
                .items
                .iter()
                .position(|item| !matches!(item, ContextItem::System { .. }))
                .unwrap_or(request.items.len());
            request.items.insert(
                at,
                ContextItem::System {
                    text: HINT.to_owned(),
                },
            );
            request
        });
        provide_tools(
            scope,
            vec![
                Arc::new(GuideTool(builder.clone())),
                Arc::new(SaveTool(builder.clone())),
                Arc::new(ListTool(builder.clone())),
                Arc::new(ReadTool(builder.clone())),
            ],
        )?;
        let views: Arc<dyn UiViews> = Arc::new(views::BuilderViews::new(builder));
        scope.provide(UI_VIEWS, views)?;
        Ok(())
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FileInput {
    path: String,
    content: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SaveInput {
    name: String,
    description: String,
    runtime: Runtime,
    manifest: Manifest,
    files: Vec<FileInput>,
    #[serde(default)]
    replace: bool,
}

/// Checks `plugins.save` input against every rule; nothing is written.
fn plan(input: Value, max_files: usize, max_file_bytes: usize) -> Result<Plan, String> {
    let input: SaveInput = serde_json::from_value(input).map_err(|error| error.to_string())?;
    manifest::check_name(&input.name)?;
    let description = input
        .description
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if description.is_empty() || description.chars().count() > MAX_DESCRIPTION_CHARS {
        return Err(format!(
            "`description` must be one line of 1 to {MAX_DESCRIPTION_CHARS} characters"
        ));
    }
    manifest::check(&input.name, &input.manifest)?;
    let files: Vec<(String, String)> = input
        .files
        .into_iter()
        .map(|file| (file.path, file.content))
        .collect();
    store::check_files(&files, input.runtime, max_files, max_file_bytes)?;
    Ok(Plan {
        about: About {
            name: input.name,
            description,
            runtime: input.runtime,
        },
        manifest: input.manifest,
        files,
        replace: input.replace,
    })
}

fn text(text: String) -> ToolOutput {
    ToolOutput {
        content: vec![ToolContent::Text(text)],
        is_error: false,
    }
}

fn no_input(input: &Value) -> Result<(), ToolError> {
    match input {
        Value::Null => Ok(()),
        Value::Object(map) if map.is_empty() => Ok(()),
        _ => Err(ToolError::InvalidInput(
            "this tool takes no input".to_owned(),
        )),
    }
}

fn empty_schema() -> Value {
    json!({ "type": "object", "properties": {}, "additionalProperties": false })
}

struct GuideTool(Arc<Builder>);

#[async_trait]
impl Tool for GuideTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: ToolName::new(GUIDE_TOOL),
            description: "Returns the rules for writing a personal RAH plugin: the manifest, the \
                 runtime library and what every plugin must do. Call it before `plugins.save`, \
                 only when the user asked for a RAH plugin or agreed to one you offered."
                .to_owned(),
            input_schema: empty_schema(),
            permission: PermissionLevel::None,
        }
    }

    async fn call(&self, input: Value, _: ToolContext) -> Result<ToolOutput, ToolError> {
        no_input(&input)?;
        let builder = &self.0;
        let runtime = |runtime: Runtime| {
            let found = builder.program(runtime);
            match found.warning {
                None => format!(
                    "- `{}` plugins start with `{}`.\n",
                    runtime.label(),
                    found.program
                ),
                Some(warning) => format!(
                    "- `{}` plugins: {warning}. Prefer the other runtime unless the user asks \
                     for this one, and tell the user.\n",
                    runtime.label()
                ),
            }
        };
        Ok(text(format!(
            "{GUIDE}\n## This installation\n\n- Plugins are saved in `{}`.\n{}{}- A plugin has \
             at most {} files of {} bytes each.\n",
            builder.store.dir().display(),
            runtime(Runtime::Python),
            runtime(Runtime::Node),
            builder.max_files,
            builder.max_file_bytes,
        )))
    }
}

struct SaveTool(Arc<Builder>);

#[async_trait]
impl Tool for SaveTool {
    fn definition(&self) -> ToolDefinition {
        let tool = json!({
            "type": "object",
            "properties": {
                "name": { "type": "string", "description": "`<plugin name>.<tool>`" },
                "description": { "type": "string" },
                "input_schema": { "type": "object" },
                "permission": { "type": "string", "enum": ["none", "read", "write", "execute"] }
            },
            "required": ["name", "description", "input_schema", "permission"],
            "additionalProperties": false
        });
        ToolDefinition {
            name: ToolName::new(SAVE_TOOL),
            description: "Checks a personal RAH plugin against the plugin rules and saves it to \
                 the user's plugin directory with the runtime library and a `mount.toml` that \
                 tells the user how to mount it. Only for a plugin the user asked for or agreed \
                 to. Read `plugins.guide` first. It does not mount or \
                 run the plugin. Send the whole plugin every time; `replace: true` replaces a \
                 saved plugin of the same name."
                .to_owned(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "pattern": "^[a-z][a-z0-9-]*$",
                        "maxLength": manifest::MAX_NAME_BYTES,
                        "description": "The plugin's name and mount id; tool names start with it."
                    },
                    "description": {
                        "type": "string",
                        "maxLength": MAX_DESCRIPTION_CHARS,
                        "description": "One line: what the plugin does."
                    },
                    "runtime": { "type": "string", "enum": ["python", "node"] },
                    "manifest": {
                        "type": "object",
                        "properties": {
                            "version": { "type": "string" },
                            "requires": {
                                "type": "array",
                                "items": { "type": "string", "enum": manifest::CALLABLE_SERVICES }
                            },
                            "tools": { "type": "array", "items": tool, "maxItems": manifest::MAX_TOOLS },
                            "hooks": {
                                "type": "array",
                                "items": {
                                    "type": "object",
                                    "properties": {
                                        "hook": { "type": "string", "enum": manifest::HOOKS },
                                        "priority": { "type": "integer" }
                                    },
                                    "required": ["hook"],
                                    "additionalProperties": false
                                }
                            },
                            "events": {
                                "type": "array",
                                "items": { "type": "string", "enum": manifest::EVENTS }
                            },
                            "views": {
                                "type": "array",
                                "maxItems": manifest::MAX_VIEWS,
                                "items": {
                                    "type": "object",
                                    "properties": {
                                        "id": { "type": "string" },
                                        "title": { "type": "string" }
                                    },
                                    "required": ["id", "title"],
                                    "additionalProperties": false
                                }
                            }
                        },
                        "required": ["version"],
                        "additionalProperties": false
                    },
                    "files": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "path": { "type": "string" },
                                "content": { "type": "string" }
                            },
                            "required": ["path", "content"],
                            "additionalProperties": false
                        },
                        "description": "The plugin's own files; `plugin.py` or `plugin.mjs` is required."
                    },
                    "replace": { "type": "boolean" }
                },
                "required": ["name", "description", "runtime", "manifest", "files"],
                "additionalProperties": false
            }),
            permission: PermissionLevel::Write,
        }
    }

    async fn call(&self, input: Value, _: ToolContext) -> Result<ToolOutput, ToolError> {
        let builder = &self.0;
        let plan = plan(input, builder.max_files, builder.max_file_bytes)
            .map_err(ToolError::InvalidInput)?;
        let program = builder.program(plan.about.runtime);
        let (dir, mount) =
            builder
                .store
                .save(&plan, &program)
                .await
                .map_err(|error| match error {
                    SaveError::Refused(reason) => ToolError::InvalidInput(reason),
                    SaveError::Failed(reason) => ToolError::Execution(reason),
                })?;
        let running = builder.saved_again(&plan.about.name);
        builder.announce();
        let warning = program.warning.map_or_else(String::new, |warning| {
            format!(
                " The interpreter needs the user's attention before mounting: {warning}. Tell \
                 the user."
            )
        });
        let saved = format!(
            "Saved plugin `{name}` in `{dir}` ({count} files of yours).",
            name = plan.about.name,
            dir = dir.display(),
            count = plan.files.len(),
        );
        if running {
            return Ok(text(format!(
                "{saved} The plugin that runs is still the earlier save, and this one has not \
                 run.{warning} Tell the user to read the changed files, then press Reload for \
                 it in the \"My plugins\" panel of the Web UI."
            )));
        }
        if builder.manages() {
            return Ok(text(format!(
                "{saved} It is not enabled and has not run.{warning} Tell the user to read \
                 its files, then enable it in the \"My plugins\" panel of the Web UI, which \
                 shows the permissions of its tools to confirm. Without the Web UI, \
                 `{MOUNT_FILE}` in its directory has the steps."
            )));
        }
        Ok(text(format!(
            "{saved} It is not mounted and has not run.{warning} Tell the user to read its \
             files, then follow the steps in `{MOUNT_FILE}` (also the policy entries its tools \
             need):\n\n{mount}"
        )))
    }
}

struct ListTool(Arc<Builder>);

#[async_trait]
impl Tool for ListTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: ToolName::new(LIST_TOOL),
            description: "Lists the personal plugins saved with `plugins.save`: their tools with \
                 permissions, services, hooks, events and views."
                .to_owned(),
            input_schema: empty_schema(),
            permission: PermissionLevel::Read,
        }
    }

    async fn call(&self, input: Value, _: ToolContext) -> Result<ToolOutput, ToolError> {
        no_input(&input)?;
        let saved = self.0.store.list().await;
        if saved.is_empty() {
            return Ok(text(format!(
                "No personal plugins in `{}`.",
                self.0.store.dir().display()
            )));
        }
        let plugins: Vec<Value> = saved
            .iter()
            .map(|plugin| {
                json!({
                    "name": plugin.about.name,
                    "description": plugin.about.description,
                    "runtime": plugin.about.runtime,
                    "version": plugin.manifest.version,
                    "dir": plugin.dir.display().to_string(),
                    "tools": plugin.manifest.tools.iter()
                        .map(|tool| json!({ "name": tool.name, "permission": tool.permission }))
                        .collect::<Vec<_>>(),
                    "requires": plugin.manifest.requires,
                    "hooks": plugin.manifest.hooks.iter().map(|h| &h.hook).collect::<Vec<_>>(),
                    "events": plugin.manifest.events,
                    "views": plugin.manifest.views.iter().map(|v| &v.id).collect::<Vec<_>>(),
                })
            })
            .collect();
        Ok(ToolOutput {
            content: vec![ToolContent::Json(json!({ "plugins": plugins }))],
            is_error: false,
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReadInput {
    name: String,
}

struct ReadTool(Arc<Builder>);

#[async_trait]
impl Tool for ReadTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: ToolName::new(READ_TOOL),
            description: "Returns a saved personal plugin in the form `plugins.save` takes, to \
                 change it and save it again with `replace: true`."
                .to_owned(),
            input_schema: json!({
                "type": "object",
                "properties": { "name": { "type": "string" } },
                "required": ["name"],
                "additionalProperties": false
            }),
            permission: PermissionLevel::Read,
        }
    }

    async fn call(&self, input: Value, _: ToolContext) -> Result<ToolOutput, ToolError> {
        let input: ReadInput = serde_json::from_value(input)
            .map_err(|error| ToolError::InvalidInput(error.to_string()))?;
        manifest::check_name(&input.name).map_err(ToolError::InvalidInput)?;
        let store = &self.0.store;
        store
            .load(&input.name)
            .await
            .map_err(|error| ToolError::InvalidInput(format!("no saved plugin: {error}")))?;
        let (entry, skipped) = store
            .entry(&input.name)
            .await
            .map_err(ToolError::Execution)?;
        let mut content = vec![ToolContent::Json(entry)];
        if !skipped.is_empty() {
            content.push(ToolContent::Text(format!(
                "Not returned (not UTF-8 text, too large or not a regular file): {}. A save \
                 with `replace: true` drops them.",
                skipped.join(", ")
            )));
        }
        Ok(ToolOutput {
            content,
            is_error: false,
        })
    }
}

#[cfg(test)]
mod tests;
