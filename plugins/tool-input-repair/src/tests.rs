use rah_api::{
    Kernel, MountOptions, PluginStatus,
    hooks::{ToolInputRepair, ToolRequestContext},
};
use rah_protocol::{
    PermissionLevel, SessionId, ToolCall, ToolCallId, ToolDefinition, ToolInput, ToolName,
};
use serde_json::json;

use super::*;

fn fix(input: Value, schema: Value) -> Value {
    repair(input, &schema, &schema, 0)
}

#[test]
fn strings_become_the_numbers_and_booleans_the_schema_declares() {
    let schema = json!({
        "type": "object",
        "properties": {
            "limit": { "type": "integer" },
            "ratio": { "type": "number" },
            "all": { "type": "boolean" },
            "name": { "type": "string" },
        },
    });
    assert_eq!(
        fix(
            json!({ "limit": "20", "ratio": " 0.5 ", "all": "false", "name": "20" }),
            schema.clone()
        ),
        json!({ "limit": 20, "ratio": 0.5, "all": false, "name": "20" })
    );
    // What does not parse, or does not fit, is left for the tool to report.
    let untouched = json!({ "limit": "2.5", "ratio": "", "all": "yes" });
    assert_eq!(fix(untouched.clone(), schema.clone()), untouched);
    let huge = json!({ "limit": "99999999999999999999" });
    assert_eq!(fix(huge.clone(), schema.clone()), huge);
    let named = json!({ "limit": "NaN", "ratio": "inf" });
    assert_eq!(fix(named.clone(), schema), named);
}

#[test]
fn a_stringified_object_or_array_is_parsed() {
    let schema = json!({
        "type": "object",
        "properties": {
            "limit": { "type": "integer" },
            "tags": { "type": "array", "items": { "type": "string" } },
            "options": { "type": "object", "properties": { "depth": { "type": "integer" } } },
        },
    });
    assert_eq!(
        fix(json!("{\"limit\":\"20\"}"), schema.clone()),
        json!({ "limit": 20 })
    );
    assert_eq!(
        fix(
            json!({ "tags": "[\"a\",\"b\"]", "options": "{\"depth\":\"3\"}" }),
            schema.clone()
        ),
        json!({ "tags": ["a", "b"], "options": { "depth": 3 } })
    );
    let broken = json!("{not json");
    assert_eq!(fix(broken.clone(), schema), broken);
}

#[test]
fn a_lone_value_becomes_a_one_item_array() {
    let schema = json!({
        "type": "object",
        "properties": {
            "tags": { "type": "array", "items": { "type": "string" } },
            "counts": { "type": "array", "items": { "type": "integer" } },
            "mixed": { "type": "array", "items": { "type": "string" } },
        },
    });
    assert_eq!(
        fix(json!({ "tags": "a", "counts": "2", "mixed": 3 }), schema),
        json!({ "tags": ["a"], "counts": [2], "mixed": 3 })
    );
}

#[test]
fn null_and_empty_placeholders_drop_optional_fields_only() {
    let schema = json!({
        "type": "object",
        "required": ["path"],
        "properties": {
            "path": { "type": "string" },
            "limit": { "type": "integer" },
            "offset": { "type": "integer" },
            "filter": { "anyOf": [{ "type": "string" }, { "type": "null" }] },
            "scope": { "type": "object" },
            "mode": { "enum": ["a", "b"] },
        },
    });
    assert_eq!(
        fix(
            json!({ "path": "a", "limit": null, "offset": {}, "filter": null, "scope": {}, "mode": null }),
            schema.clone()
        ),
        json!({ "path": "a", "filter": null, "scope": {}, "mode": null })
    );
    // A required field is never dropped.
    let required = json!({ "path": null });
    assert_eq!(fix(required.clone(), schema), required);
}

#[test]
fn undeclared_fields_are_dropped_only_from_closed_objects() {
    let closed = json!({
        "type": "object",
        "properties": { "limit": { "type": "integer" } },
        "additionalProperties": false,
    });
    assert_eq!(
        fix(json!({ "limit": "20", "extra": true }), closed),
        json!({ "limit": 20 })
    );
    let open = json!({ "type": "object", "properties": { "limit": { "type": "integer" } } });
    assert_eq!(
        fix(json!({ "limit": "20", "extra": true }), open),
        json!({ "limit": 20, "extra": true })
    );
    let composed = json!({
        "type": "object",
        "properties": { "limit": { "type": "integer" } },
        "additionalProperties": false,
        "allOf": [{ "properties": { "extra": {} } }],
    });
    let kept = json!({ "limit": 1, "extra": true });
    assert_eq!(fix(kept.clone(), composed), kept);
}

#[test]
fn nested_fields_tuples_dictionaries_and_references_are_repaired() {
    let schema = json!({
        "type": "object",
        "properties": {
            "items": { "type": "array", "items": { "$ref": "#/$defs/Item" } },
            "pair": { "type": "array", "prefixItems": [{ "type": "integer" }, { "type": "boolean" }] },
            "counts": { "type": "object", "additionalProperties": { "type": "integer" } },
            "maybe": { "type": ["integer", "null"] },
        },
        "$defs": { "Item": { "type": "object", "properties": { "count": { "type": "integer" } } } },
    });
    assert_eq!(
        fix(
            json!({
                "items": [{ "count": "2" }, { "count": 3 }],
                "pair": ["2", "false"],
                "counts": { "first": "2" },
                "maybe": "5",
            }),
            schema
        ),
        json!({
            "items": [{ "count": 2 }, { "count": 3 }],
            "pair": [2, false],
            "counts": { "first": 2 },
            "maybe": 5,
        })
    );
}

#[test]
fn a_union_is_repaired_only_when_one_branch_can_hold_the_value() {
    let one = json!({
        "type": "object",
        "properties": { "n": { "anyOf": [{ "type": "integer" }, { "type": "null" }] } },
    });
    assert_eq!(fix(json!({ "n": "4" }), one), json!({ "n": 4 }));
    let two = json!({
        "type": "object",
        "properties": { "n": { "anyOf": [{ "type": "integer" }, { "type": "boolean" }] } },
    });
    let untouched = json!({ "n": "4" });
    assert_eq!(fix(untouched.clone(), two), untouched);
    let fits = json!({
        "type": "object",
        "properties": { "n": { "anyOf": [{ "type": "string" }, { "type": "integer" }] } },
    });
    assert_eq!(fix(untouched.clone(), fits), untouched);
}

#[test]
fn depth_is_bounded_and_references_must_be_local() {
    let mut schema = json!({ "type": "integer" });
    for _ in 0..10 {
        schema = json!({ "type": "object", "properties": { "n": schema } });
    }
    let mut value = json!("5");
    for _ in 0..10 {
        value = json!({ "n": value });
    }
    assert_eq!(fix(value.clone(), schema), value);

    let remote =
        json!({ "type": "object", "properties": { "n": { "$ref": "https://x/y#/$defs/N" } } });
    let input = json!({ "n": "5" });
    assert_eq!(fix(input.clone(), remote), input);
    let missing = json!({ "type": "object", "properties": { "n": { "$ref": "#/$defs/N" } } });
    assert_eq!(fix(input.clone(), missing), input);
}

#[test]
fn input_that_already_fits_is_returned_unchanged() {
    let schema = json!({
        "type": "object",
        "properties": {
            "path": { "type": "string" },
            "limit": { "type": "integer" },
            "tags": { "type": "array", "items": { "type": "string" } },
        },
    });
    let fine = json!({ "path": "a.rs", "limit": 3, "tags": ["x"], "unknown": [1, { "a": 2 }] });
    assert_eq!(fix(fine.clone(), schema), fine);
}

fn request(schema: Value, input: Value) -> ToolRequestContext {
    ToolRequestContext {
        session: SessionId::new(),
        call: ToolCall {
            id: ToolCallId::new(),
            name: ToolName::new("t"),
            input: ToolInput(input),
        },
        definition: ToolDefinition {
            name: ToolName::new("t"),
            description: String::new(),
            input_schema: schema,
            permission: PermissionLevel::Read,
        },
    }
}

#[tokio::test]
async fn the_hook_repairs_input_for_object_schemas_only() {
    let kernel = Kernel::new();
    let status = kernel
        .mount(
            std::sync::Arc::new(ToolInputRepairPlugin::new("repair")),
            MountOptions::default(),
        )
        .await
        .unwrap();
    assert_eq!(status, PluginStatus::Active);
    let schema = json!({ "type": "object", "properties": { "limit": { "type": "integer" } } });
    let repaired = kernel
        .handle()
        .waterfall::<ToolInputRepair>(request(schema, json!({ "limit": "20" })))
        .await
        .unwrap();
    assert_eq!(repaired.call.input.0, json!({ "limit": 20 }));
    assert_eq!(repaired.call.name, ToolName::new("t"));

    let other = json!({ "type": "integer" });
    let kept = kernel
        .handle()
        .waterfall::<ToolInputRepair>(request(other, json!("20")))
        .await
        .unwrap();
    assert_eq!(kept.call.input.0, json!("20"));
}

#[tokio::test]
async fn configuration_is_refused() {
    let kernel = Kernel::new();
    let refused = kernel
        .mount(
            std::sync::Arc::new(ToolInputRepairPlugin::new("repair")),
            MountOptions::with_config(json!({ "depth": 3 })),
        )
        .await;
    assert!(refused.is_err());
}
