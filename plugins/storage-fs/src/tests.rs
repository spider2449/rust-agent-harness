use std::time::SystemTime;

use rah_api::{Kernel, MountOptions, PluginStatus, storage::call_json};
use serde_json::json;

use super::*;

struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("rah-storage-fs-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        Self(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn store(dir: &TempDir) -> FsStore {
    FsStore::new(dir.0.clone(), 64)
}

#[test]
fn file_names_are_lowercase_and_read_back() {
    for (name, file) in [
        ("plain-name_1", "plain-name_1"),
        ("Key", "%4bey"),
        ("a.b:c", "a%2eb%3ac"),
        ("con", "%63on"),
        ("lpt1", "%6cpt1"),
        ("CON", "%43%4f%4e"),
        ("con.txt", "con%2etxt"),
    ] {
        assert_eq!(file_name(name).unwrap(), file, "{name}");
        assert_eq!(name_of(file).as_deref(), Some(name), "{file}");
    }
    // Only the canonical encoding is a name; temporary files are none.
    for file in ["%6bey", "con", "%4Bey", "%4", ".tmp-1-0", "a.b", "%2f"] {
        assert_eq!(name_of(file), None, "{file}");
    }
    assert!(matches!(
        file_name(&"K".repeat(100)),
        Err(StorageError::InvalidName(_))
    ));
}

#[tokio::test]
async fn entries_are_stored_listed_and_deleted() {
    let dir = TempDir::new("crud");
    let store = store(&dir);
    assert_eq!(store.get("ns", "a").await.unwrap(), None);
    assert_eq!(store.list("ns", "").await.unwrap(), Vec::<String>::new());
    store.put("ns", "a", b"one").await.unwrap();
    store.put("ns", "A", b"two").await.unwrap();
    store.put("ns", "a:b", b"three").await.unwrap();
    store.put("other", "a", b"four").await.unwrap();
    store.put("ns", "a", b"five").await.unwrap();
    assert_eq!(store.get("ns", "a").await.unwrap(), Some(b"five".to_vec()));
    assert_eq!(store.get("ns", "A").await.unwrap(), Some(b"two".to_vec()));
    assert_eq!(
        store.get("other", "a").await.unwrap(),
        Some(b"four".to_vec())
    );
    assert_eq!(store.list("ns", "").await.unwrap(), ["A", "a", "a:b"]);
    assert_eq!(store.list("ns", "a").await.unwrap(), ["a", "a:b"]);
    assert!(store.delete("ns", "a").await.unwrap());
    assert!(!store.delete("ns", "a").await.unwrap());
    assert_eq!(store.list("ns", "").await.unwrap(), ["A", "a:b"]);
}

#[tokio::test]
async fn invalid_and_oversized_requests_are_refused() {
    let dir = TempDir::new("refuse");
    let store = store(&dir);
    for (namespace, key) in [("ns", "../x"), ("../x", "k"), ("ns", ""), ("a/b", "k")] {
        assert!(
            matches!(
                store.put(namespace, key, b"").await,
                Err(StorageError::InvalidName(_))
            ),
            "{namespace} {key}"
        );
    }
    assert!(matches!(
        store.list("ns", "a/").await,
        Err(StorageError::InvalidName(_))
    ));
    assert_eq!(
        store.put("ns", "big", &[0; 65]).await,
        Err(StorageError::TooLarge {
            size: 65,
            limit: 64
        })
    );
    assert_eq!(store.get("ns", "big").await.unwrap(), None);
    assert!(!dir.0.join("..").join("x").exists());
}

#[tokio::test]
async fn list_ignores_temporary_and_foreign_files() {
    let dir = TempDir::new("foreign");
    let store = store(&dir);
    store.put("ns", "k", b"v").await.unwrap();
    let namespace = dir.0.join("ns");
    std::fs::write(namespace.join(".tmp-1-0"), b"partial").unwrap();
    std::fs::write(namespace.join("Upper"), b"by hand").unwrap();
    std::fs::create_dir(namespace.join("sub")).unwrap();
    assert_eq!(store.list("ns", "").await.unwrap(), ["k"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn concurrent_puts_leave_one_whole_value() {
    let dir = TempDir::new("race");
    let store = Arc::new(FsStore::new(dir.0.clone(), 1 << 20));
    let writers: Vec<_> = (0..8u8)
        .map(|n| {
            let store = store.clone();
            tokio::spawn(async move {
                for _ in 0..10 {
                    store.put("ns", "k", &vec![n; 4096]).await.unwrap();
                }
            })
        })
        .collect();
    for writer in writers {
        writer.await.unwrap();
    }
    let value = store.get("ns", "k").await.unwrap().unwrap();
    assert_eq!(value.len(), 4096);
    assert!(value.iter().all(|byte| *byte == value[0]));
    assert_eq!(store.list("ns", "").await.unwrap(), ["k"]);
}

/// Two stores on one directory, as two `storage-fs` instances in one
/// process (or one restarting while the old one finishes a write), must not
/// share temporary files.
#[tokio::test(flavor = "multi_thread")]
async fn two_stores_on_one_directory_never_mix_values() {
    let dir = TempDir::new("two-stores");
    let writers: Vec<_> = (0..2u8)
        .map(|n| {
            let store = Arc::new(FsStore::new(dir.0.clone(), 1 << 20));
            tokio::spawn(async move {
                for _ in 0..20 {
                    store.put("ns", "k", &vec![n; 64 * 1024]).await?;
                }
                Ok::<_, StorageError>(())
            })
        })
        .collect();
    for writer in writers {
        writer.await.unwrap().unwrap();
    }
    let store = FsStore::new(dir.0.clone(), 1 << 20);
    let value = store.get("ns", "k").await.unwrap().unwrap();
    assert_eq!(value.len(), 64 * 1024);
    assert!(value.iter().all(|byte| *byte == value[0]));
}

async fn mount(kernel: &Kernel, dir: &TempDir) -> PluginStatus {
    kernel
        .mount(
            Arc::new(StorageFsPlugin::new("storage-fs")),
            MountOptions::with_config(json!({ "dir": dir.0, "max_value_bytes": 1024 })),
        )
        .await
        .unwrap()
}

#[tokio::test]
async fn the_plugin_provides_storage_that_survives_a_restart() {
    let dir = TempDir::new("plugin");
    let kernel = Kernel::new();
    assert_eq!(mount(&kernel, &dir).await, PluginStatus::Active);
    let storage = kernel.handle().get(STORAGE).unwrap();
    call_json(
        storage.as_ref(),
        "my-plugin",
        "put",
        &json!({ "key": "state", "value": { "count": 3 } }),
    )
    .await
    .unwrap();
    kernel.shutdown().await.unwrap();

    let kernel = Kernel::new();
    assert_eq!(mount(&kernel, &dir).await, PluginStatus::Active);
    let storage = kernel.handle().get(STORAGE).unwrap();
    assert_eq!(
        call_json(
            storage.as_ref(),
            "my-plugin",
            "get",
            &json!({ "key": "state" })
        )
        .await
        .unwrap(),
        json!({ "value": { "count": 3 } })
    );
    assert!(matches!(
        storage.put("my-plugin", "big", &[0; 1025]).await,
        Err(StorageError::TooLarge { .. })
    ));
}

#[tokio::test]
async fn stale_temporary_files_are_removed_at_start() {
    let dir = TempDir::new("stale");
    let namespace = dir.0.join("ns");
    std::fs::create_dir_all(&namespace).unwrap();
    let stale = namespace.join(".tmp-1-0");
    let fresh = namespace.join(".tmp-1-1");
    std::fs::write(&stale, b"old").unwrap();
    std::fs::write(&fresh, b"new").unwrap();
    std::fs::File::options()
        .write(true)
        .open(&stale)
        .unwrap()
        .set_modified(SystemTime::now() - Duration::from_secs(7200))
        .unwrap();
    let kernel = Kernel::new();
    assert_eq!(mount(&kernel, &dir).await, PluginStatus::Active);
    assert!(!stale.exists());
    assert!(fresh.exists());
}

#[tokio::test]
async fn unknown_configuration_fails_the_mount() {
    let kernel = Kernel::new();
    let status = kernel
        .mount(
            Arc::new(StorageFsPlugin::new("storage-fs")),
            MountOptions::with_config(json!({ "path": "x" })),
        )
        .await
        .unwrap();
    assert!(matches!(status, PluginStatus::Failed { .. }), "{status:?}");
}
