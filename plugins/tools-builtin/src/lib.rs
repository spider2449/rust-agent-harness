//! `tools-builtin`: `echo` and workspace-bounded file tools: `fs.read`,
//! `fs.list`, `fs.glob` and `fs.grep`.
//!
//! Configuration: `{ "root": "<dir>", "max_bytes": <n>, "max_results": <n>,
//! "max_scan": <n>, "skip_dirs": ["..."] }`, all optional. `root` defaults to
//! the current directory and is canonicalized at start; the tools only see
//! paths inside it (relative paths, no `..`, no escaping symlinks).
//! `max_bytes` (default 256 KiB) bounds the files `fs.read` reads and
//! `fs.grep` searches. `max_results` (default 500) bounds the lines a search
//! returns and `max_scan` (default 20000) the entries it visits. `skip_dirs`
//! (default `.git`, `target`, `node_modules`) are listed but not searched.

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

mod glob;
mod search;

use search::{FsGlob, FsGrep, FsList, Workspace};

const DEFAULT_MAX_BYTES: u64 = 256 * 1024;
const DEFAULT_MAX_RESULTS: usize = 500;
const DEFAULT_MAX_SCAN: usize = 20_000;
const DEFAULT_SKIP_DIRS: [&str; 3] = [".git", "target", "node_modules"];

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
    max_results: Option<usize>,
    max_scan: Option<usize>,
    skip_dirs: Option<Vec<String>>,
}

#[async_trait]
impl Plugin for BuiltinToolsPlugin {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    fn check_config(&self, config: &serde_json::Value) -> Result<(), PluginError> {
        rah_api::config::parse::<Config>(config)?;
        Ok(())
    }

    async fn apply(&self, scope: &mut Scope) -> Result<(), PluginError> {
        let config: Config = rah_api::config::parse(scope.config())?;
        let root = config.root.unwrap_or_else(|| PathBuf::from("."));
        let root = tokio::fs::canonicalize(&root)
            .await
            .map_err(|error| format!("workspace root `{}`: {error}", root.display()))?;
        let max_bytes = config.max_bytes.unwrap_or(DEFAULT_MAX_BYTES);
        let workspace = Arc::new(Workspace {
            root: root.clone(),
            max_bytes,
            max_results: config.max_results.unwrap_or(DEFAULT_MAX_RESULTS).max(1),
            max_scan: config.max_scan.unwrap_or(DEFAULT_MAX_SCAN).max(1),
            skip_dirs: config
                .skip_dirs
                .unwrap_or_else(|| DEFAULT_SKIP_DIRS.map(str::to_owned).to_vec()),
        });
        provide_tools(
            scope,
            vec![
                Arc::new(Echo),
                Arc::new(FsRead::new(root, max_bytes)),
                Arc::new(FsList(workspace.clone())),
                Arc::new(FsGlob(workspace.clone())),
                Arc::new(FsGrep(workspace)),
            ],
        )?;
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
}

/// Resolves a path relative to the canonical `root`, refusing absolute
/// paths, `..` and symbolic links that lead outside it.
pub(crate) async fn resolve(root: &Path, requested: &str) -> Result<PathBuf, ToolError> {
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
    let resolved = tokio::fs::canonicalize(root.join(relative))
        .await
        .map_err(|_| ToolError::Execution(format!("`{requested}` does not exist")))?;
    if !resolved.starts_with(root) {
        return Err(ToolError::InvalidInput(format!(
            "`{requested}` resolves outside the workspace root"
        )));
    }
    Ok(resolved)
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
        let path = resolve(&self.root, &requested).await?;
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

impl BuiltinToolsPlugin {
    /// An example configuration, mostly the defaults, for reference (the Web
    /// UI shows it in an empty configuration editor). It is never applied.
    pub const CONFIG_EXAMPLE: &str = r#"{
  "root": ".",
  "max_bytes": 262144,
  "max_results": 500,
  "max_scan": 20000,
  "skip_dirs": [
    ".git",
    "target",
    "node_modules"
  ]
}"#;
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
            call: rah_protocol::ToolCallId::new(),
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

#[cfg(test)]
#[test]
fn the_config_example_is_a_valid_configuration() {
    let parsed: Result<Config, _> = serde_json::from_str(BuiltinToolsPlugin::CONFIG_EXAMPLE);
    assert!(parsed.is_ok(), "{:?}", parsed.err());
}
