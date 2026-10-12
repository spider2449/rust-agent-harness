use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{RequestId, SessionId, ToolCallId, ToolName, ToolOutput};

/// What a recorded tool call may have changed outside the session.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolEffect {
    /// The call had no external effect (rejected, read-only or denied).
    None,
    /// The call completed and its effect, if any, happened.
    Completed,
    /// The call may or may not have taken effect (cancelled, failed or lost).
    /// Never replayed automatically.
    Uncertain,
}

/// One append-only session log entry.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SessionEntry {
    TurnStarted {
        turn: RequestId,
        input: String,
    },
    AssistantMessage {
        turn: RequestId,
        text: String,
    },
    ToolCall {
        turn: RequestId,
        call_id: String,
        tool_call_id: ToolCallId,
        name: ToolName,
        arguments: Value,
    },
    ToolResult {
        turn: RequestId,
        call_id: String,
        output: ToolOutput,
        effect: ToolEffect,
    },
    TurnCompleted {
        turn: RequestId,
    },
    TurnFailed {
        turn: RequestId,
        message: String,
    },
}

/// A sequenced entry. Sequence numbers start at 1 and continue across forks.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct SessionRecord {
    pub seq: u64,
    pub entry: SessionEntry,
}

/// The parent position a forked session continues from.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ForkPoint {
    pub session: SessionId,
    /// Last parent sequence number included in the fork.
    pub at: u64,
}

/// Session metadata.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SessionInfo {
    pub id: SessionId,
    pub parent: Option<ForkPoint>,
    /// Creation time in seconds since the Unix epoch.
    pub created: u64,
    /// A human-readable name, set by the user or generated; `None` until then.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Last change (record or title) in seconds since the Unix epoch.
    #[serde(default)]
    pub updated: u64,
    /// The start of the first user message, for lists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview: Option<String>,
}

/// The longest session title, in characters.
pub const MAX_TITLE_CHARS: usize = 120;

/// A title as stored: one line, trimmed, at most [`MAX_TITLE_CHARS`]
/// characters; `None` when nothing is left.
pub fn normalize_title(title: &str) -> Option<String> {
    let line = title.split_whitespace().collect::<Vec<_>>().join(" ");
    let title: String = line.chars().take(MAX_TITLE_CHARS).collect();
    (!title.is_empty()).then_some(title)
}
