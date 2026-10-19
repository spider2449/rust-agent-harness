//! `format-on-write`: runs the project's code formatter on files that write
//! tools changed (ADR 0053).
//!
//! The plugin attaches to `tool.after_invoke`, like `lsp-diagnostics`. When a
//! tool declared `write` completed without an error, it takes the files the
//! call named from its arguments (`path`, `edits[].path`, or the `+++` lines
//! of a `patch`), runs the formatter for each file's extension through the
//! `sandbox` service, and tells the model which files the formatter changed:
//! the file on disk then differs from what the model wrote, so it must read
//! it again before editing it. Nothing is appended when the files were
//! already formatted. The tool's result, effect and policy decision are not
//! changed.
//!
//! A formatter runs only when it applies to the project: `rustfmt` and
//! `gofmt` always, `ruff` and `black` when `pyproject.toml` configures them
//! (or `ruff.toml`), and `clang-format` when a `.clang-format` file is found
//! up the tree. A formatter that is not installed is skipped for five
//! minutes. A default formatter that fails leaves no note in the tool's
//! output; the user gets one [`HostNotice`](rah_api::host::HostNotice) the first
//! time, and a formatter named in `formatters` reports in the output (ADR
//! 0057). The defaults do not include formatters that run code from the
//! project's configuration, such as `prettier` with a `.js` configuration or
//! plugins; name such a formatter in `formatters` to opt in.
//!
//! Configuration (all optional): `{ "root": ".", "defaults": true,
//! "formatters": { "<name>": { "command": ["prettier", "--write", "$FILE"],
//! "extensions": [".ts"], "requires": ["package.json"], "env": {},
//! "disabled": false } }, "timeout_ms": 10000, "max_files": 5 }`.
//! `$FILE` in `command` is the file's path relative to `root`. An entry with
//! the name of a default overrides its fields, and `{ "disabled": true }`
//! removes it.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, PoisonError},
    time::{Duration, Instant},
};

use rah_api::{
    CancellationToken, Plugin, PluginError, PluginId, PluginManifest, Scope, async_trait,
    hooks::{ToolAfterInvoke, ToolCompletion},
    host::NoticeOnce,
    sandbox::{ExecEnv, ExecError, ProgramSpec, SANDBOX},
};
use rah_protocol::{PermissionLevel, ToolContent, ToolEffect};
use serde::Deserialize;
use serde_json::Value;

const DEFAULT_TIMEOUT_MS: u64 = 10_000;
const DEFAULT_MAX_FILES: usize = 5;
/// Largest file handed to a formatter.
const MAX_FILE_BYTES: u64 = 1024 * 1024;
/// How long a formatter that is not installed is left alone.
const RETRY_AFTER: Duration = Duration::from_secs(300);
/// Runs before `lsp-diagnostics` (-10), so diagnostics describe the
/// formatted file, and after output bounding hooks (0).
const PRIORITY: i32 = -5;
/// How much of a failing formatter's error output is shown.
const ERROR_CHARS: usize = 200;

pub struct FormatOnWritePlugin {
    manifest: PluginManifest,
}

impl FormatOnWritePlugin {
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
  "formatters": {
    "zig": { "command": ["zig", "fmt", "$FILE"], "extensions": [".zig"] }
  },
  "timeout_ms": 10000,
  "max_files": 5
}"#;
}

/// One entry of `formatters`. Fields left out keep the default's value.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FormatterConfig {
    pub command: Option<Vec<String>>,
    pub extensions: Option<Vec<String>>,
    /// Files, one of which must exist in the file's directory or above it
    /// (up to `root`) for the formatter to apply.
    pub requires: Option<Vec<String>>,
    pub env: Option<BTreeMap<String, String>>,
    pub disabled: Option<bool>,
}

/// The plugin configuration.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FormatConfig {
    pub root: Option<PathBuf>,
    pub defaults: Option<bool>,
    pub formatters: Option<BTreeMap<String, FormatterConfig>>,
    pub timeout_ms: Option<u64>,
    pub max_files: Option<usize>,
}

/// How a formatter decides that it applies to a project.
#[derive(Clone, Debug)]
enum Applies {
    Always,
    /// One of these files exists in the file's directory or above.
    File(Vec<String>),
    /// One of these files exists above, or `pyproject.toml` holds the text.
    FileOrToml(Vec<&'static str>, &'static str),
}

struct Formatter {
    name: String,
    explicit: bool,
    command: Vec<String>,
    extensions: Vec<String>,
    applies: Applies,
    env: BTreeMap<String, String>,
    /// When a missing program was seen.
    missing: Mutex<Option<Instant>>,
}

/// Formatters tried without configuration: they do not run code from the
/// project.
fn defaults() -> Vec<(&'static str, Vec<&'static str>, Vec<&'static str>, Applies)> {
    vec![
        (
            "rustfmt",
            vec!["rustfmt", "--edition", "2024", "$FILE"],
            vec![".rs"],
            Applies::Always,
        ),
        (
            "gofmt",
            vec!["gofmt", "-w", "$FILE"],
            vec![".go"],
            Applies::Always,
        ),
        (
            "ruff",
            vec!["ruff", "format", "$FILE"],
            vec![".py", ".pyi"],
            Applies::FileOrToml(vec!["ruff.toml", ".ruff.toml"], "[tool.ruff"),
        ),
        (
            "black",
            vec!["black", "--quiet", "$FILE"],
            vec![".py", ".pyi"],
            Applies::FileOrToml(vec![], "[tool.black"),
        ),
        (
            "clang-format",
            vec!["clang-format", "-i", "$FILE"],
            vec![".c", ".h", ".cc", ".cpp", ".cxx", ".hpp"],
            Applies::File(vec![".clang-format".into(), "_clang-format".into()]),
        ),
    ]
}

fn build(
    with_defaults: bool,
    configured: Option<&BTreeMap<String, FormatterConfig>>,
) -> Result<Vec<Formatter>, String> {
    let mut table: BTreeMap<String, (FormatterConfig, Option<Applies>, bool)> = BTreeMap::new();
    if with_defaults {
        for (name, command, extensions, applies) in defaults() {
            table.insert(
                name.to_owned(),
                (
                    FormatterConfig {
                        command: Some(command.into_iter().map(str::to_owned).collect()),
                        extensions: Some(extensions.into_iter().map(str::to_owned).collect()),
                        ..FormatterConfig::default()
                    },
                    Some(applies),
                    false,
                ),
            );
        }
    }
    for (name, entry) in configured.into_iter().flatten() {
        if name.trim().is_empty() {
            return Err("a formatter needs a name".into());
        }
        if entry.disabled == Some(true) {
            table.remove(name);
            continue;
        }
        let slot = table.entry(name.clone()).or_default();
        if entry.command.is_some() {
            slot.0.command = entry.command.clone();
        }
        if entry.extensions.is_some() {
            slot.0.extensions = entry.extensions.clone();
        }
        if entry.env.is_some() {
            slot.0.env = entry.env.clone();
        }
        if let Some(files) = &entry.requires {
            slot.1 = Some(Applies::File(files.clone()));
        }
        slot.2 = true;
    }
    table
        .into_iter()
        .map(|(name, (config, applies, explicit))| {
            let command = config
                .command
                .filter(|command| {
                    command
                        .first()
                        .is_some_and(|program| !program.trim().is_empty())
                })
                .ok_or_else(|| format!("formatter `{name}` needs a `command`"))?;
            if !command.iter().any(|word| word.contains("$FILE")) {
                return Err(format!(
                    "formatter `{name}`: `command` must contain `$FILE`, the file to format"
                ));
            }
            let extensions: Vec<String> = config
                .extensions
                .filter(|extensions| !extensions.is_empty())
                .ok_or_else(|| format!("formatter `{name}` needs `extensions`"))?
                .into_iter()
                .map(|extension| extension.to_ascii_lowercase())
                .collect();
            if let Some(bad) = extensions
                .iter()
                .find(|extension| extension.len() < 2 || !extension.starts_with('.'))
            {
                return Err(format!(
                    "formatter `{name}`: extension `{bad}` must start with a dot"
                ));
            }
            if let Some(Applies::File(files)) = &applies
                && files
                    .iter()
                    .any(|file| file.trim().is_empty() || file.contains(['/', '\\']))
            {
                return Err(format!(
                    "formatter `{name}`: `requires` lists file names, without a path"
                ));
            }
            Ok(Formatter {
                name,
                explicit,
                command,
                extensions,
                applies: applies.unwrap_or(Applies::Always),
                env: config.env.unwrap_or_default(),
                missing: Mutex::new(None),
            })
        })
        .collect()
}

struct Settings {
    timeout: Duration,
    max_files: usize,
}

fn parse_config(config: &Value) -> Result<(FormatConfig, Settings, Vec<Formatter>), PluginError> {
    let config: FormatConfig = rah_api::config::parse(config)?;
    let formatters = build(config.defaults.unwrap_or(true), config.formatters.as_ref())
        .map_err(PluginError::from)?;
    let settings = Settings {
        timeout: Duration::from_millis(config.timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS).max(1)),
        max_files: config.max_files.unwrap_or(DEFAULT_MAX_FILES).max(1),
    };
    Ok((config, settings, formatters))
}

#[async_trait]
impl Plugin for FormatOnWritePlugin {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    fn check_config(&self, config: &Value) -> Result<(), PluginError> {
        parse_config(config).map(drop)
    }

    async fn apply(&self, scope: &mut Scope) -> Result<(), PluginError> {
        let (config, settings, formatters) = parse_config(scope.config())?;
        let sandbox: Arc<dyn ExecEnv> = scope.inject(SANDBOX)?;
        let requested = config.root.unwrap_or_else(|| PathBuf::from("."));
        let root = tokio::fs::canonicalize(&requested).await.map_err(|error| {
            PluginError::from(format!("root `{}`: {error}", requested.display()))
        })?;
        let runner = Arc::new(Runner {
            formatters,
            sandbox,
            root,
            settings,
            stop: scope.spawner().cancellation(),
            notices: NoticeOnce::new(scope.kernel(), scope.id().clone()),
        });
        scope.hook_waterfall::<ToolAfterInvoke, _, _>(PRIORITY, move |completion| {
            let runner = runner.clone();
            async move { runner.annotate(completion).await }
        });
        Ok(())
    }
}

struct Runner {
    formatters: Vec<Formatter>,
    sandbox: Arc<dyn ExecEnv>,
    root: PathBuf,
    settings: Settings,
    /// Cancelled when the plugin stops.
    stop: CancellationToken,
    /// Tells the user, once per formatter, that a default one failed.
    notices: NoticeOnce,
}

/// What happened to one file.
enum Outcome {
    Unchanged,
    Changed,
    Failed(String),
}

impl Runner {
    async fn annotate(&self, mut completion: ToolCompletion) -> ToolCompletion {
        let wrote = completion.effect == ToolEffect::Completed
            && !completion.output.is_error
            && completion.request.definition.permission == PermissionLevel::Write;
        if !wrote {
            return completion;
        }
        let mut changed: Vec<String> = Vec::new();
        let mut notes: Vec<String> = Vec::new();
        for relative in changed_paths(&completion.request.call.input.0)
            .into_iter()
            .take(self.settings.max_files)
        {
            let Some(file) = self.resolve(&relative).await else {
                continue;
            };
            let Some(formatter) = self.formatter_for(&relative, &file).await else {
                continue;
            };
            match self.run(formatter, &relative, &file).await {
                Outcome::Unchanged => {}
                Outcome::Changed => changed.push(format!("{relative} ({})", formatter.name)),
                Outcome::Failed(reason) if formatter.explicit => {
                    notes.push(format!("{relative}: `{}` failed: {reason}", formatter.name));
                }
                // A default formatter that fails is skipped without a note in
                // the tool's output (the model has no use for it), but the
                // user is told once (ADR 0057).
                Outcome::Failed(reason) => self.notices.emit(
                    &formatter.name,
                    // The advice first: a long reason is cut at the end.
                    &format!(
                        "the default formatter `{}` failed on `{relative}` and is skipped \
                         quietly from now on (name it in `formatters` to see every error, or \
                         set it to `{{\"disabled\": true}}`): {reason}",
                        formatter.name
                    ),
                ),
            }
        }
        if changed.is_empty() && notes.is_empty() {
            return completion;
        }
        let mut text = String::new();
        if !changed.is_empty() {
            text.push_str(&format!(
                "Formatted by the project's formatter: {}. The file on disk now differs from \
                 what you wrote; read it again before you edit it.",
                changed.join(", ")
            ));
        }
        for note in notes {
            if !text.is_empty() {
                text.push('\n');
            }
            text.push_str(&flatten(&note, 300));
        }
        completion.output.content.push(ToolContent::Text(text));
        completion
    }

    /// The file named by a repository-relative path, when it is a regular
    /// file small enough to format that resolves inside `root`.
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

    /// The first formatter for the file's extension that applies to its
    /// project and was not found missing lately.
    async fn formatter_for(&self, relative: &str, file: &Path) -> Option<&Formatter> {
        let extension = Path::new(relative)
            .extension()?
            .to_string_lossy()
            .to_ascii_lowercase();
        for formatter in &self.formatters {
            if !formatter
                .extensions
                .iter()
                .any(|known| known[1..] == extension)
            {
                continue;
            }
            let skipped = formatter
                .missing
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .is_some_and(|at| at.elapsed() < RETRY_AFTER);
            if !skipped && self.applies(&formatter.applies, file).await {
                return Some(formatter);
            }
        }
        None
    }

    async fn applies(&self, applies: &Applies, file: &Path) -> bool {
        match applies {
            Applies::Always => true,
            Applies::File(names) => {
                self.find_up(file, |dir| names.iter().any(|n| dir.join(n).is_file()))
                    .await
            }
            Applies::FileOrToml(names, section) => {
                let section = *section;
                self.find_up(file, |dir| {
                    names.iter().any(|n| dir.join(n).is_file())
                        || std::fs::read_to_string(dir.join("pyproject.toml"))
                            .is_ok_and(|text| text.contains(section))
                })
                .await
            }
        }
    }

    /// Whether `found` holds for the file's directory or one above it, up to
    /// `root`.
    async fn find_up(&self, file: &Path, found: impl Fn(&Path) -> bool + Send) -> bool {
        let mut dir = file.parent();
        while let Some(current) = dir {
            if !current.starts_with(&self.root) {
                return false;
            }
            if found(current) {
                return true;
            }
            dir = current.parent();
        }
        false
    }

    async fn run(&self, formatter: &Formatter, relative: &str, file: &Path) -> Outcome {
        let Ok(before) = tokio::fs::read(file).await else {
            return Outcome::Unchanged;
        };
        let mut words = formatter
            .command
            .iter()
            .map(|word| word.replace("$FILE", relative));
        let Some(program) = words.next() else {
            return Outcome::Unchanged;
        };
        let mut spec = ProgramSpec::new(program, words.collect(), self.settings.timeout);
        spec.env = formatter.env.clone();
        let output = match self.sandbox.run(spec, self.stop.child_token()).await {
            Ok(output) => output,
            Err(ExecError::Spawn { .. } | ExecError::InvalidSpec(_)) => {
                *formatter
                    .missing
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner) = Some(Instant::now());
                return Outcome::Failed("the program could not be started".into());
            }
            Err(error) => return Outcome::Failed(error.to_string()),
        };
        if output.termination.is_some() {
            return Outcome::Failed("it did not finish in time".into());
        }
        if output.exit_code != Some(0) {
            let message = String::from_utf8_lossy(&output.stderr);
            return Outcome::Failed(format!(
                "exit code {}: {}",
                output
                    .exit_code
                    .map_or("unknown".to_owned(), |code| code.to_string()),
                flatten(message.trim(), ERROR_CHARS)
            ));
        }
        match tokio::fs::read(file).await {
            Ok(after) if after != before => Outcome::Changed,
            _ => Outcome::Unchanged,
        }
    }
}

/// One line of at most `max` characters, without control characters.
fn flatten(text: &str, max: usize) -> String {
    let line = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if line.chars().count() > max {
        line.chars().take(max).collect::<String>() + "..."
    } else {
        line
    }
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
