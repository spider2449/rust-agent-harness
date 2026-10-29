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
//! - `repo.delete-file {path}`: deletes one regular file (ADR 0055).
//! - `repo.move-file {from, to}`: moves or renames one regular file to a path
//!   that does not exist (ADR 0055). Neither tool acts on directories or
//!   symbolic links.
//! - `repo.commit {message, paths}`, declared `execute` and provided only when
//!   `commit` (`{ "name": "...", "email": "..." }`) is configured (ADR 0055):
//!   stages the named files and commits only them, as that identity. Git's own
//!   configuration files are not read. It runs the repository's hooks, which
//!   is why it asks at the `execute` level.
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
//!
//! The plugin also provides `file-undo` (ADR 0060): it keeps, in memory, the
//! content of each file before and after every completed write call of a
//! session (not `repo.commit`), and the user can undo the newest change
//! through a frontend while every file is still what the change left. A
//! `tool.after_invoke` handler that runs after the others reads the files
//! again, so a formatter's rewrite is part of the change. A file larger than
//! `max_file_bytes` or that is not a regular file leaves its call without an
//! undo. The history is forgotten when the plugin stops or is reconfigured.

mod fsops;
mod locks;
mod undo;

use std::{collections::BTreeMap, path::PathBuf, sync::Arc, time::Duration};

use rah_api::{
    Plugin, PluginError, PluginId, PluginManifest, Scope, async_trait,
    hooks::ToolAfterInvoke,
    sandbox::{ExecEnv, ExecError, OutputLimits, ProgramSpec, SANDBOX},
    tools::{TOOLS_SERVICE, Tool, ToolContext, ToolError, provide_tools},
    undo::{FILE_UNDO, FILE_UNDO_SERVICE},
};
use rah_protocol::{
    PermissionLevel, ToolContent, ToolDefinition, ToolEffect, ToolName, ToolOutput,
};
use serde::Deserialize;
use serde_json::{Value, json};

pub use crate::undo::RepoUndo;
use crate::{
    fsops::Root,
    undo::{Call, FileState, read_state},
};

/// Runs after every other `tool.after_invoke` handler (`format-on-write` is
/// -5, `lsp-diagnostics` -10), so the undo history sees the final files.
const REFRESH_PRIORITY: i32 = -100;

const MAX_EDITS: usize = 32;
const MAX_PATCH_BYTES: usize = 1024 * 1024;
const MAX_COMMIT_PATHS: usize = 32;
const MAX_MESSAGE_BYTES: usize = 4096;
const MAX_COMMIT_OUTPUT_CHARS: usize = 2000;

pub struct RepoWriteToolsPlugin {
    manifest: PluginManifest,
}

impl RepoWriteToolsPlugin {
    pub fn new(id: impl Into<PluginId>) -> Self {
        Self {
            manifest: PluginManifest::new(id, env!("CARGO_PKG_VERSION"))
                .provides([TOOLS_SERVICE, FILE_UNDO_SERVICE])
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
    /// Who `repo.commit` commits as. Without it the tool is not provided.
    pub commit: Option<CommitAuthor>,
}

/// The identity of the commits `repo.commit` makes. It comes from the host's
/// configuration; Git's own configuration files are not read.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommitAuthor {
    pub name: String,
    pub email: String,
}

impl CommitAuthor {
    fn check(&self) -> Result<(), String> {
        for (what, value) in [("name", &self.name), ("email", &self.email)] {
            if value.trim().is_empty() || value.contains(['\n', '\r', '\0', '<', '>']) {
                return Err(format!(
                    "`commit.{what}` must not be empty or contain `<`, `>` or line breaks"
                ));
            }
        }
        Ok(())
    }
}

#[async_trait]
impl Plugin for RepoWriteToolsPlugin {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    fn check_config(&self, config: &serde_json::Value) -> Result<(), PluginError> {
        let config = rah_api::config::parse::<RepoWriteConfig>(config)?;
        if let Some(author) = &config.commit {
            author.check()?;
        }
        Ok(())
    }

    async fn apply(&self, scope: &mut Scope) -> Result<(), PluginError> {
        let config: RepoWriteConfig = rah_api::config::parse(scope.config())?;
        let sandbox = scope.inject(SANDBOX)?;
        let (tools, undo) = build(sandbox, config).await?;
        provide_tools(scope, tools)?;
        scope.provide(FILE_UNDO, undo.clone())?;
        scope.hook_waterfall::<ToolAfterInvoke, _, _>(REFRESH_PRIORITY, move |completion| {
            let undo = undo.clone();
            async move {
                if completion.effect == ToolEffect::Completed {
                    undo.refresh(&completion.request.call.id).await;
                }
                completion
            }
        });
        Ok(())
    }
}

/// The call a tool's write belongs to, for the undo history.
fn call<'a>(cx: &'a ToolContext, tool: &'static str) -> Call<'a> {
    Call {
        session: &cx.session,
        id: &cx.call,
        tool,
    }
}

/// Builds the tools: the file tools and `repo.patch`, and `repo.commit` when
/// `commit` is configured.
pub async fn write_tools(
    sandbox: Arc<dyn ExecEnv>,
    config: RepoWriteConfig,
) -> Result<Vec<Arc<dyn Tool>>, String> {
    Ok(build(sandbox, config).await?.0)
}

/// Builds the tools and the `file-undo` service that undoes their changes.
pub async fn build(
    sandbox: Arc<dyn ExecEnv>,
    config: RepoWriteConfig,
) -> Result<(Vec<Arc<dyn Tool>>, Arc<RepoUndo>), String> {
    if let Some(author) = &config.commit {
        author.check()?;
    }
    let requested = config.root.unwrap_or_else(|| PathBuf::from("."));
    let root = tokio::fs::canonicalize(&requested)
        .await
        .map_err(|error| format!("repository root `{}`: {error}", requested.display()))?;
    let root = Arc::new(Root {
        path: root,
        max_file_bytes: config.max_file_bytes.unwrap_or(1024 * 1024).max(1),
        locks: Default::default(),
        history: Default::default(),
    });
    let undo = Arc::new(RepoUndo(root.clone()));
    let git = Arc::new(Git {
        sandbox,
        program: config.git.unwrap_or_else(|| "git".into()),
        cwd: config.cwd,
        timeout: Duration::from_millis(config.timeout_ms.unwrap_or(30_000).max(1)),
        root: root.clone(),
        author: config.commit,
    });
    let mut tools: Vec<Arc<dyn Tool>> = vec![
        Arc::new(CreateFile(root.clone())),
        Arc::new(EditFiles(root.clone())),
        Arc::new(DeleteFile(root.clone())),
        Arc::new(MoveFile(root)),
        Arc::new(Patch(git.clone())),
    ];
    if git.author.is_some() {
        tools.push(Arc::new(Commit(git)));
    }
    Ok((tools, undo))
}

fn definition(name: &str, description: &str, input_schema: Value) -> ToolDefinition {
    definition_at(name, description, input_schema, PermissionLevel::Write)
}

fn definition_at(
    name: &str,
    description: &str,
    input_schema: Value,
    permission: PermissionLevel,
) -> ToolDefinition {
    ToolDefinition {
        name: ToolName::new(name),
        description: description.into(),
        input_schema,
        permission,
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

    async fn call(&self, input: Value, cx: ToolContext) -> Result<ToolOutput, ToolError> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Input {
            path: String,
            content: String,
        }
        let input: Input = parse(input)?;
        self.0
            .create(&call(&cx, "repo.create-file"), &input.path, &input.content)
            .await?;
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

    async fn call(&self, input: Value, cx: ToolContext) -> Result<ToolOutput, ToolError> {
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
        let changed = self
            .0
            .edit(&call(&cx, "repo.edit-files"), input.edits)
            .await?;
        Ok(done(format!("edited {}", changed.join(", "))))
    }
}

pub struct DeleteFile(Arc<Root>);

#[async_trait]
impl Tool for DeleteFile {
    fn definition(&self) -> ToolDefinition {
        definition(
            "repo.delete-file",
            "Deletes one regular file at a repository-relative path. It does not delete \
             directories or symbolic links.",
            json!({
                "type": "object",
                "properties": { "path": { "type": "string", "minLength": 1, "maxLength": 1024 } },
                "required": ["path"],
                "additionalProperties": false
            }),
        )
    }

    async fn call(&self, input: Value, cx: ToolContext) -> Result<ToolOutput, ToolError> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Input {
            path: String,
        }
        let input: Input = parse(input)?;
        self.0
            .delete(&call(&cx, "repo.delete-file"), &input.path)
            .await?;
        Ok(done(format!("deleted {}", input.path)))
    }
}

pub struct MoveFile(Arc<Root>);

#[async_trait]
impl Tool for MoveFile {
    fn definition(&self) -> ToolDefinition {
        definition(
            "repo.move-file",
            "Moves or renames one regular file to a repository-relative path that does not \
             exist yet, creating missing parent directories. It does not move directories or \
             symbolic links and never overwrites.",
            json!({
                "type": "object",
                "properties": {
                    "from": { "type": "string", "minLength": 1, "maxLength": 1024 },
                    "to": { "type": "string", "minLength": 1, "maxLength": 1024 }
                },
                "required": ["from", "to"],
                "additionalProperties": false
            }),
        )
    }

    async fn call(&self, input: Value, cx: ToolContext) -> Result<ToolOutput, ToolError> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Input {
            from: String,
            to: String,
        }
        let input: Input = parse(input)?;
        self.0
            .rename(&call(&cx, "repo.move-file"), &input.from, &input.to)
            .await?;
        Ok(done(format!("moved {} to {}", input.from, input.to)))
    }
}

struct Git {
    sandbox: Arc<dyn ExecEnv>,
    program: String,
    cwd: Option<PathBuf>,
    timeout: Duration,
    root: Arc<Root>,
    author: Option<CommitAuthor>,
}

impl Git {
    async fn run(
        &self,
        args: Vec<String>,
        cx: &ToolContext,
    ) -> Result<(i32, String, String), ToolError> {
        self.run_with(args, BTreeMap::new(), cx).await
    }

    /// Fails unless Git's top level for the working directory is the
    /// configured root: a tool that changes the index must act on that
    /// repository and no other.
    async fn check_root(&self, cx: &ToolContext) -> Result<(), ToolError> {
        let (code, top, stderr) = self
            .run(vec!["rev-parse".into(), "--show-toplevel".into()], cx)
            .await?;
        let top = tokio::fs::canonicalize(&top).await.ok();
        if code != 0 || top.as_deref() != Some(self.root.path.as_path()) {
            return Err(ToolError::NotStarted(format!(
                "the sandbox's repository is not the configured root ({})",
                if stderr.is_empty() {
                    "different top level"
                } else {
                    stderr.as_str()
                }
            )));
        }
        Ok(())
    }

    async fn run_with(
        &self,
        args: Vec<String>,
        extra_env: BTreeMap<String, String>,
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
        spec.env.extend(extra_env);
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
        git.check_root(&cx).await?;
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
        // The files as they are, for undo; `None` when one cannot be kept.
        let mut kept = Some(Vec::new());
        for path in patch_paths(&listing) {
            let state = match fsops::validate(&path) {
                Ok(()) => read_state(&git.root.path.join(&path), git.root.max_file_bytes).await,
                Err(_) => Err(()),
            };
            match (state, kept.as_mut()) {
                (Ok(before), Some(files)) => files.push((path, before)),
                _ => kept = None,
            }
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
        let mut files = Vec::new();
        for (path, before) in kept.unwrap_or_default() {
            let file = git.root.path.join(&path);
            let Ok(after) = read_state(&file, git.root.max_file_bytes).await else {
                // A file that grew too large: the patch has no undo.
                files.clear();
                break;
            };
            files.push(FileState {
                path,
                before,
                after,
            });
        }
        git.root.history.record(&call(&cx, "repo.patch"), files);
        Ok(done("patch applied".into()))
    }
}

/// The paths `git apply --numstat -z` lists: `added\tdeleted\tpath\0`, or
/// for a rename `added\tdeleted\t\0old\0new\0`.
fn patch_paths(listing: &str) -> Vec<String> {
    let mut paths: Vec<String> = Vec::new();
    let mut add = |path: &str| {
        if !path.is_empty() && !paths.iter().any(|known| known == path) {
            paths.push(path.to_owned());
        }
    };
    let mut fields = listing.split('\0');
    while let Some(record) = fields.next() {
        let mut parts = record.splitn(3, '\t');
        let (Some(_), Some(_), Some(path)) = (parts.next(), parts.next(), parts.next()) else {
            continue;
        };
        if path.is_empty() {
            for name in fields.by_ref().take(2) {
                add(name);
            }
        } else {
            add(path);
        }
    }
    paths
}

pub struct Commit(Arc<Git>);

#[async_trait]
impl Tool for Commit {
    fn definition(&self) -> ToolDefinition {
        definition_at(
            "repo.commit",
            "Commits the named files with a message: it stages them (new, changed or deleted) \
             and commits only them, leaving anything else staged out. It never amends, never \
             skips hooks and never pushes. The commit is made as the identity the host \
             configured.",
            json!({
                "type": "object",
                "properties": {
                    "message": { "type": "string", "minLength": 1, "maxLength": MAX_MESSAGE_BYTES },
                    "paths": {
                        "type": "array",
                        "minItems": 1,
                        "maxItems": MAX_COMMIT_PATHS,
                        "items": { "type": "string", "minLength": 1, "maxLength": 1024 }
                    }
                },
                "required": ["message", "paths"],
                "additionalProperties": false
            }),
            // It writes Git history and runs the repository's hooks.
            PermissionLevel::Execute,
        )
    }

    async fn call(&self, input: Value, cx: ToolContext) -> Result<ToolOutput, ToolError> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Input {
            message: String,
            paths: Vec<String>,
        }
        let input: Input = parse(input)?;
        if input.message.trim().is_empty() || input.message.len() > MAX_MESSAGE_BYTES {
            return Err(ToolError::InvalidInput(format!(
                "the message must not be empty and is at most {MAX_MESSAGE_BYTES} bytes"
            )));
        }
        if input.paths.is_empty() || input.paths.len() > MAX_COMMIT_PATHS {
            return Err(ToolError::InvalidInput(format!(
                "between 1 and {MAX_COMMIT_PATHS} paths are allowed"
            )));
        }
        let git = &self.0;
        let mut paths: Vec<String> = Vec::new();
        for path in input.paths {
            fsops::validate(&path)?;
            // Files only: a directory would commit everything under it.
            let on_disk = tokio::fs::symlink_metadata(git.root.path.join(&path)).await;
            if on_disk.is_ok_and(|metadata| metadata.is_dir()) {
                return Err(ToolError::InvalidInput(format!(
                    "`{path}` is a directory; name the files to commit"
                )));
            }
            if !paths.contains(&path) {
                paths.push(path);
            }
        }
        let Some(author) = &git.author else {
            return Err(ToolError::NotStarted(
                "no commit identity is configured".into(),
            ));
        };
        let identity = BTreeMap::from([
            ("GIT_AUTHOR_NAME".to_owned(), author.name.clone()),
            ("GIT_AUTHOR_EMAIL".to_owned(), author.email.clone()),
            ("GIT_COMMITTER_NAME".to_owned(), author.name.clone()),
            ("GIT_COMMITTER_EMAIL".to_owned(), author.email.clone()),
        ]);
        // The index and the files change together: no other write runs.
        let _exclusive = git.root.locks.all().await;
        git.check_root(&cx).await?;
        // Literal paths: `*` or `?` in a name is not a pattern.
        let named = |mut args: Vec<String>| {
            args.insert(0, "--literal-pathspecs".into());
            args.push("--".into());
            args.extend(paths.iter().cloned());
            args
        };
        let (code, _, stderr) = git.run(named(vec!["add".into()]), &cx).await?;
        if code != 0 {
            return Err(ToolError::InvalidInput(format!("cannot stage: {stderr}")));
        }
        // 0: nothing staged for these paths.
        let (code, _, stderr) = git
            .run(
                named(vec!["diff".into(), "--cached".into(), "--quiet".into()]),
                &cx,
            )
            .await?;
        match code {
            0 => {
                return Err(ToolError::InvalidInput(
                    "nothing to commit for these paths".into(),
                ));
            }
            1 => {}
            _ => return Err(ToolError::Execution(format!("cannot compare: {stderr}"))),
        }
        let (code, stdout, stderr) = git
            .run_with(
                named(vec![
                    "commit".into(),
                    format!("--message={}", input.message),
                    "--only".into(),
                ]),
                identity,
                &cx,
            )
            .await?;
        if code != 0 {
            // The paths are staged by now, so this is not a refusal.
            return Err(ToolError::Execution(format!(
                "git commit failed ({code}): {}",
                if stderr.is_empty() { stdout } else { stderr }
            )));
        }
        Ok(done(stdout.chars().take(MAX_COMMIT_OUTPUT_CHARS).collect()))
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
