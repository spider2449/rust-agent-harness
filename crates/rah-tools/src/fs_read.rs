use std::path::Path;

use async_trait::async_trait;
use rah_protocol::{PermissionLevel, ToolContent, ToolDefinition, ToolInput, ToolName, ToolOutput};
use rah_sandbox::{WorkspacePathError, WorkspacePolicy};
use serde_json::json;
use tokio::io::AsyncReadExt;

use crate::{Tool, ToolContext, ToolError};

/// Workspace-bounded UTF-8 file reader.
pub struct FsReadTool {
    workspace: WorkspacePolicy,
    max_bytes: usize,
}

impl FsReadTool {
    /// Creates a file reader rooted at an existing workspace directory.
    pub fn new(
        workspace_root: impl AsRef<Path>,
        max_bytes: usize,
    ) -> Result<Self, WorkspacePathError> {
        Ok(Self {
            workspace: WorkspacePolicy::new(workspace_root)?,
            max_bytes,
        })
    }
}

#[async_trait]
impl Tool for FsReadTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: ToolName::new("fs.read"),
            description: "Reads a UTF-8 text file within the configured workspace.".to_owned(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"}
                },
                "required": ["path"],
                "additionalProperties": false
            }),
            permission: PermissionLevel::Read,
        }
    }

    async fn execute(
        &self,
        input: ToolInput,
        _context: ToolContext,
    ) -> Result<ToolOutput, ToolError> {
        let path = input
            .0
            .as_object()
            .and_then(|object| object.get("path"))
            .and_then(|value| value.as_str())
            .ok_or_else(|| ToolError::InvalidInput {
                message: "`path` must be a string".to_owned(),
            })?;
        let resolved = self
            .workspace
            .resolve_existing(path)
            .map_err(tool_execution_error)?;
        let metadata =
            tokio::fs::metadata(&resolved)
                .await
                .map_err(|error| ToolError::Execution {
                    message: format!("failed to inspect `{}`: {error}", resolved.display()),
                })?;
        if !metadata.is_file() {
            return Err(ToolError::Execution {
                message: format!("`{}` is not a file", resolved.display()),
            });
        }

        let file =
            tokio::fs::File::open(&resolved)
                .await
                .map_err(|error| ToolError::Execution {
                    message: format!("failed to open `{}`: {error}", resolved.display()),
                })?;
        let limit = u64::try_from(self.max_bytes)
            .unwrap_or(u64::MAX)
            .saturating_add(1);
        let mut bytes = Vec::new();
        file.take(limit)
            .read_to_end(&mut bytes)
            .await
            .map_err(|error| ToolError::Execution {
                message: format!("failed to read `{}`: {error}", resolved.display()),
            })?;
        if bytes.len() > self.max_bytes {
            return Err(ToolError::Execution {
                message: format!("file exceeds maximum size of {} bytes", self.max_bytes),
            });
        }
        if bytes.contains(&0) {
            return Err(ToolError::Execution {
                message: "file appears to be binary".to_owned(),
            });
        }
        let text = String::from_utf8(bytes).map_err(|_| ToolError::Execution {
            message: "file is not valid UTF-8 text".to_owned(),
        })?;

        Ok(ToolOutput {
            content: vec![ToolContent::Text(text)],
            is_error: false,
        })
    }
}

fn tool_execution_error(error: WorkspacePathError) -> ToolError {
    ToolError::Execution {
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    use rah_protocol::{PermissionLevel, ToolContent, ToolInput};
    use serde_json::json;

    use super::FsReadTool;
    use crate::{Tool, ToolContext, ToolError};

    static NEXT_TEST_DIR: AtomicU64 = AtomicU64::new(0);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time should follow Unix epoch")
                .as_nanos();
            let sequence = NEXT_TEST_DIR.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "rah-fs-read-{}-{timestamp}-{sequence}",
                std::process::id()
            ));
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
    async fn reads_utf8_file_within_workspace() {
        let workspace = TestDirectory::new();
        fs::write(workspace.0.join("note.txt"), "hello").expect("file should be written");
        let tool = FsReadTool::new(&workspace.0, 32).expect("workspace should be valid");

        let output = tool
            .execute(
                ToolInput(json!({"path": "note.txt"})),
                ToolContext::default(),
            )
            .await
            .expect("text file should be read");

        assert_eq!(output.content, vec![ToolContent::Text("hello".to_owned())]);
        assert_eq!(tool.definition().permission, PermissionLevel::Read);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn rejects_outside_workspace_read() {
        let base = TestDirectory::new();
        let workspace = base.0.join("workspace");
        fs::create_dir(&workspace).expect("workspace should be created");
        fs::write(base.0.join("outside.txt"), "outside").expect("file should be written");
        let tool = FsReadTool::new(&workspace, 32).expect("workspace should be valid");

        let error = tool
            .execute(
                ToolInput(json!({"path": "../outside.txt"})),
                ToolContext::default(),
            )
            .await
            .expect_err("outside read should fail");

        assert!(matches!(error, ToolError::Execution { .. }));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn rejects_absolute_outside_workspace_read() {
        let base = TestDirectory::new();
        let workspace = base.0.join("workspace");
        fs::create_dir(&workspace).expect("workspace should be created");
        let outside = base.0.join("outside.txt");
        fs::write(&outside, "outside").expect("file should be written");
        let tool = FsReadTool::new(&workspace, 32).expect("workspace should be valid");

        let error = tool
            .execute(
                ToolInput(json!({"path": outside.to_string_lossy()})),
                ToolContext::default(),
            )
            .await
            .expect_err("absolute outside read should fail");

        assert!(matches!(error, ToolError::Execution { .. }));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn rejects_missing_and_non_string_path_input() {
        let workspace = TestDirectory::new();
        let tool = FsReadTool::new(&workspace.0, 32).expect("workspace should be valid");

        for input in [json!({}), json!({"path": 7})] {
            let error = tool
                .execute(ToolInput(input), ToolContext::default())
                .await
                .expect_err("invalid path input should fail");

            assert_eq!(
                error,
                ToolError::InvalidInput {
                    message: "`path` must be a string".to_owned()
                }
            );
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn rejects_file_larger_than_limit() {
        let workspace = TestDirectory::new();
        fs::write(workspace.0.join("large.txt"), "12345").expect("file should be written");
        let tool = FsReadTool::new(&workspace.0, 4).expect("workspace should be valid");

        let error = tool
            .execute(
                ToolInput(json!({"path": "large.txt"})),
                ToolContext::default(),
            )
            .await
            .expect_err("oversized file should fail");

        assert_eq!(
            error,
            ToolError::Execution {
                message: "file exceeds maximum size of 4 bytes".to_owned()
            }
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn rejects_binary_file() {
        let workspace = TestDirectory::new();
        fs::write(workspace.0.join("binary.dat"), [0, 1, 2]).expect("file should be written");
        let tool = FsReadTool::new(&workspace.0, 32).expect("workspace should be valid");

        let error = tool
            .execute(
                ToolInput(json!({"path": "binary.dat"})),
                ToolContext::default(),
            )
            .await
            .expect_err("binary file should fail");

        assert_eq!(
            error,
            ToolError::Execution {
                message: "file appears to be binary".to_owned()
            }
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn rejects_non_utf8_text_without_binary_marker() {
        let workspace = TestDirectory::new();
        fs::write(workspace.0.join("invalid-utf8.dat"), [0xff]).expect("file should be written");
        let tool = FsReadTool::new(&workspace.0, 32).expect("workspace should be valid");

        let error = tool
            .execute(
                ToolInput(json!({"path": "invalid-utf8.dat"})),
                ToolContext::default(),
            )
            .await
            .expect_err("invalid UTF-8 should fail");

        assert_eq!(
            error,
            ToolError::Execution {
                message: "file is not valid UTF-8 text".to_owned()
            }
        );
    }
}
