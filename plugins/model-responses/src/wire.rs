//! Responses API wire translation. Provider formats stay inside this plugin.

use std::collections::{BTreeMap, HashMap, HashSet};

use rah_api::tools::describe_image;
use rah_protocol::{ContextItem, ToolContent, ToolDefinition, ToolOutput};
use serde_json::{Value, json};

use crate::{error::E, sse::MAX_FRAME};

pub(crate) const MAX_BYTES: usize = 4 * 1024 * 1024;
/// The largest request input; images make it larger than [`MAX_BYTES`].
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
                "name": names.wire(d.name.as_str()),
                "description": d.description,
                "parameters": d.input_schema,
                "strict": strict(&d.input_schema),
            }))
        })
        .collect()
}

/// Conservative strict-mode recognition. Never normalizes a schema; other
/// object schemas are sent non-strict with their exact JSON.
fn strict(v: &Value) -> bool {
    let Some(map) = v.as_object() else {
        return false;
    };
    if map.keys().any(|k| {
        !matches!(
            k.as_str(),
            "type"
                | "properties"
                | "required"
                | "additionalProperties"
                | "items"
                | "enum"
                | "description"
        )
    }) {
        return false;
    }
    match v["type"].as_str() {
        Some("object") => {
            let (Some(p), Some(r)) = (v["properties"].as_object(), v["required"].as_array()) else {
                return false;
            };
            v["additionalProperties"] == false
                && p.len() == r.len()
                && p.iter()
                    .all(|(k, s)| r.iter().any(|a| a.as_str() == Some(k)) && strict(s))
        }
        Some("array") => strict(&v["items"]),
        Some("string" | "number" | "integer" | "boolean" | "null") => true,
        _ => false,
    }
}

/// A function call output: text, or with `images`, a list of text and image
/// parts. Without `images`, an image becomes a short text.
fn output(output: &ToolOutput, images: bool) -> Value {
    let mut texts = Vec::new();
    let mut pictures = Vec::new();
    for content in &output.content {
        match content {
            ToolContent::Text(text) => texts.push(text.clone()),
            ToolContent::Json(value) => texts.push(value.to_string()),
            ToolContent::Image(image) if images => pictures.push(json!({
                "type": "input_image",
                "image_url": format!("data:{};base64,{}", image.media_type, image.data),
            })),
            ToolContent::Image(image) => texts.push(describe_image(image)),
        }
    }
    let text = texts.join("\n");
    let text = if output.is_error {
        format!("[error] {text}")
    } else {
        text
    };
    if pictures.is_empty() {
        return text.into();
    }
    let mut parts = Vec::with_capacity(pictures.len() + 1);
    if !text.is_empty() {
        parts.push(json!({"type": "input_text", "text": text}));
    }
    parts.append(&mut pictures);
    parts.into()
}

/// Converts neutral context into Responses input items. llama.cpp needs an
/// explicit `type: message` to tell replayed assistant text from user input.
/// `images` says whether the model takes image input.
pub(crate) fn input(
    items: &[ContextItem],
    names: &ToolNames,
    llamacpp: bool,
    images: bool,
) -> Result<Vec<Value>, E> {
    let input: Vec<Value> = items
        .iter()
        .map(|item| match item {
            ContextItem::System { text } => json!({"role": "system", "content": text}),
            ContextItem::User { text } => json!({"role": "user", "content": text}),
            ContextItem::Assistant { text } if llamacpp => {
                json!({"type": "message", "role": "assistant", "content": text})
            }
            ContextItem::Assistant { text } => json!({"role": "assistant", "content": text}),
            ContextItem::ToolCall {
                call_id,
                name,
                arguments,
            } => json!({
                "type": "function_call",
                "call_id": call_id,
                "name": names.wire(name.as_str()),
                "arguments": arguments.to_string(),
            }),
            ContextItem::ToolResult { call_id, output } => json!({
                "type": "function_call_output",
                "call_id": call_id,
                "output": self::output(output, images),
            }),
        })
        .collect();
    if serde_json::to_vec(&input).map_err(|_| E::Protocol)?.len() > MAX_INPUT_BYTES {
        return Err(E::Protocol);
    }
    Ok(input)
}

pub(crate) fn bounded(items: &[Value]) -> Result<(), E> {
    if serde_json::to_vec(items).map_err(|_| E::Protocol)?.len() > MAX_BYTES {
        Err(E::Protocol)
    } else {
        Ok(())
    }
}

pub(crate) fn string<'a>(v: &'a Value, key: &str) -> Result<&'a str, E> {
    v.get(key).and_then(Value::as_str).ok_or(E::Protocol)
}

/// Validating accumulator for one streamed response.
#[derive(Default)]
pub(crate) struct Response {
    calls: BTreeMap<u64, Call>,
    text: String,
    completed: bool,
}

struct Call {
    item: Value,
    arguments: String,
    args_done: bool,
    done: bool,
}

pub(crate) enum Update {
    Text(String),
    /// Reasoning progress; the reasoning content is discarded.
    Reasoning,
    Complete,
    None,
}

impl Response {
    pub fn event(&mut self, v: Value) -> Result<Update, E> {
        if self.completed {
            return Err(E::Protocol);
        }
        match string(&v, "type")? {
            "response.output_text.delta" => {
                let delta = string(&v, "delta")?.to_owned();
                if self.text.len() + delta.len() > MAX_BYTES {
                    return Err(E::Protocol);
                }
                self.text.push_str(&delta);
                Ok(Update::Text(delta))
            }
            "response.output_item.added" => {
                let item = &v["item"];
                match string(item, "type")? {
                    "function_call" => {
                        let index = v["output_index"].as_u64().ok_or(E::Protocol)?;
                        for key in ["id", "call_id", "name", "arguments"] {
                            string(item, key)?;
                        }
                        if self.calls.len() >= MAX_CALLS
                            || self.calls.contains_key(&index)
                            || self.calls.values().any(|c| {
                                c.item["call_id"] == item["call_id"] || c.item["id"] == item["id"]
                            })
                        {
                            return Err(E::Protocol);
                        }
                        self.calls.insert(
                            index,
                            Call {
                                item: item.clone(),
                                arguments: string(item, "arguments")?.into(),
                                args_done: false,
                                done: false,
                            },
                        );
                    }
                    "message" | "reasoning" => {}
                    _ => return Err(E::Protocol),
                }
                Ok(Update::None)
            }
            "response.function_call_arguments.delta" | "response.function_call_arguments.done" => {
                let index = v["output_index"].as_u64().ok_or(E::Protocol)?;
                let call = self.calls.get_mut(&index).ok_or(E::Protocol)?;
                if call.args_done || call.done || v["item_id"] != call.item["id"] {
                    return Err(E::Protocol);
                }
                if v["type"] == "response.function_call_arguments.delta" {
                    let delta = string(&v, "delta")?;
                    if call.arguments.len() + delta.len() > MAX_FRAME {
                        return Err(E::Protocol);
                    }
                    call.arguments.push_str(delta);
                } else {
                    if string(&v, "arguments")? != call.arguments {
                        return Err(E::Protocol);
                    }
                    call.args_done = true;
                }
                Ok(Update::None)
            }
            "response.output_item.done" => {
                let item = &v["item"];
                match string(item, "type")? {
                    "function_call" => {
                        let index = v["output_index"].as_u64().ok_or(E::Protocol)?;
                        let call = self.calls.get_mut(&index).ok_or(E::Protocol)?;
                        if call.done
                            || !call.args_done
                            || item["id"] != call.item["id"]
                            || item["call_id"] != call.item["call_id"]
                            || item["name"] != call.item["name"]
                            || string(item, "arguments")? != call.arguments
                        {
                            return Err(E::Protocol);
                        }
                        let arguments: Value =
                            serde_json::from_str(&call.arguments).map_err(|_| E::EventJson)?;
                        if !arguments.is_object() {
                            return Err(E::Protocol);
                        }
                        call.item = item.clone();
                        call.done = true;
                    }
                    "message" | "reasoning" => {}
                    _ => return Err(E::Protocol),
                }
                Ok(Update::None)
            }
            "response.completed" => {
                if v["response"]["status"] != "completed" {
                    return Err(E::Protocol);
                }
                let output = v["response"]["output"].as_array().ok_or(E::Protocol)?;
                bounded(output)?;
                let mut count = 0;
                let mut text = String::new();
                for (index, item) in output.iter().enumerate() {
                    match string(item, "type")? {
                        "function_call" => {
                            let call = self.calls.get(&(index as u64)).ok_or(E::Protocol)?;
                            if !call.done || call.item != *item {
                                return Err(E::Protocol);
                            }
                            count += 1;
                        }
                        "message" => {
                            for content in item["content"].as_array().ok_or(E::Protocol)? {
                                if content["type"] != "output_text" {
                                    return Err(E::Protocol);
                                }
                                text.push_str(string(content, "text")?);
                            }
                        }
                        "reasoning" => {}
                        _ => return Err(E::Protocol),
                    }
                }
                if count != self.calls.len() || text != self.text {
                    return Err(E::Protocol);
                }
                self.completed = true;
                Ok(Update::Complete)
            }
            "error" | "response.failed" | "response.incomplete" => Err(E::Api),
            "response.created"
            | "response.in_progress"
            | "response.content_part.added"
            | "response.content_part.done"
            | "response.output_text.done"
            | "response.output_text.annotation.added"
            | "response.reasoning_summary_part.added"
            | "response.reasoning_summary_part.done"
            | "response.reasoning_summary_text.done"
            | "response.reasoning_text.done" => Ok(Update::None),
            "response.reasoning_summary_text.delta" | "response.reasoning_text.delta" => {
                Ok(Update::Reasoning)
            }
            _ => Err(E::Protocol),
        }
    }

    /// Completed function calls in output order: `(call_id, wire name, arguments)`.
    pub fn calls(&self) -> Result<Vec<(String, String, Value)>, E> {
        self.calls
            .values()
            .map(|call| {
                let arguments = serde_json::from_str(&call.arguments).map_err(|_| E::EventJson)?;
                Ok((
                    string(&call.item, "call_id")?.to_owned(),
                    string(&call.item, "name")?.to_owned(),
                    arguments,
                ))
            })
            .collect()
    }
}

/// llama-server omits `output_index` and the separate arguments-done event.
/// Resolves only by the previously observed item id; the ordinary validator
/// still checks complete arguments, call id, name and final output. Never
/// applied to official OpenAI streams.
#[derive(Default)]
pub(crate) struct EventNormalizer {
    calls: HashMap<String, (u64, bool)>,
    items: HashSet<String>,
    indices: HashSet<u64>,
    next_index: u64,
}

impl EventNormalizer {
    pub fn normalize(&mut self, mut event: Value) -> Result<Vec<Value>, E> {
        let kind = string(&event, "type")?.to_owned();
        if kind == "response.output_item.added" {
            let id = string(&event["item"], "id")?.to_owned();
            if self.items.len() >= MAX_CALLS * 3 || !self.items.insert(id.clone()) {
                return Err(E::Protocol);
            }
            let index = match event.get("output_index") {
                Some(value) => value.as_u64().ok_or(E::Protocol)?,
                None => self.next_index,
            };
            if index >= MAX_CALLS as u64 * 3 || !self.indices.insert(index) {
                return Err(E::Protocol);
            }
            self.next_index = self.next_index.max(index + 1);
            if event["item"]["type"] == "function_call" {
                self.calls.insert(id, (index, false));
            }
            event["output_index"] = index.into();
        } else if matches!(
            kind.as_str(),
            "response.function_call_arguments.delta" | "response.function_call_arguments.done"
        ) || (kind == "response.output_item.done"
            && event["item"]["type"] == "function_call")
        {
            let id = if kind == "response.output_item.done" {
                string(&event["item"], "id")?
            } else {
                string(&event, "item_id")?
            }
            .to_owned();
            let (index, args_done) = self.calls.get_mut(&id).ok_or(E::Protocol)?;
            if event
                .get("output_index")
                .is_some_and(|v| v.as_u64() != Some(*index))
            {
                return Err(E::Protocol);
            }
            event["output_index"] = (*index).into();
            if kind == "response.function_call_arguments.done" {
                *args_done = true;
            }
            if kind == "response.output_item.done" && !*args_done {
                *args_done = true;
                let arguments = string(&event["item"], "arguments")?;
                let done = json!({
                    "type": "response.function_call_arguments.done",
                    "item_id": id,
                    "output_index": *index,
                    "arguments": arguments,
                });
                return Ok(vec![done, event]);
            }
        }
        Ok(vec![event])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rah_protocol::{PermissionLevel, ToolName};

    fn definition(name: &str) -> ToolDefinition {
        ToolDefinition {
            name: ToolName::new(name),
            description: "d".into(),
            input_schema: json!({"type": "object"}),
            permission: PermissionLevel::None,
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
        assert_eq!(names.original("echo"), Some("echo"));
        assert_eq!(names.original("nope"), None);
        assert_eq!(names.wire("gone.tool"), "gone_tool");
    }

    #[test]
    fn strict_schemas_are_recognized_conservatively() {
        assert!(strict(&json!({
            "type": "object",
            "properties": {"text": {"type": "string"}},
            "required": ["text"],
            "additionalProperties": false
        })));
        assert!(!strict(
            &json!({"type": "object", "properties": {}, "required": []})
        ));
        assert!(!strict(&json!({"type": "string", "format": "uri"})));
        let not_object = ToolDefinition {
            input_schema: json!(true),
            ..definition("x")
        };
        assert_eq!(
            tools(&[not_object], &ToolNames::default()),
            Err(E::ToolSchema)
        );
    }

    #[test]
    fn context_converts_to_responses_input() {
        let names = ToolNames::new(&[definition("fs.read")]);
        let items = [
            ContextItem::System { text: "s".into() },
            ContextItem::User { text: "u".into() },
            ContextItem::Assistant { text: "a".into() },
            ContextItem::ToolCall {
                call_id: "c1".into(),
                name: ToolName::new("fs.read"),
                arguments: json!({"path": "x"}),
            },
            ContextItem::ToolResult {
                call_id: "c1".into(),
                output: ToolOutput {
                    content: vec![ToolContent::Text("nope".into())],
                    is_error: true,
                },
            },
        ];
        let openai = input(&items, &names, false, true).unwrap();
        assert_eq!(openai[2], json!({"role": "assistant", "content": "a"}));
        assert_eq!(
            openai[3],
            json!({"type":"function_call","call_id":"c1","name":"fs_read","arguments":"{\"path\":\"x\"}"})
        );
        assert_eq!(
            openai[4],
            json!({"type":"function_call_output","call_id":"c1","output":"[error] nope"})
        );
        let llama = input(&items, &names, true, true).unwrap();
        assert_eq!(llama[2]["type"], "message");
    }

    #[test]
    fn images_go_in_function_call_output_parts_or_become_text() {
        let names = ToolNames::new(&[definition("shot")]);
        let items = [ContextItem::ToolResult {
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
        }];
        assert_eq!(
            input(&items, &names, true, true).unwrap()[0]["output"],
            json!([
                {"type": "input_text", "text": "window"},
                {"type": "input_image", "image_url": "data:image/png;base64,AAAA"}
            ])
        );
        assert_eq!(
            input(&items, &names, true, false).unwrap()[0]["output"],
            json!("window\n[image/png image, 1 KiB]")
        );
    }
}
