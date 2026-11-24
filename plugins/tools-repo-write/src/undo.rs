//! The undo history (ADR 0060): the content of each file before and after
//! every write call, per session, in memory. The user undoes the newest
//! change of a session through the `file-undo` service; it is written back
//! only when every file is still what the change left.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard, PoisonError},
    time::{SystemTime, UNIX_EPOCH},
};

use rah_api::{
    async_trait,
    undo::{FileChange, FileUndo, FormatClaim, UndoError, UndoReport},
};
use rah_protocol::{RequestId, SessionEntry, SessionId, SessionRecord, ToolCallId};

use crate::fsops::Root;
use crate::locks::Locks;

/// Changes kept per session; older ones are forgotten.
const MAX_CHANGES_PER_SESSION: usize = 50;
/// File content kept in all, oldest changes forgotten first.
const MAX_BYTES: usize = 64 * 1024 * 1024;
/// Notes kept per session until its next turn.
const MAX_NOTES: usize = 20;

/// The call a write belongs to.
pub(crate) struct Call<'a> {
    pub session: &'a SessionId,
    pub id: &'a ToolCallId,
    pub tool: &'static str,
}

/// One file of a change. `None` means there was no file.
pub(crate) struct FileState {
    pub path: String,
    pub before: Option<FileImage>,
    pub after: Option<Vec<u8>>,
}

/// Bytes and ordinary Unix access bits saved before replacing a file.
#[derive(Clone)]
pub(crate) struct FileImage {
    pub bytes: Vec<u8>,
    #[cfg(unix)]
    pub mode: u32,
}

impl FileImage {
    pub async fn capture(_path: &Path, bytes: Vec<u8>) -> std::io::Result<Self> {
        Ok(Self {
            bytes,
            #[cfg(unix)]
            mode: {
                use std::os::unix::fs::PermissionsExt;
                tokio::fs::metadata(_path).await?.permissions().mode() & 0o777
            },
        })
    }

    pub async fn read(path: &Path, limit: usize) -> Result<Option<Self>, ()> {
        match read_state(path, limit).await? {
            Some(bytes) => Self::capture(path, bytes).await.map(Some).map_err(drop),
            None => Ok(None),
        }
    }
}

impl FileState {
    fn bytes(&self) -> usize {
        self.before.as_ref().map_or(0, |image| image.bytes.len())
            + self.after.as_ref().map_or(0, Vec::len)
    }
}

#[derive(Clone)]
struct RecordedTarget {
    key: PathBuf,
    #[cfg(unix)]
    mode: Option<u32>,
}

#[cfg(unix)]
async fn access_mode(path: &Path) -> std::io::Result<Option<u32>> {
    use std::os::unix::fs::PermissionsExt;
    match tokio::fs::metadata(path).await {
        Ok(metadata) => Ok(Some(metadata.permissions().mode() & 0o777)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

impl RecordedTarget {
    async fn matches(&self, path: &Path) -> bool {
        #[cfg(unix)]
        if access_mode(path).await.ok() != Some(self.mode) {
            return false;
        }
        self.key == Locks::key(path)
    }
}

struct Entry {
    session: SessionId,
    call: ToolCallId,
    tool: &'static str,
    at_ms: u64,
    files: Vec<FileState>,
    /// Resolved file identities in the same order as the snapshots.
    targets: Vec<RecordedTarget>,
    superseded: bool,
}

impl Entry {
    fn bytes(&self) -> usize {
        self.files.iter().map(FileState::bytes).sum()
    }

    fn change(&self) -> FileChange {
        FileChange {
            tool: self.tool.to_owned(),
            files: self.files.iter().map(|file| file.path.clone()).collect(),
            at_ms: self.at_ms,
        }
    }
}

#[derive(Default)]
struct State {
    /// Oldest first.
    entries: Vec<Entry>,
    bytes: usize,
    notes: HashMap<SessionId, Vec<String>>,
    reserved: HashMap<SessionId, (RequestId, Vec<String>)>,
}

impl State {
    fn remove(&mut self, index: usize) -> Entry {
        let entry = self.entries.remove(index);
        self.bytes -= entry.bytes();
        entry
    }
}

#[derive(Default)]
pub(crate) struct History {
    state: Mutex<State>,
}

/// What a file is now: its content, `None` when there is none, or `Err` when
/// it cannot be kept (a link, a directory, too large or unreadable).
pub(crate) async fn read_state(file: &Path, max_bytes: usize) -> Result<Option<Vec<u8>>, ()> {
    match tokio::fs::symlink_metadata(file).await {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err(()),
        Ok(metadata) if !metadata.is_file() || metadata.len() > max_bytes as u64 => Err(()),
        Ok(_) => tokio::fs::read(file).await.map(Some).map_err(drop),
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis() as u64)
}

impl Root {
    pub(crate) async fn record(&self, call: &Call<'_>, files: Vec<FileState>) {
        let mut targets = Vec::with_capacity(files.len());
        for file in &files {
            let Ok(path) = self.undo_path(&file.path).await else {
                return;
            };
            targets.push(RecordedTarget {
                key: Locks::key(&path),
                #[cfg(unix)]
                mode: match access_mode(&path).await {
                    Ok(mode) => mode,
                    Err(_) => return,
                },
            });
        }
        self.history.record(call, files, targets);
    }
}

impl History {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Keeps a completed call's files, newest last. Called while the call's
    /// file locks are held.
    fn record(&self, call: &Call<'_>, files: Vec<FileState>, targets: Vec<RecordedTarget>) {
        if files.is_empty() {
            return;
        }
        let entry = Entry {
            session: call.session.clone(),
            call: call.id.clone(),
            tool: call.tool,
            at_ms: now_ms(),
            files,
            targets,
            superseded: false,
        };
        let mut state = self.lock();
        for prior in &mut state.entries {
            if !prior.superseded
                && prior
                    .targets
                    .iter()
                    .any(|path| entry.targets.iter().any(|next| next.key == path.key))
            {
                prior.superseded = true;
            }
        }
        state.bytes += entry.bytes();
        state.entries.push(entry);
        let session = call.session;
        if state
            .entries
            .iter()
            .filter(|entry| entry.session == *session)
            .count()
            > MAX_CHANGES_PER_SESSION
            && let Some(oldest) = state
                .entries
                .iter()
                .position(|entry| entry.session == *session)
        {
            state.remove(oldest);
        }
        while state.bytes > MAX_BYTES && !state.entries.is_empty() {
            state.remove(0);
        }
    }

    /// Discards a change whose final bytes cannot be attributed to its call.
    pub async fn refresh(&self, root: &Root, call: &ToolCallId) {
        let (paths, targets): (Vec<String>, _) = {
            let state = self.lock();
            let Some(entry) = state.entries.iter().find(|entry| entry.call == *call) else {
                return;
            };
            (
                entry.files.iter().map(|file| file.path.clone()).collect(),
                entry.targets.clone(),
            )
        };
        let keys: Vec<_> = targets.iter().map(|target| target.key.clone()).collect();
        let _writing = root.locks.files(&keys).await;
        let mut now = Vec::with_capacity(paths.len());
        for (path, target) in paths.into_iter().zip(targets) {
            now.push(match root.undo_path(&path).await {
                Ok(file) if target.matches(&file).await => {
                    read_state(&file, root.max_file_bytes).await
                }
                _ => Err(()),
            });
        }
        let mut state = self.lock();
        let Some(index) = state.entries.iter().position(|entry| entry.call == *call) else {
            return;
        };
        if state.entries[index].superseded
            || state.entries[index]
                .files
                .iter()
                .zip(now)
                .any(|(file, bytes)| bytes.as_ref().ok() != Some(&file.after))
        {
            state.remove(index);
        }
    }

    /// Undoes the newest change of `session` in `root`.
    pub async fn undo_last(
        &self,
        root: &Root,
        session: &SessionId,
    ) -> Result<UndoReport, UndoError> {
        // Taken out first: a failed or refused undo does not leave it at the
        // top, where it would block the older changes.
        let _writing = root.locks.all().await;
        let entry = {
            let mut state = self.lock();
            let index = state
                .entries
                .iter()
                .rposition(|entry| entry.session == *session)
                .ok_or(UndoError::Nothing)?;
            state.remove(index)
        };
        let tool = entry.tool.to_owned();
        let mut resolved = Vec::with_capacity(entry.files.len());
        let mut changed = Vec::new();
        for (file, target) in entry.files.iter().zip(&entry.targets) {
            let Ok(path) = root.undo_path(&file.path).await else {
                changed.push(file.path.clone());
                continue;
            };
            if !target.matches(&path).await
                || read_state(&path, root.max_file_bytes).await.ok().as_ref() != Some(&file.after)
            {
                changed.push(file.path.clone());
            }
            resolved.push(path);
        }
        if !changed.is_empty() {
            return Err(UndoError::Changed {
                tool,
                files: changed,
            });
        }
        let mut report = UndoReport {
            change: entry.change(),
            restored: Vec::new(),
            removed: Vec::new(),
        };
        let mut failure = None;
        for (file, path) in entry.files.iter().zip(&resolved) {
            if root.undo_path(&file.path).await.as_ref().ok() != Some(path) {
                failure = Some(format!("the target of `{}` changed", file.path));
                break;
            }
            let result = match &file.before {
                Some(content) => root.write_back(path, &file.path, content).await,
                None if file.after.is_some() => tokio::fs::remove_file(path)
                    .await
                    .map_err(|error| format!("cannot delete `{}`: {error}", file.path)),
                // Missing before and after: nothing to do.
                None => continue,
            };
            if let Err(message) = result {
                failure = Some(message);
                break;
            }
            if file.before.is_some() {
                report.restored.push(file.path.clone());
            } else {
                report.removed.push(file.path.clone());
            }
        }
        let done = !report.restored.is_empty() || !report.removed.is_empty();
        // The model is told about whatever was written back, also when a
        // later file failed.
        if done {
            let mut state = self.lock();
            let notes = state.notes.entry(session.clone()).or_default();
            notes.push(report.note());
            if notes.len() > MAX_NOTES {
                notes.remove(0);
            }
        }
        match failure {
            None => Ok(report),
            Some(message) if done => Err(UndoError::Failed {
                tool,
                message: format!("{message}; already done: {}", report.summary()),
            }),
            Some(message) => Err(UndoError::Failed { tool, message }),
        }
    }
}

/// The `file-undo` service of one plugin instance.
pub struct RepoUndo(pub(crate) Arc<Root>);

#[async_trait]
impl FileUndo for RepoUndo {
    async fn begin_format(
        &self,
        call: &ToolCallId,
        relative: &str,
        canonical: &Path,
    ) -> FormatClaim {
        let root = self.0.clone();
        if !root.history.lock().entries.iter().any(|entry| {
            entry.call == *call && entry.files.iter().any(|file| file.path == relative)
        }) {
            return FormatClaim::Untracked;
        }
        let guard = root
            .locks
            .files(&[canonical.to_owned(), root.path.join(relative)])
            .await;
        if root.undo_path(relative).await.ok().as_deref() != Some(canonical) {
            return FormatClaim::Changed;
        }
        let Ok(Some(bytes)) = read_state(canonical, root.max_file_bytes).await else {
            return FormatClaim::Changed;
        };
        let owns_bytes = root
            .history
            .lock()
            .entries
            .iter()
            .find(|entry| entry.call == *call && !entry.superseded)
            .is_some_and(|entry| {
                entry
                    .files
                    .iter()
                    .zip(&entry.targets)
                    .any(|(file, target)| {
                        file.path == relative
                            && target.key == Locks::key(canonical)
                            && file.after.as_deref() == Some(&bytes)
                    })
            });
        if !owns_bytes {
            return FormatClaim::Changed;
        }
        let call = call.clone();
        let relative = relative.to_owned();
        FormatClaim::Tracked(Box::new(move |before, after| {
            let _guard = guard;
            let mut state = root.history.lock();
            let Some(index) = state.entries.iter().position(|entry| entry.call == call) else {
                return;
            };
            let Some(file) = state.entries[index]
                .files
                .iter_mut()
                .find(|file| file.path == relative)
            else {
                return;
            };
            if file.after.as_deref() != Some(before) || after.len() > root.max_file_bytes {
                state.remove(index);
                return;
            }
            let size = after.len();
            file.after = Some(after);
            state.bytes = state.bytes - before.len() + size;
            while state.bytes > MAX_BYTES {
                state.remove(0);
            }
        }))
    }

    fn changes(&self, session: &SessionId) -> Vec<FileChange> {
        self.0
            .history
            .lock()
            .entries
            .iter()
            .rev()
            .filter(|entry| entry.session == *session)
            .map(Entry::change)
            .collect()
    }

    async fn undo_last(&self, session: &SessionId) -> Result<UndoReport, UndoError> {
        self.0.history.undo_last(&self.0, session).await
    }

    fn reserve_notes(
        &self,
        session: &SessionId,
        turn: &RequestId,
        history: &[SessionRecord],
    ) -> Vec<String> {
        let mut state = self.0.history.lock();
        let mut batch = Vec::new();
        if let Some((previous, notes)) = state.reserved.remove(session) {
            let recorded = history.iter().any(|record| {
                matches!(
                    &record.entry, SessionEntry::TurnStarted { turn, .. } if *turn == previous
                )
            });
            if !recorded {
                batch = notes;
            }
        }
        batch.extend(state.notes.remove(session).unwrap_or_default());
        if batch.len() > MAX_NOTES {
            batch.drain(..batch.len() - MAX_NOTES);
        }
        if !batch.is_empty() {
            state
                .reserved
                .insert(session.clone(), (turn.clone(), batch.clone()));
        }
        batch
    }

    fn acknowledge_notes(&self, session: &SessionId, turn: &RequestId) {
        let mut state = self.0.history.lock();
        if state
            .reserved
            .get(session)
            .is_some_and(|(id, _)| id == turn)
        {
            state.reserved.remove(session);
        }
    }

    fn take_notes(&self, session: &SessionId) -> Vec<String> {
        self.0
            .history
            .lock()
            .notes
            .remove(session)
            .unwrap_or_default()
    }
}

impl RepoUndo {
    /// See [`History::refresh`].
    pub async fn refresh(&self, call: &ToolCallId) {
        self.0.history.refresh(&self.0, call).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_targets() -> Vec<RecordedTarget> {
        vec![RecordedTarget {
            key: "a.txt".into(),
            #[cfg(unix)]
            mode: Some(0o644),
        }]
    }

    fn file(bytes: usize) -> Vec<FileState> {
        vec![FileState {
            path: "a.txt".into(),
            before: None,
            after: Some(vec![0; bytes]),
        }]
    }

    #[test]
    fn the_history_keeps_the_newest_changes_within_its_bounds() {
        let history = History::default();
        let (busy, quiet) = (SessionId::new(), SessionId::new());
        let id = ToolCallId::new();
        let quiet_call = Call {
            session: &quiet,
            id: &id,
            tool: "repo.create-file",
        };
        history.record(&quiet_call, file(1), test_targets());
        for _ in 0..MAX_CHANGES_PER_SESSION + 5 {
            let id = ToolCallId::new();
            let call = Call {
                session: &busy,
                id: &id,
                tool: "repo.edit-files",
            };
            history.record(&call, file(1), test_targets());
        }
        let state = history.lock();
        let count = |session: &SessionId| {
            state
                .entries
                .iter()
                .filter(|entry| entry.session == *session)
                .count()
        };
        assert_eq!(count(&busy), MAX_CHANGES_PER_SESSION);
        assert_eq!(count(&quiet), 1, "another session keeps its own");
        assert_eq!(state.bytes, MAX_CHANGES_PER_SESSION + 1);
        drop(state);

        // Over the byte bound, the oldest changes go first.
        let id = ToolCallId::new();
        history.record(
            &Call {
                session: &busy,
                id: &id,
                tool: "repo.create-file",
            },
            file(MAX_BYTES),
            test_targets(),
        );
        let state = history.lock();
        assert_eq!(state.entries.len(), 1);
        assert_eq!(state.bytes, MAX_BYTES);
    }
}
