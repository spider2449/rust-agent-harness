//! Session export as Markdown or JSON.

use rah_protocol::{SessionEntry, SessionInfo, SessionRecord, ToolContent, ToolOutput};
use serde_json::json;

/// The session as a JSON document: `{session, records}`.
pub(crate) fn json(info: &SessionInfo, records: &[SessionRecord]) -> String {
    serde_json::to_string_pretty(&json!({ "session": info, "records": records }))
        .unwrap_or_default()
}

/// The session as a readable Markdown transcript.
pub(crate) fn markdown(info: &SessionInfo, records: &[SessionRecord]) -> String {
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
                let effect = serde_json::to_value(effect)
                    .ok()
                    .and_then(|value| value.as_str().map(str::to_owned))
                    .unwrap_or_default();
                out.push_str(&format!(
                    "\n**{label}** (effect: {effect})\n\n{}",
                    fenced("", &render(output))
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

fn render(output: &ToolOutput) -> String {
    output
        .content
        .iter()
        .map(|item| match item {
            ToolContent::Text(text) => text.clone(),
            ToolContent::Json(value) => serde_json::to_string_pretty(value).unwrap_or_default(),
        })
        .collect::<Vec<_>>()
        .join("\n")
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
    use rah_protocol::{RequestId, SessionId, ToolCallId, ToolEffect, ToolName};

    #[test]
    fn markdown_lists_the_turns_and_fences_tool_output_safely() {
        let turn = RequestId::new();
        let info = SessionInfo {
            id: SessionId::new(),
            parent: None,
            created: 0,
            title: Some("Kyoto trip".into()),
            updated: 0,
            preview: None,
            workspace: None,
        };
        let records = [
            SessionRecord {
                seq: 1,
                entry: SessionEntry::TurnStarted {
                    turn: turn.clone(),
                    input: "plan it".into(),
                },
            },
            SessionRecord {
                seq: 2,
                entry: SessionEntry::ToolCall {
                    turn: turn.clone(),
                    call_id: "c1".into(),
                    tool_call_id: ToolCallId::new(),
                    name: ToolName::new("fs.read"),
                    arguments: json!({ "path": "a.md" }),
                },
            },
            SessionRecord {
                seq: 3,
                entry: SessionEntry::ToolResult {
                    turn: turn.clone(),
                    call_id: "c1".into(),
                    output: ToolOutput {
                        content: vec![ToolContent::Text("```\ninner\n```".into())],
                        is_error: false,
                    },
                    effect: ToolEffect::None,
                },
            },
            SessionRecord {
                seq: 4,
                entry: SessionEntry::AssistantMessage {
                    turn,
                    text: "Done.".into(),
                },
            },
        ];
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
}
