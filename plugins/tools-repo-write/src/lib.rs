//! `tools-repo-write`: repository write tools, all declared `write` (Ask in
//! the presets' policy).
//!
//! - `repo.create-file {path, content}`: creates a new file (and missing
//!   parent directories); fails if the path exists.
//! - `repo.edit-files {edits: [{path, old, new}]}`: replaces one exact
//!   occurrence of `old` in each file. Every edit is checked before any file
//!   is written; files are replaced through a temporary file and a rename.
//! - `repo.patch {patch}`: applies a unified diff with `git apply` through the
//!   `sandbox` service, after `git apply --check` succeeds.
//!
//! Paths are repository-relative (`a/b`, no `.`, `..`, `.git`, backslashes,
//! colons or a leading `-`) and must resolve inside `root` without following
//! a symbolic link out of it. Validation failures record no effect; a failure
//! after the first write is reported as an execution error, which the
//! `ToolInvoker` records as `uncertain`.
//!
//! Writes of one plugin instance are serialized: each file has a lock (taken in
//! sorted order for a multi-file edit, so two edits cannot wait for each
//! other), and `repo.patch` waits for all of them, so edits that run at once
//! (several sessions, background jobs, subagents) do not overwrite each
//! other. The locks do not stop an editor, a build or another process from
//! writing the same file. A UTF-8 byte order mark at the start of a file is
//! kept as it is when the file is edited.
//!
//! Configuration: `{ "root": ".", "cwd": "<sandbox-relative repository
//! directory>", "git": "git", "max_file_bytes": 1048576, "timeout_ms": 30000 }`.
//! `repo.patch` refuses to run unless Git's top level for `cwd` is `root`.

mod fsops;
mod locks;

use std::{collections::BTreeMap, path::PathBuf, sync::Arc, time::Duration};

use rah_api::{
    Plugin, PluginError, PluginId, PluginManifest, Scope, async_trait,
    sandbox::{ExecEnv, ExecError, OutputLimits, ProgramSpec, SANDBOX},
    tools::{TOOLS_SERVICE, Tool, ToolContext, ToolError, provide_tools},
};
use rah_protocol::{PermissionLevel, ToolContent, ToolDefinition, ToolName, ToolOutput};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::fsops::Root;

const MAX_EDITS: usize = 32;
const MAX_PATCH_BYTES: usize = 1024 * 1024;

pub struct RepoWriteToolsPlugin {
    manifest: PluginManifest,
}

impl RepoWriteToolsPlugin {
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
pub struct RepoWriteConfig {
    pub root: Option<PathBuf>,
    pub cwd: Option<PathBuf>,
    pub git: Option<String>,
    pub max_file_bytes: Option<usize>,
    pub timeout_ms: Option<u64>,
}

#[async_trait]
impl Plugin for RepoWriteToolsPlugin {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    fn check_config(&self, config: &serde_json::Value) -> Result<(), PluginError> {
        rah_api::config::parse::<RepoWriteConfig>(config)?;
        Ok(())
    }

    async fn apply(&self, scope: &mut Scope) -> Result<(), PluginError> {
        let config: RepoWriteConfig = rah_api::config::parse(scope.config())?;
        let sandbox = scope.inject(SANDBOX)?;
        provide_tools(scope, write_tools(sandbox, config).await?)?;
        Ok(())
    }
}

/// Builds the three tools.
pub async fn write_tools(
    sandbox: Arc<dyn ExecEnv>,
    config: RepoWriteConfig,
) -> Result<Vec<Arc<dyn Tool>>, String> {
    let requested = config.root.unwrap_or_else(|| PathBuf::from("."));
    let root = tokio::fs::canonicalize(&requested)
        .await
        .map_err(|error| format!("repository root `{}`: {error}", requested.display()))?;
    let root = Arc::new(Root {
        path: root,
        max_file_bytes: config.max_file_bytes.unwrap_or(1024 * 1024).max(1),
        locks: Default::default(),
    });
    let git = Arc::new(Git {
        sandbox,
        program: config.git.unwrap_or_else(|| "git".into()),
        cwd: config.cwd,
        timeout: Duration::from_millis(config.timeout_ms.unwrap_or(30_000).max(1)),
        root: root.clone(),
    });
    Ok(vec![
        Arc::new(CreateFile(root.clone())),
        Arc::new(EditFiles(root)),
        Arc::new(Patch(git)),
    ])
}

fn definition(name: &str, description: &str, input_schema: Value) -> ToolDefinition {
    ToolDefinition {
        name: ToolName::new(name),
        description: description.into(),
        input_schema,
        permission: PermissionLevel::Write,
    }
}

fn parse<T: serde::de::DeserializeOwned>(input: Value) -> Result<T, ToolError> {
    serde_json::from_value(input).map_err(|error| ToolError::InvalidInput(error.to_string()))
}

fn done(text: String) -> ToolOutput {
    ToolOutput {
        content: vec![ToolContent::Text(text)],
        is_error: false,
    }
}

pub struct CreateFile(Arc<Root>);

#[async_trait]
impl Tool for CreateFile {
    fn definition(&self) -> ToolDefinition {
        definition(
            "repo.create-file",
            "Creates a new UTF-8 file at a repository-relative path, with missing parent \
             directories. Fails if the path already exists.",
            json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "minLength": 1, "maxLength": 1024 },
                    "content": { "type": "string" }
                },
                "required": ["path", "content"],
                "additionalProperties": false
            }),
        )
    }

    async fn call(&self, input: Value, _cx: ToolContext) -> Result<ToolOutput, ToolError> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Input {
            path: String,
            content: String,
        }
        let input: Input = parse(input)?;
        self.0.create(&input.path, &input.content).await?;
        Ok(done(format!(
            "created {} ({} bytes)",
            input.path,
            input.content.len()
        )))
    }
}

pub struct EditFiles(Arc<Root>);

#[async_trait]
impl Tool for EditFiles {
    fn definition(&self) -> ToolDefinition {
        definition(
            "repo.edit-files",
            "Edits existing files: each edit replaces exactly one occurrence of `old` with `new`. \
             All edits are checked first; if any fails, no file is changed. Include enough \
             surrounding text in `old` to make it unique.",
            json!({
                "type": "object",
                "properties": {
                    "edits": {
                        "type": "array",
                        "minItems": 1,
                        "maxItems": MAX_EDITS,
                        "items": {
                            "type": "object",
                            "properties": {
                                "path": { "type": "string", "minLength": 1, "maxLength": 1024 },
                                "old": { "type": "string", "minLength": 1 },
                                "new": { "type": "string" }
                            },
                            "required": ["path", "old", "new"],
                            "additionalProperties": false
                        }
                    }
                },
                "required": ["edits"],
                "additionalProperties": false
            }),
        )
    }

    async fn call(&self, input: Value, _cx: ToolContext) -> Result<ToolOutput, ToolError> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Input {
            edits: Vec<fsops::Edit>,
        }
        let input: Input = parse(input)?;
        if input.edits.is_empty() || input.edits.len() > MAX_EDITS {
            return Err(ToolError::InvalidInput(format!(
                "between 1 and {MAX_EDITS} edits are allowed"
            )));
        }
        let changed = self.0.edit(input.edits).await?;
        Ok(done(format!("edited {}", changed.join(", "))))
    }
}

struct Git {
    sandbox: Arc<dyn ExecEnv>,
    program: String,
    cwd: Option<PathBuf>,
    timeout: Duration,
    root: Arc<Root>,
}

impl Git {
    async fn run(
        &self,
        args: Vec<String>,
        cx: &ToolContext,
    ) -> Result<(i32, String, String), ToolError> {
        let null_device = if cfg!(windows) { "NUL" } else { "/dev/null" };
        let mut spec = ProgramSpec::new(self.program.clone(), args, self.timeout);
        spec.cwd = self.cwd.clone();
        spec.env = BTreeMap::from([
            ("GIT_CONFIG_NOSYSTEM".to_owned(), "1".to_owned()),
            ("GIT_CONFIG_GLOBAL".to_owned(), null_device.to_owned()),
            ("GIT_TERMINAL_PROMPT".to_owned(), "0".to_owned()),
        ]);
        spec.limits = OutputLimits {
            stdout_bytes: 64 * 1024,
            stderr_bytes: 64 * 1024,
        };
        let output =
            self.sandbox
                .run(spec, cx.cancel.clone())
                .await
                .map_err(|error| match error {
                    ExecError::InvalidSpec(_) | ExecError::Spawn { .. } => {
                        ToolError::NotStarted(error.to_string())
                    }
                    ExecError::Supervision(_) => ToolError::Execution(error.to_string()),
                })?;
        if let Some(termination) = output.termination {
            return Err(ToolError::Execution(format!(
                "git was terminated: {termination:?}"
            )));
        }
        Ok((
            output.exit_code.unwrap_or(-1),
            String::from_utf8_lossy(&output.stdout).trim().to_owned(),
            String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        ))
    }
}

pub struct Patch(Arc<Git>);

#[async_trait]
impl Tool for Patch {
    fn definition(&self) -> ToolDefinition {
        definition(
            "repo.patch",
            "Applies a unified diff (as produced by `git diff`) to the worktree with `git apply`. \
             The patch is checked first; if it does not apply cleanly, nothing is changed.",
            json!({
                "type": "object",
                "properties": { "patch": { "type": "string", "minLength": 1, "maxLength": MAX_PATCH_BYTES } },
                "required": ["patch"],
                "additionalProperties": false
            }),
        )
    }

    async fn call(&self, input: Value, cx: ToolContext) -> Result<ToolOutput, ToolError> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Input {
            patch: String,
        }
        let input: Input = parse(input)?;
        if input.patch.len() > MAX_PATCH_BYTES {
            return Err(ToolError::InvalidInput(format!(
                "the patch exceeds {MAX_PATCH_BYTES} bytes"
            )));
        }
        let git = &self.0;
        // `git apply` may touch any file, so no other write runs meanwhile.
        let _exclusive = git.root.locks.all().await;
        let (code, top, stderr) = git
            .run(vec!["rev-parse".into(), "--show-toplevel".into()], &cx)
            .await?;
        let top = tokio::fs::canonicalize(&top).await.ok();
        if code != 0 || top.as_deref() != Some(git.root.path.as_path()) {
            return Err(ToolError::NotStarted(format!(
                "the sandbox's repository is not the configured root ({})",
                if stderr.is_empty() {
                    "different top level"
                } else {
                    stderr.as_str()
                }
            )));
        }
        let file = git.root.scratch_file(&input.patch).await?;
        let path = file.path().display().to_string();
        // Every path the patch names, old and new names of renames included.
        let (code, listing, stderr) = git
            .run(
                vec![
                    "apply".into(),
                    "--numstat".into(),
                    "-z".into(),
                    "--".into(),
                    path.clone(),
                ],
                &cx,
            )
            .await?;
        if code != 0 {
            return Err(ToolError::InvalidInput(format!(
                "the patch does not apply: {stderr}"
            )));
        }
        if let Some(name) = listing
            .split(['\0', '\t'])
            .find(|name| fsops::is_harness_state(name))
        {
            return Err(fsops::harness_state(name));
        }
        let (code, _, stderr) = git
            .run(
                vec!["apply".into(), "--check".into(), "--".into(), path.clone()],
                &cx,
            )
            .await?;
        if code != 0 {
            return Err(ToolError::InvalidInput(format!(
                "the patch does not apply: {stderr}"
            )));
        }
        let (code, _, stderr) = git
            .run(vec!["apply".into(), "--".into(), path], &cx)
            .await?;
        if code != 0 {
            return Err(ToolError::Execution(format!(
                "git apply failed after a clean check: {stderr}"
            )));
        }
        Ok(done("patch applied".into()))
    }
}

impl RepoWriteToolsPlugin {
    /// An example configuration, mostly the defaults, for reference (the Web
    /// UI shows it in an empty configuration editor). It is never applied.
    pub const CONFIG_EXAMPLE: &str = r#"{
  "root": ".",
  "git": "git",
  "max_file_bytes": 1048576,
  "timeout_ms": 30000
}"#;
}

#[cfg(test)]
mod tests;

#[cfg(test)]
#[test]
fn the_config_example_is_a_valid_configuration() {
    let parsed: Result<RepoWriteConfig, _> =
        serde_json::from_str(RepoWriteToolsPlugin::CONFIG_EXAMPLE);
    assert!(parsed.is_ok(), "{:?}", parsed.err());
}
