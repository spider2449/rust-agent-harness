use std::{path::Path, process::Stdio};

use async_trait::async_trait;
use tokio::{process::Command, time};

use crate::{
    CommandSpec, ExecutionResult, Sandbox, SandboxError, SandboxPolicy, WorkspacePathError,
    WorkspacePolicy,
};

/// Local subprocess executor with workspace-CWD validation.
///
/// This executor does not provide OS isolation and therefore accepts only
/// `FullAccess`. Dropping its execution future terminates the child process.
pub struct ProcessSandbox {
    workspace: WorkspacePolicy,
}

impl ProcessSandbox {
    /// Creates an executor rooted at an existing workspace directory.
    pub fn new(workspace_root: impl AsRef<Path>) -> Result<Self, WorkspacePathError> {
        Ok(Self {
            workspace: WorkspacePolicy::new(workspace_root)?,
        })
    }

    fn working_directory(
        &self,
        requested: Option<&Path>,
    ) -> Result<std::path::PathBuf, SandboxError> {
        let Some(requested) = requested else {
            return Ok(self.workspace.root().to_path_buf());
        };
        let resolved = self
            .workspace
            .resolve_existing(requested)
            .map_err(|error| SandboxError::InvalidWorkingDirectory {
                path: requested.to_path_buf(),
                message: error.to_string(),
            })?;
        if !resolved.is_dir() {
            return Err(SandboxError::InvalidWorkingDirectory {
                path: requested.to_path_buf(),
                message: "path is not a directory".to_owned(),
            });
        }
        Ok(resolved)
    }
}

#[async_trait]
impl Sandbox for ProcessSandbox {
    async fn execute(
        &self,
        command: CommandSpec,
        policy: SandboxPolicy,
    ) -> Result<ExecutionResult, SandboxError> {
        if policy != SandboxPolicy::FullAccess {
            return Err(SandboxError::PolicyDenied {
                policy,
                message: "local process execution cannot enforce this isolation policy".to_owned(),
            });
        }
        if command.program.is_empty() {
            return Err(SandboxError::Execution {
                program: command.program,
                message: "program must not be empty".to_owned(),
            });
        }
        let cwd = self.working_directory(command.cwd.as_deref())?;
        let mut process = Command::new(&command.program);
        process
            .args(&command.args)
            .current_dir(cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let output = if let Some(timeout) = command.timeout {
            match time::timeout(timeout, process.output()).await {
                Ok(output) => output.map_err(|error| SandboxError::Execution {
                    program: command.program.clone(),
                    message: error.to_string(),
                })?,
                Err(_) => {
                    return Ok(ExecutionResult {
                        stdout: Vec::new(),
                        stderr: Vec::new(),
                        exit_code: None,
                        timed_out: true,
                    });
                }
            }
        } else {
            process
                .output()
                .await
                .map_err(|error| SandboxError::Execution {
                    program: command.program.clone(),
                    message: error.to_string(),
                })?
        };

        Ok(ExecutionResult {
            stdout: output.stdout,
            stderr: output.stderr,
            exit_code: output.status.code(),
            timed_out: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf, time::Duration};

    use super::ProcessSandbox;
    use crate::{CommandSpec, Sandbox, SandboxError, SandboxPolicy};

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "rah-process-sandbox-{}-{:?}",
                std::process::id(),
                std::thread::current().id()
            ));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir(&path).expect("test directory should be created");
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn captures_output_and_exit_code() {
        let workspace = TestDirectory::new();
        let sandbox = ProcessSandbox::new(&workspace.0).expect("workspace should be valid");

        let result = sandbox
            .execute(
                CommandSpec {
                    program: "rustc".to_owned(),
                    args: vec!["--version".to_owned()],
                    cwd: None,
                    timeout: Some(Duration::from_secs(10)),
                },
                SandboxPolicy::FullAccess,
            )
            .await
            .expect("rustc should execute");

        assert_eq!(result.exit_code, Some(0));
        assert!(String::from_utf8_lossy(&result.stdout).contains("rustc"));
        assert!(!result.timed_out);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn rejects_outside_working_directory() {
        let base = TestDirectory::new();
        let workspace = base.0.join("workspace");
        let outside = base.0.join("outside");
        fs::create_dir(&workspace).expect("workspace should be created");
        fs::create_dir(&outside).expect("outside directory should be created");
        let sandbox = ProcessSandbox::new(&workspace).expect("workspace should be valid");

        let error = sandbox
            .execute(
                CommandSpec {
                    program: "rustc".to_owned(),
                    args: vec!["--version".to_owned()],
                    cwd: Some(outside),
                    timeout: None,
                },
                SandboxPolicy::FullAccess,
            )
            .await
            .expect_err("outside cwd should fail");

        assert!(matches!(
            error,
            SandboxError::InvalidWorkingDirectory { .. }
        ));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn rejects_policy_it_cannot_enforce() {
        let workspace = TestDirectory::new();
        let sandbox = ProcessSandbox::new(&workspace.0).expect("workspace should be valid");

        let error = sandbox
            .execute(
                CommandSpec {
                    program: "rustc".to_owned(),
                    args: vec!["--version".to_owned()],
                    cwd: None,
                    timeout: None,
                },
                SandboxPolicy::ReadOnly,
            )
            .await
            .expect_err("unenforceable policy should fail");

        assert!(matches!(error, SandboxError::PolicyDenied { .. }));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn terminates_process_after_timeout() {
        let workspace = TestDirectory::new();
        let sandbox = ProcessSandbox::new(&workspace.0).expect("workspace should be valid");
        let test_binary = std::env::current_exe().expect("test binary path should be available");

        let result = sandbox
            .execute(
                CommandSpec {
                    program: test_binary.display().to_string(),
                    args: vec![
                        "--ignored".to_owned(),
                        "--exact".to_owned(),
                        "process::tests::timeout_child".to_owned(),
                    ],
                    cwd: None,
                    timeout: Some(Duration::from_millis(50)),
                },
                SandboxPolicy::FullAccess,
            )
            .await
            .expect("timed out process should return a result");

        assert!(result.timed_out);
        assert_eq!(result.exit_code, None);
    }

    #[test]
    #[ignore = "helper process for timeout test"]
    fn timeout_child() {
        std::thread::sleep(Duration::from_secs(5));
    }
}
