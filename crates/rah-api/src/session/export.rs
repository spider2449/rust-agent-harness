//! Session export as Markdown, JSON or a plain-text trace.
//!
//! The trace uses the line grammar harness optimizers read (`[USER]`,
//! `[ASSISTANT]`, `[ACTION]`, `[OBSERVATION]`, `[OBSERVATION ERROR]`,
//! `[ERROR]`), one tagged entry per record, with long arguments and results
//! cut at [`TraceLimits`].

use std::{fmt, str::FromStr};

use rah_protocol::{SessionEntry, SessionInfo, SessionRecord, ToolContent, ToolEffect, ToolOutput};
use serde_json::json;

use super::UNRECORDED_RESULT;
use crate::tools::describe_image;

/// The export formats.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportFormat {
    Markdown,
    Json,
    Trace,
}

impl ExportFormat {
    /// The names [`FromStr`] accepts, for error messages.
    pub const NAMES: &str = "md, json or trace";

    /// The file name extension for a download.
    pub fn extension(self) -> &'static str {
        match self {
            Self::Markdown => "md",
            Self::Json => "json",
            Self::Trace => "txt",
        }
    }

    /// The HTTP content type.
    pub fn content_type(self) -> &'static str {
        match self {
            Self::Markdown => "text/markdown; charset=utf-8",
            Self::Json => "application/json; charset=utf-8",
            Self::Trace => "text/plain; charset=utf-8",
        }
    }

    /// The session in this format; a trace uses the default limits.
    pub fn render(self, info: &SessionInfo, records: &[SessionRecord]) -> String {
        match self {
            Self::Markdown => markdown(info, records),
            Self::Json => json(info, records),
            Self::Trace => trace(records, TraceLimits::default()),
        }
    }
}

impl FromStr for ExportFormat {
    type Err = UnknownFormat;

    fn from_str(name: &str) -> Result<Self, Self::Err> {
        match name {
            "md" | "markdown" => Ok(Self::Markdown),
            "json" => Ok(Self::Json),
            "trace" => Ok(Self::Trace),
            other => Err(UnknownFormat(other.to_owned())),
        }
    }
}

/// A format name [`ExportFormat`] does not know.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnknownFormat(pub String);

impl fmt::Display for UnknownFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "unknown export format `{}`; use {}",
            self.0,
            ExportFormat::NAMES
        )
    }
}

impl std::error::Error for UnknownFormat {}

/// The session as a JSON document: `{session, records}`.
pub fn json(info: &SessionInfo, records: &[SessionRecord]) -> String {
    serde_json::to_string_pretty(&json!({ "session": info, "records": records }))
        .unwrap_or_default()
}

/// The session as a readable Markdown transcript.
pub fn markdown(info: &SessionInfo, records: &[SessionRecord]) -> String {
    let title = info
        .title
        .clone()
        .unwrap_or_else(|| format!("Session {}", info.id));
    let mut out = format!("# {title}\n\nSession `{}`", info.id);
    if let Some(parent) = &info.parent {
        out.push_str(&format!(
            ", forked from `{}` after record {}",
            parent.session, parent.at
        ));
    }
    out.push_str(".\n");
    for record in records {
        match &record.entry {
            SessionEntry::TurnStarted { input, .. } => {
                out.push_str(&format!("\n## You\n\n{input}\n"));
            }
            SessionEntry::AssistantMessage { text, .. } => {
                out.push_str(&format!("\n## Assistant\n\n{text}\n"));
            }
            SessionEntry::ToolCall {
                name, arguments, ..
            } => {
                let arguments = serde_json::to_string_pretty(arguments).unwrap_or_default();
                out.push_str(&format!(
                    "\n**Tool call** `{name}`\n\n{}",
                    fenced("json", &arguments)
                ));
            }
            SessionEntry::ToolResult { output, effect, .. } => {
                let label = if output.is_error {
                    "Tool error"
                } else {
                    "Tool result"
                };
                out.push_str(&format!(
                    "\n**{label}** (effect: {})\n\n{}",
                    effect_name(effect),
                    fenced("", &render(output, "\n", true))
                ));
            }
            SessionEntry::TurnCompleted { .. } => {}
            SessionEntry::TurnFailed { message, .. } => {
                out.push_str(&format!("\n> **Turn failed:** {message}\n"));
            }
        }
    }
    out
}

/// Where a trace cuts long text, in characters. Zero means no limit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TraceLimits {
    /// Tool-call arguments (compact JSON).
    pub arguments: usize,
    /// Tool results, user input and assistant text.
    pub text: usize,
}

impl Default for TraceLimits {
    fn default() -> Self {
        Self {
            arguments: 400,
            text: 50_000,
        }
    }
}

/// The session as a plain-text trace, one tagged entry per record.
///
/// A result whose effect is `uncertain` says so, and a call whose result was
/// never recorded gets an `[OBSERVATION ERROR]`, so neither reads as a
/// success.
pub fn trace(records: &[SessionRecord], limits: TraceLimits) -> String {
    let mut lines = Vec::new();
    let mut open: Vec<&str> = Vec::new();
    let close = |lines: &mut Vec<String>, open: &mut Vec<&str>| {
        for _ in open.drain(..) {
            lines.push(format!("[OBSERVATION ERROR] {UNRECORDED_RESULT}"));
        }
    };
    for record in records {
        match &record.entry {
            SessionEntry::TurnStarted { input, .. } => {
                close(&mut lines, &mut open);
                lines.push(format!("[USER] {}", clip(input, limits.text)));
            }
            SessionEntry::AssistantMessage { text, .. } => {
                close(&mut lines, &mut open);
                if !text.trim().is_empty() {
                    lines.push(format!("[ASSISTANT] {}", clip(text, limits.text)));
                }
            }
            SessionEntry::ToolCall {
                call_id,
                name,
                arguments,
                ..
            } => {
                open.push(call_id);
                lines.push(format!(
                    "[ACTION] {name}({})",
                    clip(&arguments.to_string(), limits.arguments)
                ));
            }
            SessionEntry::ToolResult {
                call_id,
                output,
                effect,
                ..
            } => {
                open.retain(|id| id != call_id);
                let tag = if output.is_error {
                    "[OBSERVATION ERROR]"
                } else {
                    "[OBSERVATION]"
                };
                let note = if *effect == ToolEffect::Uncertain {
                    "(effect uncertain: it may or may not have happened) "
                } else {
                    ""
                };
                let text = render(output, "\n", false);
                lines.push(format!("{tag} {note}{}", clip(&text, limits.text)));
            }
            SessionEntry::TurnCompleted { .. } => close(&mut lines, &mut open),
            SessionEntry::TurnFailed { message, .. } => {
                close(&mut lines, &mut open);
                lines.push(format!("[ERROR] {}", clip(message, limits.text)));
            }
        }
    }
    close(&mut lines, &mut open);
    let mut out = lines.join("\n");
    out.push('\n');
    out
}

/// `text` cut to `limit` characters, followed by how many were left out.
fn clip(text: &str, limit: usize) -> String {
    let text = text.replace('\0', "");
    let total = text.chars().count();
    if limit == 0 || total <= limit {
        return text;
    }
    let kept: String = text.chars().take(limit).collect();
    format!("{kept}…(+{} chars truncated)", total - limit)
}

fn effect_name(effect: &ToolEffect) -> &'static str {
    match effect {
        ToolEffect::None => "none",
        ToolEffect::Completed => "completed",
        ToolEffect::Uncertain => "uncertain",
    }
}

fn render(output: &ToolOutput, separator: &str, pretty: bool) -> String {
    output
        .content
        .iter()
        .map(|item| match item {
            ToolContent::Text(text) => text.clone(),
            ToolContent::Json(value) if pretty => {
                serde_json::to_string_pretty(value).unwrap_or_default()
            }
            ToolContent::Json(value) => value.to_string(),
            ToolContent::Image(image) => describe_image(image),
        })
        .collect::<Vec<_>>()
        .join(separator)
}

/// A code block whose fence is longer than any backtick run in `text`.
fn fenced(language: &str, text: &str) -> String {
    let longest = text.split(|c| c != '`').map(str::len).max().unwrap_or(0);
    let fence = "`".repeat(longest.max(2) + 1);
    format!("{fence}{language}\n{text}\n{fence}\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use rah_protocol::{RequestId, SessionId, ToolCallId, ToolName};

    fn info() -> SessionInfo {
        SessionInfo {
            id: SessionId::new(),
            parent: None,
            created: 0,
            title: Some("Kyoto trip".into()),
            updated: 0,
            preview: None,
            workspace: None,
        }
    }

    fn records(entries: Vec<SessionEntry>) -> Vec<SessionRecord> {
        entries
            .into_iter()
            .zip(1..)
            .map(|(entry, seq)| SessionRecord { seq, entry })
            .collect()
    }

    fn call(turn: &RequestId, id: &str, tool: &str, arguments: serde_json::Value) -> SessionEntry {
        SessionEntry::ToolCall {
            turn: turn.clone(),
            call_id: id.into(),
            tool_call_id: ToolCallId::new(),
            name: ToolName::new(tool),
            arguments,
        }
    }

    fn result(
        turn: &RequestId,
        id: &str,
        text: &str,
        is_error: bool,
        effect: ToolEffect,
    ) -> SessionEntry {
        SessionEntry::ToolResult {
            turn: turn.clone(),
            call_id: id.into(),
            output: ToolOutput {
                content: vec![ToolContent::Text(text.into())],
                is_error,
            },
            effect,
        }
    }

    #[test]
    fn markdown_lists_the_turns_and_fences_tool_output_safely() {
        let turn = RequestId::new();
        let info = info();
        let records = records(vec![
            SessionEntry::TurnStarted {
                turn: turn.clone(),
                input: "plan it".into(),
            },
            call(&turn, "c1", "fs.read", json!({ "path": "a.md" })),
            result(&turn, "c1", "```\ninner\n```", false, ToolEffect::None),
            SessionEntry::AssistantMessage {
                turn,
                text: "Done.".into(),
            },
        ]);
        let text = markdown(&info, &records);
        assert!(text.starts_with("# Kyoto trip\n"), "{text}");
        assert!(text.contains("## You\n\nplan it"));
        assert!(text.contains("**Tool call** `fs.read`"));
        assert!(text.contains("**Tool result** (effect: none)\n\n````\n```\ninner\n```\n````"));
        assert!(text.contains("## Assistant\n\nDone."));
        let parsed: serde_json::Value = serde_json::from_str(&json(&info, &records)).unwrap();
        assert_eq!(parsed["records"].as_array().unwrap().len(), 4);
        assert_eq!(parsed["session"]["title"], "Kyoto trip");
    }

    #[test]
    fn a_trace_tags_every_record_and_marks_failures_and_uncertain_effects() {
        let turn = RequestId::new();
        let records = records(vec![
            SessionEntry::TurnStarted {
                turn: turn.clone(),
                input: "fix it".into(),
            },
            call(&turn, "c1", "repo.read", json!({ "path": "a.rs" })),
            result(&turn, "c1", "fn main() {}", false, ToolEffect::None),
            call(&turn, "c2", "shell.exec", json!({ "program": "cargo" })),
            result(&turn, "c2", "timed out", true, ToolEffect::Uncertain),
            SessionEntry::AssistantMessage {
                turn: turn.clone(),
                text: "Tried.".into(),
            },
            SessionEntry::TurnCompleted { turn: turn.clone() },
            SessionEntry::TurnStarted {
                turn: turn.clone(),
                input: "again".into(),
            },
            call(&turn, "c3", "repo.edit-files", json!({})),
            SessionEntry::TurnFailed {
                turn,
                message: "connection lost".into(),
            },
        ]);
        let text = trace(&records, TraceLimits::default());
        let expected = [
            "[USER] fix it",
            r#"[ACTION] repo.read({"path":"a.rs"})"#,
            "[OBSERVATION] fn main() {}",
            r#"[ACTION] shell.exec({"program":"cargo"})"#,
            "[OBSERVATION ERROR] (effect uncertain: it may or may not have happened) timed out",
            "[ASSISTANT] Tried.",
            "[USER] again",
            "[ACTION] repo.edit-files({})",
            &format!("[OBSERVATION ERROR] {UNRECORDED_RESULT}"),
            "[ERROR] connection lost",
        ]
        .join("\n");
        assert_eq!(text, format!("{expected}\n"));
    }

    #[test]
    fn a_trace_cuts_long_text_by_characters_and_says_how_much() {
        let turn = RequestId::new();
        let records = records(vec![
            call(&turn, "c1", "repo.read", json!({ "path": "ä".repeat(10) })),
            result(&turn, "c1", &"é".repeat(12), false, ToolEffect::None),
        ]);
        let limits = TraceLimits {
            arguments: 5,
            text: 10,
        };
        let text = trace(&records, limits);
        assert!(
            text.contains(r#"[ACTION] repo.read({"pat…(+16 chars truncated))"#),
            "{text}"
        );
        assert!(
            text.contains(&format!(
                "[OBSERVATION] {}…(+2 chars truncated)",
                "é".repeat(10)
            )),
            "{text}"
        );
        let unlimited = trace(
            &records,
            TraceLimits {
                arguments: 0,
                text: 0,
            },
        );
        assert!(!unlimited.contains("truncated"), "{unlimited}");
    }

    #[test]
    fn format_names_parse_and_unknown_ones_list_the_choices() {
        assert_eq!("md".parse(), Ok(ExportFormat::Markdown));
        assert_eq!("markdown".parse(), Ok(ExportFormat::Markdown));
        assert_eq!("json".parse(), Ok(ExportFormat::Json));
        assert_eq!("trace".parse(), Ok(ExportFormat::Trace));
        let error = "html".parse::<ExportFormat>().unwrap_err();
        assert_eq!(
            error.to_string(),
            "unknown export format `html`; use md, json or trace"
        );
    }
}
