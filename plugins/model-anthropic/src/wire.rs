//! Messages API wire translation. Provider formats stay inside this plugin.

use std::collections::{BTreeMap, HashMap, HashSet};

use rah_protocol::{ContextItem, ToolContent, ToolDefinition, ToolOutput};
use serde_json::{Map, Value, json};

use crate::{error::E, sse::MAX_FRAME};

pub(crate) const MAX_BYTES: usize = 4 * 1024 * 1024;
/// The largest request; images make it larger than [`MAX_BYTES`].
pub(crate) const MAX_REQUEST_BYTES: usize = 32 * 1024 * 1024;
pub(crate) const MAX_CALLS: usize = 128;
const MAX_BLOCKS: usize = 1024;

/// Replaces characters outside `[A-Za-z0-9_-]`, the alphabet the Messages
/// API accepts for tool names and tool-use ids.
fn sanitize(name: &str) -> String {
    let wire: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if wire.is_empty() {
        "_".to_owned()
    } else {
        wire
    }
}

/// Maps RAH tool names to provider-safe names (at most 64 characters) and
/// back. Collisions get a numeric suffix.
#[derive(Default)]
pub(crate) struct ToolNames {
    to_wire: HashMap<String, String>,
    to_original: HashMap<String, String>,
}

impl ToolNames {
    pub fn new(definitions: &[ToolDefinition]) -> Self {
        let mut names = Self::default();
        for definition in definitions {
            let original = definition.name.as_str();
            if names.to_wire.contains_key(original) {
                continue;
            }
            let mut base = sanitize(original);
            base.truncate(60);
            let mut wire = base.clone();
            let mut suffix = 2;
            while names.to_original.contains_key(&wire) {
                wire = format!("{base}_{suffix}");
                suffix += 1;
            }
            names.to_wire.insert(original.to_owned(), wire.clone());
            names.to_original.insert(wire, original.to_owned());
        }
        names
    }

    pub fn wire(&self, original: &str) -> String {
        self.to_wire.get(original).cloned().unwrap_or_else(|| {
            let mut wire = sanitize(original);
            wire.truncate(60);
            wire
        })
    }

    pub fn original(&self, wire: &str) -> Option<&str> {
        self.to_original.get(wire).map(String::as_str)
    }
}

/// Tool definitions. A schema without `type` is an object schema; any other
/// type cannot describe tool input.
pub(crate) fn tools(definitions: &[ToolDefinition], names: &ToolNames) -> Result<Vec<Value>, E> {
    let mut seen = HashSet::new();
    definitions
        .iter()
        .filter(|d| seen.insert(d.name.as_str().to_owned()))
        .map(|d| {
            let mut schema = d.input_schema.as_object().cloned().ok_or(E::ToolSchema)?;
            match schema.get("type") {
                None => {
                    schema.insert("type".into(), "object".into());
                }
                Some(kind) if kind == "object" => {}
                Some(_) => return Err(E::ToolSchema),
            }
            let size = serde_json::to_vec(&schema)
                .map_err(|_| E::ToolSchema)?
                .len();
            if size > MAX_BYTES {
                return Err(E::ToolSchema);
            }
            let mut tool = json!({
                "name": names.wire(d.name.as_str()),
                "input_schema": schema,
            });
            if !d.description.is_empty() {
                tool["description"] = d.description.clone().into();
            }
            Ok(tool)
        })
        .collect()
}

/// A tool result's content: text, or a list of text and image blocks when
/// it has images.
fn output_content(output: &ToolOutput) -> Option<Value> {
    let mut texts = Vec::new();
    let mut images = Vec::new();
    for content in &output.content {
        match content {
            ToolContent::Text(text) => texts.push(text.clone()),
            ToolContent::Json(value) => texts.push(value.to_string()),
            ToolContent::Image(image) => images.push(json!({
                "type": "image",
                "source": {"type": "base64", "media_type": image.media_type, "data": image.data},
            })),
        }
    }
    let text = texts.join("\n");
    if images.is_empty() {
        return (!text.is_empty()).then(|| text.into());
    }
    let mut blocks = Vec::with_capacity(images.len() + 1);
    if !text.is_empty() {
        blocks.push(json!({"type": "text", "text": text}));
    }
    blocks.append(&mut images);
    Some(blocks.into())
}

/// Appends a block to the last message when it has the same role, so roles
/// alternate. Tool results go before any other block of their user message.
fn push(messages: &mut Vec<Value>, role: &str, block: Value) {
    if let Some(last) = messages.last_mut()
        && last["role"] == role
        && let Some(content) = last["content"].as_array_mut()
    {
        if block["type"] == "tool_result" {
            let at = content
                .iter()
                .position(|b| b["type"] != "tool_result")
                .unwrap_or(content.len());
            content.insert(at, block);
        } else {
            content.push(block);
        }
        return;
    }
    messages.push(json!({"role": role, "content": [block]}));
}

/// The top-level system prompt and the messages for neutral context.
///
/// Leading system items become the system prompt. A later system item is
/// operator text in the middle of the conversation; it is sent as a marked
/// user text block, which every model accepts at any position. Thinking
/// blocks are never replayed (neutral context does not carry them), so the
/// history never trips the thinking-block checks.
pub(crate) fn conversation(
    items: &[ContextItem],
    names: &ToolNames,
) -> Result<(Option<String>, Vec<Value>), E> {
    let split = items
        .iter()
        .position(|item| !matches!(item, ContextItem::System { .. }))
        .unwrap_or(items.len());
    let system: Vec<&str> = items[..split]
        .iter()
        .filter_map(|item| match item {
            ContextItem::System { text } if !text.is_empty() => Some(text.as_str()),
            _ => None,
        })
        .collect();
    let mut messages = Vec::new();
    for item in &items[split..] {
        match item {
            ContextItem::System { text } if !text.is_empty() => push(
                &mut messages,
                "user",
                json!({"type": "text", "text": format!("[System note] {text}")}),
            ),
            ContextItem::User { text } if !text.is_empty() => {
                push(&mut messages, "user", json!({"type": "text", "text": text}));
            }
            ContextItem::Assistant { text } if !text.is_empty() => {
                push(
                    &mut messages,
                    "assistant",
                    json!({"type": "text", "text": text}),
                );
            }
            ContextItem::System { .. }
            | ContextItem::User { .. }
            | ContextItem::Assistant { .. } => {}
            ContextItem::ToolCall {
                call_id,
                name,
                arguments,
            } => {
                let input = if arguments.is_object() {
                    arguments.clone()
                } else {
                    json!({"arguments": arguments})
                };
                push(
                    &mut messages,
                    "assistant",
                    json!({
                        "type": "tool_use",
                        "id": sanitize(call_id),
                        "name": names.wire(name.as_str()),
                        "input": input,
                    }),
                );
            }
            ContextItem::ToolResult { call_id, output } => {
                let mut block = json!({"type": "tool_result", "tool_use_id": sanitize(call_id)});
                if let Some(content) = output_content(output) {
                    block["content"] = content;
                }
                if output.is_error {
                    block["is_error"] = true.into();
                }
                push(&mut messages, "user", block);
            }
        }
    }
    let system = (!system.is_empty()).then(|| system.join("\n\n"));
    let size = serde_json::to_vec(&messages)
        .map_err(|_| E::Protocol)?
        .len()
        + system.as_ref().map_or(0, String::len);
    if size > MAX_REQUEST_BYTES {
        return Err(E::Protocol);
    }
    Ok((system, messages))
}

pub(crate) fn string<'a>(v: &'a Value, key: &str) -> Result<&'a str, E> {
    v.get(key).and_then(Value::as_str).ok_or(E::Protocol)
}

enum Block {
    Text,
    Thinking,
    ToolUse {
        id: String,
        name: String,
        initial: Map<String, Value>,
        json: String,
        input: Option<Value>,
    },
    /// A block type this plugin does not use; its events are skipped.
    Other,
}

/// Validating accumulator for one streamed message.
#[derive(Default)]
pub(crate) struct Response {
    blocks: BTreeMap<u64, (Block, bool)>,
    started: bool,
    stop_reason: Option<String>,
    stopped: bool,
    text: usize,
}

pub(crate) enum Update {
    Text(String),
    /// Thinking started; its content is discarded.
    Reasoning,
    Complete,
    None,
}

impl Response {
    fn text(&mut self, text: &str) -> Result<Update, E> {
        self.text += text.len();
        if self.text > MAX_BYTES {
            return Err(E::Protocol);
        }
        Ok(if text.is_empty() {
            Update::None
        } else {
            Update::Text(text.to_owned())
        })
    }

    pub fn event(&mut self, v: Value) -> Result<Update, E> {
        if self.stopped {
            return Err(E::Protocol);
        }
        let kind = string(&v, "type")?;
        if !self.started && !matches!(kind, "message_start" | "ping" | "error") {
            return Err(E::Protocol);
        }
        match kind {
            "message_start" => {
                let message = &v["message"];
                if self.started || message["type"] != "message" || message["role"] != "assistant" {
                    return Err(E::Protocol);
                }
                self.started = true;
                Ok(Update::None)
            }
            "content_block_start" => {
                let index = v["index"].as_u64().ok_or(E::Protocol)?;
                if self.blocks.contains_key(&index) || self.blocks.len() >= MAX_BLOCKS {
                    return Err(E::Protocol);
                }
                let block = &v["content_block"];
                let (parsed, update) = match string(block, "type")? {
                    "text" => (Block::Text, self.text(string(block, "text")?)?),
                    "thinking" | "redacted_thinking" => (Block::Thinking, Update::Reasoning),
                    "tool_use" => {
                        let id = string(block, "id")?.to_owned();
                        let name = string(block, "name")?.to_owned();
                        // Compatible servers (llama.cpp) omit the empty start input.
                        let initial = match block.get("input") {
                            None => Map::new(),
                            Some(input) => input.as_object().cloned().ok_or(E::Protocol)?,
                        };
                        let calls = self
                            .blocks
                            .values()
                            .filter(|(b, _)| matches!(b, Block::ToolUse { .. }));
                        let mut count = 0;
                        for (call, _) in calls {
                            count += 1;
                            if matches!(call, Block::ToolUse { id: other, .. } if *other == id) {
                                return Err(E::Protocol);
                            }
                        }
                        if count >= MAX_CALLS {
                            return Err(E::Protocol);
                        }
                        let tool = Block::ToolUse {
                            id,
                            name,
                            initial,
                            json: String::new(),
                            input: None,
                        };
                        (tool, Update::None)
                    }
                    _ => (Block::Other, Update::None),
                };
                self.blocks.insert(index, (parsed, false));
                Ok(update)
            }
            "content_block_delta" => {
                let index = v["index"].as_u64().ok_or(E::Protocol)?;
                let delta = &v["delta"];
                let delta_kind = string(delta, "type")?;
                let (block, done) = self.blocks.get_mut(&index).ok_or(E::Protocol)?;
                if *done {
                    return Err(E::Protocol);
                }
                match (block, delta_kind) {
                    (Block::Text, "text_delta") => {
                        let text = string(delta, "text")?.to_owned();
                        self.text(&text)
                    }
                    (Block::Text, "citations_delta")
                    | (Block::Thinking, "thinking_delta" | "signature_delta")
                    | (Block::Other, _) => Ok(Update::None),
                    (Block::ToolUse { json, .. }, "input_json_delta") => {
                        let partial = string(delta, "partial_json")?;
                        if json.len() + partial.len() > MAX_FRAME {
                            return Err(E::Protocol);
                        }
                        json.push_str(partial);
                        Ok(Update::None)
                    }
                    _ => Err(E::Protocol),
                }
            }
            "content_block_stop" => {
                let index = v["index"].as_u64().ok_or(E::Protocol)?;
                let (block, done) = self.blocks.get_mut(&index).ok_or(E::Protocol)?;
                if *done {
                    return Err(E::Protocol);
                }
                if let Block::ToolUse {
                    initial,
                    json,
                    input,
                    ..
                } = block
                {
                    let parsed = if json.trim().is_empty() {
                        Value::Object(std::mem::take(initial))
                    } else {
                        serde_json::from_str(json).map_err(|_| E::EventJson)?
                    };
                    if !parsed.is_object() {
                        return Err(E::Protocol);
                    }
                    *input = Some(parsed);
                }
                *done = true;
                Ok(Update::None)
            }
            "message_delta" => {
                // A server-side refusal fallback can send this more than once;
                // the last stop reason wins.
                if let Some(reason) = v["delta"]["stop_reason"].as_str() {
                    self.stop_reason = Some(reason.to_owned());
                }
                Ok(Update::None)
            }
            "message_stop" => {
                if self.blocks.values().any(|(_, done)| !done) {
                    return Err(E::Protocol);
                }
                match self.stop_reason.as_deref() {
                    Some("end_turn" | "tool_use" | "stop_sequence") => {}
                    Some("max_tokens" | "model_context_window_exceeded") => {
                        return Err(E::Truncated);
                    }
                    Some("refusal") => return Err(E::Refused),
                    _ => return Err(E::Protocol),
                }
                self.stopped = true;
                Ok(Update::Complete)
            }
            "ping" => Ok(Update::None),
            "error" => Err(if v["error"]["type"] == "overloaded_error" {
                E::Overloaded
            } else {
                E::Api
            }),
            // The Messages API may add event types; unknown ones are inert.
            _ => Ok(Update::None),
        }
    }

    /// Completed tool calls in block order: `(id, wire name, input)`.
    pub fn calls(&self) -> Result<Vec<(String, String, Value)>, E> {
        self.blocks
            .values()
            .filter_map(|(block, _)| match block {
                Block::ToolUse {
                    id, name, input, ..
                } => Some(
                    input
                        .clone()
                        .map(|input| (id.clone(), name.clone(), input))
                        .ok_or(E::Protocol),
                ),
                _ => None,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rah_protocol::{PermissionLevel, ToolName};

    fn definition(name: &str, schema: Value) -> ToolDefinition {
        ToolDefinition {
            name: ToolName::new(name),
            description: "d".into(),
            input_schema: schema,
            permission: PermissionLevel::None,
        }
    }

    #[test]
    fn tool_names_are_wire_safe_unique_and_reversible() {
        let schema = json!({"type": "object"});
        let names = ToolNames::new(&[
            definition("fs.read", schema.clone()),
            definition("fs_read", schema.clone()),
            definition("echo", schema),
        ]);
        assert_eq!(names.wire("fs.read"), "fs_read");
        assert_eq!(names.wire("fs_read"), "fs_read_2");
        assert_eq!(names.original("fs_read"), Some("fs.read"));
        assert_eq!(names.original("fs_read_2"), Some("fs_read"));
        assert_eq!(names.original("nope"), None);
    }

    #[test]
    fn tool_schemas_must_describe_objects() {
        let names = ToolNames::default();
        let tools = tools(&[definition("a", json!({"properties": {}}))], &names).unwrap();
        assert_eq!(tools[0]["input_schema"]["type"], "object");
        assert_eq!(tools[0]["description"], "d");
        for schema in [json!(true), json!({"type": "string"})] {
            assert_eq!(
                super::tools(&[definition("a", schema)], &names),
                Err(E::ToolSchema)
            );
        }
    }

    #[test]
    fn context_converts_to_alternating_messages() {
        let names = ToolNames::new(&[definition("fs.read", json!({"type": "object"}))]);
        let items = [
            ContextItem::System { text: "s1".into() },
            ContextItem::System { text: "s2".into() },
            ContextItem::User { text: "u".into() },
            ContextItem::Assistant { text: "a".into() },
            ContextItem::ToolCall {
                call_id: "call.1".into(),
                name: ToolName::new("fs.read"),
                arguments: json!({"path": "x"}),
            },
            ContextItem::System {
                text: "note".into(),
            },
            ContextItem::ToolResult {
                call_id: "call.1".into(),
                output: ToolOutput {
                    content: vec![ToolContent::Text("nope".into())],
                    is_error: true,
                },
            },
            ContextItem::Assistant {
                text: String::new(),
            },
        ];
        let (system, messages) = conversation(&items, &names).unwrap();
        assert_eq!(system.as_deref(), Some("s1\n\ns2"));
        assert_eq!(
            Value::Array(messages),
            json!([
                {"role": "user", "content": [{"type": "text", "text": "u"}]},
                {"role": "assistant", "content": [
                    {"type": "text", "text": "a"},
                    {"type": "tool_use", "id": "call_1", "name": "fs_read", "input": {"path": "x"}}
                ]},
                {"role": "user", "content": [
                    {"type": "tool_result", "tool_use_id": "call_1", "content": "nope", "is_error": true},
                    {"type": "text", "text": "[System note] note"}
                ]}
            ])
        );
    }

    #[test]
    fn tool_result_images_become_image_blocks() {
        let names = ToolNames::new(&[definition("shot", json!({"type": "object"}))]);
        let items = [
            ContextItem::User { text: "u".into() },
            ContextItem::ToolCall {
                call_id: "c1".into(),
                name: ToolName::new("shot"),
                arguments: json!({}),
            },
            ContextItem::ToolResult {
                call_id: "c1".into(),
                output: ToolOutput {
                    content: vec![
                        ToolContent::Text("window".into()),
                        ToolContent::Image(rah_protocol::ImageContent {
                            media_type: "image/png".into(),
                            data: "AAAA".into(),
                        }),
                    ],
                    is_error: false,
                },
            },
        ];
        let (_, messages) = conversation(&items, &names).unwrap();
        assert_eq!(
            messages[2]["content"][0]["content"],
            json!([
                {"type": "text", "text": "window"},
                {"type": "image", "source": {"type": "base64", "media_type": "image/png", "data": "AAAA"}}
            ])
        );
    }

    fn start() -> Response {
        let mut response = Response::default();
        response
            .event(json!({"type": "message_start", "message": {"type": "message", "role": "assistant"}}))
            .unwrap();
        response
    }

    #[test]
    fn events_before_message_start_after_stop_or_on_unknown_blocks_fail() {
        let mut fresh = Response::default();
        assert!(matches!(
            fresh.event(json!({"type": "content_block_stop", "index": 0})),
            Err(E::Protocol)
        ));
        let mut response = start();
        assert!(matches!(
            response.event(json!({"type": "content_block_delta", "index": 3, "delta": {"type": "text_delta", "text": "x"}})),
            Err(E::Protocol)
        ));
        let mut response = start();
        response
            .event(json!({"type": "message_delta", "delta": {"stop_reason": "end_turn"}}))
            .unwrap();
        response.event(json!({"type": "message_stop"})).unwrap();
        assert!(matches!(
            response.event(json!({"type": "ping"})),
            Err(E::Protocol)
        ));
    }

    #[test]
    fn stop_reasons_map_to_typed_failures() {
        for (reason, expected) in [
            ("max_tokens", E::Truncated),
            ("model_context_window_exceeded", E::Truncated),
            ("refusal", E::Refused),
            ("pause_turn", E::Protocol),
        ] {
            let mut response = start();
            response
                .event(json!({"type": "message_delta", "delta": {"stop_reason": reason}}))
                .unwrap();
            assert!(
                matches!(response.event(json!({"type": "message_stop"})), Err(e) if e == expected),
                "{reason}"
            );
        }
    }

    #[test]
    fn unknown_event_and_block_types_are_inert() {
        let mut response = start();
        for event in [
            json!({"type": "future_event"}),
            json!({"type": "content_block_start", "index": 0, "content_block": {"type": "fallback"}}),
            json!({"type": "content_block_delta", "index": 0, "delta": {"type": "anything"}}),
            json!({"type": "content_block_stop", "index": 0}),
            json!({"type": "message_delta", "delta": {"stop_reason": "end_turn"}}),
        ] {
            assert!(matches!(response.event(event), Ok(Update::None)));
        }
        assert!(matches!(
            response.event(json!({"type": "message_stop"})),
            Ok(Update::Complete)
        ));
    }

    /// llama.cpp's compatible endpoint: interleaved block lifetimes and a
    /// tool-use start without `input`.
    #[test]
    fn llamacpp_style_streams_are_accepted() {
        let mut response = start();
        for event in [
            json!({"type": "content_block_start", "index": 0, "content_block": {"type": "thinking", "thinking": ""}}),
            json!({"type": "content_block_delta", "index": 0, "delta": {"type": "thinking_delta", "thinking": "hm"}}),
            json!({"type": "content_block_start", "index": 1, "content_block": {"type": "tool_use", "id": "abc", "name": "fs_read"}}),
            json!({"type": "content_block_delta", "index": 1, "delta": {"type": "input_json_delta", "partial_json": "{\"path\":\"a\"}"}}),
            json!({"type": "content_block_delta", "index": 0, "delta": {"type": "signature_delta", "signature": ""}}),
            json!({"type": "content_block_stop", "index": 0}),
            json!({"type": "content_block_stop", "index": 1}),
            json!({"type": "message_delta", "delta": {"stop_reason": "tool_use"}}),
            json!({"type": "message_stop"}),
        ] {
            response.event(event).unwrap();
        }
        assert_eq!(
            response.calls().unwrap(),
            [("abc".to_owned(), "fs_read".to_owned(), json!({"path": "a"}))]
        );
        let mut bad = start();
        assert!(matches!(
            bad.event(json!({"type": "content_block_start", "index": 0, "content_block": {"type": "tool_use", "id": "x", "name": "n", "input": "nope"}})),
            Err(E::Protocol)
        ));
    }
}
