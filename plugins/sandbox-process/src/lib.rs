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
//! "inherit_env_extra": ["NAME", ...], "max_timeout_ms": <n> }`. `root`
//! defaults to the current directory; `inherit_env` replaces the default
//! platform list (see [`DEFAULT_INHERIT_ENV`]), while `inherit_env_extra` adds
//! names to whichever list applies; `max_timeout_ms` defaults to 10 minutes.
//!
//! The crate also provides `sandbox-os` ([`SandboxOsPlugin`]), the same
//! service with OS-level confinement on Linux and Windows.

mod os;
mod process;
mod spawn;
mod supervise;

pub use os::SandboxOsPlugin;

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

use os::Confinement;
use process::Process;

/// What a started program reads as standard input.
#[derive(Clone, Copy)]
enum Stdin {
    Null,
    Piped,
}

/// Variables copied from the RAH process environment by default. The home
/// directory variables let programs find the user's own configuration (Git
/// reads `safe.directory` and the identity from `~/.gitconfig` and fails
/// with "$HOME not set" without them; rustup finds its toolchains through
/// them). They name a directory; they carry no secret. `ProgramData` lets
/// rustc find the MSVC linker: the Visual Studio setup locator reads its
/// instance registrations from `%ProgramData%`, and without it every build
/// fails with "linker `link.exe` not found".
#[cfg(windows)]
pub const DEFAULT_INHERIT_ENV: &[&str] = &[
    "PATH",
    "PATHEXT",
    "SYSTEMROOT",
    "WINDIR",
    "TEMP",
    "TMP",
    "HOME",
    "USERPROFILE",
    "HOMEDRIVE",
    "HOMEPATH",
    "ProgramData",
];
#[cfg(not(windows))]
pub const DEFAULT_INHERIT_ENV: &[&str] = &["PATH", "LANG", "TMPDIR", "HOME"];

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
    #[serde(default)]
    inherit_env_extra: Vec<String>,
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
        let env = ProcessEnv {
            root: canonical_root(config.root).await?,
            base_env: inherited_env(config.inherit_env, config.inherit_env_extra)?,
            max_timeout: max_timeout(config.max_timeout_ms)?,
            stop: scope.spawner().cancellation(),
            spawner: Some(scope.spawner()),
            confinement: None,
        };
        scope.provide(SANDBOX, Arc::new(env))?;
        Ok(())
    }
}

/// The configured root, canonicalized; the current directory by default.
async fn canonical_root(root: Option<PathBuf>) -> Result<PathBuf, String> {
    let root = root.unwrap_or_else(|| PathBuf::from("."));
    tokio::fs::canonicalize(&root)
        .await
        .map_err(|error| format!("sandbox root `{}`: {error}", root.display()))
}

/// The base environment: the named variables of the RAH process that are
/// set, from `names` ([`DEFAULT_INHERIT_ENV`] when `None`) followed by
/// `extra`.
fn inherited_env(
    names: Option<Vec<String>>,
    extra: Vec<String>,
) -> Result<BTreeMap<OsString, OsString>, String> {
    let names = names.unwrap_or_else(|| {
        DEFAULT_INHERIT_ENV
            .iter()
            .map(|name| (*name).to_owned())
            .collect()
    });
    for name in &names {
        validate_env_name(name).map_err(|message| format!("inherit_env: {message}"))?;
    }
    for name in &extra {
        validate_env_name(name).map_err(|message| format!("inherit_env_extra: {message}"))?;
    }
    let mut base_env = BTreeMap::new();
    for name in names.into_iter().chain(extra) {
        if let Some(value) = std::env::var_os(&name) {
            base_env.insert(OsString::from(name), value);
        }
    }
    Ok(base_env)
}

fn max_timeout(ms: Option<u64>) -> Result<Duration, String> {
    match ms {
        Some(0) => Err("max_timeout_ms must be greater than zero".into()),
        Some(ms) => Ok(Duration::from_millis(ms)),
        None => Ok(DEFAULT_MAX_TIMEOUT),
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
    /// OS-level confinement applied to every program (`sandbox-os`); `None`
    /// for plain supervision.
    confinement: Option<Arc<Confinement>>,
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
            confinement: None,
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
        #[cfg(not(windows))]
        if let Some(confinement) = &self.confinement {
            confinement.configure(&mut command);
        }
        command
    }

    /// Starts a validated program, confined when this environment confines.
    async fn start(
        &self,
        program: &str,
        args: &[String],
        cwd: PathBuf,
        env: &BTreeMap<String, String>,
        stdin: Stdin,
    ) -> Result<Process, ExecError> {
        let failed = |error: std::io::Error| ExecError::Spawn {
            program: program.to_owned(),
            message: error.to_string(),
        };
        #[cfg(windows)]
        if let Some(confinement) = &self.confinement {
            return confinement
                .spawn(program, args, &cwd, &self.base_env, env, stdin)
                .await
                .map_err(failed);
        }
        let mut command = self.command(program, args, cwd, env);
        command.stdin(match stdin {
            Stdin::Null => Stdio::null(),
            Stdin::Piped => Stdio::piped(),
        });
        command.spawn().map(Process::from_tokio).map_err(failed)
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
        let text = self.root.display().to_string();
        Some(match text.strip_prefix(r"\\?\") {
            Some(rest) if !rest.starts_with("UNC\\") => PathBuf::from(rest),
            _ => self.root.clone(),
        })
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
        let child = self
            .start(&spec.program, &spec.args, cwd, &spec.env, Stdin::Null)
            .await?;
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
        let child = self
            .start(&spec.program, &spec.args, cwd, &spec.env, Stdin::Piped)
            .await?;
        spawn::start(child, spec.stderr_bytes, spawner, self.stop.child_token())
    }
}

fn validate_env_name(name: &str) -> Result<(), String> {
    if name.is_empty() || name.contains(['=', '\0']) {
        return Err(format!("`{name}` is not a valid environment variable name"));
    }
    Ok(())
}

impl SandboxProcessPlugin {
    /// An example configuration, mostly the defaults, for reference (the Web
    /// UI shows it in an empty configuration editor). It is never applied.
    /// `inherit_env` lists [`DEFAULT_INHERIT_ENV`], so the defaults are
    /// visible; `inherit_env_extra` adds to them without restating them.
    #[cfg(windows)]
    pub const CONFIG_EXAMPLE: &str = r#"{
  "root": ".",
  "inherit_env": [
    "PATH",
    "PATHEXT",
    "SYSTEMROOT",
    "WINDIR",
    "TEMP",
    "TMP",
    "HOME",
    "USERPROFILE",
    "HOMEDRIVE",
    "HOMEPATH",
    "ProgramData"
  ],
  "inherit_env_extra": [],
  "max_timeout_ms": 600000
}"#;
    /// An example configuration, mostly the defaults, for reference (the Web
    /// UI shows it in an empty configuration editor). It is never applied.
    /// `inherit_env` lists [`DEFAULT_INHERIT_ENV`], so the defaults are
    /// visible; `inherit_env_extra` adds to them without restating them.
    #[cfg(not(windows))]
    pub const CONFIG_EXAMPLE: &str = r#"{
  "root": ".",
  "inherit_env": [
    "PATH",
    "LANG",
    "TMPDIR",
    "HOME"
  ],
  "inherit_env_extra": [],
  "max_timeout_ms": 600000
}"#;

    /// What each configuration key does, as `(key, help)` pairs, for the
    /// Web UI's configuration editor.
    pub const CONFIG_HELP: &[(&str, &str)] = &[
        ROOT_HELP,
        INHERIT_ENV_HELP,
        INHERIT_ENV_EXTRA_HELP,
        MAX_TIMEOUT_HELP,
    ];
}

const ROOT_HELP: (&str, &str) = (
    "root",
    "Directory programs run in; a program's working directory must stay \
     inside it. Defaults to the directory rah was started in.",
);
const INHERIT_ENV_HELP: (&str, &str) = (
    "inherit_env",
    "Programs start with an empty environment. These variables are copied \
     from rah's own environment. Setting it replaces the whole default list \
     shown in the example, so prefer inherit_env_extra.",
);
const INHERIT_ENV_EXTRA_HELP: (&str, &str) = (
    "inherit_env_extra",
    "Variable names added to the default list, for a program that needs one \
     more, for example [\"CARGO_HOME\"]. If a build fails with \"link.exe \
     not found\" on an older rah, add \"ProgramData\".",
);
const MAX_TIMEOUT_HELP: (&str, &str) = (
    "max_timeout_ms",
    "Longest time one program may run, in milliseconds (default 600000, 10 \
     minutes). A tool's own timeout must fit within it.",
);

#[cfg(test)]
mod tests;

#[cfg(test)]
#[test]
fn the_config_example_is_a_valid_configuration() {
    let parsed: Result<Config, _> = serde_json::from_str(SandboxProcessPlugin::CONFIG_EXAMPLE);
    assert!(parsed.is_ok(), "{:?}", parsed.err());
}

/// Asserts that a configuration example's `inherit_env` lists exactly
/// [`DEFAULT_INHERIT_ENV`].
#[cfg(test)]
fn assert_example_lists_the_defaults(example: &str) {
    let example: serde_json::Value = serde_json::from_str(example).unwrap();
    let listed: Vec<&str> = example["inherit_env"]
        .as_array()
        .unwrap()
        .iter()
        .map(|name| name.as_str().unwrap())
        .collect();
    assert_eq!(listed, DEFAULT_INHERIT_ENV);
}

#[cfg(test)]
#[test]
fn the_config_examples_list_the_default_inherit_env() {
    assert_example_lists_the_defaults(SandboxProcessPlugin::CONFIG_EXAMPLE);
    assert_example_lists_the_defaults(SandboxOsPlugin::CONFIG_EXAMPLE);
}

/// Asserts that the help describes exactly the keys of the example.
#[cfg(test)]
fn assert_help_matches_example(help: &[(&str, &str)], example: &str) {
    let example: serde_json::Map<String, serde_json::Value> =
        serde_json::from_str(example).unwrap();
    let mut example_keys: Vec<&str> = example.keys().map(String::as_str).collect();
    let mut help_keys: Vec<&str> = help.iter().map(|(key, _)| *key).collect();
    example_keys.sort_unstable();
    help_keys.sort_unstable();
    assert_eq!(help_keys, example_keys);
}

#[cfg(test)]
#[test]
fn the_config_help_describes_every_example_key() {
    assert_help_matches_example(
        SandboxProcessPlugin::CONFIG_HELP,
        SandboxProcessPlugin::CONFIG_EXAMPLE,
    );
    assert_help_matches_example(
        SandboxOsPlugin::CONFIG_HELP,
        SandboxOsPlugin::CONFIG_EXAMPLE,
    );
}
