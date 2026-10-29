//! The undo history (ADR 0060): the content of each file before and after
//! every write call, per session, in memory. The user undoes the newest
//! change of a session through the `file-undo` service; it is written back
//! only when every file is still what the change left.

use std::{
    collections::HashMap,
    path::Path,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
    time::{SystemTime, UNIX_EPOCH},
};

use rah_api::{
    async_trait,
    undo::{FileChange, FileUndo, UndoError, UndoReport},
};
use rah_protocol::{SessionId, ToolCallId};

use crate::fsops::Root;

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
    pub before: Option<Vec<u8>>,
    pub after: Option<Vec<u8>>,
}

impl FileState {
    fn bytes(&self) -> usize {
        self.before.as_ref().map_or(0, Vec::len) + self.after.as_ref().map_or(0, Vec::len)
    }
}

struct Entry {
    session: SessionId,
    call: ToolCallId,
    tool: &'static str,
    at_ms: u64,
    files: Vec<FileState>,
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

impl History {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Keeps a completed call's files, newest last. Called while the call's
    /// file locks are held.
    pub fn record(&self, call: &Call<'_>, files: Vec<FileState>) {
        if files.is_empty() {
            return;
        }
        let entry = Entry {
            session: call.session.clone(),
            call: call.id.clone(),
            tool: call.tool,
            at_ms: now_ms(),
            files,
        };
        let mut state = self.lock();
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

    /// Reads the files of `call`'s change again at the end of the call, so a
    /// formatter that rewrote one is part of the change. A file that can no
    /// longer be kept drops the change.
    pub async fn refresh(&self, root: &Root, call: &ToolCallId) {
        let paths: Vec<String> = {
            let state = self.lock();
            let Some(entry) = state.entries.iter().find(|entry| entry.call == *call) else {
                return;
            };
            entry.files.iter().map(|file| file.path.clone()).collect()
        };
        let resolved: Vec<_> = paths.iter().map(|path| root.path.join(path)).collect();
        let _writing = root.locks.files(&resolved).await;
        let mut now = Vec::with_capacity(paths.len());
        for file in &resolved {
            now.push(read_state(file, root.max_file_bytes).await);
        }
        let mut state = self.lock();
        let Some(index) = state.entries.iter().position(|entry| entry.call == *call) else {
            return;
        };
        if now.iter().any(Result::is_err) {
            state.remove(index);
            return;
        }
        let before = state.entries[index].bytes();
        for (file, current) in state.entries[index].files.iter_mut().zip(now) {
            file.after = current.unwrap_or_default();
        }
        let after = state.entries[index].bytes();
        state.bytes = state.bytes - before + after;
    }

    /// Undoes the newest change of `session` in `root`.
    pub async fn undo_last(
        &self,
        root: &Root,
        session: &SessionId,
    ) -> Result<UndoReport, UndoError> {
        // Taken out first: a failed or refused undo does not leave it at the
        // top, where it would block the older changes.
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
        let resolved: Vec<_> = entry
            .files
            .iter()
            .map(|file| root.path.join(&file.path))
            .collect();
        let _writing = root.locks.files(&resolved).await;
        let mut changed = Vec::new();
        for (file, path) in entry.files.iter().zip(&resolved) {
            if read_state(path, root.max_file_bytes).await.ok().as_ref() != Some(&file.after) {
                changed.push(file.path.clone());
            }
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
        history.record(&quiet_call, file(1));
        for _ in 0..MAX_CHANGES_PER_SESSION + 5 {
            let id = ToolCallId::new();
            let call = Call {
                session: &busy,
                id: &id,
                tool: "repo.edit-files",
            };
            history.record(&call, file(1));
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
        );
        let state = history.lock();
        assert_eq!(state.entries.len(), 1);
        assert_eq!(state.bytes, MAX_BYTES);
    }
}
