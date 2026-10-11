//! `tools-builtin`: `echo` and a workspace-bounded `fs.read`.
//!
//! Configuration: `{ "root": "<dir>", "max_bytes": <n> }`. `root` defaults to
//! the current directory and is canonicalized at start; `fs.read` only reads
//! regular UTF-8 files inside it (relative paths, no `..`, no escaping
//! symlinks), up to `max_bytes` (default 256 KiB).

use std::{
    path::{Component, Path, PathBuf},
    sync::Arc,
};

use rah_api::{
    Plugin, PluginError, PluginId, PluginManifest, Scope, async_trait,
    tools::{TOOLS_SERVICE, Tool, ToolContext, ToolError, provide_tools},
};
use rah_protocol::{PermissionLevel, ToolContent, ToolDefinition, ToolName, ToolOutput};
use serde::Deserialize;
use serde_json::{Value, json};

const DEFAULT_MAX_BYTES: u64 = 256 * 1024;

pub struct BuiltinToolsPlugin {
    manifest: PluginManifest,
}

impl BuiltinToolsPlugin {
    pub fn new(id: impl Into<PluginId>) -> Self {
        Self {
            manifest: PluginManifest::new(id, env!("CARGO_PKG_VERSION")).provides([TOOLS_SERVICE]),
        }
    }
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    root: Option<PathBuf>,
    max_bytes: Option<u64>,
}

#[async_trait]
impl Plugin for BuiltinToolsPlugin {
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
            .map_err(|error| format!("workspace root `{}`: {error}", root.display()))?;
        let fs_read = FsRead {
            root,
            max_bytes: config.max_bytes.unwrap_or(DEFAULT_MAX_BYTES),
        };
        provide_tools(scope, vec![Arc::new(Echo), Arc::new(fs_read)])?;
        Ok(())
    }
}

fn text_output(text: String) -> ToolOutput {
    ToolOutput {
        content: vec![ToolContent::Text(text)],
        is_error: false,
    }
}

fn string_field(input: &Value, field: &str) -> Result<String, ToolError> {
    input
        .get(field)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| ToolError::InvalidInput(format!("expected a string field `{field}`")))
}

/// Returns its `text` input unchanged.
pub struct Echo;

#[async_trait]
impl Tool for Echo {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: ToolName::new("echo"),
            description: "Returns the given text unchanged.".into(),
            input_schema: json!({
                "type": "object",
                "properties": { "text": { "type": "string" } },
                "required": ["text"],
                "additionalProperties": false
            }),
            permission: PermissionLevel::None,
        }
    }

    async fn call(&self, input: Value, _cx: ToolContext) -> Result<ToolOutput, ToolError> {
        Ok(text_output(string_field(&input, "text")?))
    }
}

/// Reads a UTF-8 file inside the workspace root.
pub struct FsRead {
    root: PathBuf,
    max_bytes: u64,
}

impl FsRead {
    pub fn new(root: PathBuf, max_bytes: u64) -> Self {
        Self { root, max_bytes }
    }

    async fn resolve(&self, requested: &str) -> Result<PathBuf, ToolError> {
        let relative = Path::new(requested);
        let lexically_bounded = !requested.is_empty()
            && !relative.has_root()
            && relative
                .components()
                .all(|c| matches!(c, Component::Normal(_) | Component::CurDir));
        if !lexically_bounded {
            return Err(ToolError::InvalidInput(
                "path must be relative to the workspace root and must not contain `..`".into(),
            ));
        }
        let resolved = tokio::fs::canonicalize(self.root.join(relative))
            .await
            .map_err(|_| ToolError::Execution(format!("`{requested}` does not exist")))?;
        if !resolved.starts_with(&self.root) {
            return Err(ToolError::InvalidInput(format!(
                "`{requested}` resolves outside the workspace root"
            )));
        }
        Ok(resolved)
    }
}

#[async_trait]
impl Tool for FsRead {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: ToolName::new("fs.read"),
            description: "Reads a UTF-8 text file relative to the workspace root.".into(),
            input_schema: json!({
                "type": "object",
                "properties": { "path": { "type": "string" } },
                "required": ["path"],
                "additionalProperties": false
            }),
            permission: PermissionLevel::Read,
        }
    }

    async fn call(&self, input: Value, _cx: ToolContext) -> Result<ToolOutput, ToolError> {
        let requested = string_field(&input, "path")?;
        let path = self.resolve(&requested).await?;
        let metadata = tokio::fs::metadata(&path)
            .await
            .map_err(|error| ToolError::Execution(error.to_string()))?;
        if !metadata.is_file() {
            return Err(ToolError::InvalidInput(format!(
                "`{requested}` is not a regular file"
            )));
        }
        if metadata.len() > self.max_bytes {
            return Err(ToolError::Execution(format!(
                "`{requested}` exceeds the {} byte limit",
                self.max_bytes
            )));
        }
        let bytes = tokio::fs::read(&path)
            .await
            .map_err(|error| ToolError::Execution(error.to_string()))?;
        let text = String::from_utf8(bytes)
            .map_err(|_| ToolError::Execution(format!("`{requested}` is not UTF-8 text")))?;
        Ok(text_output(text))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rah_api::CancellationToken;
    use rah_protocol::SessionId;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir()
                .join(format!("rah-tools-builtin-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn cx() -> ToolContext {
        ToolContext {
            session: SessionId::new(),
            cancel: CancellationToken::new(),
        }
    }

    fn workspace(name: &str) -> (TempDir, FsRead) {
        let dir = TempDir::new(name);
        let root = dir.0.join("root");
        std::fs::create_dir_all(root.join("sub")).unwrap();
        std::fs::write(root.join("sub/a.txt"), "alpha").unwrap();
        std::fs::write(root.join("big.txt"), "x".repeat(32)).unwrap();
        std::fs::write(dir.0.join("secret.txt"), "outside").unwrap();
        let tool = FsRead::new(std::fs::canonicalize(&root).unwrap(), 16);
        (dir, tool)
    }

    #[tokio::test]
    async fn echo_returns_text_and_validates_input() {
        let output = Echo.call(json!({"text": "hi"}), cx()).await.unwrap();
        assert_eq!(output.content, vec![ToolContent::Text("hi".into())]);
        assert!(matches!(
            Echo.call(json!({"text": 1}), cx()).await,
            Err(ToolError::InvalidInput(_))
        ));
    }

    #[tokio::test]
    async fn fs_read_reads_files_inside_the_root() {
        let (_dir, tool) = workspace("inside");
        let output = tool.call(json!({"path": "sub/a.txt"}), cx()).await.unwrap();
        assert_eq!(output.content, vec![ToolContent::Text("alpha".into())]);
        let output = tool
            .call(json!({"path": "./sub/a.txt"}), cx())
            .await
            .unwrap();
        assert!(!output.is_error);
    }

    #[tokio::test]
    async fn fs_read_rejects_traversal_absolute_paths_dirs_and_large_files() {
        let (dir, tool) = workspace("bounds");
        let absolute = dir.0.join("secret.txt").display().to_string();
        for path in [
            "../secret.txt",
            "sub/../../secret.txt",
            absolute.as_str(),
            "",
            "sub",
        ] {
            assert!(
                matches!(
                    tool.call(json!({ "path": path }), cx()).await,
                    Err(ToolError::InvalidInput(_))
                ),
                "{path}"
            );
        }
        assert!(matches!(
            tool.call(json!({"path": "big.txt"}), cx()).await,
            Err(ToolError::Execution(message)) if message.contains("byte limit")
        ));
        assert!(matches!(
            tool.call(json!({"path": "missing.txt"}), cx()).await,
            Err(ToolError::Execution(_))
        ));
    }

    #[tokio::test]
    async fn fs_read_rejects_symlinks_that_escape_the_root() {
        let (dir, tool) = workspace("symlink");
        let link = dir.0.join("root/escape.txt");
        #[cfg(unix)]
        let created = std::os::unix::fs::symlink(dir.0.join("secret.txt"), &link).is_ok();
        #[cfg(windows)]
        let created = std::os::windows::fs::symlink_file(dir.0.join("secret.txt"), &link).is_ok();
        if !created {
            eprintln!("skipping: symlink creation is not permitted on this host");
            return;
        }
        assert!(matches!(
            tool.call(json!({"path": "escape.txt"}), cx()).await,
            Err(ToolError::InvalidInput(message)) if message.contains("outside")
        ));
    }
}
