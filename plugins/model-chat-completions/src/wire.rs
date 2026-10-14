//! Chat Completions wire translation. Provider formats stay inside this
//! plugin.

use std::collections::{BTreeMap, HashMap, HashSet};

use rah_api::tools::describe_image;
use rah_protocol::{ContextItem, ModelUsage, ToolContent, ToolDefinition, ToolOutput};
use serde_json::{Map, Value, json};

use crate::{error::E, sse::MAX_FRAME};

pub(crate) const MAX_BYTES: usize = 4 * 1024 * 1024;
/// The largest request body.
pub(crate) const MAX_INPUT_BYTES: usize = 32 * 1024 * 1024;
pub(crate) const MAX_CALLS: usize = 128;

/// Maps RAH tool names to provider-safe function names (`[A-Za-z0-9_-]`,
/// at most 64 characters) and back. Collisions get a numeric suffix.
#[derive(Default)]
pub(crate) struct ToolNames {
    to_wire: HashMap<String, String>,
    to_original: HashMap<String, String>,
}

fn sanitize(name: &str) -> String {
    let mut wire: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    wire.truncate(60);
    if wire.is_empty() {
        wire.push('_');
    }
    wire
}

impl ToolNames {
    pub fn new(definitions: &[ToolDefinition]) -> Self {
        let mut names = Self::default();
        for definition in definitions {
            let original = definition.name.as_str();
            if names.to_wire.contains_key(original) {
                continue;
            }
            let base = sanitize(original);
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
        self.to_wire
            .get(original)
            .cloned()
            .unwrap_or_else(|| sanitize(original))
    }

    pub fn original(&self, wire: &str) -> Option<&str> {
        self.to_original.get(wire).map(String::as_str)
    }
}

pub(crate) fn tools(definitions: &[ToolDefinition], names: &ToolNames) -> Result<Vec<Value>, E> {
    let mut seen = HashSet::new();
    definitions
        .iter()
        .filter(|d| seen.insert(d.name.as_str().to_owned()))
        .map(|d| {
            let size = serde_json::to_vec(&d.input_schema)
                .map_err(|_| E::ToolSchema)?
                .len();
            if !d.input_schema.is_object() || size > MAX_BYTES {
                return Err(E::ToolSchema);
            }
            Ok(json!({
                "type": "function",
                "function": {
                    "name": names.wire(d.name.as_str()),
                    "description": d.description,
                    "parameters": d.input_schema,
                },
            }))
        })
        .collect()
}

/// A tool message's content: text only. Many servers accept nothing else in
/// a `tool` message, so an image becomes a short text.
fn output(output: &ToolOutput) -> String {
    let text = output
        .content
        .iter()
        .map(|content| match content {
            ToolContent::Text(text) => text.clone(),
            ToolContent::Json(value) => value.to_string(),
            ToolContent::Image(image) => describe_image(image),
        })
        .collect::<Vec<_>>()
        .join("\n");
    if output.is_error {
        format!("[error] {text}")
    } else {
        text
    }
}

/// Converts neutral context into Chat Completions messages. Tool calls join
/// the assistant message right before them (or a new one with no content),
/// as the format requires; results become `tool` messages.
pub(crate) fn messages(items: &[ContextItem], names: &ToolNames) -> Result<Vec<Value>, E> {
    let mut messages: Vec<Value> = Vec::with_capacity(items.len());
    for item in items {
        match item {
            ContextItem::System { text } => {
                messages.push(json!({"role": "system", "content": text}));
            }
            ContextItem::User { text } => messages.push(json!({"role": "user", "content": text})),
            ContextItem::Assistant { text } => {
                messages.push(json!({"role": "assistant", "content": text}));
            }
            ContextItem::ToolCall {
                call_id,
                name,
                arguments,
            } => {
                let call = json!({
                    "id": call_id,
                    "type": "function",
                    "function": {
                        "name": names.wire(name.as_str()),
                        "arguments": arguments.to_string(),
                    },
                });
                let open = messages
                    .last_mut()
                    .filter(|last| last["role"] == "assistant")
                    .and_then(Value::as_object_mut);
                match open {
                    Some(last) => match last.get_mut("tool_calls").and_then(Value::as_array_mut) {
                        Some(calls) => calls.push(call),
                        None => {
                            last.insert("tool_calls".into(), json!([call]));
                        }
                    },
                    None => messages.push(json!({
                        "role": "assistant",
                        "content": null,
                        "tool_calls": [call],
                    })),
                }
            }
            ContextItem::ToolResult { call_id, output } => messages.push(json!({
                "role": "tool",
                "tool_call_id": call_id,
                "content": self::output(output),
            })),
        }
    }
    if serde_json::to_vec(&messages)
        .map_err(|_| E::Protocol)?
        .len()
        > MAX_INPUT_BYTES
    {
        return Err(E::Protocol);
    }
    Ok(messages)
}

/// What one chunk adds to the response.
#[derive(Debug, PartialEq)]
pub(crate) enum Update {
    Text(String),
    /// Reasoning progress; the reasoning content is discarded.
    Reasoning,
}

#[derive(Default)]
struct Call {
    id: Option<String>,
    name: String,
    arguments: String,
}

/// A completed tool call: `(call_id, wire name, arguments)`.
pub(crate) type CompletedCall = (String, String, Value);

/// Validating accumulator for one streamed response. Only choice 0 is
/// accepted, since requests never ask for more.
#[derive(Default)]
pub(crate) struct Response {
    calls: BTreeMap<u64, Call>,
    text: usize,
    finish: Option<String>,
    usage: ModelUsage,
}

fn text_field<'a>(delta: &'a Value, key: &str) -> Result<Option<&'a str>, E> {
    match delta.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) => Ok((!text.is_empty()).then_some(text.as_str())),
        Some(_) => Err(E::Protocol),
    }
}

impl Response {
    pub fn chunk(&mut self, chunk: &Value) -> Result<Vec<Update>, E> {
        if !chunk.is_object() {
            return Err(E::Protocol);
        }
        if chunk.get("error").is_some_and(|error| !error.is_null()) {
            return Err(E::Api);
        }
        if let Some(usage) = chunk.get("usage").filter(|usage| usage.is_object()) {
            self.usage = ModelUsage {
                input_tokens: usage["prompt_tokens"].as_u64(),
                output_tokens: usage["completion_tokens"].as_u64(),
                // OpenAI and vLLM; DeepSeek reports its cache hits separately.
                cached_input_tokens: usage["prompt_tokens_details"]["cached_tokens"]
                    .as_u64()
                    .or_else(|| usage["prompt_cache_hit_tokens"].as_u64()),
            };
        }
        let mut updates = Vec::new();
        let choices = match chunk.get("choices") {
            None | Some(Value::Null) => return Ok(updates),
            Some(Value::Array(choices)) => choices,
            Some(_) => return Err(E::Protocol),
        };
        for choice in choices {
            if choice["index"].as_u64().unwrap_or(0) != 0 {
                return Err(E::Protocol);
            }
            let delta = match choice.get("delta") {
                None | Some(Value::Null) => &Value::Null,
                Some(delta) if delta.is_object() => delta,
                Some(_) => return Err(E::Protocol),
            };
            let reasoning = text_field(delta, "reasoning_content")?
                .or(text_field(delta, "reasoning")?)
                .is_some();
            let text = text_field(delta, "content")?;
            let calls = match delta.get("tool_calls") {
                None | Some(Value::Null) => &[][..],
                Some(Value::Array(calls)) => calls.as_slice(),
                Some(_) => return Err(E::Protocol),
            };
            if self.finish.is_some() && (reasoning || text.is_some() || !calls.is_empty()) {
                return Err(E::Protocol);
            }
            if reasoning {
                updates.push(Update::Reasoning);
            }
            if let Some(text) = text {
                self.text += text.len();
                if self.text > MAX_BYTES {
                    return Err(E::Protocol);
                }
                updates.push(Update::Text(text.to_owned()));
            }
            for call in calls {
                self.call(call)?;
            }
            match choice.get("finish_reason") {
                None | Some(Value::Null) => {}
                Some(Value::String(reason)) => {
                    if self.finish.as_ref().is_some_and(|seen| seen != reason) {
                        return Err(E::Protocol);
                    }
                    self.finish = Some(reason.clone());
                }
                Some(_) => return Err(E::Protocol),
            }
        }
        Ok(updates)
    }

    /// One tool-call fragment. The first fragment of a call names it; later
    /// ones append arguments. A server may also send a whole call at once.
    fn call(&mut self, fragment: &Value) -> Result<(), E> {
        let id = text_field(fragment, "id")?;
        let index = match fragment.get("index") {
            Some(index) => index.as_u64().ok_or(E::Protocol)?,
            // Without an index, a new id starts a call; anything else
            // continues the last one.
            None => match (id, self.calls.last_key_value()) {
                (Some(id), Some((_, last))) if last.id.as_deref() != Some(id) => {
                    self.calls.len() as u64
                }
                (_, Some((index, _))) => *index,
                (_, None) => 0,
            },
        };
        if !self.calls.contains_key(&index) && self.calls.len() >= MAX_CALLS {
            return Err(E::Protocol);
        }
        let call = self.calls.entry(index).or_default();
        if let Some(id) = id {
            match &call.id {
                Some(seen) if seen != id => return Err(E::Protocol),
                Some(_) => {}
                None => call.id = Some(id.to_owned()),
            }
        }
        let function = match fragment.get("function") {
            None | Some(Value::Null) => &Value::Null,
            Some(function) if function.is_object() => function,
            Some(_) => return Err(E::Protocol),
        };
        if let Some(name) = text_field(function, "name")? {
            if call.name.is_empty() {
                call.name = name.to_owned();
            } else if call.name != name {
                return Err(E::Protocol);
            }
        }
        match function.get("arguments") {
            None | Some(Value::Null) => {}
            Some(Value::String(arguments)) => call.arguments.push_str(arguments),
            // A few servers send the arguments as an object, all at once.
            Some(arguments @ Value::Object(_)) if call.arguments.is_empty() => {
                call.arguments = arguments.to_string();
            }
            Some(_) => return Err(E::Protocol),
        }
        if call.arguments.len() > MAX_FRAME * 4 {
            return Err(E::Protocol);
        }
        Ok(())
    }

    /// Checks how the response ended and returns its tool calls in order.
    /// A call without an id gets `call_<index>`, unique within the response.
    pub fn finish(&self) -> Result<Vec<CompletedCall>, E> {
        match self.finish.as_deref() {
            None => return Err(E::Protocol),
            Some("length") => return Err(E::Truncated),
            Some("content_filter") => return Err(E::Filtered),
            Some(_) => {}
        }
        let mut ids = HashSet::new();
        self.calls
            .iter()
            .map(|(index, call)| {
                if call.name.is_empty() {
                    return Err(E::Protocol);
                }
                let id = call.id.clone().unwrap_or_else(|| format!("call_{index}"));
                if !ids.insert(id.clone()) {
                    return Err(E::Protocol);
                }
                let arguments = if call.arguments.trim().is_empty() {
                    Value::Object(Map::new())
                } else {
                    serde_json::from_str(&call.arguments).map_err(|_| E::ChunkJson)?
                };
                if !arguments.is_object() {
                    return Err(E::Protocol);
                }
                Ok((id, call.name.clone(), arguments))
            })
            .collect()
    }

    /// The token counts the server reported, if any.
    pub fn usage(&self) -> Option<ModelUsage> {
        (!self.usage.is_empty()).then_some(self.usage)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rah_protocol::{ImageContent, PermissionLevel, ToolName};

    fn definition(name: &str) -> ToolDefinition {
        ToolDefinition {
            name: ToolName::new(name),
            description: "d".into(),
            input_schema: json!({"type": "object"}),
            permission: PermissionLevel::None,
        }
    }

    fn call(call_id: &str, name: &str) -> ContextItem {
        ContextItem::ToolCall {
            call_id: call_id.into(),
            name: ToolName::new(name),
            arguments: json!({"path": "x"}),
        }
    }

    fn result(call_id: &str, content: Vec<ToolContent>, is_error: bool) -> ContextItem {
        ContextItem::ToolResult {
            call_id: call_id.into(),
            output: ToolOutput { content, is_error },
        }
    }

    #[test]
    fn tool_names_are_wire_safe_unique_and_reversible() {
        let names = ToolNames::new(&[
            definition("fs.read"),
            definition("fs_read"),
            definition("echo"),
        ]);
        assert_eq!(names.wire("fs.read"), "fs_read");
        assert_eq!(names.wire("fs_read"), "fs_read_2");
        assert_eq!(names.original("fs_read"), Some("fs.read"));
        assert_eq!(names.original("fs_read_2"), Some("fs_read"));
        assert_eq!(names.original("nope"), None);
        let tools = tools(&[definition("fs.read")], &names).unwrap();
        assert_eq!(tools[0]["type"], "function");
        assert_eq!(tools[0]["function"]["name"], "fs_read");
        let not_object = ToolDefinition {
            input_schema: json!(true),
            ..definition("x")
        };
        assert_eq!(
            super::tools(&[not_object], &ToolNames::default()),
            Err(E::ToolSchema)
        );
    }

    #[test]
    fn tool_calls_join_the_assistant_message_before_them() {
        let names = ToolNames::new(&[definition("fs.read")]);
        let items = [
            ContextItem::System { text: "s".into() },
            ContextItem::User { text: "u".into() },
            ContextItem::Assistant { text: "a".into() },
            call("c1", "fs.read"),
            call("c2", "fs.read"),
            result("c1", vec![ToolContent::Text("nope".into())], true),
            result("c2", vec![ToolContent::Json(json!({"ok": true}))], false),
            call("c3", "fs.read"),
        ];
        let messages = messages(&items, &names).unwrap();
        assert_eq!(messages.len(), 6, "{messages:?}");
        assert_eq!(messages[0], json!({"role": "system", "content": "s"}));
        assert_eq!(messages[2]["content"], "a");
        let calls = messages[2]["tool_calls"].as_array().unwrap();
        assert_eq!(calls.len(), 2);
        assert_eq!(
            calls[0],
            json!({"id": "c1", "type": "function", "function": {"name": "fs_read", "arguments": "{\"path\":\"x\"}"}})
        );
        assert_eq!(
            messages[3],
            json!({"role": "tool", "tool_call_id": "c1", "content": "[error] nope"})
        );
        assert_eq!(messages[4]["content"], "{\"ok\":true}");
        // A call after a tool result starts an assistant message of its own.
        assert_eq!(messages[5]["role"], "assistant");
        assert_eq!(messages[5]["content"], Value::Null);
        assert_eq!(messages[5]["tool_calls"][0]["id"], "c3");
    }

    #[test]
    fn images_in_tool_results_become_text() {
        let image = ToolContent::Image(ImageContent {
            media_type: "image/png".into(),
            data: "AAAA".into(),
        });
        let items = [result(
            "c1",
            vec![ToolContent::Text("window".into()), image],
            false,
        )];
        let messages = messages(&items, &ToolNames::default()).unwrap();
        assert_eq!(messages[0]["content"], "window\n[image/png image, 1 KiB]");
    }

    fn delta(delta: Value) -> Value {
        json!({"choices": [{"index": 0, "delta": delta, "finish_reason": null}]})
    }

    fn finish(reason: &str) -> Value {
        json!({"choices": [{"index": 0, "delta": {}, "finish_reason": reason}]})
    }

    #[test]
    fn text_reasoning_calls_and_usage_accumulate() {
        let mut response = Response::default();
        assert_eq!(
            response
                .chunk(&delta(json!({"role": "assistant", "content": ""})))
                .unwrap(),
            []
        );
        assert_eq!(
            response
                .chunk(&delta(json!({"reasoning_content": "hmm"})))
                .unwrap(),
            [Update::Reasoning]
        );
        assert_eq!(
            response.chunk(&delta(json!({"reasoning": "hmm"}))).unwrap(),
            [Update::Reasoning]
        );
        assert_eq!(
            response.chunk(&delta(json!({"content": "Hi"}))).unwrap(),
            [Update::Text("Hi".into())]
        );
        for fragment in [
            json!({"index": 0, "id": "call_a", "type": "function", "function": {"name": "fs_read", "arguments": ""}}),
            json!({"index": 0, "function": {"arguments": "{\"pa"}}),
            json!({"index": 1, "id": "call_b", "function": {"name": "echo", "arguments": "{}"}}),
            json!({"index": 0, "function": {"arguments": "th\":\"x\"}"}}),
        ] {
            response
                .chunk(&delta(json!({"tool_calls": [fragment]})))
                .unwrap();
        }
        response.chunk(&finish("tool_calls")).unwrap();
        response
            .chunk(&json!({"choices": [], "usage": {"prompt_tokens": 10, "completion_tokens": 4, "prompt_cache_hit_tokens": 6}}))
            .unwrap();
        assert_eq!(
            response.finish().unwrap(),
            [
                ("call_a".into(), "fs_read".into(), json!({"path": "x"})),
                ("call_b".into(), "echo".into(), json!({})),
            ]
        );
        assert_eq!(
            response.usage(),
            Some(ModelUsage {
                input_tokens: Some(10),
                output_tokens: Some(4),
                cached_input_tokens: Some(6),
            })
        );
    }

    #[test]
    fn whole_calls_without_index_or_id_are_accepted() {
        let mut response = Response::default();
        response
            .chunk(&delta(json!({"tool_calls": [
                {"function": {"name": "echo", "arguments": {"text": "hi"}}},
            ]})))
            .unwrap();
        response
            .chunk(&delta(json!({"tool_calls": [
                {"id": "x", "function": {"name": "fs_read", "arguments": ""}},
            ]})))
            .unwrap();
        response.chunk(&finish("stop")).unwrap();
        assert_eq!(
            response.finish().unwrap(),
            [
                ("call_0".into(), "echo".into(), json!({"text": "hi"})),
                ("x".into(), "fs_read".into(), json!({})),
            ]
        );
    }

    #[test]
    fn malformed_and_unfinished_responses_fail() {
        let failures: Vec<(Vec<Value>, E)> = vec![
            (vec![json!({"error": {"message": "boom"}})], E::Api),
            (
                vec![json!({"choices": [{"index": 1, "delta": {}}]})],
                E::Protocol,
            ),
            (vec![delta(json!({"content": 5}))], E::Protocol),
            (vec![json!({"choices": {}})], E::Protocol),
            (
                vec![
                    delta(
                        json!({"tool_calls": [{"index": 0, "id": "a", "function": {"name": "x"}}]}),
                    ),
                    delta(json!({"tool_calls": [{"index": 0, "id": "b"}]})),
                ],
                E::Protocol,
            ),
            (
                vec![
                    delta(json!({"tool_calls": [{"index": 0, "function": {"name": "x"}}]})),
                    delta(json!({"tool_calls": [{"index": 0, "function": {"name": "y"}}]})),
                ],
                E::Protocol,
            ),
            (
                vec![finish("stop"), delta(json!({"content": "late"}))],
                E::Protocol,
            ),
            (vec![finish("stop"), finish("length")], E::Protocol),
        ];
        for (chunks, expected) in failures {
            let mut response = Response::default();
            let error = chunks
                .iter()
                .find_map(|chunk| response.chunk(chunk).err())
                .unwrap_or_else(|| panic!("{chunks:?} was accepted"));
            assert_eq!(error, expected, "{chunks:?}");
        }

        let ends: Vec<(Vec<Value>, E)> = vec![
            (vec![delta(json!({"content": "cut"}))], E::Protocol),
            (vec![finish("length")], E::Truncated),
            (vec![finish("content_filter")], E::Filtered),
            (
                vec![
                    delta(json!({"tool_calls": [{"index": 0, "function": {"arguments": "{}"}}]})),
                    finish("tool_calls"),
                ],
                E::Protocol,
            ),
            (
                vec![
                    delta(
                        json!({"tool_calls": [{"index": 0, "function": {"name": "x", "arguments": "{"}}]}),
                    ),
                    finish("tool_calls"),
                ],
                E::ChunkJson,
            ),
            (
                vec![
                    delta(
                        json!({"tool_calls": [{"index": 0, "function": {"name": "x", "arguments": "[1]"}}]}),
                    ),
                    finish("tool_calls"),
                ],
                E::Protocol,
            ),
            (
                vec![
                    delta(json!({"tool_calls": [
                        {"index": 0, "id": "same", "function": {"name": "x"}},
                        {"index": 1, "id": "same", "function": {"name": "y"}},
                    ]})),
                    finish("tool_calls"),
                ],
                E::Protocol,
            ),
        ];
        for (chunks, expected) in ends {
            let mut response = Response::default();
            for chunk in &chunks {
                response.chunk(chunk).unwrap();
            }
            assert_eq!(response.finish().err(), Some(expected), "{chunks:?}");
        }
    }

    #[test]
    fn too_many_calls_fail() {
        let mut response = Response::default();
        let calls: Vec<Value> = (0..=MAX_CALLS)
            .map(|index| json!({"index": index, "function": {"name": "x"}}))
            .collect();
        assert_eq!(
            response.chunk(&delta(json!({"tool_calls": calls}))),
            Err(E::Protocol)
        );
    }
}
