//! The manifest of a personal plugin and the checks it passes before it is
//! saved. They follow what the process plugin host accepts at `initialize`
//! (`docs/PROCESS_PLUGINS.md`), plus rules for plugins the model writes:
//! tool names carry the plugin's name, and a `tool.before_invoke` hook runs
//! after the policy.

use std::collections::BTreeSet;

use rah_api::ui::ViewSpec;
use rah_protocol::ToolDefinition;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// Host services a process plugin may require and call.
pub const CALLABLE_SERVICES: [&str; 2] = ["session", "storage"];
/// Hooks a process plugin may attach to.
pub const HOOKS: [&str; 3] = [
    "tool.before_invoke",
    "tool.after_invoke",
    "model.before_request",
];
/// Events a process plugin may follow.
pub const EVENTS: [&str; 1] = ["session.appended"];
/// The hook that decides tool calls next to the policy.
pub const BEFORE_INVOKE: &str = "tool.before_invoke";
/// The most tools one process plugin provides.
pub const MAX_TOOLS: usize = 128;
/// The most views one process plugin offers.
pub const MAX_VIEWS: usize = 32;
const MAX_TOOL_NAME_BYTES: usize = 128;
const MAX_DESCRIPTION_CHARS: usize = 2000;
const MAX_TITLE_CHARS: usize = 64;
const MAX_VERSION_CHARS: usize = 64;
/// The longest plugin name, in bytes.
pub const MAX_NAME_BYTES: usize = 48;

/// Names a personal plugin cannot take: namespaces of RAH's own tools and
/// plugins. A personal `repo.*` tool would fall under a policy entry the user
/// wrote for the built-in repository tools.
pub const RESERVED_NAMES: [&str; 24] = [
    "admin",
    "agent",
    "context",
    "fs",
    "http",
    "loop",
    "mcp",
    "memory",
    "model",
    "other",
    "plugin",
    "plugins",
    "policy",
    "process",
    "rah",
    "repo",
    "sandbox",
    "scheduler",
    "session",
    "shell",
    "skills",
    "storage",
    "todo",
    "web",
];

/// One `hooks` entry.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HookSpec {
    pub hook: String,
    #[serde(default)]
    pub priority: i32,
}

/// What the model declares; `protocol` and `provides` are derived.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub version: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requires: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<ToolDefinition>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hooks: Vec<HookSpec>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub events: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub views: Vec<ViewSpec>,
}

impl Manifest {
    /// The services the plugin provides, derived from what it lists.
    pub fn provides(&self) -> Vec<&'static str> {
        let mut provides = Vec::new();
        if !self.tools.is_empty() {
            provides.push("tools");
        }
        if !self.views.is_empty() {
            provides.push("ui-views");
        }
        provides
    }

    /// `manifest.json`: the `initialize` result without `protocol`, which the
    /// runtime library adds.
    pub fn to_file(&self) -> Value {
        let mut value = serde_json::to_value(self).unwrap_or_else(|_| json!({}));
        if let Value::Object(map) = &mut value {
            map.insert("provides".to_owned(), json!(self.provides()));
        }
        value
    }

    /// Reads `manifest.json` back; `provides` is derived again.
    pub fn from_file(mut value: Value) -> Result<Self, String> {
        if let Value::Object(map) = &mut value {
            map.remove("provides");
        }
        serde_json::from_value(value).map_err(|error| error.to_string())
    }
}

/// Whether `name` can name a personal plugin: 1 to [`MAX_NAME_BYTES`] of
/// lowercase letters, digits and `-`, starting with a letter, not ending with
/// `-`, and not reserved.
pub fn check_name(name: &str) -> Result<(), String> {
    let well_formed = !name.is_empty()
        && name.len() <= MAX_NAME_BYTES
        && name.starts_with(|c: char| c.is_ascii_lowercase())
        && !name.ends_with('-')
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
    if !well_formed {
        return Err(format!(
            "invalid plugin name `{name}`: 1 to {MAX_NAME_BYTES} lowercase letters, digits and \
             `-`, starting with a letter"
        ));
    }
    if RESERVED_NAMES.contains(&name) {
        return Err(format!(
            "`{name}` is a namespace of RAH itself; choose another name"
        ));
    }
    Ok(())
}

/// Checks the manifest of the plugin `name`.
pub fn check(name: &str, manifest: &Manifest) -> Result<(), String> {
    let version = manifest.version.trim();
    if version.is_empty()
        || version.chars().count() > MAX_VERSION_CHARS
        || version.chars().any(char::is_control)
    {
        return Err(format!(
            "`version` must be one line of 1 to {MAX_VERSION_CHARS} characters"
        ));
    }
    if manifest.tools.is_empty()
        && manifest.hooks.is_empty()
        && manifest.events.is_empty()
        && manifest.views.is_empty()
    {
        return Err("the manifest lists no tools, hooks, events or views".into());
    }
    unique_within("requires", &manifest.requires, &CALLABLE_SERVICES)?;
    check_tools(name, &manifest.tools)?;
    let hooks: Vec<String> = manifest.hooks.iter().map(|h| h.hook.clone()).collect();
    unique_within("hooks", &hooks, &HOOKS)?;
    if let Some(hook) = manifest
        .hooks
        .iter()
        .find(|hook| hook.hook == BEFORE_INVOKE && hook.priority >= 0)
    {
        return Err(format!(
            "`{BEFORE_INVOKE}` needs a priority below 0 (it has {}), so the policy decides \
             first and the plugin cannot let through a call the policy refuses",
            hook.priority
        ));
    }
    unique_within("events", &manifest.events, &EVENTS)?;
    check_views(&manifest.views)
}

fn unique_within(field: &str, values: &[String], allowed: &[&str]) -> Result<(), String> {
    let mut seen = BTreeSet::new();
    for value in values {
        if !allowed.contains(&value.as_str()) {
            return Err(format!(
                "`{field}` cannot contain `{value}`; only: {}",
                allowed.join(", ")
            ));
        }
        if !seen.insert(value) {
            return Err(format!("`{field}` lists `{value}` twice"));
        }
    }
    Ok(())
}

fn check_tools(name: &str, tools: &[ToolDefinition]) -> Result<(), String> {
    if tools.len() > MAX_TOOLS {
        return Err(format!("more than {MAX_TOOLS} tools"));
    }
    let prefix = format!("{name}.");
    let mut names = BTreeSet::new();
    for tool in tools {
        let tool_name = tool.name.as_str();
        let suffix = tool_name.strip_prefix(&prefix).unwrap_or_default();
        if suffix.is_empty()
            || tool_name.len() > MAX_TOOL_NAME_BYTES
            || tool_name
                .chars()
                .any(|c| c.is_whitespace() || c.is_control())
        {
            return Err(format!(
                "invalid tool name `{tool_name}`: it must be `{prefix}<tool>`, without \
                 whitespace, at most {MAX_TOOL_NAME_BYTES} bytes"
            ));
        }
        if !names.insert(tool_name) {
            return Err(format!("tool `{tool_name}` is listed twice"));
        }
        let description = tool.description.trim();
        if description.is_empty() || description.chars().count() > MAX_DESCRIPTION_CHARS {
            return Err(format!(
                "tool `{tool_name}` needs a description of at most {MAX_DESCRIPTION_CHARS} \
                 characters"
            ));
        }
        if tool.input_schema.get("type") != Some(&json!("object")) {
            return Err(format!(
                "the `input_schema` of `{tool_name}` must be a JSON Schema with \
                 `\"type\": \"object\"`"
            ));
        }
    }
    Ok(())
}

fn check_views(views: &[ViewSpec]) -> Result<(), String> {
    if views.len() > MAX_VIEWS {
        return Err(format!("more than {MAX_VIEWS} views"));
    }
    let mut ids = BTreeSet::new();
    for view in views {
        if !ViewSpec::valid_id(&view.id) {
            return Err(format!(
                "invalid view id `{}`: lowercase letters, digits, `-` and `_`",
                view.id
            ));
        }
        if !ids.insert(view.id.as_str()) {
            return Err(format!("view `{}` is listed twice", view.id));
        }
        let title = view.title.trim();
        if title.is_empty()
            || title.chars().count() > MAX_TITLE_CHARS
            || title.chars().any(char::is_control)
        {
            return Err(format!(
                "view `{}` needs a one-line title of at most {MAX_TITLE_CHARS} characters",
                view.id
            ));
        }
    }
    Ok(())
}
