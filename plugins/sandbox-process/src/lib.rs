//! `sandbox-process`: provides the `sandbox` service by running programs as
//! supervised child processes.
//!
//! **This is process supervision, not an OS sandbox.** Programs run with the
//! authority of the RAH process; there is no filesystem or network
//! isolation. What it does guarantee:
//!
//! - a fixed program and argument vector, never a shell string;
//! - a working directory inside the configured root;
//! - a cleared environment, refilled only from `inherit_env` and the spec;
//! - null stdin, bounded stdout and stderr, and a timeout capped by
//!   `max_timeout_ms`;
//! - termination of the process tree (a Job Object on Windows, a process
//!   group on Unix) on timeout, output overflow, cancellation, unmount of
//!   this plugin, or when the caller drops the future.
//!
//! Long-running programs ([`ExecEnv::spawn`]) get the same environment and
//! working-directory rules with piped stdin and stdout, a bounded stderr
//! tail, and no timeout. Their supervision tasks belong to this plugin's
//! scope: unmounting the plugin or dropping the returned handle terminates
//! the process tree.
//!
//! Configuration: `{ "root": "<dir>", "inherit_env": ["PATH", ...],
//! "max_timeout_ms": <n> }`. `root` defaults to the current directory;
//! `inherit_env` defaults to a small platform list (see
//! [`DEFAULT_INHERIT_ENV`]); `max_timeout_ms` defaults to 10 minutes.

mod spawn;
mod supervise;

use std::{
    collections::BTreeMap,
    ffi::OsString,
    path::{Component, Path, PathBuf},
    process::Stdio,
    sync::Arc,
    time::Duration,
};

use rah_api::{
    CancellationToken, Plugin, PluginError, PluginId, PluginManifest, Scope, async_trait,
    kernel::Spawner,
    sandbox::{ExecEnv, ExecError, ExecOutput, ProgramSpec, SANDBOX, SpawnSpec, Spawned},
};
use serde::Deserialize;
use tokio::process::Command;

/// Variables copied from the RAH process environment by default.
#[cfg(windows)]
pub const DEFAULT_INHERIT_ENV: &[&str] =
    &["PATH", "PATHEXT", "SYSTEMROOT", "WINDIR", "TEMP", "TMP"];
#[cfg(not(windows))]
pub const DEFAULT_INHERIT_ENV: &[&str] = &["PATH", "LANG", "TMPDIR"];

const DEFAULT_MAX_TIMEOUT: Duration = Duration::from_secs(600);

pub struct SandboxProcessPlugin {
    manifest: PluginManifest,
}

impl SandboxProcessPlugin {
    pub fn new(id: impl Into<PluginId>) -> Self {
        Self {
            manifest: PluginManifest::new(id, env!("CARGO_PKG_VERSION")).provides(["sandbox"]),
        }
    }
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    root: Option<PathBuf>,
    inherit_env: Option<Vec<String>>,
    max_timeout_ms: Option<u64>,
}

#[async_trait]
impl Plugin for SandboxProcessPlugin {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    async fn apply(&self, scope: &mut Scope) -> Result<(), PluginError> {
        let config: Config = if scope.config().is_null() {
            Config::default()
        } else {
            scope.config_as()?
        };
        let root = config.root.unwrap_or_else(|| PathBuf::from("."));
        let root = tokio::fs::canonicalize(&root)
            .await
            .map_err(|error| format!("sandbox root `{}`: {error}", root.display()))?;
        let names = config.inherit_env.unwrap_or_else(|| {
            DEFAULT_INHERIT_ENV
                .iter()
                .map(|name| (*name).to_owned())
                .collect()
        });
        let mut base_env = BTreeMap::new();
        for name in names {
            validate_env_name(&name).map_err(|message| format!("inherit_env: {message}"))?;
            if let Some(value) = std::env::var_os(&name) {
                base_env.insert(OsString::from(name), value);
            }
        }
        let max_timeout = match config.max_timeout_ms {
            Some(0) => return Err("max_timeout_ms must be greater than zero".into()),
            Some(ms) => Duration::from_millis(ms),
            None => DEFAULT_MAX_TIMEOUT,
        };
        let env = ProcessEnv {
            root,
            base_env,
            max_timeout,
            stop: scope.spawner().cancellation(),
            spawner: Some(scope.spawner()),
        };
        scope.provide(SANDBOX, Arc::new(env))?;
        Ok(())
    }
}

/// The [`ExecEnv`] implementation.
pub struct ProcessEnv {
    root: PathBuf,
    base_env: BTreeMap<OsString, OsString>,
    max_timeout: Duration,
    /// Cancelled when the owning plugin scope is disposed.
    stop: CancellationToken,
    /// Owns the supervision tasks of spawned programs; `None` without a
    /// scope, which disables `spawn`.
    spawner: Option<Spawner>,
}

impl ProcessEnv {
    /// An environment rooted at an already canonical directory, for embedding
    /// and tests. It has no owning scope, so it cannot `spawn`.
    pub fn new(
        root: PathBuf,
        base_env: BTreeMap<OsString, OsString>,
        max_timeout: Duration,
    ) -> Self {
        Self {
            root,
            base_env,
            max_timeout,
            stop: CancellationToken::new(),
            spawner: None,
        }
    }

    fn validate_command(
        &self,
        program: &str,
        args: &[String],
        env: &BTreeMap<String, String>,
    ) -> Result<(), ExecError> {
        let invalid = |message: String| Err(ExecError::InvalidSpec(message));
        if program.is_empty() || program.contains('\0') {
            return invalid("program must be a non-empty string without NUL".into());
        }
        if args.iter().any(|arg| arg.contains('\0')) {
            return invalid("arguments must not contain NUL".into());
        }
        for (name, value) in env {
            validate_env_name(name).map_err(ExecError::InvalidSpec)?;
            if value.contains('\0') {
                return invalid(format!("environment value of `{name}` contains NUL"));
            }
        }
        Ok(())
    }

    fn command(
        &self,
        program: &str,
        args: &[String],
        cwd: PathBuf,
        env: &BTreeMap<String, String>,
    ) -> Command {
        let mut command = Command::new(program);
        command
            .args(args)
            .current_dir(cwd)
            .env_clear()
            .envs(&self.base_env)
            .envs(env)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        supervise::configure(&mut command);
        command
    }

    fn validate(&self, spec: &ProgramSpec) -> Result<(), ExecError> {
        let invalid = |message: String| Err(ExecError::InvalidSpec(message));
        self.validate_command(&spec.program, &spec.args, &spec.env)?;
        if spec.timeout.is_zero() || spec.timeout > self.max_timeout {
            return invalid(format!(
                "timeout must be between 1 ms and {} ms",
                self.max_timeout.as_millis()
            ));
        }
        if spec.limits.stdout_bytes == 0 || spec.limits.stderr_bytes == 0 {
            return invalid("output limits must be greater than zero".into());
        }
        Ok(())
    }

    async fn working_directory(&self, requested: Option<&Path>) -> Result<PathBuf, ExecError> {
        let Some(requested) = requested else {
            return Ok(self.root.clone());
        };
        let shown = requested.display();
        let bounded = !requested.as_os_str().is_empty()
            && !requested.has_root()
            && requested
                .components()
                .all(|c| matches!(c, Component::Normal(_) | Component::CurDir));
        if !bounded {
            return Err(ExecError::InvalidSpec(format!(
                "working directory `{shown}` must be relative to the sandbox root without `..`"
            )));
        }
        let resolved = tokio::fs::canonicalize(self.root.join(requested))
            .await
            .map_err(|_| {
                ExecError::InvalidSpec(format!("working directory `{shown}` does not exist"))
            })?;
        if !resolved.starts_with(&self.root) || !resolved.is_dir() {
            return Err(ExecError::InvalidSpec(format!(
                "working directory `{shown}` is not a directory inside the sandbox root"
            )));
        }
        Ok(resolved)
    }
}

#[async_trait]
impl ExecEnv for ProcessEnv {
    fn root(&self) -> Option<PathBuf> {
        Some(self.root.clone())
    }

    async fn run(
        &self,
        spec: ProgramSpec,
        cancel: CancellationToken,
    ) -> Result<ExecOutput, ExecError> {
        self.validate(&spec)?;
        let cwd = self.working_directory(spec.cwd.as_deref()).await?;
        if cancel.is_cancelled() || self.stop.is_cancelled() {
            return Err(ExecError::InvalidSpec("cancelled before start".into()));
        }
        let mut command = self.command(&spec.program, &spec.args, cwd, &spec.env);
        command.stdin(Stdio::null());
        let child = command.spawn().map_err(|error| ExecError::Spawn {
            program: spec.program.clone(),
            message: error.to_string(),
        })?;
        supervise::supervise(child, &spec, &cancel, &self.stop).await
    }

    async fn spawn(&self, spec: SpawnSpec) -> Result<Spawned, ExecError> {
        let Some(spawner) = &self.spawner else {
            return Err(ExecError::InvalidSpec(
                "this environment has no owning scope and cannot spawn".into(),
            ));
        };
        self.validate_command(&spec.program, &spec.args, &spec.env)?;
        if spec.stderr_bytes == 0 {
            return Err(ExecError::InvalidSpec(
                "stderr_bytes must be greater than zero".into(),
            ));
        }
        let cwd = self.working_directory(spec.cwd.as_deref()).await?;
        if self.stop.is_cancelled() {
            return Err(ExecError::InvalidSpec(
                "the sandbox is shutting down".into(),
            ));
        }
        let mut command = self.command(&spec.program, &spec.args, cwd, &spec.env);
        command.stdin(Stdio::piped());
        let child = command.spawn().map_err(|error| ExecError::Spawn {
            program: spec.program.clone(),
            message: error.to_string(),
        })?;
        spawn::start(child, spec.stderr_bytes, spawner, self.stop.child_token())
    }
}

fn validate_env_name(name: &str) -> Result<(), String> {
    if name.is_empty() || name.contains(['=', '\0']) {
        return Err(format!("`{name}` is not a valid environment variable name"));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
