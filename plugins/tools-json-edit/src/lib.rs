//! `json.set-integer`: replace one configured JSON integer through the
//! workspace's read and write tools, with a byte-for-byte write precondition.

mod edit;

use std::sync::Arc;

use rah_api::{
    Plugin, PluginError, PluginId, PluginManifest, Scope, async_trait,
    tools::{
        Invocation, InvocationOutcome, TOOLS_SERVICE, Tool, ToolContext, ToolError, ToolInvoker,
        provide_tools,
    },
};
use rah_protocol::{
    PermissionLevel, ToolCall, ToolCallId, ToolContent, ToolDefinition, ToolEffect, ToolInput,
    ToolName, ToolOutput,
};
use serde::Deserialize;
use serde_json::{Value, json};

pub const TOOL_NAME: &str = "json.set-integer";

pub struct JsonEditPlugin(PluginManifest);

impl JsonEditPlugin {
    pub fn new(id: impl Into<PluginId>) -> Self {
        Self(PluginManifest::new(id, env!("CARGO_PKG_VERSION")).provides([TOOLS_SERVICE]))
    }

    pub const CONFIG_EXAMPLE: &str = r#"{"path":"settings.json","pointer":"/batch/max_items"}"#;

    pub const CONFIG_HELP: &[(&str, &str)] = &[
        ("path", "Workspace-relative path of the existing JSON file."),
        ("pointer", "JSON Pointer to the existing unsigned integer."),
    ];
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    path: String,
    pointer: String,
}

fn config(value: &Value) -> Result<(Config, Vec<String>), PluginError> {
    let config: Config = serde_json::from_value(value.clone())?;
    if config.path.len() > 1024
        || config.path.starts_with('-')
        || config.path.contains(['\\', ':', '\0'])
        || config.path.split('/').any(|part| {
            matches!(part, "" | "." | "..")
                || part
                    .trim_end_matches(['.', ' '])
                    .eq_ignore_ascii_case(".git")
        })
        || config.path.split('/').next().is_some_and(|first| {
            first
                .trim_end_matches(['.', ' '])
                .eq_ignore_ascii_case(".rah")
        })
    {
        return Err("`path` must be a safe workspace-relative file path".into());
    }
    let tokens = edit::pointer_tokens(&config.pointer).map_err(PluginError::from)?;
    Ok((config, tokens))
}

#[async_trait]
impl Plugin for JsonEditPlugin {
    fn manifest(&self) -> &PluginManifest {
        &self.0
    }

    fn check_config(&self, value: &Value) -> Result<(), PluginError> {
        config(value).map(|_| ())
    }

    async fn apply(&self, scope: &mut Scope) -> Result<(), PluginError> {
        let (Config { path, pointer }, tokens) = config(scope.config())?;
        provide_tools(
            scope,
            vec![Arc::new(SetInteger {
                invoker: ToolInvoker::new(scope.kernel()),
                path,
                pointer,
                tokens,
            })],
        )?;
        Ok(())
    }
}

struct SetInteger {
    invoker: ToolInvoker,
    path: String,
    pointer: String,
    tokens: Vec<String>,
}

impl SetInteger {
    async fn invoke(&self, cx: &ToolContext, name: &str, input: Value) -> InvocationOutcome {
        self.invoker
            .invoke(
                Invocation {
                    session: cx.session.clone(),
                    cancel: cx.cancel.clone(),
                    call: ToolCall {
                        id: ToolCallId::new(),
                        name: ToolName::new(name),
                        input: ToolInput(input),
                    },
                },
                || {},
            )
            .await
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    value: u64,
}

#[async_trait]
impl Tool for SetInteger {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: ToolName::new(TOOL_NAME),
            description: format!(
                "Set the unsigned integer at {:?} in {:?}. Supply the new value only.",
                self.pointer, self.path
            ),
            permission: PermissionLevel::Write,
            input_schema: json!({"type":"object","properties":{"value":{"type":"integer","minimum":0}},
                "required":["value"],"additionalProperties":false}),
        }
    }

    async fn call(&self, input: Value, cx: ToolContext) -> Result<ToolOutput, ToolError> {
        let Input { value } = serde_json::from_value(input)
            .map_err(|error| ToolError::InvalidInput(error.to_string()))?;
        let read = self.invoke(&cx, "fs.read", json!({"path":self.path})).await;
        if read.output.is_error {
            return Err(ToolError::NotStarted(tool_error(&read.output, "fs.read")));
        }
        let [ToolContent::Text(original)] = read.output.content.as_slice() else {
            return Err(ToolError::NotStarted(
                "fs.read did not return one text item".into(),
            ));
        };
        let replacement = edit::replace_integer(original, &self.tokens, value)?;
        let write = self
            .invoke(
                &cx,
                "repo.edit-files",
                json!({"edits":[{
                    "path":self.path,"old":original,"new":replacement,"match_entire_file":true
                }]}),
            )
            .await;
        match write.effect {
            ToolEffect::Completed if !write.output.is_error => Ok(write.output),
            ToolEffect::None => Err(ToolError::NotStarted(tool_error(
                &write.output,
                "repo.edit-files",
            ))),
            _ => Err(ToolError::Execution(tool_error(
                &write.output,
                "repo.edit-files",
            ))),
        }
    }
}

fn tool_error(output: &ToolOutput, name: &str) -> String {
    match output.content.first() {
        Some(ToolContent::Text(text)) => format!("{name}: {text}"),
        _ => format!("{name} failed"),
    }
}

#[cfg(test)]
#[test]
fn only_host_configured_workspace_paths_and_valid_pointers_are_accepted() {
    let plugin = JsonEditPlugin::new("json-edit");
    let example = serde_json::from_str(JsonEditPlugin::CONFIG_EXAMPLE).unwrap();
    plugin.check_config(&example).unwrap();
    for path in [
        "",
        "../settings.json",
        "D:/settings.json",
        "/settings.json",
        ".RAH./settings.json",
        "sub/.git/settings.json",
    ] {
        assert!(
            plugin
                .check_config(&json!({"path":path,"pointer":"/limit"}))
                .is_err(),
            "{path}"
        );
    }
    assert!(
        plugin
            .check_config(&json!({"path":"settings.json","pointer":"limit"}))
            .is_err()
    );
}
