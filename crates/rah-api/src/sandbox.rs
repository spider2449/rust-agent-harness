//! The `sandbox` service: supervised program execution.
//!
//! An [`ExecEnv`] runs one program with a fixed argument vector (never a
//! shell string), an explicit working directory and environment, a timeout,
//! bounded output and cancellation. Implementations must state honestly what
//! isolation they provide; process supervision is not an OS sandbox.

use std::{collections::BTreeMap, path::PathBuf, time::Duration};

use async_trait::async_trait;
use rah_kernel::ServiceKey;
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

#[async_trait]
pub trait ExecEnv: Send + Sync {
    /// Runs one program to completion, termination or cancellation. Dropping
    /// the future must stop the program as well.
    async fn run(
        &self,
        spec: ProgramSpec,
        cancel: CancellationToken,
    ) -> Result<ExecOutput, ExecError>;
}
