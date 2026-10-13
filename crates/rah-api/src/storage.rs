//! The `storage` service: a namespaced key-value store for plugin state that
//! must survive a restart (ADR 0041).
//!
//! Every plugin keeps its entries in its own namespace, by convention its
//! plugin id. In-process plugins are trusted and name their namespace;
//! process and WASM plugins reach the store only through their host, which
//! fixes the namespace to their plugin id (see [`call_json`]).
//!
//! Namespaces and keys are 1 to [`MAX_NAME_BYTES`] bytes of ASCII letters,
//! digits and `.`, `_`, `:`, `-` (the characters of a plugin id). Keys are
//! case-sensitive. A provider may refuse a name it cannot store with
//! [`StorageError::InvalidName`].

use std::{
    collections::BTreeMap,
    sync::{Mutex, PoisonError},
};

use async_trait::async_trait;
use rah_kernel::ServiceKey;
use serde_json::{Value, json};

pub const STORAGE: ServiceKey<dyn KvStore> = ServiceKey::new("storage");

/// The longest namespace or key, in bytes.
pub const MAX_NAME_BYTES: usize = 128;

#[async_trait]
pub trait KvStore: Send + Sync {
    /// The value of `key`, or `None` if it was never stored or was deleted.
    async fn get(&self, namespace: &str, key: &str) -> Result<Option<Vec<u8>>, StorageError>;

    /// Stores `value` under `key`, replacing any earlier value. A reader sees
    /// the old or the new value, never a mix of both.
    async fn put(&self, namespace: &str, key: &str, value: &[u8]) -> Result<(), StorageError>;

    /// Removes `key`. Returns whether it existed.
    async fn delete(&self, namespace: &str, key: &str) -> Result<bool, StorageError>;

    /// The keys of `namespace` that start with `prefix`, sorted.
    async fn list(&self, namespace: &str, prefix: &str) -> Result<Vec<String>, StorageError>;
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum StorageError {
    #[error("invalid storage name `{0}`")]
    InvalidName(String),
    #[error("the value has {size} bytes; this store keeps at most {limit}")]
    TooLarge { size: usize, limit: usize },
    #[error("unknown storage method `{0}`")]
    UnknownMethod(String),
    #[error("invalid storage parameters: {0}")]
    InvalidParams(String),
    #[error("storage failed: {0}")]
    Io(String),
}

fn is_name_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || b"._:-".contains(&byte)
}

/// Checks a namespace or key against the contract.
pub fn check_name(name: &str) -> Result<(), StorageError> {
    if name.is_empty() || name.len() > MAX_NAME_BYTES || !name.bytes().all(is_name_byte) {
        return Err(StorageError::InvalidName(name.to_owned()));
    }
    Ok(())
}

/// Checks a `list` prefix: empty, or the start of a valid name.
pub fn check_prefix(prefix: &str) -> Result<(), StorageError> {
    if prefix.len() > MAX_NAME_BYTES || !prefix.bytes().all(is_name_byte) {
        return Err(StorageError::InvalidName(prefix.to_owned()));
    }
    Ok(())
}

/// A store kept in memory, lost when dropped: for tests and for plugins
/// that want the contract without persistence.
#[derive(Debug, Default)]
pub struct MemoryStore {
    entries: Mutex<BTreeMap<(String, String), Vec<u8>>>,
}

impl MemoryStore {
    pub fn new() -> Self {
        Self::default()
    }

    fn entries(&self) -> std::sync::MutexGuard<'_, BTreeMap<(String, String), Vec<u8>>> {
        self.entries.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[async_trait]
impl KvStore for MemoryStore {
    async fn get(&self, namespace: &str, key: &str) -> Result<Option<Vec<u8>>, StorageError> {
        check_name(namespace)?;
        check_name(key)?;
        Ok(self
            .entries()
            .get(&(namespace.to_owned(), key.to_owned()))
            .cloned())
    }

    async fn put(&self, namespace: &str, key: &str, value: &[u8]) -> Result<(), StorageError> {
        check_name(namespace)?;
        check_name(key)?;
        self.entries()
            .insert((namespace.to_owned(), key.to_owned()), value.to_vec());
        Ok(())
    }

    async fn delete(&self, namespace: &str, key: &str) -> Result<bool, StorageError> {
        check_name(namespace)?;
        check_name(key)?;
        Ok(self
            .entries()
            .remove(&(namespace.to_owned(), key.to_owned()))
            .is_some())
    }

    async fn list(&self, namespace: &str, prefix: &str) -> Result<Vec<String>, StorageError> {
        check_name(namespace)?;
        check_prefix(prefix)?;
        Ok(self
            .entries()
            .keys()
            .filter(|(space, key)| space == namespace && key.starts_with(prefix))
            .map(|(_, key)| key.clone())
            .collect())
    }
}

/// The `storage` service as process and WASM hosts expose it, with JSON
/// values in one fixed `namespace`:
///
/// - `get {key}` returns `{value}`, `null` when the key is not stored;
/// - `put {key, value}` stores any JSON value and returns `{}`;
/// - `delete {key}` returns `{deleted}`;
/// - `list {prefix?}` returns `{keys}`.
///
/// A stored value that is not JSON (written by an in-process plugin into the
/// same namespace) is an [`StorageError::Io`] error.
pub async fn call_json(
    store: &dyn KvStore,
    namespace: &str,
    method: &str,
    params: &Value,
) -> Result<Value, StorageError> {
    let text = |name: &str| -> Result<&str, StorageError> {
        params
            .get(name)
            .and_then(Value::as_str)
            .ok_or_else(|| StorageError::InvalidParams(format!("expected a string `{name}`")))
    };
    match method {
        "get" => match store.get(namespace, text("key")?).await? {
            None => Ok(json!({ "value": null })),
            Some(bytes) => {
                let value: Value = serde_json::from_slice(&bytes).map_err(|error| {
                    StorageError::Io(format!("the stored value is not JSON: {error}"))
                })?;
                Ok(json!({ "value": value }))
            }
        },
        "put" => {
            let key = text("key")?;
            let value = params
                .get("value")
                .ok_or_else(|| StorageError::InvalidParams("expected a `value`".into()))?;
            let bytes =
                serde_json::to_vec(value).map_err(|error| StorageError::Io(error.to_string()))?;
            store.put(namespace, key, &bytes).await?;
            Ok(json!({}))
        }
        "delete" => {
            let deleted = store.delete(namespace, text("key")?).await?;
            Ok(json!({ "deleted": deleted }))
        }
        "list" => {
            let prefix = match params.get("prefix") {
                None | Some(Value::Null) => "",
                Some(_) => text("prefix")?,
            };
            let keys = store.list(namespace, prefix).await?;
            Ok(json!({ "keys": keys }))
        }
        other => Err(StorageError::UnknownMethod(other.to_owned())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_follow_the_plugin_id_alphabet() {
        for valid in [
            "a",
            "context-compact",
            "x.y_z:1",
            &"k".repeat(MAX_NAME_BYTES),
        ] {
            assert_eq!(check_name(valid), Ok(()), "{valid}");
        }
        for invalid in [
            "",
            "a/b",
            "..\\x",
            "a b",
            "ü",
            &"k".repeat(MAX_NAME_BYTES + 1),
        ] {
            assert!(check_name(invalid).is_err(), "{invalid}");
        }
        assert_eq!(check_prefix(""), Ok(()));
        assert!(check_prefix("a/").is_err());
    }

    #[tokio::test]
    async fn the_memory_store_keeps_namespaces_apart() {
        let store = MemoryStore::new();
        store.put("one", "k", b"1").await.unwrap();
        store.put("two", "k", b"2").await.unwrap();
        store.put("one", "k2", b"3").await.unwrap();
        assert_eq!(store.get("one", "k").await.unwrap(), Some(b"1".to_vec()));
        assert_eq!(store.get("two", "k").await.unwrap(), Some(b"2".to_vec()));
        assert_eq!(store.list("one", "").await.unwrap(), ["k", "k2"]);
        assert_eq!(store.list("one", "k2").await.unwrap(), ["k2"]);
        assert!(store.delete("one", "k").await.unwrap());
        assert!(!store.delete("one", "k").await.unwrap());
        assert_eq!(store.get("one", "k").await.unwrap(), None);
        assert!(matches!(
            store.put("one", "../x", b"").await,
            Err(StorageError::InvalidName(_))
        ));
    }

    #[tokio::test]
    async fn json_calls_store_json_values() {
        let store = MemoryStore::new();
        let call = |method: &'static str, params: Value| {
            let store = &store;
            async move { call_json(store, "plugin", method, &params).await }
        };
        assert_eq!(
            call("get", json!({ "key": "a" })).await,
            Ok(json!({ "value": null }))
        );
        assert_eq!(
            call("put", json!({ "key": "a", "value": { "n": [1, 2] } })).await,
            Ok(json!({}))
        );
        assert_eq!(
            call("get", json!({ "key": "a" })).await,
            Ok(json!({ "value": { "n": [1, 2] } }))
        );
        assert_eq!(call("list", json!({})).await, Ok(json!({ "keys": ["a"] })));
        assert_eq!(
            call("delete", json!({ "key": "a" })).await,
            Ok(json!({ "deleted": true }))
        );
        assert!(matches!(
            call("put", json!({ "key": "a" })).await,
            Err(StorageError::InvalidParams(_))
        ));
        assert!(matches!(
            call("get", json!({ "key": "a/b" })).await,
            Err(StorageError::InvalidName(_))
        ));
        assert!(matches!(
            call("drop", json!({})).await,
            Err(StorageError::UnknownMethod(_))
        ));
        store.put("plugin", "raw", b"\xff").await.unwrap();
        assert!(matches!(
            call("get", json!({ "key": "raw" })).await,
            Err(StorageError::Io(_))
        ));
    }
}
