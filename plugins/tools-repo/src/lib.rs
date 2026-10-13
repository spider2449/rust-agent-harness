//! `tools-repo`: read-only Git repository tools.
//!
//! - `repo.status`: branch line and changed paths (`git status`).
//! - `repo.diff`: worktree-versus-index diff, or index-versus-HEAD with
//!   `staged`, optionally for one path.
//! - `repo.list`: tracked children of the root or a subdirectory, `depth`
//!   levels deep (default 1, at most 16), so one call can show a tree.
//! - `repo.search`: tracked paths containing `query` (`mode = "path"`), or
//!   tracked lines containing it literally (`mode = "text"`).
//!
//! Git runs through the `sandbox` service with a hardened environment (no
//! system or global configuration, no fsmonitor, no optional locks, literal
//! pathspecs). Every call first checks that the configured directory is the
//! top level of a work tree. Model-supplied paths must be safe
//! repository-relative paths. The tools declare `read` permission. Git still
//! honors the repository's own `.git/config` (for example filter drivers), so
//! the tools are only as inert as the configured repository.
//!
//! Configuration (all optional): `{ "cwd": "<dir relative to the sandbox
//! root>", "git": "git", "timeout_ms": 15000, "max_output_bytes": 1048576,
//! "max_results": 500 }`.

mod git;

use std::{collections::BTreeMap, path::PathBuf, sync::Arc, time::Duration};

use rah_api::{
    Plugin, PluginError, PluginId, PluginManifest, Scope, async_trait,
    sandbox::{ExecEnv, SANDBOX},
    tools::{TOOLS_SERVICE, Tool, ToolContext, ToolError, provide_tools},
};
use rah_protocol::{PermissionLevel, ToolContent, ToolDefinition, ToolName, ToolOutput};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::git::Git;

/// Longest accepted repository-relative path or search query, in bytes.
pub const MAX_PATH_BYTES: usize = 1024;
/// Longest line text returned by a text search.
const MAX_LINE_CHARS: usize = 300;

pub struct RepoToolsPlugin {
    manifest: PluginManifest,
}

impl RepoToolsPlugin {
    pub fn new(id: impl Into<PluginId>) -> Self {
        Self {
            manifest: PluginManifest::new(id, env!("CARGO_PKG_VERSION"))
                .provides([TOOLS_SERVICE])
                .requires(["sandbox"]),
        }
    }
}

/// The plugin configuration.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepoConfig {
    pub cwd: Option<PathBuf>,
    pub git: Option<String>,
    pub timeout_ms: Option<u64>,
    pub max_output_bytes: Option<usize>,
    pub max_results: Option<usize>,
}

#[async_trait]
impl Plugin for RepoToolsPlugin {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    async fn apply(&self, scope: &mut Scope) -> Result<(), PluginError> {
        let config: RepoConfig = if scope.config().is_null() {
            RepoConfig::default()
        } else {
            scope.config_as()?
        };
        let sandbox = scope.inject(SANDBOX)?;
        provide_tools(scope, repo_tools(sandbox, config)?)?;
        Ok(())
    }
}

/// Builds the four tools over one sandbox.
pub fn repo_tools(
    sandbox: Arc<dyn ExecEnv>,
    config: RepoConfig,
) -> Result<Vec<Arc<dyn Tool>>, String> {
    let timeout_ms = config.timeout_ms.unwrap_or(15_000);
    let max_output_bytes = config.max_output_bytes.unwrap_or(1024 * 1024);
    let max_results = config.max_results.unwrap_or(500);
    if timeout_ms == 0 || max_output_bytes == 0 || max_results == 0 {
        return Err("timeout_ms, max_output_bytes and max_results must be positive".into());
    }
    let git = Arc::new(Git {
        sandbox,
        program: config.git.unwrap_or_else(|| "git".to_owned()),
        cwd: config.cwd,
        timeout: Duration::from_millis(timeout_ms),
        max_output_bytes,
    });
    Ok(vec![
        Arc::new(RepoStatus(git.clone())),
        Arc::new(RepoDiff(git.clone())),
        Arc::new(RepoList(git.clone(), max_results)),
        Arc::new(RepoSearch(git, max_results)),
    ])
}

fn definition(name: &str, description: &str, input_schema: Value) -> ToolDefinition {
    ToolDefinition {
        name: ToolName::new(name),
        description: description.into(),
        input_schema,
        permission: PermissionLevel::Read,
    }
}

fn path_schema(description: &str) -> Value {
    json!({
        "type": "string",
        "description": description,
        "minLength": 1,
        "maxLength": MAX_PATH_BYTES
    })
}

fn json_output(value: Value) -> ToolOutput {
    ToolOutput {
        content: vec![ToolContent::Json(value)],
        is_error: false,
    }
}

fn parse<T: serde::de::DeserializeOwned>(input: Value) -> Result<T, ToolError> {
    serde_json::from_value(input).map_err(|error| ToolError::InvalidInput(error.to_string()))
}

/// Accepts `a/b/c`: nonempty components separated by `/`, without `.`,
/// `..`, `.git`, backslashes, colons, NUL, a leading `-` or a trailing `/`.
pub fn validate_repo_path(path: &str) -> Result<(), ToolError> {
    let valid = !path.is_empty()
        && path.len() <= MAX_PATH_BYTES
        && !path.starts_with('-')
        && !path.contains(['\\', ':', '\0'])
        && path.split('/').all(|component| {
            !component.is_empty()
                && component != "."
                && component != ".."
                && !component.eq_ignore_ascii_case(".git")
        });
    if valid {
        Ok(())
    } else {
        Err(ToolError::InvalidInput(format!(
            "`{path}` is not a safe repository-relative path (use `a/b`, no `.`, `..`, `.git`, \
             backslashes, colons or a leading `-`)"
        )))
    }
}

fn split_nul(bytes: &[u8]) -> impl Iterator<Item = String> + '_ {
    bytes
        .split(|b| *b == 0)
        .filter(|record| !record.is_empty())
        .map(|record| String::from_utf8_lossy(record).into_owned())
}

/// Tracked file paths, optionally under `prefix`.
async fn tracked_files(
    git: &Git,
    prefix: Option<&str>,
    cx: &ToolContext,
) -> Result<(Vec<String>, bool), ToolError> {
    let mut args = vec!["ls-files", "-z", "--full-name"];
    if let Some(prefix) = prefix {
        args.extend(["--", prefix]);
    }
    let output = git.run(&args, &[0], cx.cancel.clone()).await?;
    let mut files: Vec<String> = split_nul(&output.stdout).collect();
    if output.truncated {
        // The last record may be cut off.
        files.pop();
    }
    Ok((files, output.truncated))
}

pub struct RepoStatus(Arc<Git>);

#[async_trait]
impl Tool for RepoStatus {
    fn definition(&self) -> ToolDefinition {
        definition(
            "repo.status",
            "Reports the current branch and the changed, staged and untracked paths of the \
             repository. Pass {}.",
            json!({ "type": "object", "properties": {}, "additionalProperties": false }),
        )
    }

    async fn call(&self, input: Value, cx: ToolContext) -> Result<ToolOutput, ToolError> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Input {}
        let Input {} = parse(input)?;
        self.0.ensure_top_level(cx.cancel.clone()).await?;
        let args = [
            "status",
            "--porcelain=v1",
            "-z",
            "--branch",
            "--untracked-files=normal",
            "--ignored=no",
            "--no-renames",
            "--ignore-submodules=all",
        ];
        let output = self.0.run(&args, &[0], cx.cancel).await?;
        let mut branch = None;
        let mut entries = Vec::new();
        for record in split_nul(&output.stdout) {
            if let Some(line) = record.strip_prefix("## ") {
                branch = Some(line.to_owned());
            } else if record.len() > 3 && record.is_char_boundary(3) {
                let (codes, path) = record.split_at(3);
                let mut codes = codes.chars();
                entries.push(json!({
                    "index": codes.next().map(String::from),
                    "worktree": codes.next().map(String::from),
                    "path": path,
                }));
            }
        }
        Ok(json_output(json!({
            "branch": branch,
            "entries": entries,
            "truncated": output.truncated,
        })))
    }
}

pub struct RepoDiff(Arc<Git>);

#[async_trait]
impl Tool for RepoDiff {
    fn definition(&self) -> ToolDefinition {
        definition(
            "repo.diff",
            "Shows the unstaged diff (worktree versus index), or with {\"staged\": true} the \
             staged diff (index versus HEAD). Optionally limited to one repository-relative \
             `path`. Large diffs are truncated.",
            json!({
                "type": "object",
                "properties": {
                    "staged": { "type": "boolean" },
                    "path": path_schema("Optional file or directory, relative to the repository, using `/`.")
                },
                "additionalProperties": false
            }),
        )
    }

    async fn call(&self, input: Value, cx: ToolContext) -> Result<ToolOutput, ToolError> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Input {
            #[serde(default)]
            staged: bool,
            path: Option<String>,
        }
        let input: Input = parse(input)?;
        if let Some(path) = &input.path {
            validate_repo_path(path)?;
        }
        self.0.ensure_top_level(cx.cancel.clone()).await?;
        let mut args = vec!["diff", "--no-color", "--no-ext-diff", "--no-textconv"];
        if input.staged {
            args.push("--cached");
        }
        if let Some(path) = &input.path {
            args.extend(["--", path.as_str()]);
        }
        let output = self.0.run(&args, &[0], cx.cancel).await?;
        let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
        if text.is_empty() {
            text = "no differences".into();
        } else if output.truncated {
            text.push_str("\n[diff truncated]");
        }
        Ok(ToolOutput {
            content: vec![ToolContent::Text(text)],
            is_error: false,
        })
    }
}

/// The deepest `repo.list` listing.
const MAX_LIST_DEPTH: usize = 16;

pub struct RepoList(Arc<Git>, usize);

#[async_trait]
impl Tool for RepoList {
    fn definition(&self) -> ToolDefinition {
        definition(
            "repo.list",
            "Lists the tracked files and directories of the repository root, or of a \
             subdirectory such as {\"path\": \"crates\"}. Pass {} for the root. `depth` \
             lists that many levels at once (default 1, at most 16), so one call with \
             {\"depth\": 4} shows the project structure; entry names are paths relative \
             to `path`. Prefer one deeper call over listing directories one by one.",
            json!({
                "type": "object",
                "properties": {
                    "path": path_schema("Optional subdirectory relative to the repository, using `/`."),
                    "depth": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": MAX_LIST_DEPTH,
                        "description": "Levels to list; 1 lists direct children only."
                    }
                },
                "additionalProperties": false
            }),
        )
    }

    async fn call(&self, input: Value, cx: ToolContext) -> Result<ToolOutput, ToolError> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Input {
            path: Option<String>,
            depth: Option<usize>,
        }
        let input: Input = parse(input)?;
        if let Some(path) = &input.path {
            validate_repo_path(path)?;
        }
        let depth = input.depth.unwrap_or(1);
        if !(1..=MAX_LIST_DEPTH).contains(&depth) {
            return Err(ToolError::InvalidInput(format!(
                "`depth` must be from 1 to {MAX_LIST_DEPTH}"
            )));
        }
        self.0.ensure_top_level(cx.cancel.clone()).await?;
        let (files, mut truncated) = tracked_files(&self.0, input.path.as_deref(), &cx).await?;
        let prefix = input
            .path
            .as_ref()
            .map(|p| format!("{p}/"))
            .unwrap_or_default();
        // Every path up to `depth` levels below the prefix: the ancestors of a
        // tracked file are directories.
        let mut children: BTreeMap<&str, &str> = BTreeMap::new();
        for file in &files {
            let Some(rest) = file.strip_prefix(prefix.as_str()) else {
                continue;
            };
            let mut levels = 0;
            for (end, _) in rest.match_indices('/').take(depth) {
                children.insert(&rest[..end], "dir");
                levels += 1;
            }
            if levels < depth {
                children.entry(rest).or_insert("file");
            }
        }
        if children.is_empty() && input.path.is_some() {
            return Err(ToolError::InvalidInput(format!(
                "`{}` is not a directory with tracked files",
                input.path.unwrap_or_default()
            )));
        }
        truncated |= children.len() > self.1;
        let entries: Vec<Value> = children
            .into_iter()
            .take(self.1)
            .map(|(name, kind)| json!({ "name": name, "kind": kind }))
            .collect();
        Ok(json_output(json!({
            "path": input.path,
            "entries": entries,
            "truncated": truncated,
        })))
    }
}

pub struct RepoSearch(Arc<Git>, usize);

#[async_trait]
impl Tool for RepoSearch {
    fn definition(&self) -> ToolDefinition {
        definition(
            "repo.search",
            "Searches tracked files. mode \"path\" finds file paths containing `query` \
             (case-insensitive); mode \"text\" finds lines containing `query` literally \
             (case-sensitive, binary files skipped). Optional `path_prefix` limits the search \
             to a subdirectory.",
            json!({
                "type": "object",
                "properties": {
                    "mode": { "type": "string", "enum": ["path", "text"] },
                    "query": { "type": "string", "minLength": 1, "maxLength": MAX_PATH_BYTES },
                    "path_prefix": path_schema("Optional subdirectory relative to the repository, using `/`.")
                },
                "required": ["mode", "query"],
                "additionalProperties": false
            }),
        )
    }

    async fn call(&self, input: Value, cx: ToolContext) -> Result<ToolOutput, ToolError> {
        #[derive(Deserialize)]
        #[serde(rename_all = "lowercase")]
        enum Mode {
            Path,
            Text,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Input {
            mode: Mode,
            query: String,
            path_prefix: Option<String>,
        }
        let input: Input = parse(input)?;
        if input.query.is_empty()
            || input.query.len() > MAX_PATH_BYTES
            || input.query.contains(['\0', '\n', '\r'])
        {
            return Err(ToolError::InvalidInput(format!(
                "query must be 1 to {MAX_PATH_BYTES} bytes on one line"
            )));
        }
        if let Some(prefix) = &input.path_prefix {
            validate_repo_path(prefix)?;
        }
        self.0.ensure_top_level(cx.cancel.clone()).await?;
        let limit = self.1;
        match input.mode {
            Mode::Path => {
                let (files, truncated) =
                    tracked_files(&self.0, input.path_prefix.as_deref(), &cx).await?;
                let needle = input.query.to_lowercase();
                let matches: Vec<&String> = files
                    .iter()
                    .filter(|file| file.to_lowercase().contains(&needle))
                    .collect();
                Ok(json_output(json!({
                    "mode": "path",
                    "paths": matches.iter().take(limit).collect::<Vec<_>>(),
                    "truncated": truncated || matches.len() > limit,
                })))
            }
            Mode::Text => {
                let mut args = vec![
                    "grep",
                    "-z",
                    "-n",
                    "-I",
                    "--no-color",
                    "--full-name",
                    "-F",
                    "-e",
                ];
                args.push(&input.query);
                if let Some(prefix) = &input.path_prefix {
                    args.extend(["--", prefix.as_str()]);
                }
                // Exit code 1 means no match.
                let output = self.0.run(&args, &[0, 1], cx.cancel).await?;
                let text = String::from_utf8_lossy(&output.stdout);
                let mut lines: Vec<&str> = text.lines().collect();
                if output.truncated {
                    lines.pop();
                }
                let matches: Vec<Value> = lines
                    .iter()
                    .filter_map(|line| {
                        let mut fields = line.splitn(3, '\0');
                        let path = fields.next()?;
                        let number: u64 = fields.next()?.parse().ok()?;
                        let content: String = fields.next()?.chars().take(MAX_LINE_CHARS).collect();
                        Some(json!({ "path": path, "line": number, "text": content }))
                    })
                    .collect();
                let truncated = output.truncated || matches.len() > limit;
                Ok(json_output(json!({
                    "mode": "text",
                    "matches": matches.into_iter().take(limit).collect::<Vec<_>>(),
                    "truncated": truncated,
                })))
            }
        }
    }
}

#[cfg(test)]
mod tests;
