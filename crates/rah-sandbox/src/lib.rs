//! Sandbox abstractions for RAH.

use std::{path::PathBuf, time::Duration};

use async_trait::async_trait;
use thiserror::Error;

mod workspace;

pub use workspace::{WorkspacePathError, WorkspacePolicy};

/// Authority available to a sandboxed command.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SandboxPolicy {
    /// Command may inspect configured resources but must not modify them.
    ReadOnly,
    /// Command may write within the configured workspace boundary.
    WorkspaceWrite,
    /// Command may use the host authority granted to the RAH process.
    FullAccess,
}

/// Direct subprocess request that avoids shell-string interpolation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandSpec {
    /// Executable name or path.
    pub program: String,
    /// Arguments passed directly to the executable.
    pub args: Vec<String>,
    /// Optional validated working directory.
    pub cwd: Option<PathBuf>,
    /// Optional maximum execution duration.
    pub timeout: Option<Duration>,
}

/// Captured result of a subprocess execution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutionResult {
    /// Raw standard output bytes.
    pub stdout: Vec<u8>,
    /// Raw standard error bytes.
    pub stderr: Vec<u8>,
    /// Process exit code, or `None` when no code was available.
    pub exit_code: Option<i32>,
    /// Whether execution ended because its timeout elapsed.
    pub timed_out: bool,
}

/// Error returned before or during sandbox execution.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum SandboxError {
    /// The requested working directory is invalid.
    #[error("invalid working directory `{path}`: {message}")]
    InvalidWorkingDirectory {
        /// Rejected working directory.
        path: PathBuf,
        /// Validation failure detail.
        message: String,
    },
    /// The requested operation exceeds its configured authority.
    #[error("sandbox policy {policy:?} denied execution: {message}")]
    PolicyDenied {
        /// Policy that denied the operation.
        policy: SandboxPolicy,
        /// Denial detail.
        message: String,
    },
    /// The subprocess could not be started or managed.
    #[error("failed to execute `{program}`: {message}")]
    Execution {
        /// Requested executable.
        program: String,
        /// Execution failure detail.
        message: String,
    },
}

/// Execution boundary for subprocess implementations.
#[async_trait]
pub trait Sandbox: Send + Sync {
    /// Executes a direct command under the selected policy.
    async fn execute(
        &self,
        command: CommandSpec,
        policy: SandboxPolicy,
    ) -> Result<ExecutionResult, SandboxError>;
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use futures::executor::block_on;

    use super::{CommandSpec, ExecutionResult, Sandbox, SandboxError, SandboxPolicy};

    struct TestSandbox;

    #[async_trait]
    impl Sandbox for TestSandbox {
        async fn execute(
            &self,
            command: CommandSpec,
            _policy: SandboxPolicy,
        ) -> Result<ExecutionResult, SandboxError> {
            Ok(ExecutionResult {
                stdout: command.program.into_bytes(),
                stderr: Vec::new(),
                exit_code: Some(0),
                timed_out: false,
            })
        }
    }

    #[test]
    fn sandbox_trait_returns_captured_result() {
        block_on(async {
            let result = TestSandbox
                .execute(
                    CommandSpec {
                        program: "example".to_owned(),
                        args: vec!["argument".to_owned()],
                        cwd: None,
                        timeout: None,
                    },
                    SandboxPolicy::ReadOnly,
                )
                .await
                .expect("test sandbox should execute");

            assert_eq!(result.stdout, b"example");
            assert_eq!(result.exit_code, Some(0));
            assert!(!result.timed_out);
        });
    }
}
