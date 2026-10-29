//! `context-instructions`: instruction files for the model (ADR 0061).
//!
//! On every model request, through `model.before_request`, the plugin reads
//!
//! - the workspace's instruction file: the first of `names` (default
//!   `["AGENTS.md"]`) that exists directly in the root of the `sandbox`
//!   service's workspace. Parent directories are not searched, so a file the
//!   user did not expect is never read. Without a `sandbox` service there is
//!   no workspace file.
//! - every file in `files`, such as standing rules or notes on how to use the
//!   tools. `rah` resolves a relative path against the data directory.
//!
//! and adds them, each under a heading naming its file, as one system item
//! after the leading system items. Each file is cut at `max_bytes` (32768),
//! with a note saying so. The files are read on every request, so edits apply
//! immediately and switching the workspace switches its file.
//!
//! A workspace without an instruction file is normal. A file in `files` that
//! is missing or unreadable is told to the user once with a
//! [`HostNotice`](rah_api::host::HostNotice) (ADR 0057), never to the model.
//!
//! The files are text for the model only: they grant no permission and every
//! tool call is still decided by the policy. A workspace file may come from
//! someone else's repository, so it can carry prompt injection like any text
//! the model reads (`docs/SECURITY.md`).
//!
//! Configuration (all optional): `{ "names": ["AGENTS.md"], "files": [],
//! "max_bytes": 32768 }`.

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use rah_api::{
    KernelHandle, Plugin, PluginError, PluginId, PluginManifest, Scope, async_trait,
    hooks::ModelBeforeRequest, host::NoticeOnce, sandbox::SANDBOX,
};
use rah_protocol::{ContextItem, ModelRequest};
use serde::Deserialize;
use tokio::io::AsyncReadExt;

const DEFAULT_NAME: &str = "AGENTS.md";
const DEFAULT_MAX_BYTES: u64 = 32 * 1024;

pub struct InstructionsPlugin {
    manifest: PluginManifest,
}

impl InstructionsPlugin {
    pub fn new(id: impl Into<PluginId>) -> Self {
        Self {
            manifest: PluginManifest::new(id, env!("CARGO_PKG_VERSION")),
        }
    }

    /// An example configuration, mostly the defaults, for reference (the Web
    /// UI shows it in an empty configuration editor). It is never applied.
    pub const CONFIG_EXAMPLE: &str = r#"{
  "names": ["AGENTS.md"],
  "files": [],
  "max_bytes": 32768
}"#;
}

/// The plugin configuration.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstructionsConfig {
    pub names: Option<Vec<String>>,
    pub files: Option<Vec<PathBuf>>,
    pub max_bytes: Option<u64>,
}

impl InstructionsConfig {
    /// The file names to look for in the workspace root, checked.
    fn names(&self) -> Result<Vec<String>, String> {
        let names = self
            .names
            .clone()
            .unwrap_or_else(|| vec![DEFAULT_NAME.to_owned()]);
        for name in &names {
            let plain = Path::new(name)
                .file_name()
                .is_some_and(|file| file == name.as_str());
            if !plain {
                return Err(format!(
                    "`names` takes file names in the workspace root, not paths: `{name}`"
                ));
            }
        }
        Ok(names)
    }
}

#[async_trait]
impl Plugin for InstructionsPlugin {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    fn check_config(&self, config: &serde_json::Value) -> Result<(), PluginError> {
        let config: InstructionsConfig = rah_api::config::parse(config)?;
        config.names().map_err(PluginError::from)?;
        Ok(())
    }

    async fn apply(&self, scope: &mut Scope) -> Result<(), PluginError> {
        let config: InstructionsConfig = rah_api::config::parse(scope.config())?;
        let reader = Arc::new(Reader {
            kernel: scope.kernel(),
            names: config.names().map_err(PluginError::from)?,
            files: config.files.unwrap_or_default(),
            max_bytes: config.max_bytes.unwrap_or(DEFAULT_MAX_BYTES).max(1),
            notices: NoticeOnce::new(scope.kernel(), scope.id().clone()),
        });
        scope.hook_waterfall::<ModelBeforeRequest, _, _>(0, move |request| {
            let reader = reader.clone();
            async move {
                let text = reader.read().await;
                insert(request, text)
            }
        });
        Ok(())
    }
}

/// Adds `text` as a system item after the leading system items.
pub fn insert(mut request: ModelRequest, text: Option<String>) -> ModelRequest {
    if let Some(text) = text {
        let at = request
            .items
            .iter()
            .position(|item| !matches!(item, ContextItem::System { .. }))
            .unwrap_or(request.items.len());
        request.items.insert(at, ContextItem::System { text });
    }
    request
}

struct Reader {
    kernel: KernelHandle,
    names: Vec<String>,
    files: Vec<PathBuf>,
    max_bytes: u64,
    notices: NoticeOnce,
}

impl Reader {
    /// Every instruction file's text under its heading, or `None` when there
    /// is none.
    async fn read(&self) -> Option<String> {
        let mut sections = Vec::new();
        if let Some(root) = self.kernel.get(SANDBOX).and_then(|sandbox| sandbox.root()) {
            for name in &self.names {
                if let Ok(Some(text)) = read_bounded(&root.join(name), self.max_bytes).await {
                    sections.push(format!(
                        "Instructions from `{name}` in the workspace:\n\n{text}"
                    ));
                    break;
                }
            }
        }
        for path in &self.files {
            match read_bounded(path, self.max_bytes).await {
                Ok(Some(text)) => {
                    let name = path.file_name().map_or_else(
                        || path.display().to_string(),
                        |name| name.to_string_lossy().into_owned(),
                    );
                    sections.push(format!("Instructions from `{name}`:\n\n{text}"));
                }
                Ok(None) => {
                    let shown = path.display().to_string();
                    self.notices.emit(
                        &shown,
                        &format!(
                            "the instruction file `{shown}` in `files` does not exist; \
                             the model does not get it"
                        ),
                    );
                }
                Err(error) => {
                    let shown = path.display().to_string();
                    self.notices.emit(
                        &shown,
                        &format!(
                            "the instruction file `{shown}` in `files` cannot be read \
                             ({error}); the model does not get it"
                        ),
                    );
                }
            }
        }
        (!sections.is_empty()).then(|| sections.join("\n\n"))
    }
}

/// The text of the regular file at `path`, cut at `max_bytes` (at a
/// character boundary, with a note). `Ok(None)` when there is no such file;
/// bytes that are not UTF-8 are replaced.
async fn read_bounded(path: &Path, max_bytes: u64) -> std::io::Result<Option<String>> {
    let metadata = match tokio::fs::metadata(path).await {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    if !metadata.is_file() {
        return Ok(None);
    }
    let mut bytes = Vec::new();
    tokio::fs::File::open(path)
        .await?
        .take(max_bytes)
        .read_to_end(&mut bytes)
        .await?;
    let mut text = String::from_utf8_lossy(&bytes).into_owned();
    if metadata.len() > max_bytes {
        // The cut may split a character; drop its replacement character.
        if text.ends_with('\u{fffd}') {
            text.pop();
        }
        text.push_str(&format!(
            "\n[The file has {} bytes; only the first {max_bytes} are shown.]",
            metadata.len()
        ));
    }
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text).trim_end();
    Ok(Some(text.to_owned()))
}

#[cfg(test)]
mod tests;

#[cfg(test)]
#[test]
fn the_config_example_is_a_valid_configuration() {
    let parsed: Result<InstructionsConfig, _> =
        serde_json::from_str(InstructionsPlugin::CONFIG_EXAMPLE);
    assert!(parsed.is_ok(), "{:?}", parsed.err());
}
