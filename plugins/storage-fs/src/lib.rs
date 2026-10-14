//! `storage-fs`: provides the `storage` service (ADR 0041) as one file per
//! entry, `<dir>/<namespace>/<key>`.
//!
//! File names encode the names: lowercase letters, digits, `_` and `-` stay
//! as they are, every other byte becomes `%` and two lowercase hex digits,
//! and a Windows device name (`con`, `nul`, `com1`, ...) has its first letter
//! encoded. So every name maps to exactly one lowercase file name, which
//! works on case-insensitive file systems and can be read back by `list`.
//! A name whose file name would be longer than [`MAX_FILE_NAME`] is refused.
//!
//! `put` writes a temporary file (`.tmp-*`, flushed to disk), then renames it
//! over the entry, so a reader sees the old or the new value. Temporary
//! names carry the process id and a random per-store tag and are created
//! exclusively, so stores sharing a directory (in one process or several)
//! never write into each other's files; concurrent `put`s to one key leave
//! one whole value. There is no compare-and-swap: a `get` then `put` is not
//! atomic. Temporary files older than an hour, left by a crash, are removed
//! when the plugin starts.
//!
//! Configuration (all optional): `{ "dir": ".rah/storage",
//! "max_value_bytes": 16777216 }`.

use std::{
    hash::{BuildHasher, Hasher, RandomState},
    io::ErrorKind,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use rah_api::{
    Plugin, PluginError, PluginId, PluginManifest, Scope, async_trait,
    storage::{KvStore, STORAGE, StorageError, check_name, check_prefix},
};
use serde::Deserialize;
use tokio::io::AsyncWriteExt;

const DEFAULT_DIR: &str = ".rah/storage";
const DEFAULT_MAX_VALUE_BYTES: usize = 16 * 1024 * 1024;
/// The longest encoded file name.
pub const MAX_FILE_NAME: usize = 240;
const TEMP_PREFIX: &str = ".tmp-";
/// Temporary files older than this are left over from a crash.
const STALE_TEMP: Duration = Duration::from_secs(3600);

pub struct StorageFsPlugin {
    manifest: PluginManifest,
}

impl StorageFsPlugin {
    pub fn new(id: impl Into<PluginId>) -> Self {
        Self {
            manifest: PluginManifest::new(id, env!("CARGO_PKG_VERSION")).provides(["storage"]),
        }
    }
}

/// The plugin configuration.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StorageFsConfig {
    pub dir: Option<PathBuf>,
    pub max_value_bytes: Option<usize>,
}

#[async_trait]
impl Plugin for StorageFsPlugin {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    async fn apply(&self, scope: &mut Scope) -> Result<(), PluginError> {
        let config: StorageFsConfig = if scope.config().is_null() {
            StorageFsConfig::default()
        } else {
            scope.config_as()?
        };
        let dir = config.dir.unwrap_or_else(|| PathBuf::from(DEFAULT_DIR));
        tokio::fs::create_dir_all(&dir)
            .await
            .map_err(|error| format!("storage directory `{}`: {error}", dir.display()))?;
        remove_stale_temps(&dir).await;
        let store = FsStore::new(
            dir,
            config.max_value_bytes.unwrap_or(DEFAULT_MAX_VALUE_BYTES),
        );
        scope.provide(STORAGE, Arc::new(store))?;
        Ok(())
    }
}

/// A key-value store in a directory.
pub struct FsStore {
    dir: PathBuf,
    max_value_bytes: usize,
    /// Random per store, so two stores on one directory in one process
    /// (two instances, or a restart while the old one still writes) do not
    /// pick the same temporary names.
    temp_tag: u64,
    next_temp: AtomicU64,
}

impl FsStore {
    pub fn new(dir: PathBuf, max_value_bytes: usize) -> Self {
        Self {
            dir,
            max_value_bytes,
            temp_tag: RandomState::new().build_hasher().finish(),
            next_temp: AtomicU64::new(0),
        }
    }

    /// Creates a new temporary file in `parent`. It never opens an existing
    /// one, so a writer only ever writes, renames or removes its own file.
    async fn create_temp(&self, parent: &Path) -> std::io::Result<(PathBuf, tokio::fs::File)> {
        let mut attempts = 0;
        loop {
            let temp = parent.join(format!(
                "{TEMP_PREFIX}{}-{:016x}-{}",
                std::process::id(),
                self.temp_tag,
                self.next_temp.fetch_add(1, Ordering::Relaxed)
            ));
            match tokio::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp)
                .await
            {
                Ok(file) => return Ok((temp, file)),
                Err(error) if error.kind() == ErrorKind::AlreadyExists && attempts < 16 => {
                    attempts += 1;
                }
                Err(error) => return Err(error),
            }
        }
    }

    fn namespace_dir(&self, namespace: &str) -> Result<PathBuf, StorageError> {
        check_name(namespace)?;
        Ok(self.dir.join(file_name(namespace)?))
    }

    fn entry(&self, namespace: &str, key: &str) -> Result<PathBuf, StorageError> {
        check_name(key)?;
        Ok(self.namespace_dir(namespace)?.join(file_name(key)?))
    }
}

fn io(error: std::io::Error) -> StorageError {
    StorageError::Io(error.to_string())
}

#[async_trait]
impl KvStore for FsStore {
    async fn get(&self, namespace: &str, key: &str) -> Result<Option<Vec<u8>>, StorageError> {
        match tokio::fs::read(self.entry(namespace, key)?).await {
            Ok(bytes) => Ok(Some(bytes)),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
            Err(error) => Err(io(error)),
        }
    }

    async fn put(&self, namespace: &str, key: &str, value: &[u8]) -> Result<(), StorageError> {
        if value.len() > self.max_value_bytes {
            return Err(StorageError::TooLarge {
                size: value.len(),
                limit: self.max_value_bytes,
            });
        }
        let path = self.entry(namespace, key)?;
        let Some(parent) = path.parent() else {
            return Err(StorageError::Io("the entry has no directory".into()));
        };
        tokio::fs::create_dir_all(parent).await.map_err(io)?;
        let (temp, mut file) = self.create_temp(parent).await.map_err(io)?;
        let written = async {
            file.write_all(value).await?;
            file.sync_all().await?;
            drop(file);
            tokio::fs::rename(&temp, &path).await
        }
        .await;
        if let Err(error) = written {
            let _ = tokio::fs::remove_file(&temp).await;
            return Err(io(error));
        }
        Ok(())
    }

    async fn delete(&self, namespace: &str, key: &str) -> Result<bool, StorageError> {
        match tokio::fs::remove_file(self.entry(namespace, key)?).await {
            Ok(()) => Ok(true),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(false),
            Err(error) => Err(io(error)),
        }
    }

    async fn list(&self, namespace: &str, prefix: &str) -> Result<Vec<String>, StorageError> {
        check_prefix(prefix)?;
        let mut entries = match tokio::fs::read_dir(self.namespace_dir(namespace)?).await {
            Ok(entries) => entries,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(io(error)),
        };
        let mut keys = Vec::new();
        while let Some(entry) = entries.next_entry().await.map_err(io)? {
            let is_file = entry.file_type().await.is_ok_and(|kind| kind.is_file());
            let key = entry.file_name().to_str().and_then(name_of);
            if let (true, Some(key)) = (is_file, key)
                && key.starts_with(prefix)
            {
                keys.push(key);
            }
        }
        keys.sort();
        Ok(keys)
    }
}

const DEVICE_NAMES: [&str; 4] = ["con", "prn", "aux", "nul"];

fn is_device_name(name: &str) -> bool {
    DEVICE_NAMES.contains(&name)
        || (name.len() == 4
            && (name.starts_with("com") || name.starts_with("lpt"))
            && name.as_bytes()[3].is_ascii_digit())
}

fn is_plain(byte: u8) -> bool {
    byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_' || byte == b'-'
}

fn escape(out: &mut String, byte: u8) {
    out.push_str(&format!("%{byte:02x}"));
}

/// The file name of a valid storage name.
pub fn file_name(name: &str) -> Result<String, StorageError> {
    let mut out = String::with_capacity(name.len());
    let device = is_device_name(name);
    for (index, byte) in name.bytes().enumerate() {
        if is_plain(byte) && !(device && index == 0) {
            out.push(char::from(byte));
        } else {
            escape(&mut out, byte);
        }
    }
    if out.len() > MAX_FILE_NAME {
        return Err(StorageError::InvalidName(name.to_owned()));
    }
    Ok(out)
}

/// The storage name of a file name, if it is the encoding of one.
pub fn name_of(file: &str) -> Option<String> {
    let bytes = file.as_bytes();
    let mut name = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let hex = file.get(index + 1..index + 3)?;
            name.push(u8::from_str_radix(hex, 16).ok()?);
            index += 3;
        } else {
            name.push(bytes[index]);
            index += 1;
        }
    }
    let name = String::from_utf8(name).ok()?;
    // Only the one canonical encoding counts.
    (check_name(&name).is_ok() && file_name(&name).ok()? == file).then_some(name)
}

/// Removes temporary files left by a crash in any namespace.
async fn remove_stale_temps(dir: &Path) {
    let Ok(mut namespaces) = tokio::fs::read_dir(dir).await else {
        return;
    };
    while let Ok(Some(namespace)) = namespaces.next_entry().await {
        let Ok(mut files) = tokio::fs::read_dir(namespace.path()).await else {
            continue;
        };
        while let Ok(Some(file)) = files.next_entry().await {
            let is_temp = file
                .file_name()
                .to_str()
                .is_some_and(|name| name.starts_with(TEMP_PREFIX));
            let stale = file
                .metadata()
                .await
                .and_then(|meta| meta.modified())
                .ok()
                .and_then(|modified| modified.elapsed().ok())
                .is_some_and(|age| age > STALE_TEMP);
            if is_temp && stale {
                let _ = tokio::fs::remove_file(file.path()).await;
            }
        }
    }
}

impl StorageFsPlugin {
    /// An example configuration, mostly the defaults, for reference (the Web
    /// UI shows it in an empty configuration editor). It is never applied.
    pub const CONFIG_EXAMPLE: &str = r#"{
  "dir": ".rah/storage",
  "max_value_bytes": 16777216
}"#;
}

#[cfg(test)]
mod tests;

#[cfg(test)]
#[test]
fn the_config_example_is_a_valid_configuration() {
    let parsed: Result<StorageFsConfig, _> = serde_json::from_str(StorageFsPlugin::CONFIG_EXAMPLE);
    assert!(parsed.is_ok(), "{:?}", parsed.err());
}
