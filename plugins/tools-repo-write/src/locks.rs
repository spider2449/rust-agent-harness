//! Locks that keep the write tools of one plugin instance from overwriting
//! each other. They serialize cooperating RAH tool calls (several sessions,
//! background jobs and subagents run turns at once); they do not stop an
//! editor, a build or another process from writing the same file.

use std::{
    collections::{BTreeSet, HashMap},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, PoisonError},
};

use tokio::sync::{Mutex as AsyncMutex, OwnedMutexGuard, OwnedRwLockWriteGuard, RwLock};

/// Entries kept before unused ones are dropped.
const PRUNE_ABOVE: usize = 256;

#[derive(Default)]
pub(crate) struct Locks {
    files: Mutex<HashMap<PathBuf, Arc<AsyncMutex<()>>>>,
    /// File operations hold it shared; an operation that may touch any file
    /// (`git apply`) holds it exclusively.
    everything: Arc<RwLock<()>>,
}

/// Held while files are read and rewritten.
pub(crate) struct FilesGuard {
    _shared: tokio::sync::OwnedRwLockReadGuard<()>,
    _files: Vec<OwnedMutexGuard<()>>,
}

impl Locks {
    /// The lock key of a path: file systems that ignore case share a lock
    /// between spellings.
    fn key(path: &Path) -> PathBuf {
        if cfg!(windows) {
            PathBuf::from(path.to_string_lossy().to_lowercase())
        } else {
            path.to_path_buf()
        }
    }

    /// Waits for every path in `paths`. They are taken in sorted order, so
    /// two calls that name the same files in different orders cannot wait
    /// for each other.
    pub async fn files(&self, paths: &[PathBuf]) -> FilesGuard {
        let shared = self.everything.clone().read_owned().await;
        let keys: BTreeSet<PathBuf> = paths.iter().map(|path| Self::key(path)).collect();
        let locks: Vec<Arc<AsyncMutex<()>>> = {
            let mut files = self.files.lock().unwrap_or_else(PoisonError::into_inner);
            if files.len() > PRUNE_ABOVE {
                files.retain(|_, lock| Arc::strong_count(lock) > 1);
            }
            keys.into_iter()
                .map(|key| files.entry(key).or_default().clone())
                .collect()
        };
        let mut held = Vec::with_capacity(locks.len());
        for lock in locks {
            held.push(lock.lock_owned().await);
        }
        FilesGuard {
            _shared: shared,
            _files: held,
        }
    }

    /// Waits until no file operation is running and keeps new ones out.
    pub async fn all(&self) -> OwnedRwLockWriteGuard<()> {
        self.everything.clone().write_owned().await
    }
}
