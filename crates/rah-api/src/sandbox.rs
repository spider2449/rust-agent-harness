//! The `sandbox` service: supervised program execution.
//!
//! An [`ExecEnv`] runs one program with a fixed argument vector (never a
//! shell string), an explicit working directory and environment, a timeout,
//! bounded output and cancellation ([`ExecEnv::run`]), or keeps one running
//! with piped stdin and stdout for a protocol such as MCP
//! ([`ExecEnv::spawn`]). Implementations must state honestly what isolation
//! they provide; process supervision is not an OS sandbox.

use std::{collections::BTreeMap, path::PathBuf, time::Duration};

use async_trait::async_trait;
use rah_kernel::ServiceKey;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_util::sync::CancellationToken;

pub const SANDBOX: ServiceKey<dyn ExecEnv> = ServiceKey::new("sandbox");

/// One program to run. Callers are trusted plugins; when a field comes from
/// model input, the calling tool must validate it first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProgramSpec {
    /// A bare program name (resolved through the environment's `PATH`) or a
    /// path. Never interpreted by a shell.
    pub program: String,
    pub args: Vec<String>,
    /// Working directory relative to the environment's root; `None` is the
    /// root itself.
    pub cwd: Option<PathBuf>,
    /// Variables added to the environment's base environment.
    pub env: BTreeMap<String, String>,
    pub timeout: Duration,
    pub limits: OutputLimits,
}

impl ProgramSpec {
    pub fn new(program: impl Into<String>, args: Vec<String>, timeout: Duration) -> Self {
        Self {
            program: program.into(),
            args,
            cwd: None,
            env: BTreeMap::new(),
            timeout,
            limits: OutputLimits::default(),
        }
    }
}

/// Bytes retained per stream. Exceeding a limit terminates the program.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OutputLimits {
    pub stdout_bytes: usize,
    pub stderr_bytes: usize,
}

impl Default for OutputLimits {
    fn default() -> Self {
        Self {
            stdout_bytes: 256 * 1024,
            stderr_bytes: 256 * 1024,
        }
    }
}

/// Why the environment stopped a program before it exited on its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Termination {
    TimedOut,
    OutputLimit,
    Cancelled,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExecOutput {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    /// The exit code when the program exited on its own and the platform
    /// reported one.
    pub exit_code: Option<i32>,
    /// Set when the environment terminated the program. Its external effects
    /// are then uncertain.
    pub termination: Option<Termination>,
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ExecError {
    /// The spec was rejected; nothing was started.
    #[error("invalid program spec: {0}")]
    InvalidSpec(String),
    /// The program could not be started; nothing ran.
    #[error("cannot start `{program}`: {message}")]
    Spawn { program: String, message: String },
    /// Supervision failed after the program started; its effects are
    /// uncertain.
    #[error("process supervision failed: {0}")]
    Supervision(String),
}

/// A long-running program with piped stdin and stdout.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpawnSpec {
    /// As in [`ProgramSpec::program`].
    pub program: String,
    pub args: Vec<String>,
    /// Relative to the environment's root; `None` is the root itself.
    pub cwd: Option<PathBuf>,
    pub env: BTreeMap<String, String>,
    /// Bytes of the most recent stderr output kept for diagnostics.
    pub stderr_bytes: usize,
}

impl SpawnSpec {
    pub fn new(program: impl Into<String>, args: Vec<String>) -> Self {
        Self {
            program: program.into(),
            args,
            cwd: None,
            env: BTreeMap::new(),
            stderr_bytes: 64 * 1024,
        }
    }
}

/// A spawned program. Dropping [`Spawned::process`] terminates it.
pub struct Spawned {
    pub stdin: Box<dyn AsyncWrite + Send + Unpin>,
    pub stdout: Box<dyn AsyncRead + Send + Unpin>,
    pub process: Box<dyn ProcessHandle>,
}

/// Supervision of a spawned program. The environment also terminates it when
/// the environment itself goes away.
#[async_trait]
pub trait ProcessHandle: Send + Sync {
    /// Waits until the program has ended. Returns its exit code when it
    /// exited on its own and the platform reported one.
    async fn wait(&self) -> Option<i32>;

    /// Terminates the program and its descendants. Idempotent.
    fn terminate(&self);

    /// The retained tail of the program's stderr.
    fn stderr(&self) -> String;
}

#[async_trait]
pub trait ExecEnv: Send + Sync {
    /// The directory programs run in by default, when the environment has
    /// one on the host file system, in a form people recognize (on Windows
    /// without the verbatim `\\?\` prefix). For display and for recording
    /// where work happened; it grants nothing.
    fn root(&self) -> Option<PathBuf> {
        None
    }

    /// Runs one program to completion, termination or cancellation. Dropping
    /// the future must stop the program as well.
    async fn run(
        &self,
        spec: ProgramSpec,
        cancel: CancellationToken,
    ) -> Result<ExecOutput, ExecError>;

    /// Starts a long-running program. Environments that cannot supervise one
    /// refuse.
    async fn spawn(&self, spec: SpawnSpec) -> Result<Spawned, ExecError> {
        Err(ExecError::InvalidSpec(format!(
            "this sandbox cannot spawn long-running programs (`{}`)",
            spec.program
        )))
    }
}
