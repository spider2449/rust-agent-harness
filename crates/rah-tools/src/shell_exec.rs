use std::{path::PathBuf, sync::Arc, time::Duration};

use async_trait::async_trait;
use rah_protocol::{PermissionLevel, ToolContent, ToolDefinition, ToolInput, ToolName, ToolOutput};
use rah_sandbox::{CommandSpec, Sandbox, SandboxPolicy};
use serde_json::{Map, Value, json};

use crate::{Tool, ToolContext, ToolError};

/// Direct subprocess tool routed through a configured sandbox implementation.
///
/// This generic tool accepts model-selected process details and is unsuitable
/// for live model exposure without the host-owned policy required by ADR 0009.
pub struct ShellExecTool {
    sandbox: Arc<dyn Sandbox>,
    policy: SandboxPolicy,
    default_timeout: Duration,
    max_timeout: Duration,
}

impl ShellExecTool {
    /// Creates a subprocess tool with explicit sandbox policy and timeout limits.
    #[must_use]
    pub fn new(
        sandbox: Arc<dyn Sandbox>,
        policy: SandboxPolicy,
        default_timeout: Duration,
        max_timeout: Duration,
    ) -> Self {
        Self {
            sandbox,
            policy,
            default_timeout,
            max_timeout,
        }
    }
}

#[async_trait]
impl Tool for ShellExecTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: ToolName::new("shell.exec"),
            description: "Executes a program with a direct argument vector through the sandbox."
                .to_owned(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "program": {"type": "string"},
                    "args": {
                        "type": "array",
                        "items": {"type": "string"}
                    },
                    "cwd": {"type": "string"},
                    "timeout_ms": {"type": "integer", "minimum": 1}
                },
                "required": ["program"],
                "additionalProperties": false
            }),
            permission: PermissionLevel::Execute,
        }
    }

    async fn execute(
        &self,
        input: ToolInput,
        _context: ToolContext,
    ) -> Result<ToolOutput, ToolError> {
        let object = input.0.as_object().ok_or_else(|| ToolError::InvalidInput {
            message: "input must be an object".to_owned(),
        })?;
        reject_unknown_fields(object)?;
        let program = required_string(object, "program")?;
        if program.is_empty() {
            return Err(ToolError::InvalidInput {
                message: "`program` must not be empty".to_owned(),
            });
        }
        let args = string_array(object, "args")?;
        let cwd = optional_string(object, "cwd")?.map(PathBuf::from);
        let timeout = optional_timeout(object)?.unwrap_or(self.default_timeout);
        if timeout.is_zero() || timeout > self.max_timeout {
            return Err(ToolError::InvalidInput {
                message: format!(
                    "`timeout_ms` must be between 1 and {}",
                    self.max_timeout.as_millis()
                ),
            });
        }

        let result = self
            .sandbox
            .execute(
                CommandSpec {
                    program: program.to_owned(),
                    args,
                    cwd,
                    timeout: Some(timeout),
                },
                self.policy,
            )
            .await
            .map_err(|error| ToolError::Execution {
                message: error.to_string(),
            })?;
        let content = json!({
            "stdout": String::from_utf8_lossy(&result.stdout),
            "stderr": String::from_utf8_lossy(&result.stderr),
            "exit_code": result.exit_code,
            "timed_out": result.timed_out
        });

        Ok(ToolOutput {
            content: vec![ToolContent::Json(content)],
            is_error: false,
        })
    }
}

fn reject_unknown_fields(object: &Map<String, Value>) -> Result<(), ToolError> {
    if let Some(name) = object
        .keys()
        .find(|name| !matches!(name.as_str(), "program" | "args" | "cwd" | "timeout_ms"))
    {
        return Err(ToolError::InvalidInput {
            message: format!("unknown field `{name}`"),
        });
    }
    Ok(())
}

fn required_string<'a>(object: &'a Map<String, Value>, name: &str) -> Result<&'a str, ToolError> {
    object
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| ToolError::InvalidInput {
            message: format!("`{name}` must be a string"),
        })
}

fn optional_string<'a>(
    object: &'a Map<String, Value>,
    name: &str,
) -> Result<Option<&'a str>, ToolError> {
    object
        .get(name)
        .map(|value| {
            value.as_str().ok_or_else(|| ToolError::InvalidInput {
                message: format!("`{name}` must be a string"),
            })
        })
        .transpose()
}

fn string_array(object: &Map<String, Value>, name: &str) -> Result<Vec<String>, ToolError> {
    let Some(value) = object.get(name) else {
        return Ok(Vec::new());
    };
    let values = value.as_array().ok_or_else(|| ToolError::InvalidInput {
        message: format!("`{name}` must be an array of strings"),
    })?;
    values
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| ToolError::InvalidInput {
                    message: format!("`{name}` must be an array of strings"),
                })
        })
        .collect()
}

fn optional_timeout(object: &Map<String, Value>) -> Result<Option<Duration>, ToolError> {
    object
        .get("timeout_ms")
        .map(|value| {
            value
                .as_u64()
                .map(Duration::from_millis)
                .ok_or_else(|| ToolError::InvalidInput {
                    message: "`timeout_ms` must be a positive integer".to_owned(),
                })
        })
        .transpose()
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf, sync::Arc, time::Duration};

    use rah_protocol::{PermissionLevel, ToolContent, ToolInput};
    use rah_sandbox::{ProcessSandbox, SandboxPolicy};
    use serde_json::json;

    use super::ShellExecTool;
    use crate::{Tool, ToolContext, ToolError};

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "rah-shell-exec-{}-{:?}",
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
    async fn executes_direct_program_and_captures_result() {
        let workspace = TestDirectory::new();
        let sandbox =
            Arc::new(ProcessSandbox::new(&workspace.0).expect("workspace should be valid"));
        let tool = ShellExecTool::new(
            sandbox,
            SandboxPolicy::FullAccess,
            Duration::from_secs(10),
            Duration::from_secs(30),
        );

        let output = tool
            .execute(
                ToolInput(json!({
                    "program": "rustc",
                    "args": ["--version"],
                    "cwd": ".",
                    "timeout_ms": 10_000
                })),
                ToolContext::default(),
            )
            .await
            .expect("rustc should execute");

        let ToolContent::Json(result) = &output.content[0] else {
            panic!("shell output should be JSON");
        };
        assert_eq!(result["exit_code"], 0);
        assert!(
            result["stdout"]
                .as_str()
                .is_some_and(|text| text.contains("rustc"))
        );
        assert_eq!(tool.definition().permission, PermissionLevel::Execute);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn rejects_shell_string_instead_of_argument_array() {
        let workspace = TestDirectory::new();
        let sandbox =
            Arc::new(ProcessSandbox::new(&workspace.0).expect("workspace should be valid"));
        let tool = ShellExecTool::new(
            sandbox,
            SandboxPolicy::FullAccess,
            Duration::from_secs(10),
            Duration::from_secs(30),
        );

        let error = tool
            .execute(
                ToolInput(json!({"program": "rustc", "args": "--version"})),
                ToolContext::default(),
            )
            .await
            .expect_err("string args should fail");

        assert!(matches!(error, ToolError::InvalidInput { .. }));
    }
}
