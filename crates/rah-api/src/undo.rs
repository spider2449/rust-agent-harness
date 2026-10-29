//! The `file-undo` service: the user takes back the file changes write tools
//! made in a session (ADR 0060).
//!
//! A provider (`tools-repo-write`) records the content of each file before
//! and after every write call it runs. Frontends call it on the user's
//! request; it is not a tool, so the model cannot. An undo restores the
//! newest change of a session only when every file is still what the change
//! left, and leaves a note that the next turn's input carries to the model
//! ([`with_notes`]).

use async_trait::async_trait;
use rah_kernel::{KernelHandle, ServiceKey};
use rah_protocol::SessionId;

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
    /// The changes of a session that can be undone, newest first.
    fn changes(&self, session: &SessionId) -> Vec<FileChange>;

    /// Undoes the newest change of a session, all files or none.
    async fn undo_last(&self, session: &SessionId) -> Result<UndoReport, UndoError>;

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

/// `input` with the session's pending undo notes before it, for a turn the
/// user starts. Without notes it is `input` unchanged.
pub fn with_notes(kernel: &KernelHandle, session: &SessionId, input: String) -> String {
    let notes: Vec<String> = kernel
        .get_all(FILE_UNDO)
        .into_iter()
        .flat_map(|(_, provider)| provider.take_notes(session))
        .collect();
    if notes.is_empty() {
        input
    } else {
        format!("{}\n\n{input}", notes.join("\n"))
    }
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
