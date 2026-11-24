//! The `file-undo` service: the user takes back the file changes write tools
//! made in a session (ADR 0060).
//!
//! A provider (`tools-repo-write`) records the content of each file before
//! and after every write call it runs. Frontends call it on the user's
//! request; it is not a tool, so the model cannot. An undo restores the
//! newest change of a session only when every file is still what the change
//! left, and leaves a note that the next turn's input carries to the model
//! ([`record_turn_start`]).

use async_trait::async_trait;
use rah_kernel::{KernelHandle, ServiceKey};
use rah_protocol::{RequestId, SessionEntry, SessionId, SessionRecord, ToolCallId};
use std::path::Path;

/// Holds the owning writer's file lock while a formatter runs. On success
/// the formatter supplies the bytes it actually read before and after.
/// Dropping the callback releases the lock without changing the history.
pub type FormatReceipt = Box<dyn FnOnce(&[u8], Vec<u8>) + Send>;

pub enum FormatClaim {
    /// This provider has no record for the call and file.
    Untracked,
    /// The recorded file has changed since the call. Skip formatting it.
    Changed,
    /// Run the formatter while holding this receipt.
    Tracked(FormatReceipt),
}

/// Service name `file-undo` providers declare in their manifest's `provides`.
pub const FILE_UNDO_SERVICE: &str = "file-undo";

pub const FILE_UNDO: ServiceKey<dyn FileUndo> = ServiceKey::new(FILE_UNDO_SERVICE);

/// One recorded tool call that changed files.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileChange {
    /// The tool, such as `repo.edit-files`.
    pub tool: String,
    /// Repository-relative paths, in the order the call named them.
    pub files: Vec<String>,
    /// When the call completed, in milliseconds since the Unix epoch.
    pub at_ms: u64,
}

/// What an undo did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UndoReport {
    pub change: FileChange,
    /// Files written back with their content from before the change.
    pub restored: Vec<String>,
    /// Files the change created, deleted again.
    pub removed: Vec<String>,
}

impl UndoReport {
    /// `restored a; removed b`.
    fn done(&self) -> String {
        let mut parts = Vec::new();
        if !self.restored.is_empty() {
            parts.push(format!("restored {}", self.restored.join(", ")));
        }
        if !self.removed.is_empty() {
            parts.push(format!("removed {}", self.removed.join(", ")));
        }
        parts.join("; ")
    }

    /// One line for the user.
    pub fn summary(&self) -> String {
        format!("undid `{}`: {}", self.change.tool, self.done())
    }

    /// What the model reads at its next turn.
    pub fn note(&self) -> String {
        format!(
            "[RAH: the user undid your `{}` call ({}). Those files are back to how they were \
             before it; read them again before you edit them.]",
            self.change.tool,
            self.done()
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum UndoError {
    #[error("there is no file change to undo in this session")]
    Nothing,
    /// Nothing was written; the change was dropped from the history.
    #[error(
        "`{tool}` was not undone: {} changed since, so nothing was written; the change was \
         dropped from the undo history",
        files.join(", ")
    )]
    Changed { tool: String, files: Vec<String> },
    #[error("undoing `{tool}` failed: {message}")]
    Failed { tool: String, message: String },
}

#[async_trait]
pub trait FileUndo: Send + Sync {
    /// Claims a formatter transition for a recorded call and canonical file.
    async fn begin_format(
        &self,
        _call: &ToolCallId,
        _relative: &str,
        _canonical: &Path,
    ) -> FormatClaim {
        FormatClaim::Untracked
    }

    /// The changes of a session that can be undone, newest first.
    fn changes(&self, session: &SessionId) -> Vec<FileChange>;

    /// Undoes the newest change of a session, all files or none.
    async fn undo_last(&self, session: &SessionId) -> Result<UndoReport, UndoError>;

    /// Reserves notes for an admitted user turn. Reconcile an earlier
    /// unacknowledged append against the history read for this session.
    fn reserve_notes(
        &self,
        session: &SessionId,
        turn: &RequestId,
        history: &[SessionRecord],
    ) -> Vec<String>;
    fn acknowledge_notes(&self, session: &SessionId, turn: &RequestId);

    /// Takes the notes about undone changes the model has not seen yet.
    fn take_notes(&self, session: &SessionId) -> Vec<String>;
}

/// The changes of a session across every provider, newest first.
pub fn changes(kernel: &KernelHandle, session: &SessionId) -> Vec<FileChange> {
    let mut all: Vec<FileChange> = kernel
        .get_all(FILE_UNDO)
        .into_iter()
        .flat_map(|(_, provider)| provider.changes(session))
        .collect();
    all.sort_by_key(|change| std::cmp::Reverse(change.at_ms));
    all
}

/// Undoes the newest change of a session among every provider. `None` when
/// no provider is mounted.
pub async fn undo_newest(
    kernel: &KernelHandle,
    session: &SessionId,
) -> Option<Result<UndoReport, UndoError>> {
    let providers = kernel.get_all(FILE_UNDO);
    if providers.is_empty() {
        return None;
    }
    let newest = providers
        .into_iter()
        .filter_map(|(_, provider)| {
            let at = provider.changes(session).first()?.at_ms;
            Some((at, provider))
        })
        .max_by_key(|(at, _)| *at);
    Some(match newest {
        Some((_, provider)) => provider.undo_last(session).await,
        None => Err(UndoError::Nothing),
    })
}

/// Records the accepted input before acknowledging the notes included in it.
/// An interrupted append leaves the reservation with its provider.
pub async fn record_turn_start(
    kernel: &KernelHandle,
    log: &dyn crate::session::SessionLog,
    session: &SessionId,
    turn: &RequestId,
    origin: crate::agent_loop::TurnOrigin,
    input: String,
    history: &[SessionRecord],
) -> Result<String, crate::session::SessionError> {
    let providers = if origin == crate::agent_loop::TurnOrigin::User {
        kernel.get_all(FILE_UNDO)
    } else {
        Vec::new()
    };
    let notes: Vec<_> = providers
        .iter()
        .flat_map(|(_, p)| p.reserve_notes(session, turn, history))
        .collect();
    let input = if notes.is_empty() {
        input
    } else {
        format!("{}\n\n{input}", notes.join("\n"))
    };
    log.append(
        session,
        SessionEntry::TurnStarted {
            turn: turn.clone(),
            input: input.clone(),
        },
    )
    .await?;
    for (_, provider) in providers {
        provider.acknowledge_notes(session, turn);
    }
    Ok(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report() -> UndoReport {
        UndoReport {
            change: FileChange {
                tool: "repo.move-file".into(),
                files: vec!["a.txt".into(), "b.txt".into()],
                at_ms: 1,
            },
            restored: vec!["a.txt".into()],
            removed: vec!["b.txt".into()],
        }
    }

    #[test]
    fn the_summary_and_the_note_name_the_tool_and_the_files() {
        let report = report();
        assert_eq!(
            report.summary(),
            "undid `repo.move-file`: restored a.txt; removed b.txt"
        );
        assert_eq!(
            report.note(),
            "[RAH: the user undid your `repo.move-file` call (restored a.txt; removed b.txt). \
             Those files are back to how they were before it; read them again before you edit \
             them.]"
        );
    }

    #[test]
    fn a_refused_undo_names_the_files_that_changed() {
        let error = UndoError::Changed {
            tool: "repo.edit-files".into(),
            files: vec!["a.txt".into()],
        };
        assert!(error.to_string().starts_with(
            "`repo.edit-files` was not undone: a.txt changed since, so nothing was written"
        ));
    }
}
