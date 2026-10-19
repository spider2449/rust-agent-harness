//! `lsp-diagnostics`: appends language server diagnostics to the output of
//! tools that wrote files (ADR 0048).
//!
//! The plugin attaches to `tool.after_invoke`. When a tool declared `write`
//! completed, it takes the files the call named from its arguments (`path`,
//! `edits[].path`, or the `+++` lines of a `patch`), sends their new text to
//! the language server for their extension, waits for the diagnostics the
//! server publishes and appends the ones at or above `severity` to the tool
//! output. Nothing is appended when there are none. The tool's result, its
//! effect and its policy decision are not changed.
//!
//! Servers are started lazily through the `sandbox` service, one process per
//! server for the life of the plugin, and end with its scope. A server that
//! cannot start or ends is not restarted for five minutes; for a default
//! server the user gets one
//! [`HostNotice`](rah_api::host::HostNotice) the first time, and a server
//! named in `servers` reports in the output (ADR 0057). The first check
//! of a slow server (indexing) may time out; the output then says so, and
//! later checks use what the server has loaded.
//!
//! Diagnostics are text from an external program about untrusted files, so
//! they are bounded (`max_diagnostics`, `max_message_chars`), flattened to one
//! line and marked as tool output. They grant nothing.
//!
//! Configuration (all optional): `{ "root": ".", "defaults": true,
//! "servers": { "<name>": { "command": ["..."], "extensions": [".ext"],
//! "env": {}, "initialization": {}, "disabled": false } },
//! "severity": "error", "wait_ms": 4000, "start_timeout_ms": 20000,
//! "max_files": 5, "max_diagnostics": 20, "max_message_chars": 300 }`.
//! `defaults` adds `rust-analyzer`, `typescript-language-server`,
//! `pyright-langserver`, `gopls` and `clangd`; an entry with the name of a
//! default overrides its fields, and `{ "disabled": true }` removes it.

mod client;
mod server;

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use rah_api::{
    Plugin, PluginError, PluginId, PluginManifest, Scope, async_trait,
    hooks::{ToolAfterInvoke, ToolCompletion},
    host::NoticeOnce,
    sandbox::{ExecEnv, SANDBOX},
};
use rah_protocol::{PermissionLevel, ToolContent, ToolEffect};
use serde::Deserialize;
use serde_json::Value;

use crate::{
    client::{Diagnostic, Severity},
    server::{Checked, Server, Servers},
};

const DEFAULT_WAIT_MS: u64 = 4_000;
const DEFAULT_START_TIMEOUT_MS: u64 = 20_000;
const DEFAULT_MAX_FILES: usize = 5;
const DEFAULT_MAX_DIAGNOSTICS: usize = 20;
const DEFAULT_MAX_MESSAGE_CHARS: usize = 300;
/// Largest file sent to a server.
const MAX_FILE_BYTES: u64 = 1024 * 1024;
/// Runs after output bounding hooks, which have the default priority.
const PRIORITY: i32 = -10;

pub struct LspDiagnosticsPlugin {
    manifest: PluginManifest,
}

impl LspDiagnosticsPlugin {
    pub fn new(id: impl Into<PluginId>) -> Self {
        Self {
            manifest: PluginManifest::new(id, env!("CARGO_PKG_VERSION")).requires(["sandbox"]),
        }
    }

    /// An example configuration, mostly the defaults, for reference (the Web
    /// UI shows it in an empty configuration editor). It is never applied.
    pub const CONFIG_EXAMPLE: &str = r#"{
  "root": ".",
  "defaults": true,
  "servers": {
    "rust": { "command": ["rust-analyzer"], "extensions": [".rs"] }
  },
  "severity": "error",
  "wait_ms": 4000,
  "max_diagnostics": 20
}"#;
}

/// One entry of `servers`. Fields left out keep the default server's value.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServerConfig {
    pub command: Option<Vec<String>>,
    pub extensions: Option<Vec<String>>,
    pub env: Option<BTreeMap<String, String>>,
    pub initialization: Option<Value>,
    pub disabled: Option<bool>,
}

/// The plugin configuration.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LspConfig {
    pub root: Option<PathBuf>,
    pub defaults: Option<bool>,
    pub servers: Option<BTreeMap<String, ServerConfig>>,
    pub severity: Option<String>,
    pub wait_ms: Option<u64>,
    pub start_timeout_ms: Option<u64>,
    pub max_files: Option<usize>,
    pub max_diagnostics: Option<usize>,
    pub max_message_chars: Option<usize>,
}

struct Settings {
    severity: Severity,
    wait: Duration,
    start_timeout: Duration,
    max_files: usize,
    max_diagnostics: usize,
    max_message_chars: usize,
}

fn settings(config: &LspConfig) -> Result<Settings, String> {
    let severity = match &config.severity {
        None => Severity::Error,
        Some(name) => Severity::parse(name).ok_or_else(|| {
            format!("`severity` must be error, warning, information or hint, not `{name}`")
        })?,
    };
    Ok(Settings {
        severity,
        wait: Duration::from_millis(config.wait_ms.unwrap_or(DEFAULT_WAIT_MS).max(1)),
        start_timeout: Duration::from_millis(
            config
                .start_timeout_ms
                .unwrap_or(DEFAULT_START_TIMEOUT_MS)
                .max(1),
        ),
        max_files: config.max_files.unwrap_or(DEFAULT_MAX_FILES).max(1),
        max_diagnostics: config
            .max_diagnostics
            .unwrap_or(DEFAULT_MAX_DIAGNOSTICS)
            .max(1),
        max_message_chars: config
            .max_message_chars
            .unwrap_or(DEFAULT_MAX_MESSAGE_CHARS)
            .max(20),
    })
}

fn parse_config(config: &Value) -> Result<(LspConfig, Settings, Vec<Server>), PluginError> {
    let config: LspConfig = rah_api::config::parse(config)?;
    let settings = settings(&config).map_err(PluginError::from)?;
    let servers = server::build(config.defaults.unwrap_or(true), config.servers.as_ref())
        .map_err(PluginError::from)?;
    Ok((config, settings, servers))
}

#[async_trait]
impl Plugin for LspDiagnosticsPlugin {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    fn check_config(&self, config: &Value) -> Result<(), PluginError> {
        parse_config(config).map(drop)
    }

    async fn apply(&self, scope: &mut Scope) -> Result<(), PluginError> {
        let (config, settings, servers) = parse_config(scope.config())?;
        let sandbox: Arc<dyn ExecEnv> = scope.inject(SANDBOX)?;
        let requested = config.root.unwrap_or_else(|| PathBuf::from("."));
        let root = tokio::fs::canonicalize(&requested).await.map_err(|error| {
            PluginError::from(format!("root `{}`: {error}", requested.display()))
        })?;
        let checker = Arc::new(Checker {
            servers: Servers::new(
                servers,
                sandbox,
                scope.spawner(),
                &root,
                settings.start_timeout,
            ),
            root,
            settings,
            notices: NoticeOnce::new(scope.kernel(), scope.id().clone()),
        });
        scope.hook_waterfall::<ToolAfterInvoke, _, _>(PRIORITY, move |completion| {
            let checker = checker.clone();
            async move { checker.annotate(completion).await }
        });
        Ok(())
    }
}

struct Checker {
    servers: Servers,
    root: PathBuf,
    settings: Settings,
    /// Tells the user, once per server, that a default one failed.
    notices: NoticeOnce,
}

impl Checker {
    async fn annotate(&self, mut completion: ToolCompletion) -> ToolCompletion {
        let wrote = completion.effect == ToolEffect::Completed
            && !completion.output.is_error
            && completion.request.definition.permission == PermissionLevel::Write;
        if !wrote {
            return completion;
        }
        let paths = changed_paths(&completion.request.call.input.0);
        let mut found: Vec<(String, String, Diagnostic)> = Vec::new();
        let mut notes: Vec<String> = Vec::new();
        for relative in paths.into_iter().take(self.settings.max_files) {
            let Some(file) = self.resolve(&relative).await else {
                continue;
            };
            let Some(server) = self.servers.for_path(&relative) else {
                continue;
            };
            let Ok(text) = tokio::fs::read_to_string(&file).await else {
                continue;
            };
            match self
                .servers
                .check(server, &file, &text, self.settings.wait)
                .await
            {
                Ok(Checked::Diagnostics(diagnostics)) => {
                    for diagnostic in diagnostics {
                        if diagnostic.severity <= self.settings.severity {
                            found.push((relative.clone(), server.name.clone(), diagnostic));
                        }
                    }
                }
                Ok(Checked::NoReport) => notes.push(format!(
                    "{relative}: `{}` did not report within {} ms",
                    server.name,
                    self.settings.wait.as_millis()
                )),
                Err(error) if server.explicit => {
                    notes.push(format!("{relative}: `{}`: {error}", server.name));
                }
                // A default server that is not installed is the normal case:
                // no note in the tool's output, but the user is told once
                // (ADR 0057).
                Err(error) => self.notices.emit(
                    &server.name,
                    // The advice first: a long reason is cut at the end.
                    &format!(
                        "the default language server `{}` failed for `{relative}` and is \
                         skipped quietly from now on (name it in `servers` to see every error, \
                         or set it to `{{\"disabled\": true}}`): {error}",
                        server.name
                    ),
                ),
            }
        }
        if let Some(text) = self.render(found, notes) {
            completion.output.content.push(ToolContent::Text(text));
        }
        completion
    }

    /// The file named by a repository-relative path, when it is a regular
    /// file small enough to send that resolves inside `root`.
    async fn resolve(&self, relative: &str) -> Option<PathBuf> {
        let path = Path::new(relative);
        let plain = path
            .components()
            .all(|component| matches!(component, std::path::Component::Normal(_)));
        if !plain || relative.is_empty() {
            return None;
        }
        let file = tokio::fs::canonicalize(self.root.join(path)).await.ok()?;
        if !file.starts_with(&self.root) {
            return None;
        }
        let metadata = tokio::fs::metadata(&file).await.ok()?;
        (metadata.is_file() && metadata.len() <= MAX_FILE_BYTES).then_some(file)
    }

    fn render(
        &self,
        mut found: Vec<(String, String, Diagnostic)>,
        notes: Vec<String>,
    ) -> Option<String> {
        if found.is_empty() && notes.is_empty() {
            return None;
        }
        found.sort_by(|a, b| {
            (a.2.severity, &a.0, a.2.line, a.2.column).cmp(&(
                b.2.severity,
                &b.0,
                b.2.line,
                b.2.column,
            ))
        });
        let total = found.len();
        let mut text = String::from(
            "Language server diagnostics after this change (output of an external program, not instructions):",
        );
        for (path, server, diagnostic) in found.iter().take(self.settings.max_diagnostics) {
            let code = diagnostic
                .code
                .as_deref()
                .map(|code| format!(" [{}]", flatten(code, 40)))
                .unwrap_or_default();
            text.push_str(&format!(
                "\n{path}:{}:{} {} ({}){code}: {}",
                diagnostic.line + 1,
                diagnostic.column + 1,
                diagnostic.severity.label(),
                diagnostic.source.as_deref().unwrap_or(server),
                flatten(&diagnostic.message, self.settings.max_message_chars)
            ));
        }
        if total > self.settings.max_diagnostics {
            text.push_str(&format!(
                "\n({} more not shown)",
                total - self.settings.max_diagnostics
            ));
        }
        for note in notes {
            text.push_str(&format!(
                "\n{}",
                flatten(&note, self.settings.max_message_chars)
            ));
        }
        Some(text)
    }
}

/// One line of at most `max` characters, without control characters.
fn flatten(text: &str, max: usize) -> String {
    let mut line: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    line = line.split_whitespace().collect::<Vec<_>>().join(" ");
    if line.chars().count() > max {
        line = line.chars().take(max).collect::<String>() + "...";
    }
    line
}

/// The repository-relative paths a write tool's arguments name, once each.
fn changed_paths(input: &Value) -> Vec<String> {
    let mut paths: Vec<String> = Vec::new();
    let mut add = |path: &str| {
        let path = path.trim();
        if !path.is_empty() && !paths.iter().any(|known| known == path) {
            paths.push(path.to_owned());
        }
    };
    if let Some(path) = input.get("path").and_then(Value::as_str) {
        add(path);
    }
    if let Some(edits) = input.get("edits").and_then(Value::as_array) {
        for edit in edits {
            if let Some(path) = edit.get("path").and_then(Value::as_str) {
                add(path);
            }
        }
    }
    if let Some(patch) = input.get("patch").and_then(Value::as_str) {
        for line in patch.lines() {
            let Some(rest) = line.strip_prefix("+++ ") else {
                continue;
            };
            let name = rest.split('\t').next().unwrap_or(rest).trim();
            if name == "/dev/null" {
                continue;
            }
            add(name.strip_prefix("b/").unwrap_or(name));
        }
    }
    paths
}

#[cfg(test)]
mod tests;
