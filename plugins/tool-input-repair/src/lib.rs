//! `tool-input-repair`: fixes tool call arguments that are one unambiguous
//! step away from valid for the tool's input schema (ADR 0049).
//!
//! The plugin attaches to `tool.repair_input`, which runs before the policy
//! sees a call. It changes only what the schema makes certain, and leaves
//! every other value for the tool's own validation to report:
//!
//! - a JSON string where an object or array is declared
//!   (`"{\"limit\":\"20\"}"` becomes `{ "limit": "20" }`);
//! - a numeric or boolean string where a number, integer or boolean is
//!   declared (`"20"` becomes `20`, `"false"` becomes `false`);
//! - a lone value where an array of such values is declared (`"a"` becomes
//!   `["a"]`);
//! - `null`, or `{}` where no object is declared, for an optional field that
//!   can be neither (the field is dropped);
//! - fields a closed object (`additionalProperties: false`) does not declare
//!   (dropped);
//! - the same inside tuples (`prefixItems`), typed dictionaries
//!   (`additionalProperties`), nested fields and local `$ref`s, to a depth of
//!   6.
//!
//! It never adds a field and never changes a value that already fits. It has
//! no configuration.

use rah_api::{
    Plugin, PluginError, PluginId, PluginManifest, Scope, async_trait, hooks::ToolInputRepair,
};
use serde_json::{Map, Value};

/// Deepest schema nesting repaired.
const MAX_DEPTH: usize = 6;

pub struct ToolInputRepairPlugin {
    manifest: PluginManifest,
}

impl ToolInputRepairPlugin {
    pub fn new(id: impl Into<PluginId>) -> Self {
        Self {
            manifest: PluginManifest::new(id, env!("CARGO_PKG_VERSION")),
        }
    }

    /// An example configuration for reference; this plugin takes none.
    pub const CONFIG_EXAMPLE: &str = "{}";
}

#[async_trait]
impl Plugin for ToolInputRepairPlugin {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    fn check_config(&self, config: &Value) -> Result<(), PluginError> {
        match config {
            Value::Null => Ok(()),
            Value::Object(map) if map.is_empty() => Ok(()),
            _ => Err("this plugin takes no configuration".into()),
        }
    }

    async fn apply(&self, scope: &mut Scope) -> Result<(), PluginError> {
        scope.hook_waterfall::<ToolInputRepair, _, _>(0, |mut request| async move {
            let schema = request.definition.input_schema.clone();
            if schema.get("type").and_then(Value::as_str) == Some("object") {
                let input = std::mem::take(&mut request.call.input.0);
                request.call.input.0 = repair(input, &schema, &schema, 0);
            }
            request
        });
        Ok(())
    }
}

/// What JavaScript's `typeof` says about a JSON value, which is how the
/// schema's `type` lists are matched.
fn js_type(value: &Value) -> &'static str {
    match value {
        Value::String(_) => "string",
        Value::Number(_) => "number",
        Value::Bool(_) => "boolean",
        Value::Null | Value::Array(_) | Value::Object(_) => "object",
    }
}

fn type_is(schema: &Value, name: &str) -> bool {
    schema.get("type").and_then(Value::as_str) == Some(name)
}

fn with_type(schema: &Value, name: &str) -> Value {
    let mut copy = schema.clone();
    copy["type"] = Value::String(name.to_owned());
    copy
}

fn repair(value: Value, schema: &Value, root: &Value, depth: usize) -> Value {
    if depth > MAX_DEPTH || !schema.is_object() {
        return value;
    }

    if let Some(reference) = schema.get("$ref").and_then(Value::as_str) {
        let section = if reference.starts_with("#/$defs/") {
            "$defs"
        } else if reference.starts_with("#/definitions/") {
            "definitions"
        } else {
            return value;
        };
        let name = reference.rsplit('/').next().unwrap_or("");
        if reference.matches('/').count() != 2 || name.is_empty() {
            return value;
        }
        let name = name.replace("~1", "/").replace("~0", "~");
        return match root.get(section).and_then(|defs| defs.get(&name)) {
            Some(target) if target.is_object() => repair(value, target, root, depth + 1),
            _ => value,
        };
    }

    if let Some(types) = schema.get("type").and_then(Value::as_array) {
        let names: Vec<&str> = types.iter().filter_map(Value::as_str).collect();
        if value.is_null() && names.contains(&"null") {
            return value;
        }
        if names.contains(&js_type(&value)) {
            return value;
        }
        let rest: Vec<&str> = names.into_iter().filter(|name| *name != "null").collect();
        return match rest.as_slice() {
            [only] => repair(value, &with_type(schema, only), root, depth + 1),
            _ => value,
        };
    }

    let Some(kind) = schema.get("type").and_then(Value::as_str) else {
        let (any_of, one_of) = (schema.get("anyOf"), schema.get("oneOf"));
        if any_of.is_some_and(Value::is_array) && one_of.is_some_and(Value::is_array) {
            return value;
        }
        let branches = if any_of.is_some_and(Value::is_array) {
            any_of
        } else {
            one_of
        };
        let Some(Value::Array(branches)) = branches else {
            return value;
        };
        if value.is_null() {
            return value;
        }
        if branches.iter().any(|branch| {
            !branch.is_object()
                || branch.get("type").and_then(Value::as_str) == Some(js_type(&value))
        }) {
            return value;
        }
        let candidates: Vec<&Value> = branches
            .iter()
            .filter(|branch| branch.is_object() && !type_is(branch, "null"))
            .collect();
        return match candidates.as_slice() {
            [only] => repair(value, only, root, depth + 1),
            _ => value,
        };
    };

    match kind {
        "number" | "integer" => {
            let Value::String(text) = &value else {
                return value;
            };
            let text = text.trim();
            if text.is_empty() {
                return value;
            }
            match text.parse::<f64>() {
                Ok(parsed)
                    if parsed.is_finite()
                        && (kind != "integer" || (parsed.fract() == 0.0 && is_safe(parsed))) =>
                {
                    number(parsed).unwrap_or(value)
                }
                _ => value,
            }
        }
        "boolean" => match &value {
            Value::String(text) if text == "true" => Value::Bool(true),
            Value::String(text) if text == "false" => Value::Bool(false),
            _ => value,
        },
        "object" => repair_object(value, schema, root, depth),
        "array" => repair_array(value, schema, root, depth),
        _ => value,
    }
}

/// Whether a float is an integer JSON consumers read back exactly.
fn is_safe(value: f64) -> bool {
    value.abs() <= 9_007_199_254_740_991.0
}

/// A JSON number that is an integer whenever the float is one, so that a
/// field declared as an integer reads it back.
fn number(parsed: f64) -> Option<Value> {
    if parsed.fract() == 0.0 && is_safe(parsed) {
        // Exact: the magnitude is below 2^53.
        Some(Value::from(parsed as i64))
    } else {
        serde_json::Number::from_f64(parsed).map(Value::Number)
    }
}

fn parse_text(value: &Value) -> Option<Value> {
    match value {
        Value::String(text) => serde_json::from_str(text).ok(),
        other => Some(other.clone()),
    }
}

fn repair_object(value: Value, schema: &Value, root: &Value, depth: usize) -> Value {
    let Some(Value::Object(parsed)) = parse_text(&value) else {
        return value;
    };
    let properties = schema.get("properties").and_then(Value::as_object);
    let required: Vec<&str> = schema
        .get("required")
        .and_then(Value::as_array)
        .map(|names| names.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    let patterned = schema
        .get("patternProperties")
        .is_some_and(Value::is_object);
    let composed = ["allOf", "anyOf", "oneOf"]
        .iter()
        .any(|keyword| schema.get(*keyword).is_some_and(Value::is_array));
    let additional = schema.get("additionalProperties");

    let mut result: Map<String, Value> = parsed.clone();
    for key in parsed.keys() {
        let current = result.get(key).cloned().unwrap_or(Value::Null);
        let declared = properties.is_some_and(|properties| properties.contains_key(key));
        let property = if declared {
            properties.and_then(|properties| properties.get(key))
        } else if !patterned {
            additional
        } else {
            None
        };

        if !declared && additional == Some(&Value::Bool(false)) && !patterned && !composed {
            result.remove(key);
            continue;
        }
        let Some(property) = property.filter(|property| property.is_object()) else {
            continue;
        };

        // Only a bare single-type property provably rejects null and `{}`.
        // Compositions, enums, constants, references and nullable flags may
        // accept them, so those are left alone.
        let plain = property
            .get("type")
            .and_then(Value::as_str)
            .is_some_and(|kind| kind != "null")
            && !composed
            && [
                "anyOf", "oneOf", "allOf", "enum", "const", "$ref", "nullable",
            ]
            .iter()
            .all(|keyword| property.get(*keyword).is_none());
        let placeholder = matches!(&current, Value::Object(map) if map.is_empty())
            && !type_is(property, "object");
        if declared
            && !required.contains(&key.as_str())
            && plain
            && (current.is_null() || placeholder)
        {
            result.remove(key);
            continue;
        }

        let repaired = repair(current.clone(), property, root, depth + 1);
        if repaired != current {
            result.insert(key.clone(), repaired);
        }
    }
    Value::Object(result)
}

fn repair_array(value: Value, schema: &Value, root: &Value, depth: usize) -> Value {
    let parsed = parse_text(&value);
    let prefix = schema.get("prefixItems").and_then(Value::as_array);
    let tuple = prefix.or_else(|| schema.get("items").and_then(Value::as_array));
    let items = schema.get("items");

    if let Some(Value::Array(parsed)) = parsed {
        return Value::Array(
            parsed
                .into_iter()
                .enumerate()
                .map(|(index, item)| {
                    let member = match tuple {
                        Some(tuple) => tuple.get(index).or(if prefix.is_some() {
                            items
                        } else {
                            schema.get("additionalItems")
                        }),
                        None => items,
                    };
                    match member {
                        Some(member) if member.is_object() => repair(item, member, root, depth + 1),
                        _ => item,
                    }
                })
                .collect(),
        );
    }

    let Some(items) = items.filter(|items| items.is_object()) else {
        return value;
    };
    if tuple.is_some() {
        return value;
    }
    let repaired = repair(value.clone(), items, root, depth + 1);
    let compatible = match items.get("type").and_then(Value::as_str) {
        Some("object") => repaired.is_object(),
        Some("array") => repaired.is_array(),
        Some("integer") => repaired.as_i64().is_some() || repaired.as_u64().is_some(),
        Some("number") => repaired.is_number(),
        Some("string") => repaired.is_string(),
        Some("boolean") => repaired.is_boolean(),
        _ => false,
    };
    if compatible {
        Value::Array(vec![repaired])
    } else {
        value
    }
}

#[cfg(test)]
mod tests;
