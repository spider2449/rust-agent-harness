//! `todo`: a to-do list per session that the model keeps while it works.
//!
//! The plugin provides two tools and one view:
//!
//! - `todo.write` replaces the session's list with the items it is sent,
//!   each with a status (`pending`, `in_progress` or `completed`). The model
//!   sends the whole list every time; items left out are removed.
//! - `todo.read` returns the session's list, for a model whose history no
//!   longer holds its last `todo.write`.
//! - The `list` view (ADR 0062) shows the list of the session open in the
//!   window, read-only, and is announced with `UiChanged` after each write.
//!
//! Tool input is untrusted: items are checked against [`MAX_ITEMS`] and
//! [`MAX_ITEM_CHARS`], and whitespace is folded to single spaces. Both tools
//! declare permission `none`; the list lives in this plugin's memory and has
//! no effect outside it.
//!
//! The lists are kept in memory. A session whose list is not known (after a
//! restart, or in a fork) is rebuilt from its log through the `session`
//! service, if one is mounted: the last `todo.write` that succeeded (no
//! error, effect not `uncertain`) wins. Rebuilding only reads the log; nothing is executed again.
//!
//! The plugin takes no configuration.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex, PoisonError},
};

use rah_api::{
    KernelHandle, Plugin, PluginError, PluginId, PluginManifest, Scope, async_trait,
    session::SESSION,
    tools::{TOOLS_SERVICE, Tool, ToolContext, ToolError, provide_tools},
    ui::{
        Node, UI_VIEWS, UI_VIEWS_SERVICE, UiChanged, UiError, UiViews, View, ViewContext, ViewSpec,
    },
};
use rah_protocol::{
    PermissionLevel, SessionEntry, SessionId, ToolContent, ToolDefinition, ToolEffect, ToolName,
    ToolOutput,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// The tool that replaces a session's list.
pub const WRITE_TOOL: &str = "todo.write";
/// The tool that returns a session's list.
pub const READ_TOOL: &str = "todo.read";
/// The view that shows the list of the open session.
pub const VIEW: &str = "list";
/// The most items in one list.
pub const MAX_ITEMS: usize = 50;
/// The longest item, in characters.
pub const MAX_ITEM_CHARS: usize = 300;
/// The most session lists kept in memory; past it, one is dropped and
/// rebuilt from its log when it is needed again.
const MAX_SESSIONS: usize = 256;

pub struct TodoPlugin {
    manifest: PluginManifest,
}

impl TodoPlugin {
    pub fn new(id: impl Into<PluginId>) -> Self {
        Self {
            manifest: PluginManifest::new(id, env!("CARGO_PKG_VERSION"))
                .provides([TOOLS_SERVICE, UI_VIEWS_SERVICE]),
        }
    }

    /// An example configuration for reference; this plugin takes none.
    pub const CONFIG_EXAMPLE: &str = "{}";
}

#[async_trait]
impl Plugin for TodoPlugin {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    fn check_config(&self, config: &Value) -> Result<(), PluginError> {
        match config {
            Value::Null => Ok(()),
            Value::Object(map) if map.is_empty() => Ok(()),
            _ => Err("this plugin takes no configuration".into()),
        }
    }

    async fn apply(&self, scope: &mut Scope) -> Result<(), PluginError> {
        let lists = Arc::new(Lists {
            kernel: scope.kernel(),
            instance: scope.id().to_string(),
            known: Mutex::new(HashMap::new()),
        });
        provide_tools(
            scope,
            vec![
                Arc::new(WriteTool(lists.clone())),
                Arc::new(ReadTool(lists.clone())),
            ],
        )?;
        let views: Arc<dyn UiViews> = Arc::new(TodoViews(lists));
        scope.provide(UI_VIEWS, views)?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Pending,
    InProgress,
    Completed,
}

impl Status {
    fn mark(self) -> &'static str {
        match self {
            Self::Pending => "[ ]",
            Self::InProgress => "[>]",
            Self::Completed => "[x]",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Item {
    pub content: String,
    pub status: Status,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WriteInput {
    todos: Vec<Item>,
}

/// The list in `input` of `todo.write`, checked and cleaned.
fn parse_write(input: Value) -> Result<Vec<Item>, String> {
    let input: WriteInput = serde_json::from_value(input).map_err(|error| error.to_string())?;
    if input.todos.len() > MAX_ITEMS {
        return Err(format!(
            "{} items; a list holds at most {MAX_ITEMS}",
            input.todos.len()
        ));
    }
    input
        .todos
        .into_iter()
        .enumerate()
        .map(|(index, item)| {
            let content = item
                .content
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            if content.is_empty() {
                return Err(format!("item {} is empty", index + 1));
            }
            if content.chars().count() > MAX_ITEM_CHARS {
                return Err(format!(
                    "item {} is longer than {MAX_ITEM_CHARS} characters",
                    index + 1
                ));
            }
            Ok(Item {
                content,
                status: item.status,
            })
        })
        .collect()
}

/// One line per item, as the tools return it and the view shows it.
fn lines(items: &[Item]) -> Vec<String> {
    items
        .iter()
        .map(|item| format!("{} {}", item.status.mark(), item.content))
        .collect()
}

fn summary(items: &[Item]) -> String {
    let done = items
        .iter()
        .filter(|item| item.status == Status::Completed)
        .count();
    format!("{done} of {} completed", items.len())
}

/// The session lists, shared by the tools and the view.
struct Lists {
    kernel: KernelHandle,
    instance: String,
    known: Mutex<HashMap<SessionId, Vec<Item>>>,
}

impl Lists {
    fn known(&self) -> std::sync::MutexGuard<'_, HashMap<SessionId, Vec<Item>>> {
        self.known.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Keeps `items` as `session`'s list; `replace` decides whether a list
    /// already kept is replaced.
    fn keep(&self, session: &SessionId, items: Vec<Item>, replace: bool) -> Vec<Item> {
        let mut known = self.known();
        if let Some(kept) = known.get_mut(session) {
            if replace {
                *kept = items;
            }
            return kept.clone();
        }
        if known.len() >= MAX_SESSIONS
            && let Some(other) = known.keys().next().cloned()
        {
            known.remove(&other);
        }
        known.insert(session.clone(), items.clone());
        items
    }

    fn write(&self, session: &SessionId, items: Vec<Item>) {
        self.keep(session, items, true);
        self.kernel.emit(&UiChanged {
            instance: self.instance.clone(),
            view: VIEW.to_owned(),
        });
    }

    async fn get(&self, session: &SessionId) -> Vec<Item> {
        if let Some(items) = self.known().get(session) {
            return items.clone();
        }
        let rebuilt = self.rebuild(session).await;
        // A write that happened while the log was read wins.
        self.keep(session, rebuilt, false)
    }

    /// The list of the last `todo.write` in `session`'s log that succeeded
    /// (no error, effect not uncertain); empty without a `session` service.
    async fn rebuild(&self, session: &SessionId) -> Vec<Item> {
        let Some(log) = self.kernel.get(SESSION) else {
            return Vec::new();
        };
        let Ok(records) = log.read(session).await else {
            return Vec::new();
        };
        let mut calls: HashMap<String, Value> = HashMap::new();
        let mut items = Vec::new();
        for record in records {
            match record.entry {
                SessionEntry::ToolCall {
                    call_id,
                    name,
                    arguments,
                    ..
                } if name.as_str() == WRITE_TOOL => {
                    calls.insert(call_id, arguments);
                }
                SessionEntry::ToolResult {
                    call_id,
                    output,
                    effect,
                    ..
                } => {
                    if let Some(arguments) = calls.remove(&call_id)
                        && !output.is_error
                        // A `none` tool's success is recorded with effect
                        // `none`; only an uncertain write is left out.
                        && effect != ToolEffect::Uncertain
                        && let Ok(written) = parse_write(arguments)
                    {
                        items = written;
                    }
                }
                _ => {}
            }
        }
        items
    }
}

fn text(text: String) -> ToolOutput {
    ToolOutput {
        content: vec![ToolContent::Text(text)],
        is_error: false,
    }
}

/// The list as the tools return it.
fn listing(heading: &str, items: &[Item]) -> String {
    if items.is_empty() {
        return format!("{heading}: the list is empty.");
    }
    let mut text = format!("{heading} ({}):", summary(items));
    for line in lines(items) {
        text.push('\n');
        text.push_str(&line);
    }
    text
}

struct WriteTool(Arc<Lists>);

#[async_trait]
impl Tool for WriteTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: ToolName::new(WRITE_TOOL),
            description: "Replaces this session's to-do list, which the user sees beside the \
                 conversation. Use it for work with several steps: list the steps first, mark \
                 one `in_progress` before you start it and `completed` as soon as it is done. \
                 Send the whole list every time; items left out are removed, and an empty \
                 list clears it."
                .to_owned(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "todos": {
                        "type": "array",
                        "maxItems": MAX_ITEMS,
                        "items": {
                            "type": "object",
                            "properties": {
                                "content": {
                                    "type": "string",
                                    "minLength": 1,
                                    "maxLength": MAX_ITEM_CHARS,
                                    "description": "What to do, in one line."
                                },
                                "status": {
                                    "type": "string",
                                    "enum": ["pending", "in_progress", "completed"]
                                }
                            },
                            "required": ["content", "status"],
                            "additionalProperties": false
                        }
                    }
                },
                "required": ["todos"],
                "additionalProperties": false
            }),
            permission: PermissionLevel::None,
        }
    }

    async fn call(&self, input: Value, cx: ToolContext) -> Result<ToolOutput, ToolError> {
        let items = parse_write(input).map_err(ToolError::InvalidInput)?;
        let reply = listing("To-do list updated", &items);
        self.0.write(&cx.session, items);
        Ok(text(reply))
    }
}

struct ReadTool(Arc<Lists>);

#[async_trait]
impl Tool for ReadTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: ToolName::new(READ_TOOL),
            description: "Returns this session's to-do list, as last written with `todo.write`."
                .to_owned(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
            permission: PermissionLevel::None,
        }
    }

    async fn call(&self, input: Value, cx: ToolContext) -> Result<ToolOutput, ToolError> {
        if !matches!(&input, Value::Null) && !matches!(&input, Value::Object(map) if map.is_empty())
        {
            return Err(ToolError::InvalidInput(
                "this tool takes no input".to_owned(),
            ));
        }
        let items = self.0.get(&cx.session).await;
        Ok(text(listing("To-do list", &items)))
    }
}

struct TodoViews(Arc<Lists>);

#[async_trait]
impl UiViews for TodoViews {
    fn views(&self) -> Vec<ViewSpec> {
        vec![ViewSpec::new(VIEW, "To-do")]
    }

    async fn render(&self, view: &str, cx: &ViewContext) -> Result<View, UiError> {
        if view != VIEW {
            return Err(UiError::UnknownView(view.to_owned()));
        }
        let Some(session) = &cx.session else {
            return Ok(View {
                nodes: vec![Node::Text {
                    text: "Open a session to see its to-do list.".to_owned(),
                }],
            });
        };
        let items = self.0.get(session).await;
        if items.is_empty() {
            return Ok(View {
                nodes: vec![Node::Text {
                    text: format!("No to-do items. The model keeps this list with `{WRITE_TOOL}`."),
                }],
            });
        }
        Ok(View {
            nodes: vec![
                Node::Text {
                    text: format!("{}.", summary(&items)),
                },
                Node::List {
                    items: lines(&items),
                },
            ],
        })
    }
}

#[cfg(test)]
mod tests;
