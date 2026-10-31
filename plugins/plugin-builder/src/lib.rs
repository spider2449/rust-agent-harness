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
//! - The `plugins` view (ADR 0062) lists the saved plugins, and is announced
//!   with `UiChanged` after each save. A plugin's `details:<name>` button
//!   shows its tools, directory and `mount.toml` instead of the list, until
//!   the `list` button goes back. Its
//!   `export` form writes the ticked plugins to one bundle file
//!   (`{"format": "rah-personal-plugins", "version": 1, "plugins": [...]}`,
//!   each entry in the form `plugins.save` takes) and never overwrites a
//!   file; its `import` form saves the plugins of a bundle after every one
//!   passed the checks of `plugins.save`. Both run only on the user's click.
//!
//! Every model request gets one system item ([`HINT`]) after the leading
//! system items: a request for "a plugin" means a personal RAH plugin and
//! starts with `plugins.guide`.
//!
//! `mount.toml` sets `personal = true` on the process entry, so the Web UI
//! lists the mounted plugin apart from the distribution's own.
//!
//! Saving never mounts or runs a plugin. The model's output is untrusted:
//! the manifest is checked like the process host checks it at `initialize`,
//! tool names must be `<plugin name>.<tool>` (so a plugin cannot take a
//! built-in tool's name or fall under the user's policy entry for one), a
//! `tool.before_invoke` hook needs a priority below the policy's, and file
//! paths stay inside the plugin's directory. Mounting the result is the
//! user's decision; the host checks the running plugin's manifest again.
//!
//! Configuration: `{ "dir": ".rah/plugins", "python": "python", "node":
//! "node", "max_files": 16, "max_file_bytes": 131072 }`. `python` and `node`
//! are the programs `mount.toml` starts. On each save they are looked up on
//! `PATH` (and a Python the configuration does not name also among uv's
//! installs) and written as full paths; one that is not found is written as
//! given, with a warning in `mount.toml` and in the reply. Looking only
//! checks that files exist; nothing is run.

mod manifest;
mod programs;
mod store;

use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex, PoisonError},
};

use rah_api::{
    KernelHandle, Plugin, PluginError, PluginId, PluginManifest, Scope, async_trait,
    hooks::ModelBeforeRequest,
    tools::{TOOLS_SERVICE, Tool, ToolContext, ToolError, provide_tools},
    ui::{
        ActionOutcome, Field, FieldKind, KeyValue, Node, UI_VIEWS, UI_VIEWS_SERVICE, UiChanged,
        UiError, UiViews, View, ViewContext, ViewSpec,
    },
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
/// The view action that goes back from a plugin's details to the list.
pub const LIST_ACTION: &str = "list";
/// The prefix of the view action that shows a plugin's details; the rest is a
/// plugin name.
pub const DETAILS_PREFIX: &str = "details:";
/// The longest plugin description, in characters.
pub const MAX_DESCRIPTION_CHARS: usize = 200;
const GUIDE: &str = include_str!("guide.md");
/// The system item every model request gets, so a request for "a plugin"
/// reaches these tools.
pub const HINT: &str = "You run inside RAH (Rust Agent Harness) with the plugin builder. When \
     the user asks for a plugin, or for a tool or automation you do not have, they mean a \
     personal RAH plugin: call `plugins.guide` first, then write the plugin and save it with \
     `plugins.save`. Do not search the workspace for a plugin system; the plugin is saved in \
     the user's plugin directory, not in the project.";
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
    pub const CONFIG_EXAMPLE: &str = r#"{ "dir": ".rah/plugins", "python": "python", "node": "node", "max_files": 16, "max_file_bytes": 131072 }"#;
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
}

impl Builder {
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
            store: Store::new(config.dir.unwrap_or_else(|| PathBuf::from(".rah/plugins"))),
            python_named: config.python.is_some(),
            python: config.python.unwrap_or_else(|| default_python.to_owned()),
            node: config.node.unwrap_or_else(|| "node".to_owned()),
            max_files: config.max_files.unwrap_or(DEFAULT_MAX_FILES),
            max_file_bytes: config.max_file_bytes.unwrap_or(DEFAULT_MAX_FILE_BYTES),
            search: self.search.clone().unwrap_or_else(Search::from_env),
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
        let views: Arc<dyn UiViews> = Arc::new(BuilderViews {
            builder,
            shown: Mutex::new(None),
        });
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
                 runtime library and what every plugin must do. Call it before `plugins.save` \
                 when the user asks for a plugin, or for a tool or automation RAH does not have."
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
                 tells the user how to mount it. Read `plugins.guide` first. It does not mount or \
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
        builder.kernel.emit(&UiChanged {
            instance: builder.instance.clone(),
            view: VIEW.to_owned(),
        });
        let warning = program.warning.map_or_else(String::new, |warning| {
            format!(
                " The interpreter needs the user's attention before mounting: {warning}. Tell \
                 the user."
            )
        });
        Ok(text(format!(
            "Saved plugin `{name}` in `{dir}` ({count} files of yours). It is not mounted and \
             has not run.{warning} Tell the user to read its files, then follow the steps in \
             `{MOUNT_FILE}` (also the policy entries its tools need):\n\n{mount}",
            name = plan.about.name,
            dir = dir.display(),
            count = plan.files.len(),
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

/// The view lists the saved plugins; a plugin's details show on its button.
struct BuilderViews {
    builder: Arc<Builder>,
    /// The plugin whose details are shown instead of the list.
    shown: Mutex<Option<String>>,
}

#[async_trait]
impl UiViews for BuilderViews {
    fn views(&self) -> Vec<ViewSpec> {
        vec![ViewSpec::new(VIEW, "My plugins")]
    }

    async fn render(&self, view: &str, _: &ViewContext) -> Result<View, UiError> {
        if view != VIEW {
            return Err(UiError::UnknownView(view.to_owned()));
        }
        let store = &self.builder.store;
        let saved = store.list().await;
        let mut nodes = vec![Node::Text {
            text: format!(
                "Plugins the model wrote for you, in {}. Describe what you need and the model \
                 writes one; it is never mounted for you. Read its files, then add its \
                 mount.toml entry to rah.toml.",
                store.dir().display()
            ),
        }];
        if saved.is_empty() {
            nodes.push(Node::Text {
                text: "No personal plugins yet.".to_owned(),
            });
            nodes.extend(import_form());
            return Ok(View { nodes });
        }
        let shown = self.shown();
        if let Some(plugin) = shown.and_then(|name| saved.iter().find(|p| p.about.name == name)) {
            let tools: Vec<&str> = plugin
                .manifest
                .tools
                .iter()
                .map(|t| t.name.as_str())
                .collect();
            nodes.push(Node::Button {
                action: LIST_ACTION.to_owned(),
                label: "Back to the list".to_owned(),
            });
            nodes.push(Node::Heading {
                text: plugin.about.name.clone(),
            });
            nodes.push(Node::KeyValue {
                items: vec![
                    KeyValue::new("Runtime", plugin.about.runtime.label()),
                    KeyValue::new("Tools", tools.join(", ")),
                    KeyValue::new("Description", plugin.about.description.clone()),
                    KeyValue::new("Directory", plugin.dir.display().to_string()),
                ],
            });
            if let Ok(mount) = tokio::fs::read_to_string(plugin.dir.join(MOUNT_FILE)).await {
                nodes.push(Node::Markdown {
                    text: format!("````toml\n{}\n````", mount.trim_end()),
                });
            }
            return Ok(View { nodes });
        }
        nodes.push(Node::Table {
            columns: ["Plugin", "Runtime", "Description"]
                .map(str::to_owned)
                .to_vec(),
            rows: saved
                .iter()
                .map(|plugin| {
                    vec![
                        plugin.about.name.clone(),
                        plugin.about.runtime.label().to_owned(),
                        plugin.about.description.clone(),
                    ]
                })
                .collect(),
        });
        nodes.extend(saved.iter().map(|plugin| Node::Button {
            action: format!("{DETAILS_PREFIX}{}", plugin.about.name),
            label: format!("Details: {}", plugin.about.name),
        }));
        nodes.push(Node::Heading {
            text: "Export".to_owned(),
        });
        nodes.push(Node::Text {
            text: "Tick the plugins to put in one bundle file, to keep or to import on another \
                   machine. An existing file is never overwritten."
                .to_owned(),
        });
        let mut fields: Vec<Field> = saved
            .iter()
            .map(|plugin| Field {
                name: format!("{EXPORT_PREFIX}{}", plugin.about.name),
                label: plugin.about.name.clone(),
                kind: FieldKind::Boolean,
                value: String::new(),
                options: Vec::new(),
            })
            .collect();
        fields.push(Field {
            name: "file".to_owned(),
            label: format!(
                "Bundle file (empty: {}); relative to the plugin directory",
                Path::new("exports")
                    .join("personal-plugins-<time>.json")
                    .display()
            ),
            kind: FieldKind::Text,
            value: String::new(),
            options: Vec::new(),
        });
        nodes.push(Node::Form {
            action: EXPORT_ACTION.to_owned(),
            submit: "Export".to_owned(),
            fields,
        });
        nodes.extend(import_form());
        Ok(View { nodes })
    }

    async fn act(
        &self,
        view: &str,
        action: &str,
        input: Value,
        _: &ViewContext,
    ) -> Result<ActionOutcome, UiError> {
        if view != VIEW {
            return Err(UiError::UnknownView(view.to_owned()));
        }
        if action == LIST_ACTION {
            self.show(None);
            return Ok(ActionOutcome {
                message: None,
                rerender: true,
            });
        }
        if let Some(name) = action.strip_prefix(DETAILS_PREFIX) {
            manifest::check_name(name).map_err(UiError::InvalidInput)?;
            self.show(Some(name.to_owned()));
            return Ok(ActionOutcome {
                message: None,
                rerender: true,
            });
        }
        let Value::Object(input) = input else {
            return Err(UiError::InvalidInput("the form sends an object".to_owned()));
        };
        let file = match input.get("file") {
            None | Some(Value::Null) => String::new(),
            Some(Value::String(file)) => file.clone(),
            Some(_) => return Err(UiError::InvalidInput("`file` must be text".to_owned())),
        };
        match action {
            EXPORT_ACTION => {
                let names: Vec<&str> = input
                    .iter()
                    .filter(|(_, value)| **value == Value::Bool(true))
                    .filter_map(|(key, _)| key.strip_prefix(EXPORT_PREFIX))
                    .collect();
                self.export(&names, &file).await
            }
            IMPORT_ACTION => {
                let replace = input.get("replace") == Some(&Value::Bool(true));
                self.import(&file, replace).await
            }
            other => Err(UiError::UnknownAction(other.to_owned())),
        }
    }
}

impl BuilderViews {
    /// The plugin whose details the view shows, if any.
    fn shown(&self) -> Option<String> {
        self.shown
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn show(&self, name: Option<String>) {
        *self.shown.lock().unwrap_or_else(PoisonError::into_inner) = name;
    }

    /// Writes the plugins `names` to a new bundle file.
    async fn export(&self, names: &[&str], file: &str) -> Result<ActionOutcome, UiError> {
        if names.is_empty() {
            return Err(UiError::InvalidInput(
                "tick at least one plugin to export".to_owned(),
            ));
        }
        let store = &self.builder.store;
        let mut entries = Vec::new();
        let mut left_out = Vec::new();
        for name in names {
            manifest::check_name(name).map_err(UiError::InvalidInput)?;
            let (entry, skipped) = store
                .entry(name)
                .await
                .map_err(|error| UiError::InvalidInput(format!("`{name}`: {error}")))?;
            entries.push(entry);
            left_out.extend(skipped.into_iter().map(|path| format!("{name}/{path}")));
        }
        let path = store.bundle_path(file);
        store
            .write_bundle(&path, entries)
            .await
            .map_err(UiError::Failed)?;
        let mut message = format!("Exported {} to {}.", names.join(", "), path.display());
        if !left_out.is_empty() {
            message.push_str(&format!(
                " Left out (not UTF-8 text or too large): {}.",
                left_out.join(", ")
            ));
        }
        Ok(ActionOutcome {
            message: Some(message),
            rerender: false,
        })
    }

    /// Saves the plugins of a bundle file. Every plugin passes the checks
    /// of `plugins.save` before any is written; a plugin whose name is
    /// taken is skipped unless `replace` is set.
    async fn import(&self, file: &str, replace: bool) -> Result<ActionOutcome, UiError> {
        if file.trim().is_empty() {
            return Err(UiError::InvalidInput("name the bundle file".to_owned()));
        }
        let builder = &self.builder;
        let path = builder.store.bundle_path(file);
        let entries = Store::read_bundle(&path)
            .await
            .map_err(UiError::InvalidInput)?;
        let mut plans = Vec::new();
        let mut names = std::collections::BTreeSet::new();
        for (index, mut entry) in entries.into_iter().enumerate() {
            let label = entry.get("name").and_then(Value::as_str).map_or_else(
                || format!("plugin {}", index + 1),
                |name| format!("`{name}`"),
            );
            if let Value::Object(map) = &mut entry {
                map.insert("replace".to_owned(), Value::Bool(replace));
            }
            let plan = plan(entry, builder.max_files, builder.max_file_bytes)
                .map_err(|error| UiError::InvalidInput(format!("{label}: {error}")))?;
            if !names.insert(plan.about.name.clone()) {
                return Err(UiError::InvalidInput(format!(
                    "{label} is in the bundle twice"
                )));
            }
            plans.push(plan);
        }
        if plans.is_empty() {
            return Err(UiError::InvalidInput(
                "the bundle holds no plugins".to_owned(),
            ));
        }
        let (mut imported, mut skipped, mut failed) = (Vec::new(), Vec::new(), Vec::new());
        for plan in &plans {
            let program = builder.program(plan.about.runtime);
            match builder.store.save(plan, &program).await {
                Ok(_) => imported.push(plan.about.name.clone()),
                Err(SaveError::Refused(reason)) => {
                    skipped.push(format!("{} ({reason})", plan.about.name));
                }
                Err(SaveError::Failed(reason)) => failed.push(reason),
            }
        }
        if !imported.is_empty() {
            builder.kernel.emit(&UiChanged {
                instance: builder.instance.clone(),
                view: VIEW.to_owned(),
            });
        }
        let mut message = if imported.is_empty() {
            "Imported nothing.".to_owned()
        } else {
            format!(
                "Imported {}; nothing was mounted, read the files first.",
                imported.join(", ")
            )
        };
        if !skipped.is_empty() {
            message.push_str(&format!(" Skipped: {}.", skipped.join("; ")));
        }
        if !failed.is_empty() {
            message.push_str(&format!(" Failed: {}.", failed.join("; ")));
        }
        Ok(ActionOutcome {
            message: Some(message),
            rerender: !imported.is_empty(),
        })
    }
}

/// The form that imports a bundle file.
fn import_form() -> [Node; 2] {
    [
        Node::Heading {
            text: "Import".to_owned(),
        },
        Node::Form {
            action: IMPORT_ACTION.to_owned(),
            submit: "Import".to_owned(),
            fields: vec![
                Field {
                    name: "file".to_owned(),
                    label: "Bundle file; relative to the plugin directory".to_owned(),
                    kind: FieldKind::Text,
                    value: String::new(),
                    options: Vec::new(),
                },
                Field {
                    name: "replace".to_owned(),
                    label: "Replace saved plugins of the same name".to_owned(),
                    kind: FieldKind::Boolean,
                    value: String::new(),
                    options: Vec::new(),
                },
            ],
        },
    ]
}

#[cfg(test)]
mod tests;
