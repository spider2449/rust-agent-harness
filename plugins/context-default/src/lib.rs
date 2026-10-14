//! `context-default`: shapes every model request through
//! `model.before_request`.
//!
//! - `system`: a system prompt, placed first.
//! - A sliding window over the history: the newest items within `max_items`
//!   and `max_chars` are kept. The window only starts at a user message, so
//!   a tool call is never separated from its result, and the current turn is
//!   always kept whole. Omitted items are replaced by one short note. The
//!   session log itself is unchanged.
//! - Images in tool results are sent only in the current turn. Earlier ones
//!   are replaced by a short text, and each counts as [`IMAGE_CHARS`]
//!   characters in the window.
//! - Memory notes, injected after the system prompt (at most
//!   `max_memory_bytes`, read on every request). With memory the plugin also
//!   provides `memory.remember {note}` (`write` permission), which appends one
//!   line. Either `memory_dir`: one file per workspace in that directory,
//!   chosen on every request from the `sandbox` service's root (so a project
//!   keeps its own notes, and switching the workspace switches them), or
//!   `memory_file`: one file for everything.
//!
//! Configuration (all optional): `{ "system": "...", "max_items": 200,
//! "max_chars": 200000, "memory_dir": ".rah/memory",
//! "max_memory_bytes": 16384 }`, or `"memory_file": ".rah/memory.md"`
//! instead of `memory_dir`.

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use rah_api::{
    KernelHandle, Plugin, PluginError, PluginId, PluginManifest, Scope, async_trait,
    hooks::ModelBeforeRequest,
    sandbox::SANDBOX,
    tools::{TOOLS_SERVICE, Tool, ToolContext, ToolError, describe_image, provide_tools},
};
use rah_protocol::{
    ContextItem, ModelRequest, PermissionLevel, ToolContent, ToolDefinition, ToolName, ToolOutput,
};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::{io::AsyncWriteExt, sync::Mutex};

const DEFAULT_MAX_ITEMS: usize = 200;
const DEFAULT_MAX_CHARS: usize = 200_000;
/// What one image counts for in `max_chars`: roughly what a model is
/// charged for an image, not the size of its data.
pub const IMAGE_CHARS: usize = 4_000;
const DEFAULT_MAX_MEMORY_BYTES: u64 = 16 * 1024;
const MAX_NOTE_CHARS: usize = 1000;

pub struct ContextPlugin {
    manifest: PluginManifest,
}

impl ContextPlugin {
    pub fn new(id: impl Into<PluginId>) -> Self {
        Self {
            manifest: PluginManifest::new(id, env!("CARGO_PKG_VERSION")).provides([TOOLS_SERVICE]),
        }
    }
}

/// The plugin configuration.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContextConfig {
    pub system: Option<String>,
    pub max_items: Option<usize>,
    pub max_chars: Option<usize>,
    pub memory_file: Option<PathBuf>,
    pub memory_dir: Option<PathBuf>,
    pub max_memory_bytes: Option<u64>,
}

#[async_trait]
impl Plugin for ContextPlugin {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    async fn apply(&self, scope: &mut Scope) -> Result<(), PluginError> {
        let config: ContextConfig = if scope.config().is_null() {
            ContextConfig::default()
        } else {
            scope.config_as()?
        };
        let window = Window {
            max_items: config.max_items.unwrap_or(DEFAULT_MAX_ITEMS).max(1),
            max_chars: config.max_chars.unwrap_or(DEFAULT_MAX_CHARS).max(1),
        };
        let location = match (config.memory_file.clone(), config.memory_dir.clone()) {
            (Some(_), Some(_)) => {
                return Err("set either `memory_file` or `memory_dir`, not both".into());
            }
            (Some(path), None) => Some(Location::File(path)),
            (None, Some(dir)) => Some(Location::PerWorkspace {
                dir,
                kernel: scope.kernel(),
            }),
            (None, None) => None,
        };
        let memory = location.map(|location| {
            Arc::new(Memory {
                location,
                max_bytes: config.max_memory_bytes.unwrap_or(DEFAULT_MAX_MEMORY_BYTES),
                write: Mutex::new(()),
            })
        });
        let system = config.system.clone();
        let reader = memory.clone();
        scope.hook_waterfall::<ModelBeforeRequest, _, _>(0, move |request| {
            let system = system.clone();
            let reader = reader.clone();
            async move {
                let notes = match &reader {
                    Some(memory) => memory.read().await,
                    None => None,
                };
                shape(request, system.as_deref(), notes.as_deref(), window)
            }
        });
        let tools: Vec<Arc<dyn Tool>> = match memory {
            Some(memory) => vec![Arc::new(Remember(memory))],
            None => Vec::new(),
        };
        provide_tools(scope, tools)?;
        Ok(())
    }
}

/// History limits.
#[derive(Clone, Copy, Debug)]
pub struct Window {
    pub max_items: usize,
    pub max_chars: usize,
}

fn size(item: &ContextItem) -> usize {
    match item {
        ContextItem::System { text }
        | ContextItem::User { text }
        | ContextItem::Assistant { text } => text.len(),
        ContextItem::ToolCall { arguments, .. } => arguments.to_string().len(),
        ContextItem::ToolResult { output, .. } => output
            .content
            .iter()
            .map(|content| match content {
                ToolContent::Text(text) => text.len(),
                ToolContent::Json(value) => value.to_string().len(),
                ToolContent::Image(_) => IMAGE_CHARS,
            })
            .sum(),
    }
}

/// Replaces the images of turns before the last user message with text, so
/// a screenshot is sent once rather than with every later request.
fn earlier_images_as_text(history: &mut [ContextItem]) {
    let Some(last_user) = history
        .iter()
        .rposition(|item| matches!(item, ContextItem::User { .. }))
    else {
        return;
    };
    for item in &mut history[..last_user] {
        if let ContextItem::ToolResult { output, .. } = item {
            for content in &mut output.content {
                if let ToolContent::Image(image) = content {
                    *content = ToolContent::Text(format!(
                        "{} (from an earlier message; not shown again)",
                        describe_image(image)
                    ));
                }
            }
        }
    }
}

/// Applies the system prompt, memory notes and the history window.
pub fn shape(
    mut request: ModelRequest,
    system: Option<&str>,
    notes: Option<&str>,
    window: Window,
) -> ModelRequest {
    let split = request
        .items
        .iter()
        .position(|item| !matches!(item, ContextItem::System { .. }))
        .unwrap_or(request.items.len());
    let mut history = request.items.split_off(split);
    let mut leading = std::mem::take(&mut request.items);
    earlier_images_as_text(&mut history);

    // Walk back from the newest item while within both limits.
    let (mut count, mut chars, mut start) = (0, 0, history.len());
    for (index, item) in history.iter().enumerate().rev() {
        if count + 1 > window.max_items || chars + size(item) > window.max_chars {
            break;
        }
        count += 1;
        chars += size(item);
        start = index;
    }
    // Start at a user message, and never after the last one.
    let is_user = |item: &ContextItem| matches!(item, ContextItem::User { .. });
    let last_user = history.iter().rposition(is_user);
    start = history[start..]
        .iter()
        .position(is_user)
        .map(|offset| start + offset)
        .or(last_user)
        .unwrap_or(start);
    if let Some(last_user) = last_user {
        start = start.min(last_user);
    }

    let mut items = Vec::with_capacity(history.len() - start + leading.len() + 3);
    if let Some(system) = system.filter(|text| !text.is_empty()) {
        items.push(ContextItem::System {
            text: system.to_owned(),
        });
    }
    items.append(&mut leading);
    if let Some(notes) = notes.filter(|text| !text.trim().is_empty()) {
        items.push(ContextItem::System {
            text: format!("Notes saved with memory.remember:\n{}", notes.trim_end()),
        });
    }
    if start > 0 {
        items.push(ContextItem::System {
            text: format!("[{start} earlier conversation items are omitted.]"),
        });
    }
    items.extend(history.into_iter().skip(start));
    request.items = items;
    request
}

/// Where memory notes live.
enum Location {
    File(PathBuf),
    /// One file per workspace in `dir`, named after the sandbox root.
    PerWorkspace {
        dir: PathBuf,
        kernel: KernelHandle,
    },
}

struct Memory {
    location: Location,
    max_bytes: u64,
    /// Serializes appends.
    write: Mutex<()>,
}

/// The memory file name for a workspace: its folder name (made safe) and a
/// hash of its path, so two folders with one name stay apart. Windows paths
/// ignore case.
pub fn workspace_file_name(root: &Path) -> String {
    let path = root.display().to_string();
    let key = if cfg!(windows) {
        path.to_lowercase()
    } else {
        path.clone()
    };
    // FNV-1a: stable across runs and platforms, unlike the std hasher.
    let hash = key.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
    });
    let name: String = root
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, '-' | '_' | '.') {
                c
            } else {
                '_'
            }
        })
        .take(40)
        .collect();
    let name = if name.is_empty() { "workspace" } else { &name };
    format!("{name}-{:08x}.md", hash >> 32)
}

impl Memory {
    /// The file for the current workspace.
    fn path(&self) -> PathBuf {
        match &self.location {
            Location::File(path) => path.clone(),
            Location::PerWorkspace { dir, kernel } => {
                match kernel.get(SANDBOX).and_then(|sandbox| sandbox.root()) {
                    Some(root) => dir.join(workspace_file_name(&root)),
                    None => dir.join("default.md"),
                }
            }
        }
    }

    /// The newest `max_bytes` of the file, if it exists.
    async fn read(&self) -> Option<String> {
        let bytes = tokio::fs::read(self.path()).await.ok()?;
        let keep = (self.max_bytes as usize).min(bytes.len());
        let tail = &bytes[bytes.len() - keep..];
        // Start at a line boundary when the head was cut.
        let tail = if keep < bytes.len() {
            tail.iter()
                .position(|b| *b == b'\n')
                .map_or(tail, |at| &tail[at + 1..])
        } else {
            tail
        };
        Some(String::from_utf8_lossy(tail).into_owned())
    }
}

/// `memory.remember {note}`: appends one line to the memory file.
struct Remember(Arc<Memory>);

#[async_trait]
impl Tool for Remember {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: ToolName::new("memory.remember"),
            description: match self.0.location {
                Location::File(_) => {
                    "Saves a short note that will be shown at the start of later requests, \
                     also in new sessions."
                }
                Location::PerWorkspace { .. } => {
                    "Saves a short note about this project that will be shown at the start of \
                     later requests in this workspace, also in new sessions."
                }
            }
            .into(),
            input_schema: json!({
                "type": "object",
                "properties": { "note": { "type": "string", "minLength": 1, "maxLength": MAX_NOTE_CHARS } },
                "required": ["note"],
                "additionalProperties": false
            }),
            permission: PermissionLevel::Write,
        }
    }

    async fn call(&self, input: Value, _cx: ToolContext) -> Result<ToolOutput, ToolError> {
        let note = input
            .get("note")
            .and_then(Value::as_str)
            .map(|note| note.split_whitespace().collect::<Vec<_>>().join(" "))
            .filter(|note| !note.is_empty())
            .ok_or_else(|| ToolError::InvalidInput("expected a non-empty string `note`".into()))?;
        if note.chars().count() > MAX_NOTE_CHARS {
            return Err(ToolError::InvalidInput(format!(
                "a note has at most {MAX_NOTE_CHARS} characters"
            )));
        }
        let _guard = self.0.write.lock().await;
        let path = self.0.path();
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|error| ToolError::NotStarted(format!("memory directory: {error}")))?;
        }
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .await
            .map_err(|error| ToolError::NotStarted(format!("memory file: {error}")))?;
        file.write_all(format!("- {note}\n").as_bytes())
            .await
            .map_err(|error| ToolError::Execution(format!("memory file: {error}")))?;
        file.flush()
            .await
            .map_err(|error| ToolError::Execution(format!("memory file: {error}")))?;
        Ok(ToolOutput {
            content: vec![ToolContent::Text("saved".into())],
            is_error: false,
        })
    }
}

impl ContextPlugin {
    /// An example configuration, mostly the defaults, for reference (the Web
    /// UI shows it in an empty configuration editor). It is never applied.
    pub const CONFIG_EXAMPLE: &str = r#"{
  "system": "<system prompt>",
  "max_items": 200,
  "max_chars": 200000,
  "memory_dir": ".rah/memory",
  "max_memory_bytes": 16384
}"#;
}

#[cfg(test)]
mod tests;

#[cfg(test)]
#[test]
fn the_config_example_is_a_valid_configuration() {
    let parsed: Result<ContextConfig, _> = serde_json::from_str(ContextPlugin::CONFIG_EXAMPLE);
    assert!(parsed.is_ok(), "{:?}", parsed.err());
}
