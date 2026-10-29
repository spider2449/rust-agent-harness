//! The `session` service: append-only session logs with fork and replay.

use async_trait::async_trait;
use rah_kernel::ServiceKey;
use rah_protocol::{
    ContextItem, SessionEntry, SessionId, SessionInfo, SessionRecord, ToolContent, ToolOutput,
};

pub mod export;

pub const SESSION: ServiceKey<dyn SessionLog> = ServiceKey::new("session");

#[async_trait]
pub trait SessionLog: Send + Sync {
    async fn create(&self) -> Result<SessionInfo, SessionError>;

    /// Appends one entry and returns its sequence number.
    async fn append(&self, session: &SessionId, entry: SessionEntry) -> Result<u64, SessionError>;

    /// All records visible in a session, including a fork's parent prefix.
    async fn read(&self, session: &SessionId) -> Result<Vec<SessionRecord>, SessionError>;

    /// Creates a session continuing from `session` after record `at`.
    async fn fork(&self, session: &SessionId, at: u64) -> Result<SessionInfo, SessionError>;

    async fn list(&self) -> Result<Vec<SessionInfo>, SessionError>;

    /// Sets or, with `None`, clears the session's title. Records are never
    /// rewritten; providers store the title as metadata.
    async fn rename(
        &self,
        session: &SessionId,
        title: Option<String>,
    ) -> Result<SessionInfo, SessionError> {
        let _ = (session, title);
        Err(SessionError::Unsupported("rename"))
    }

    /// Sets `title` only if the session has none, so a generated title never
    /// replaces one set meanwhile. Returns whether it was set. Providers
    /// should check and write atomically; this default does not.
    async fn rename_if_untitled(
        &self,
        session: &SessionId,
        title: String,
    ) -> Result<bool, SessionError> {
        let untitled = self
            .list()
            .await?
            .iter()
            .any(|info| info.id == *session && info.title.is_none());
        if untitled {
            self.rename(session, Some(title)).await?;
        }
        Ok(untitled)
    }

    /// Removes the session from the list. Refused while forks of it exist,
    /// since they read its records. Providers should keep the data
    /// recoverable rather than destroy it.
    async fn delete(&self, session: &SessionId) -> Result<(), SessionError> {
        let _ = session;
        Err(SessionError::Unsupported("delete"))
    }
}

/// Emitted by a session provider after each successful append. Plugins that
/// react to session progress (titles, indexing) subscribe to it.
#[derive(Clone, Debug, PartialEq)]
pub struct SessionAppended {
    pub session: SessionId,
    pub seq: u64,
    pub entry: SessionEntry,
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum SessionError {
    #[error("session `{0}` does not exist")]
    NotFound(SessionId),
    #[error("cannot fork session `{session}` at {at}; it has {last} records")]
    InvalidFork {
        session: SessionId,
        at: u64,
        last: u64,
    },
    #[error("session `{session}` is corrupt: {detail}")]
    Corrupt { session: SessionId, detail: String },
    #[error("session storage failed: {0}")]
    Io(String),
    #[error("session `{session}` has forks; delete them first: {}", forks.iter().map(ToString::to_string).collect::<Vec<_>>().join(", "))]
    HasForks {
        session: SessionId,
        forks: Vec<SessionId>,
    },
    #[error("the title is empty")]
    InvalidTitle,
    #[error("this session provider does not support `{0}`")]
    Unsupported(&'static str),
}

/// Text recorded for a tool call whose result was never logged.
pub const UNRECORDED_RESULT: &str =
    "no result was recorded; the call may or may not have taken effect and is not replayed";

/// Rebuilds model context from session records. Tool calls are never
/// re-executed: a call without a recorded result gets a synthetic error
/// result marking its effect as unknown.
pub fn rebuild_context(records: &[SessionRecord]) -> Vec<ContextItem> {
    let mut items = Vec::new();
    let mut open: Vec<String> = Vec::new();
    let close = |items: &mut Vec<ContextItem>, open: &mut Vec<String>| {
        for call_id in open.drain(..) {
            items.push(ContextItem::ToolResult {
                call_id,
                output: ToolOutput {
                    content: vec![ToolContent::Text(UNRECORDED_RESULT.to_owned())],
                    is_error: true,
                },
            });
        }
    };
    for record in records {
        match &record.entry {
            SessionEntry::ToolCall {
                call_id,
                name,
                arguments,
                ..
            } => {
                open.push(call_id.clone());
                items.push(ContextItem::ToolCall {
                    call_id: call_id.clone(),
                    name: name.clone(),
                    arguments: arguments.clone(),
                });
            }
            SessionEntry::ToolResult {
                call_id, output, ..
            } => {
                open.retain(|id| id != call_id);
                items.push(ContextItem::ToolResult {
                    call_id: call_id.clone(),
                    output: output.clone(),
                });
            }
            SessionEntry::TurnStarted { input, .. } => {
                close(&mut items, &mut open);
                items.push(ContextItem::User {
                    text: input.clone(),
                });
            }
            SessionEntry::AssistantMessage { text, .. } => {
                close(&mut items, &mut open);
                items.push(ContextItem::Assistant { text: text.clone() });
            }
            SessionEntry::TurnCompleted { .. } | SessionEntry::TurnFailed { .. } => {
                close(&mut items, &mut open);
            }
        }
    }
    close(&mut items, &mut open);
    items
}

#[cfg(test)]
mod tests {
    use super::*;
    use rah_protocol::{RequestId, ToolCallId, ToolEffect, ToolName};
    use serde_json::json;

    fn record(seq: u64, entry: SessionEntry) -> SessionRecord {
        SessionRecord { seq, entry }
    }

    #[test]
    fn interrupted_tool_calls_get_an_unknown_effect_result_and_are_not_replayed() {
        let turn = RequestId::new();
        let records = [
            record(
                1,
                SessionEntry::TurnStarted {
                    turn: turn.clone(),
                    input: "write it".into(),
                },
            ),
            record(
                2,
                SessionEntry::ToolCall {
                    turn: turn.clone(),
                    call_id: "c1".into(),
                    tool_call_id: ToolCallId::new(),
                    name: ToolName::new("fs.write"),
                    arguments: json!({"path": "a"}),
                },
            ),
            record(
                3,
                SessionEntry::TurnFailed {
                    turn: turn.clone(),
                    message: "crash".into(),
                },
            ),
            record(
                4,
                SessionEntry::TurnStarted {
                    turn: RequestId::new(),
                    input: "again".into(),
                },
            ),
        ];
        let items = rebuild_context(&records);
        assert_eq!(items.len(), 4);
        assert!(matches!(&items[0], ContextItem::User { text } if text == "write it"));
        assert!(matches!(&items[1], ContextItem::ToolCall { call_id, .. } if call_id == "c1"));
        assert!(matches!(
            &items[2],
            ContextItem::ToolResult { call_id, output } if call_id == "c1" && output.is_error
        ));
        assert!(matches!(&items[3], ContextItem::User { text } if text == "again"));
    }

    #[test]
    fn recorded_results_are_used_verbatim() {
        let turn = RequestId::new();
        let output = ToolOutput {
            content: vec![ToolContent::Text("ok".into())],
            is_error: false,
        };
        let records = [
            record(
                1,
                SessionEntry::ToolCall {
                    turn: turn.clone(),
                    call_id: "c1".into(),
                    tool_call_id: ToolCallId::new(),
                    name: ToolName::new("echo"),
                    arguments: json!({}),
                },
            ),
            record(
                2,
                SessionEntry::ToolResult {
                    turn,
                    call_id: "c1".into(),
                    output: output.clone(),
                    effect: ToolEffect::None,
                },
            ),
        ];
        let items = rebuild_context(&records);
        assert_eq!(
            items[1],
            ContextItem::ToolResult {
                call_id: "c1".into(),
                output
            }
        );
    }
}
