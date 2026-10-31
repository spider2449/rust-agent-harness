//! The plugin directory: one subdirectory per personal plugin, written in
//! full by each save and swapped in by renaming, so a reader sees either the
//! old plugin or the new one.

use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::Mutex;

use crate::{manifest::Manifest, programs::Found};

/// The file with what the builder knows about a plugin; a directory without
/// it was not written by the builder and is never replaced.
pub const ABOUT_FILE: &str = "plugin-builder.json";
pub const MANIFEST_FILE: &str = "manifest.json";
pub const MOUNT_FILE: &str = "mount.toml";
const PYTHON_LIBRARY: &str = include_str!("runtime/rah_plugin.py");
const NODE_LIBRARY: &str = include_str!("runtime/rah_plugin.mjs");
/// Files the builder writes, which the model cannot send.
pub const RESERVED_FILES: [&str; 5] = [
    ABOUT_FILE,
    MANIFEST_FILE,
    MOUNT_FILE,
    "rah_plugin.py",
    "rah_plugin.mjs",
];
/// The deepest file path, in directories and file name.
pub const MAX_PATH_DEPTH: usize = 4;
const MAX_PATH_PART_BYTES: usize = 64;
/// The most plugins listed.
const MAX_LISTED: usize = 256;
/// The most bytes `read` returns over all files.
const MAX_READ_BYTES: usize = 1024 * 1024;
/// The `format` of an export bundle.
pub const BUNDLE_FORMAT: &str = "rah-personal-plugins";
/// The bundle version this builder writes and reads.
pub const BUNDLE_VERSION: u64 = 1;
/// The largest bundle file read.
const MAX_BUNDLE_BYTES: u64 = 32 * 1024 * 1024;
/// The most plugins in one bundle.
pub const MAX_BUNDLE_PLUGINS: usize = 64;

fn unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|time| time.as_secs())
        .unwrap_or_default()
}

/// The language a plugin is written in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Runtime {
    Python,
    Node,
}

impl Runtime {
    pub fn entry(self) -> &'static str {
        match self {
            Self::Python => "plugin.py",
            Self::Node => "plugin.mjs",
        }
    }

    fn library(self) -> (&'static str, &'static str) {
        match self {
            Self::Python => ("rah_plugin.py", PYTHON_LIBRARY),
            Self::Node => ("rah_plugin.mjs", NODE_LIBRARY),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Python => "python",
            Self::Node => "node",
        }
    }
}

/// `plugin-builder.json`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct About {
    pub name: String,
    pub description: String,
    pub runtime: Runtime,
}

/// A plugin that passed every check, ready to be written.
#[derive(Clone, Debug, PartialEq)]
pub struct Plan {
    pub about: About,
    pub manifest: Manifest,
    /// Relative paths (`/`-separated) and contents.
    pub files: Vec<(String, String)>,
    pub replace: bool,
}

/// A saved plugin, as listed.
#[derive(Clone, Debug, PartialEq)]
pub struct Saved {
    pub about: About,
    pub manifest: Manifest,
    pub dir: PathBuf,
}

/// Why a save did not happen or did not finish.
#[derive(Debug, PartialEq, Eq)]
pub enum SaveError {
    /// Nothing was written.
    Refused(String),
    /// Writing failed; the plugin directory may hold a partial staging
    /// directory, but the plugin itself is unchanged or replaced whole.
    Failed(String),
}

/// Checks a file path the model sent: relative, `/`-separated, at most
/// [`MAX_PATH_DEPTH`] parts of letters, digits, `.`, `_` and `-`, none
/// starting with `.` or naming a Windows device.
pub fn check_path(path: &str) -> Result<(), String> {
    let parts: Vec<&str> = path.split('/').collect();
    let part_ok = |part: &&str| {
        let stem = part
            .split('.')
            .next()
            .unwrap_or_default()
            .to_ascii_uppercase();
        let device = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
            || ((stem.starts_with("COM") || stem.starts_with("LPT"))
                && stem.len() == 4
                && stem.as_bytes()[3].is_ascii_digit());
        !part.is_empty()
            && part.len() <= MAX_PATH_PART_BYTES
            && !part.starts_with('.')
            && !part.ends_with('.')
            && !device
            && part
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
    };
    if parts.len() > MAX_PATH_DEPTH || !parts.iter().all(part_ok) {
        return Err(format!(
            "invalid file path `{path}`: a relative path of at most {MAX_PATH_DEPTH} parts \
             separated by `/`, each of letters, digits, `.`, `_` and `-`, not starting with `.`"
        ));
    }
    if parts.len() == 1
        && RESERVED_FILES
            .iter()
            .any(|reserved| reserved.eq_ignore_ascii_case(path))
    {
        return Err(format!(
            "`{path}` is written by `plugins.save`; do not send it"
        ));
    }
    Ok(())
}

/// Checks the set of files: valid paths, unique without regard to case (as
/// on Windows), no path inside another file, and the entry file present.
pub fn check_files(
    files: &[(String, String)],
    runtime: Runtime,
    max_files: usize,
    max_file_bytes: usize,
) -> Result<(), String> {
    if files.len() > max_files {
        return Err(format!(
            "{} files; a plugin has at most {max_files}",
            files.len()
        ));
    }
    let mut seen = std::collections::BTreeSet::new();
    for (path, content) in files {
        check_path(path)?;
        if content.len() > max_file_bytes {
            return Err(format!(
                "`{path}` is {} bytes; a file has at most {max_file_bytes}",
                content.len()
            ));
        }
        if !seen.insert(path.to_ascii_lowercase()) {
            return Err(format!("`{path}` is sent twice"));
        }
    }
    for path in &seen {
        let parts: Vec<&str> = path.split('/').collect();
        for depth in 1..parts.len() {
            let prefix = parts[..depth].join("/");
            if seen.contains(&prefix) {
                return Err(format!("`{prefix}` is a file and a directory of `{path}`"));
            }
        }
    }
    if !files.iter().any(|(path, _)| path == runtime.entry()) {
        return Err(format!(
            "the `{}` runtime needs the entry file `{}`",
            runtime.label(),
            runtime.entry()
        ));
    }
    Ok(())
}

/// A TOML basic string.
fn toml_string(text: &str) -> String {
    let mut quoted = String::with_capacity(text.len() + 2);
    quoted.push('"');
    for c in text.chars() {
        match c {
            '"' => quoted.push_str("\\\""),
            '\\' => quoted.push_str("\\\\"),
            '\n' => quoted.push_str("\\n"),
            '\t' => quoted.push_str("\\t"),
            c if c.is_control() => quoted.push_str(&format!("\\u{:04X}", c as u32)),
            c => quoted.push(c),
        }
    }
    quoted.push('"');
    quoted
}

/// `mount.toml`: the `rah.toml` entry that mounts the plugin, and what the
/// user should know before mounting it.
pub fn mount_toml(about: &About, manifest: &Manifest, dir: &Path, program: &Found) -> String {
    let dir_text = dir.display().to_string();
    let entry = dir.join(about.runtime.entry()).display().to_string();
    let mut text = format!(
        "# Personal plugin `{name}`, written by plugin-builder: {description}\n\
         #\n\
         # A mounted plugin is trusted code that runs with your rights. Read the\n\
         # files in this directory before mounting it:\n\
         #   {dir_text}\n\
         #\n",
        name = about.name,
        description = about.description,
    );
    if let Some(warning) = &program.warning {
        text.push_str(&format!("# WARNING: {warning}.\n#\n"));
    }
    text.push_str(
        "# To mount it:\n\
         # 1. Add the [[plugin]] entry below to rah.toml (in the data directory,\n\
         #    such as ~/.rah/rah.toml), before the agent loop entry.\n",
    );
    let assignments: Vec<String> = manifest
        .tools
        .iter()
        .map(|tool| {
            let level = serde_json::to_value(tool.permission)
                .ok()
                .and_then(|value| value.as_str().map(str::to_owned))
                .unwrap_or_default();
            format!(
                "{} = {}",
                toml_string(tool.name.as_str()),
                toml_string(&level)
            )
        })
        .collect();
    let check = if assignments.is_empty() {
        "# 2. Check it with `rah --check-config`, then restart.\n".to_owned()
    } else {
        let mut step = "# 2. REQUIRED: add its tools to the `tools = { ... }` of the\n\
             #    policy-authority entry, or the policy refuses every call to them:\n"
            .to_owned();
        for assignment in &assignments {
            step.push_str(&format!("#      {assignment}\n"));
        }
        step.push_str(&format!(
            "#    For example: tools = {{ echo = \"none\", ..., {} }}\n\
             # 3. Check it with `rah --check-config`, then restart.\n",
            assignments.join(", ")
        ));
        step
    };
    text.push_str(&check);
    text.push_str(&format!(
        "\n[[plugin]]\n\
         use = \"process\"\n\
         id = {id}\n\
         config = {{ program = {program}, args = [{entry}], cwd = {cwd}, personal = true }}\n",
        id = toml_string(&about.name),
        program = toml_string(&program.program),
        entry = toml_string(&entry),
        cwd = toml_string(&dir_text),
    ));
    if !manifest.requires.is_empty() {
        text.push_str(&format!(
            "\n# It calls these host services: {}.\n",
            manifest.requires.join(", ")
        ));
    }
    if !manifest.hooks.is_empty() {
        let hooks: Vec<String> = manifest
            .hooks
            .iter()
            .map(|hook| format!("{} (priority {})", hook.hook, hook.priority))
            .collect();
        text.push_str(&format!(
            "\n# It attaches to these hooks and runs on every call they cover: {}.\n",
            hooks.join(", ")
        ));
    }
    if !manifest.events.is_empty() {
        text.push_str(&format!(
            "\n# It follows these events: {}.\n",
            manifest.events.join(", ")
        ));
    }
    if !manifest.views.is_empty() {
        let views: Vec<String> = manifest
            .views
            .iter()
            .map(|view| format!("{}/{}", about.name, view.id))
            .collect();
        text.push_str(&format!("\n# Its Web UI panels: {}.\n", views.join(", ")));
    }
    text
}

/// The plugin directory and the lock that keeps saves apart.
pub struct Store {
    dir: PathBuf,
    saving: Mutex<()>,
    next: AtomicU64,
}

impl Store {
    pub fn new(dir: PathBuf) -> Self {
        Self {
            dir,
            saving: Mutex::new(()),
            next: AtomicU64::new(0),
        }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The directory of the plugin `name`, absolute when it can be made so.
    pub fn plugin_dir(&self, name: &str) -> PathBuf {
        let dir = self.dir.join(name);
        std::path::absolute(&dir).unwrap_or(dir)
    }

    /// A name no other save of this process or another uses.
    fn unique(&self, what: &str, name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|time| time.as_nanos())
            .unwrap_or_default();
        let n = self.next.fetch_add(1, Ordering::Relaxed);
        self.dir
            .join(format!(".{what}-{name}-{}-{nanos}-{n}", std::process::id()))
    }

    /// Writes `plan` as the plugin's directory and returns it with the
    /// content of `mount.toml`.
    pub async fn save(&self, plan: &Plan, program: &Found) -> Result<(PathBuf, String), SaveError> {
        let _saving = self.saving.lock().await;
        let name = &plan.about.name;
        let target = self.dir.join(name);
        let exists = match tokio::fs::symlink_metadata(&target).await {
            Ok(metadata) => {
                if !metadata.is_dir() || !is_file(&target.join(ABOUT_FILE)).await {
                    return Err(SaveError::Refused(format!(
                        "`{}` exists and was not written by plugin-builder; choose another name",
                        target.display()
                    )));
                }
                true
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
            Err(error) => {
                return Err(SaveError::Refused(format!(
                    "cannot read `{}`: {error}",
                    target.display()
                )));
            }
        };
        if exists && !plan.replace {
            return Err(SaveError::Refused(format!(
                "plugin `{name}` already exists; `plugins.read` it and save it with \
                 `replace: true`, or choose another name"
            )));
        }
        let failed = |what: &str, path: &Path, error: std::io::Error| {
            SaveError::Failed(format!("cannot {what} `{}`: {error}", path.display()))
        };
        tokio::fs::create_dir_all(&self.dir)
            .await
            .map_err(|error| failed("create", &self.dir, error))?;
        let dir = self.plugin_dir(name);
        let mount = mount_toml(&plan.about, &plan.manifest, &dir, program);
        let staging = self.unique("staging", name);
        if let Err(error) = self.write_staging(&staging, plan, &mount).await {
            let _ = tokio::fs::remove_dir_all(&staging).await;
            return Err(error);
        }
        if exists {
            let old = self.unique("old", name);
            if let Err(error) = tokio::fs::rename(&target, &old).await {
                let _ = tokio::fs::remove_dir_all(&staging).await;
                return Err(failed("move aside", &target, error));
            }
            if let Err(error) = tokio::fs::rename(&staging, &target).await {
                // Put the old plugin back; the save did not happen.
                let _ = tokio::fs::rename(&old, &target).await;
                let _ = tokio::fs::remove_dir_all(&staging).await;
                return Err(failed("move into place", &target, error));
            }
            let _ = tokio::fs::remove_dir_all(&old).await;
        } else if let Err(error) = tokio::fs::rename(&staging, &target).await {
            let _ = tokio::fs::remove_dir_all(&staging).await;
            return Err(failed("move into place", &target, error));
        }
        Ok((dir, mount))
    }

    async fn write_staging(
        &self,
        staging: &Path,
        plan: &Plan,
        mount: &str,
    ) -> Result<(), SaveError> {
        let json = |value: &Value| serde_json::to_string_pretty(value).unwrap_or_default() + "\n";
        let about = serde_json::to_value(&plan.about).unwrap_or_default();
        let (library, library_source) = plan.about.runtime.library();
        let mut files: Vec<(&str, String)> = plan
            .files
            .iter()
            .map(|(path, content)| (path.as_str(), content.clone()))
            .collect();
        files.push((MANIFEST_FILE, json(&plan.manifest.to_file())));
        files.push((ABOUT_FILE, json(&about)));
        files.push((MOUNT_FILE, mount.to_owned()));
        files.push((library, library_source.to_owned()));
        for (path, content) in files {
            let full = staging.join(path);
            if let Some(parent) = full.parent() {
                tokio::fs::create_dir_all(parent).await.map_err(|error| {
                    SaveError::Failed(format!("cannot create `{}`: {error}", parent.display()))
                })?;
            }
            tokio::fs::write(&full, content).await.map_err(|error| {
                SaveError::Failed(format!("cannot write `{}`: {error}", full.display()))
            })?;
        }
        Ok(())
    }

    /// The saved plugins, sorted by name. Directories that are not plugins
    /// of the builder, or that cannot be read, are left out.
    pub async fn list(&self) -> Vec<Saved> {
        let Ok(mut entries) = tokio::fs::read_dir(&self.dir).await else {
            return Vec::new();
        };
        let mut saved = Vec::new();
        while let Ok(Some(entry)) = entries.next_entry().await {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') || crate::manifest::check_name(&name).is_err() {
                continue;
            }
            if let Ok(plugin) = self.load(&name).await {
                saved.push(plugin);
            }
            if saved.len() >= MAX_LISTED {
                break;
            }
        }
        saved.sort_by(|a, b| a.about.name.cmp(&b.about.name));
        saved
    }

    /// The saved plugin `name`.
    pub async fn load(&self, name: &str) -> Result<Saved, String> {
        let dir = self.dir.join(name);
        let read_json = |file: &'static str| {
            let path = dir.join(file);
            async move {
                let bytes = tokio::fs::read(&path)
                    .await
                    .map_err(|error| format!("cannot read `{}`: {error}", path.display()))?;
                serde_json::from_slice::<Value>(&bytes)
                    .map_err(|error| format!("`{}` is not valid JSON: {error}", path.display()))
            }
        };
        let about: About = serde_json::from_value(read_json(ABOUT_FILE).await?)
            .map_err(|error| format!("invalid {ABOUT_FILE}: {error}"))?;
        if about.name != name {
            return Err(format!("{ABOUT_FILE} names `{}`, not `{name}`", about.name));
        }
        let manifest = Manifest::from_file(read_json(MANIFEST_FILE).await?)
            .map_err(|error| format!("invalid {MANIFEST_FILE}: {error}"))?;
        Ok(Saved {
            about,
            manifest,
            dir: self.plugin_dir(name),
        })
    }

    /// The model's files of the plugin `name`: everything but what the
    /// builder writes, as relative `/`-separated paths and UTF-8 contents.
    /// Files that are not UTF-8 or past [`MAX_READ_BYTES`] are named in the
    /// second list instead.
    pub async fn files(&self, name: &str) -> Result<(Vec<(String, String)>, Vec<String>), String> {
        let root = self.dir.join(name);
        let mut files = Vec::new();
        let mut skipped = Vec::new();
        let mut total = 0;
        let mut pending = vec![(root.clone(), String::new(), 0)];
        while let Some((dir, prefix, depth)) = pending.pop() {
            let mut entries = tokio::fs::read_dir(&dir)
                .await
                .map_err(|error| format!("cannot read `{}`: {error}", dir.display()))?;
            while let Ok(Some(entry)) = entries.next_entry().await {
                let file_name = entry.file_name().to_string_lossy().into_owned();
                let path = if prefix.is_empty() {
                    file_name.clone()
                } else {
                    format!("{prefix}/{file_name}")
                };
                if prefix.is_empty() && RESERVED_FILES.contains(&file_name.as_str()) {
                    continue;
                }
                let Ok(metadata) = tokio::fs::symlink_metadata(entry.path()).await else {
                    continue;
                };
                if metadata.is_dir() && depth + 1 < MAX_PATH_DEPTH {
                    pending.push((entry.path(), path, depth + 1));
                } else if metadata.is_file() {
                    let content = tokio::fs::read(entry.path()).await.ok();
                    match content.map(String::from_utf8) {
                        Some(Ok(text)) if total + text.len() <= MAX_READ_BYTES => {
                            total += text.len();
                            files.push((path, text));
                        }
                        _ => skipped.push(path),
                    }
                } else {
                    skipped.push(path);
                }
            }
        }
        files.sort();
        skipped.sort();
        Ok((files, skipped))
    }
}

impl Store {
    /// The saved plugin `name` in the form `plugins.save` takes, and the
    /// files left out of it (see [`Store::files`]).
    pub async fn entry(&self, name: &str) -> Result<(Value, Vec<String>), String> {
        let saved = self.load(name).await?;
        let (files, skipped) = self.files(name).await?;
        let entry = serde_json::json!({
            "name": saved.about.name,
            "description": saved.about.description,
            "runtime": saved.about.runtime,
            "manifest": saved.manifest,
            "files": files.iter()
                .map(|(path, content)| serde_json::json!({ "path": path, "content": content }))
                .collect::<Vec<_>>(),
        });
        Ok((entry, skipped))
    }

    /// Where a bundle named `file` goes: an absolute path as it is, a
    /// relative one under the plugin directory, and nothing under
    /// `exports/` with the time in its name.
    pub fn bundle_path(&self, file: &str) -> PathBuf {
        let file = file.trim();
        let path = if file.is_empty() {
            self.dir
                .join("exports")
                .join(format!("personal-plugins-{}.json", unix_seconds()))
        } else if Path::new(file).is_absolute() {
            PathBuf::from(file)
        } else {
            self.dir.join(file)
        };
        std::path::absolute(&path).unwrap_or(path)
    }

    /// Writes a bundle of `entries` to the new file `path`; an existing file
    /// is never overwritten.
    pub async fn write_bundle(&self, path: &Path, entries: Vec<Value>) -> Result<(), String> {
        let bundle = serde_json::json!({
            "format": BUNDLE_FORMAT,
            "version": BUNDLE_VERSION,
            "exported_at": unix_seconds(),
            "plugins": entries,
        });
        let text = serde_json::to_string_pretty(&bundle).unwrap_or_default() + "\n";
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|error| format!("cannot create `{}`: {error}", parent.display()))?;
        }
        let mut file = tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .await
            .map_err(|error| match error.kind() {
                std::io::ErrorKind::AlreadyExists => {
                    format!("`{}` exists; choose another file", path.display())
                }
                _ => format!("cannot create `{}`: {error}", path.display()),
            })?;
        tokio::io::AsyncWriteExt::write_all(&mut file, text.as_bytes())
            .await
            .map_err(|error| format!("cannot write `{}`: {error}", path.display()))
    }

    /// The plugin entries of the bundle at `path`, unchecked: each is
    /// `plugins.save` input and must pass the same checks.
    pub async fn read_bundle(path: &Path) -> Result<Vec<Value>, String> {
        let metadata = tokio::fs::metadata(path)
            .await
            .map_err(|error| format!("cannot read `{}`: {error}", path.display()))?;
        if metadata.len() > MAX_BUNDLE_BYTES {
            return Err(format!(
                "`{}` is larger than {MAX_BUNDLE_BYTES} bytes",
                path.display()
            ));
        }
        let bytes = tokio::fs::read(path)
            .await
            .map_err(|error| format!("cannot read `{}`: {error}", path.display()))?;
        let bundle: Value = serde_json::from_slice(&bytes)
            .map_err(|error| format!("`{}` is not valid JSON: {error}", path.display()))?;
        if bundle.get("format").and_then(Value::as_str) != Some(BUNDLE_FORMAT) {
            return Err(format!(
                "`{}` is not a personal plugin bundle",
                path.display()
            ));
        }
        if bundle.get("version").and_then(Value::as_u64) != Some(BUNDLE_VERSION) {
            return Err(format!(
                "`{}` is a bundle of another version; this builder reads version {BUNDLE_VERSION}",
                path.display()
            ));
        }
        match bundle.get("plugins") {
            Some(Value::Array(plugins)) if plugins.len() <= MAX_BUNDLE_PLUGINS => {
                Ok(plugins.clone())
            }
            Some(Value::Array(_)) => Err(format!(
                "a bundle holds at most {MAX_BUNDLE_PLUGINS} plugins"
            )),
            _ => Err("the bundle has no `plugins` list".into()),
        }
    }
}

async fn is_file(path: &Path) -> bool {
    tokio::fs::symlink_metadata(path)
        .await
        .is_ok_and(|metadata| metadata.is_file())
}
