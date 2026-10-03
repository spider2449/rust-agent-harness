use crate::OpenAiAdapterError as E;
use rah_protocol::{MessageRole, ToolDefinition};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashSet};

pub(crate) const MAX_BYTES: usize = 4 * 1024 * 1024;
pub(crate) const MAX_CALLS: usize = 128;

pub(crate) fn tools(definitions: &[ToolDefinition]) -> Result<Vec<Value>, E> {
    let mut names = HashSet::new();
    definitions
        .iter()
        .map(|d| {
            let name = d.name.as_str();
            if name.is_empty()
                || name.len() > 64
                || !names.insert(name)
                || !d.input_schema.is_object()
                || serde_json::to_vec(&d.input_schema)
                    .map_err(|_| E::ToolSchema)?
                    .len()
                    > MAX_BYTES
            {
                return Err(E::ToolSchema);
            }
            Ok(
                json!({"type":"function", "name":name, "description":d.description,
            "parameters":d.input_schema,"strict":strict(&d.input_schema)}),
            )
        })
        .collect()
}
// Conservative recognition only. Never normalize or add required/nullability.
// Other object schemas are explicitly non-strict, preserving their exact JSON.
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
pub(crate) fn input(request: &rah_protocol::AgentRequest) -> Result<Vec<Value>, E> {
    let mut result = Vec::new();
    for m in &request.input.messages {
        let role = match m.role {
            MessageRole::System => "system",
            MessageRole::User => "user",
            MessageRole::Assistant => "assistant",
            // Neutral text replay has no call identity for a standalone Tool role.
            MessageRole::Tool => return Err(E::Protocol),
        };
        result.push(json!({"role":role,"content":m.content}));
    }
    bounded(&result)?;
    Ok(result)
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
#[derive(Default)]
pub(crate) struct Response {
    calls: BTreeMap<u64, Call>,
    pub text: String,
    pub output: Option<Vec<Value>>,
}
struct Call {
    item: Value,
    arguments: String,
    args_done: bool,
    done: bool,
}
pub(crate) enum Update {
    Text(String),
    Complete,
    None,
}
impl Response {
    pub fn event(&mut self, v: Value) -> Result<Update, E> {
        if self.output.is_some() {
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
                    if call.arguments.len() + delta.len() > crate::sse::MAX_FRAME {
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
                        let call = self
                            .calls
                            .get_mut(&v["output_index"].as_u64().ok_or(E::Protocol)?)
                            .ok_or(E::Protocol)?;
                        if call.done
                            || !call.args_done
                            || item["id"] != call.item["id"]
                            || item["call_id"] != call.item["call_id"]
                            || item["name"] != call.item["name"]
                            || string(item, "arguments")? != call.arguments
                        {
                            return Err(E::Protocol);
                        }
                        let args: Value =
                            serde_json::from_str(&call.arguments).map_err(|_| E::EventJson)?;
                        if !args.is_object() {
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
                self.output = Some(output.clone());
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
            | "response.reasoning_summary_text.delta"
            | "response.reasoning_summary_text.done"
            | "response.reasoning_text.delta"
            | "response.reasoning_text.done" => Ok(Update::None),
            _ => Err(E::Protocol),
        }
    }
    pub fn calls(&self) -> impl Iterator<Item = &Value> {
        self.calls.values().map(|c| &c.item)
    }
}
