//! `fs.list`, `fs.glob` and `fs.grep`: finding files in the workspace root
//! without Git.
//!
//! All three walk the tree in name order without following symbolic links,
//! skip hidden entries (names starting with `.`) unless asked, and do not
//! descend into the configured `skip_dirs`. A walk visits at most `max_scan`
//! entries and a result has at most `max_results` lines; output says when a
//! limit cut it short.

use std::{
    fs::FileType,
    path::{Path, PathBuf},
    sync::Arc,
};

use rah_api::{
    CancellationToken, async_trait,
    tools::{Tool, ToolContext, ToolError},
};
use rah_protocol::{PermissionLevel, ToolDefinition, ToolName, ToolOutput};
use regex::RegexBuilder;
use serde_json::{Value, json};

use crate::{glob::Glob, resolve, string_field, text_output};

/// Longest line `fs.grep` returns, in characters.
const MAX_LINE_CHARS: usize = 300;
/// How many leading bytes `fs.grep` checks for NUL to detect binary files.
const BINARY_SNIFF_BYTES: usize = 8000;
/// Deepest `fs.list` walk.
const MAX_LIST_DEPTH: u64 = 32;

/// The workspace and limits shared by the search tools.
pub(crate) struct Workspace {
    pub(crate) root: PathBuf,
    pub(crate) max_bytes: u64,
    pub(crate) max_results: usize,
    pub(crate) max_scan: usize,
    pub(crate) skip_dirs: Vec<String>,
}

enum Kind {
    Dir { skipped: bool },
    File { len: u64 },
    Link,
    Other,
}

struct Entry {
    /// Relative to the workspace root, `/`-separated.
    rel: String,
    /// Relative to where the walk started, `/`-separated.
    sub: String,
    path: PathBuf,
    kind: Kind,
}

struct Walk {
    entries: Vec<Entry>,
    /// The walk stopped at `max_scan` entries.
    truncated: bool,
}

fn slash_path(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

impl Workspace {
    fn rel(&self, path: &Path) -> String {
        path.strip_prefix(&self.root)
            .map(slash_path)
            .unwrap_or_default()
    }

    async fn kind(&self, name: &str, path: &Path, file_type: FileType) -> Kind {
        if file_type.is_symlink() {
            Kind::Link
        } else if file_type.is_dir() {
            Kind::Dir {
                skipped: self.skip_dirs.iter().any(|skip| skip == name),
            }
        } else if file_type.is_file() {
            let len = tokio::fs::symlink_metadata(path)
                .await
                .map(|metadata| metadata.len())
                .unwrap_or(0);
            Kind::File { len }
        } else {
            Kind::Other
        }
    }

    /// Walks `start` (a canonical directory inside the root) in pre-order,
    /// descending `max_depth` levels at most.
    async fn walk(
        &self,
        start: &Path,
        max_depth: usize,
        hidden: bool,
        cancel: &CancellationToken,
    ) -> Result<Walk, ToolError> {
        let mut entries = Vec::new();
        let mut pending: Vec<(PathBuf, Kind, usize)> = Vec::new();
        self.push_children(start, 1, hidden, &mut pending, true)
            .await?;
        while let Some((path, kind, depth)) = pending.pop() {
            if cancel.is_cancelled() {
                return Err(ToolError::Cancelled);
            }
            if entries.len() >= self.max_scan {
                return Ok(Walk {
                    entries,
                    truncated: true,
                });
            }
            if matches!(kind, Kind::Dir { skipped: false }) && depth < max_depth {
                self.push_children(&path, depth + 1, hidden, &mut pending, false)
                    .await?;
            }
            let sub = path.strip_prefix(start).map(slash_path).unwrap_or_default();
            entries.push(Entry {
                rel: self.rel(&path),
                sub,
                path,
                kind,
            });
        }
        Ok(Walk {
            entries,
            truncated: false,
        })
    }

    /// Pushes the children of `dir` so that they pop in name order. An
    /// unreadable directory below the start is left out.
    async fn push_children(
        &self,
        dir: &Path,
        depth: usize,
        hidden: bool,
        pending: &mut Vec<(PathBuf, Kind, usize)>,
        is_start: bool,
    ) -> Result<(), ToolError> {
        let mut reader = match tokio::fs::read_dir(dir).await {
            Ok(reader) => reader,
            Err(error) if is_start => return Err(ToolError::Execution(error.to_string())),
            Err(_) => return Ok(()),
        };
        let mut children = Vec::new();
        loop {
            let child = match reader.next_entry().await {
                Ok(Some(child)) => child,
                Ok(None) => break,
                Err(error) if is_start => return Err(ToolError::Execution(error.to_string())),
                Err(_) => break,
            };
            let name = child.file_name().to_string_lossy().into_owned();
            if !hidden && name.starts_with('.') {
                continue;
            }
            let Ok(file_type) = child.file_type().await else {
                continue;
            };
            let path = child.path();
            let kind = self.kind(&name, &path, file_type).await;
            children.push((name, path, kind));
        }
        children.sort_by(|a, b| b.0.cmp(&a.0));
        pending.extend(
            children
                .into_iter()
                .map(|(_, path, kind)| (path, kind, depth)),
        );
        Ok(())
    }

    /// Resolves the optional `path` input to a canonical path in the root.
    async fn start(&self, input: &Value) -> Result<(String, PathBuf), ToolError> {
        let requested = optional_string(input, "path")?.unwrap_or_else(|| ".".into());
        let path = resolve(&self.root, &requested).await?;
        Ok((requested, path))
    }

    fn cut_note(&self, scan_truncated: bool, results_truncated: bool) -> Option<String> {
        if results_truncated {
            Some(format!(
                "[stopped at {} results; narrow the path or pattern]",
                self.max_results
            ))
        } else if scan_truncated {
            Some(format!(
                "[stopped after scanning {} entries; narrow the path or pattern]",
                self.max_scan
            ))
        } else {
            None
        }
    }
}

fn optional_string(input: &Value, field: &str) -> Result<Option<String>, ToolError> {
    match input.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) => Ok(Some(text.clone())),
        Some(_) => Err(ToolError::InvalidInput(format!(
            "`{field}` must be a string"
        ))),
    }
}

fn optional_bool(input: &Value, field: &str) -> Result<bool, ToolError> {
    match input.get(field) {
        None | Some(Value::Null) => Ok(false),
        Some(Value::Bool(value)) => Ok(*value),
        Some(_) => Err(ToolError::InvalidInput(format!(
            "`{field}` must be a boolean"
        ))),
    }
}

fn glob_field(input: &Value, field: &str) -> Result<Option<Glob>, ToolError> {
    optional_string(input, field)?
        .map(|pattern| Glob::new(&pattern).map_err(ToolError::InvalidInput))
        .transpose()
}

fn finish(mut lines: Vec<String>, empty: &str, note: Option<String>) -> ToolOutput {
    if lines.is_empty() {
        lines.push(empty.into());
    }
    lines.extend(note);
    text_output(lines.join("\n"))
}

const PATH_PROPERTY: &str = "Directory relative to the workspace root (default `.`).";
const HIDDEN_PROPERTY: &str = "Include entries whose name starts with `.` (default false).";

/// Lists a directory, optionally a few levels deep.
pub(crate) struct FsList(pub(crate) Arc<Workspace>);

#[async_trait]
impl Tool for FsList {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: ToolName::new("fs.list"),
            description: "Lists a directory in the workspace. Directories end with `/`, \
                          symbolic links with `@`; files show their size. Large build \
                          and dependency directories are listed but not expanded."
                .into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": PATH_PROPERTY },
                    "depth": {
                        "type": "integer", "minimum": 1, "maximum": MAX_LIST_DEPTH,
                        "description": "How many levels to list (default 1)."
                    },
                    "hidden": { "type": "boolean", "description": HIDDEN_PROPERTY }
                },
                "additionalProperties": false
            }),
            permission: PermissionLevel::Read,
        }
    }

    async fn call(&self, input: Value, cx: ToolContext) -> Result<ToolOutput, ToolError> {
        let workspace = &self.0;
        let (requested, start) = workspace.start(&input).await?;
        let depth = match input.get("depth") {
            None | Some(Value::Null) => 1,
            Some(value) => value
                .as_u64()
                .filter(|depth| (1..=MAX_LIST_DEPTH).contains(depth))
                .ok_or_else(|| {
                    ToolError::InvalidInput(format!(
                        "`depth` must be an integer from 1 to {MAX_LIST_DEPTH}"
                    ))
                })?,
        };
        let hidden = optional_bool(&input, "hidden")?;
        if !tokio::fs::metadata(&start)
            .await
            .is_ok_and(|metadata| metadata.is_dir())
        {
            return Err(ToolError::InvalidInput(format!(
                "`{requested}` is not a directory"
            )));
        }
        let walk = workspace
            .walk(&start, depth as usize, hidden, &cx.cancel)
            .await?;
        let results_truncated = walk.entries.len() > workspace.max_results;
        let lines = walk
            .entries
            .iter()
            .take(workspace.max_results)
            .map(|entry| match entry.kind {
                Kind::Dir { skipped: false } => format!("{}/", entry.rel),
                Kind::Dir { skipped: true } => format!("{}/ (not expanded)", entry.rel),
                Kind::File { len } => format!("{} ({len} bytes)", entry.rel),
                Kind::Link => format!("{}@", entry.rel),
                Kind::Other => entry.rel.clone(),
            })
            .collect();
        Ok(finish(
            lines,
            "(empty directory)",
            workspace.cut_note(walk.truncated, results_truncated),
        ))
    }
}

/// Finds files by glob pattern.
pub(crate) struct FsGlob(pub(crate) Arc<Workspace>);

#[async_trait]
impl Tool for FsGlob {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: ToolName::new("fs.glob"),
            description: "Finds files in the workspace whose path matches a glob pattern. \
                          `*` and `?` match within one path segment, `**` matches any \
                          number of directories, `{a,b}` matches either. A pattern \
                          without `/` matches file names at any depth (`*.rs`). Returns \
                          paths relative to the workspace root."
                .into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "pattern": {
                        "type": "string",
                        "description": "Glob pattern, relative to `path`."
                    },
                    "path": { "type": "string", "description": PATH_PROPERTY },
                    "hidden": { "type": "boolean", "description": HIDDEN_PROPERTY }
                },
                "required": ["pattern"],
                "additionalProperties": false
            }),
            permission: PermissionLevel::Read,
        }
    }

    async fn call(&self, input: Value, cx: ToolContext) -> Result<ToolOutput, ToolError> {
        let workspace = &self.0;
        let glob = Glob::new(&string_field(&input, "pattern")?).map_err(ToolError::InvalidInput)?;
        let (requested, start) = workspace.start(&input).await?;
        let hidden = optional_bool(&input, "hidden")?;
        if !tokio::fs::metadata(&start)
            .await
            .is_ok_and(|metadata| metadata.is_dir())
        {
            return Err(ToolError::InvalidInput(format!(
                "`{requested}` is not a directory"
            )));
        }
        let walk = workspace
            .walk(&start, usize::MAX, hidden, &cx.cancel)
            .await?;
        let mut lines = Vec::new();
        let mut results_truncated = false;
        for entry in &walk.entries {
            if matches!(entry.kind, Kind::File { .. }) && glob.matches(&entry.sub) {
                if lines.len() == workspace.max_results {
                    results_truncated = true;
                    break;
                }
                lines.push(entry.rel.clone());
            }
        }
        Ok(finish(
            lines,
            "No files matched.",
            workspace.cut_note(walk.truncated, results_truncated),
        ))
    }
}

/// Searches file contents with a regular expression.
pub(crate) struct FsGrep(pub(crate) Arc<Workspace>);

#[async_trait]
impl Tool for FsGrep {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: ToolName::new("fs.grep"),
            description: "Searches UTF-8 text files in the workspace for a regular \
                          expression (Rust `regex` syntax) and returns \
                          `path:line: text` for each matching line. Binary and very \
                          large files are skipped."
                .into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "pattern": { "type": "string", "description": "Regular expression." },
                    "path": {
                        "type": "string",
                        "description": "File or directory relative to the workspace root (default `.`)."
                    },
                    "glob": {
                        "type": "string",
                        "description": "Only search files matching this glob, relative to `path` (for example `*.rs`)."
                    },
                    "ignore_case": { "type": "boolean", "description": "Case-insensitive match (default false)." },
                    "hidden": { "type": "boolean", "description": HIDDEN_PROPERTY }
                },
                "required": ["pattern"],
                "additionalProperties": false
            }),
            permission: PermissionLevel::Read,
        }
    }

    async fn call(&self, input: Value, cx: ToolContext) -> Result<ToolOutput, ToolError> {
        let workspace = &self.0;
        let regex = RegexBuilder::new(&string_field(&input, "pattern")?)
            .case_insensitive(optional_bool(&input, "ignore_case")?)
            .build()
            .map_err(|error| ToolError::InvalidInput(error.to_string()))?;
        let glob = glob_field(&input, "glob")?;
        let hidden = optional_bool(&input, "hidden")?;
        let (requested, start) = workspace.start(&input).await?;
        let metadata = tokio::fs::metadata(&start)
            .await
            .map_err(|error| ToolError::Execution(error.to_string()))?;
        let walk = if metadata.is_file() {
            let name = start
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            Walk {
                entries: vec![Entry {
                    rel: workspace.rel(&start),
                    sub: name,
                    path: start.clone(),
                    kind: Kind::File {
                        len: metadata.len(),
                    },
                }],
                truncated: false,
            }
        } else if metadata.is_dir() {
            workspace
                .walk(&start, usize::MAX, hidden, &cx.cancel)
                .await?
        } else {
            return Err(ToolError::InvalidInput(format!(
                "`{requested}` is neither a file nor a directory"
            )));
        };

        let mut lines = Vec::new();
        let mut results_truncated = false;
        let (mut large, mut binary) = (0usize, 0usize);
        'files: for entry in &walk.entries {
            let Kind::File { len } = entry.kind else {
                continue;
            };
            if glob.as_ref().is_some_and(|glob| !glob.matches(&entry.sub)) {
                continue;
            }
            if cx.cancel.is_cancelled() {
                return Err(ToolError::Cancelled);
            }
            if len > workspace.max_bytes {
                large += 1;
                continue;
            }
            let Ok(bytes) = tokio::fs::read(&entry.path).await else {
                continue;
            };
            let sniff = &bytes[..bytes.len().min(BINARY_SNIFF_BYTES)];
            let text = match std::str::from_utf8(&bytes) {
                Ok(text) if !sniff.contains(&0) => text,
                _ => {
                    binary += 1;
                    continue;
                }
            };
            for (index, line) in text.lines().enumerate() {
                if regex.is_match(line) {
                    if lines.len() == workspace.max_results {
                        results_truncated = true;
                        break 'files;
                    }
                    lines.push(format!("{}:{}: {}", entry.rel, index + 1, clip(line)));
                }
            }
        }
        let mut notes: Vec<String> = Vec::new();
        notes.extend(workspace.cut_note(walk.truncated, results_truncated));
        if large > 0 {
            notes.push(format!(
                "[skipped {large} file(s) over {} bytes]",
                workspace.max_bytes
            ));
        }
        if binary > 0 {
            notes.push(format!("[skipped {binary} binary or non-UTF-8 file(s)]"));
        }
        let note = (!notes.is_empty()).then(|| notes.join("\n"));
        Ok(finish(lines, "No matches.", note))
    }
}

fn clip(line: &str) -> String {
    let line = line.trim_end_matches('\r');
    match line.char_indices().nth(MAX_LINE_CHARS) {
        Some((cut, _)) => format!("{}…", &line[..cut]),
        None => line.to_owned(),
    }
}

#[cfg(test)]
#[path = "search_tests.rs"]
mod tests;
