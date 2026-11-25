//! Typed integer edits for the single-writer JSON fixtures.

#[path = "json_value_lifecycle.rs"]
mod lifecycle;

use std::{path::PathBuf, sync::Arc};

use rah_api::{
    Plugin, PluginError, PluginManifest, Scope, async_trait,
    tools::{Invocation, TOOLS_SERVICE, Tool, ToolContext, ToolError, ToolInvoker, provide_tools},
};
use rah_protocol::{
    PermissionLevel, ToolCall, ToolCallId, ToolContent, ToolDefinition, ToolEffect, ToolInput,
    ToolName, ToolOutput,
};
use serde::Deserialize;
use serde_json::{Value, json};

pub(super) const NAME: &str = "fixture.set-json-value";

pub(super) struct FixtureJsonValue {
    manifest: PluginManifest,
    workspace: PathBuf,
    file: &'static str,
    pointer: &'static str,
}

impl FixtureJsonValue {
    pub(super) fn new(workspace: PathBuf, task: &str) -> Self {
        let (file, pointer) = super::json_preflight::target(task);
        Self {
            manifest: PluginManifest::new("fixture-json-value", "1").provides([TOOLS_SERVICE]),
            workspace,
            file,
            pointer,
        }
    }
}

#[async_trait]
impl Plugin for FixtureJsonValue {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    async fn apply(&self, scope: &mut Scope) -> Result<(), PluginError> {
        provide_tools(
            scope,
            vec![Arc::new(SetJsonValue {
                invoker: ToolInvoker::new(scope.kernel()),
                path: self.workspace.join(self.file),
                file: self.file,
                pointer: self.pointer,
            })],
        )?;
        Ok(())
    }
}

struct SetJsonValue {
    invoker: ToolInvoker,
    path: PathBuf,
    file: &'static str,
    pointer: &'static str,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    value: u64,
}

#[async_trait]
impl Tool for SetJsonValue {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: ToolName::new(NAME),
            description: format!(
                "Set the integer at {} in {}. Supply the new value only.",
                self.pointer, self.file
            ),
            permission: PermissionLevel::Write,
            input_schema: json!({"type":"object","properties":{"value":{"type":"integer","minimum":0}},
                "required":["value"],"additionalProperties":false}),
        }
    }

    async fn call(&self, input: Value, cx: ToolContext) -> Result<ToolOutput, ToolError> {
        let Input { value } = serde_json::from_value(input)
            .map_err(|error| ToolError::InvalidInput(error.to_string()))?;
        let original = tokio::fs::read_to_string(&self.path)
            .await
            .map_err(|error| ToolError::NotStarted(error.to_string()))?;
        let replacement = render_value(&original, self.pointer, value)?;
        let result = self
            .invoker
            .invoke(
                Invocation {
                    session: cx.session,
                    cancel: cx.cancel,
                    call: ToolCall {
                        id: ToolCallId::new(),
                        name: ToolName::new("repo.edit-files"),
                        input: ToolInput(
                            json!({"edits":[{"path":self.file,"old":original,"new":replacement}]}),
                        ),
                    },
                },
                || {},
            )
            .await;
        if result.output.is_error {
            let reason = result
                .output
                .content
                .iter()
                .find_map(|part| match part {
                    ToolContent::Text(text) => Some(text.clone()),
                    _ => None,
                })
                .unwrap_or_else(|| "repo.edit-files failed".into());
            return Err(if result.effect == ToolEffect::None {
                ToolError::NotStarted(reason)
            } else {
                ToolError::Execution(reason)
            });
        }
        Ok(result.output)
    }
}

fn render_value(original: &str, pointer: &str, value: u64) -> Result<String, ToolError> {
    let mut doc: Value = serde_json::from_str(original)
        .map_err(|error| ToolError::InvalidInput(error.to_string()))?;
    let field = doc
        .pointer_mut(pointer)
        .filter(|field| field.is_u64())
        .ok_or_else(|| {
            ToolError::InvalidInput("the configured field is not an unsigned integer".into())
        })?;
    if field.as_u64() == Some(value) {
        return Err(ToolError::NotStarted(
            "the field already has that value".into(),
        ));
    }
    *field = json!(value);
    let mut replacement = if original.lines().count() > 1 {
        serde_json::to_string_pretty(&doc)
    } else {
        serde_json::to_string(&doc)
    }
    .map_err(|error| ToolError::NotStarted(error.to_string()))?;
    if original.ends_with('\n') {
        replacement.push('\n');
    }
    if original.contains("\r\n") {
        replacement = replacement.replace('\n', "\r\n");
    }
    Ok(replacement)
}

#[tokio::test]
async fn typed_value_preserves_other_fields_and_requires_repo_write_permission() {
    use crate::common::{TempDir, headless, kernel, new_session};
    use rah_api::{CancellationToken, MountOptions};

    for approved in [false, true] {
        let dir = TempDir::new(&format!("small-model-json-value-{approved}"));
        let workspace = dir.workspace();
        let file = workspace.join("queue.json");
        let original = "{\"consumer\":{\"limit\":4},\"producer\":{\"limit\":4}}\n";
        std::fs::write(&file, original).unwrap();
        let mut distribution = headless(&dir);
        // Exercise the tool's raw input validation independently of the
        // default schema repair hook, which can coerce or drop extra fields.
        distribution
            .plugin
            .retain(|plugin| plugin.kind != "tool-input-repair");
        distribution.set_config(
            "policy-authority",
            "tools",
            json!({NAME:"write","repo.edit-files":"write"}),
        );
        let approvals = if approved {
            vec![NAME, "repo.edit-files"]
        } else {
            vec![NAME]
        };
        distribution.set_config("policy-authority", "approve", json!(approvals));
        let kernel = kernel(&distribution).await;
        kernel
            .mount(
                Arc::new(FixtureJsonValue::new(workspace.clone(), "stale-edit")),
                MountOptions::default(),
            )
            .await
            .unwrap();
        let handle = kernel.handle();
        let session = new_session(&handle).await;
        let invoker = ToolInvoker::new(handle);
        let invoke = |input| {
            invoker.invoke(
                Invocation {
                    session: session.clone(),
                    cancel: CancellationToken::new(),
                    call: ToolCall {
                        id: ToolCallId::new(),
                        name: ToolName::new(NAME),
                        input: ToolInput(input),
                    },
                },
                || {},
            )
        };
        for invalid in [
            json!({"value":8,"path":"checks.json"}),
            json!({"value":"8"}),
        ] {
            let result = invoke(invalid).await;
            assert!(
                result.output.is_error && result.effect == ToolEffect::None,
                "{result:?}"
            );
            assert_eq!(std::fs::read_to_string(&file).unwrap(), original);
        }
        let result = invoke(json!({"value":8})).await;
        assert_eq!(result.output.is_error, !approved, "{result:?}");
        assert!(
            result.authorized,
            "the outer call should have its own approval"
        );
        assert_eq!(
            result.effect,
            if approved {
                ToolEffect::Completed
            } else {
                ToolEffect::None
            }
        );
        let expected = if approved {
            original.replace("\"producer\":{\"limit\":4}", "\"producer\":{\"limit\":8}")
        } else {
            original.into()
        };
        assert_eq!(std::fs::read_to_string(&file).unwrap(), expected);
        kernel.shutdown().await.unwrap();
    }
}

#[tokio::test]
#[ignore = "requires an isolated llama.cpp with Qwen3-Coder template and the native tool choice proxy"]
async fn live_small_model_json_value() {
    use super::{TaskProbe, live_settings, sample, tasks};

    let (model, run_label, port) = live_settings();
    eprintln!(
        "small-model JSON value configuration: {}",
        json!({"model":model,"run":run_label,"port":port,"set":"development"})
    );
    for task in tasks()
        .into_iter()
        .filter(|task| matches!(task.id.as_str(), "batch-limit" | "stale-edit"))
    {
        for (index, probe) in [
            TaskProbe::NativeRetryJson,
            TaskProbe::NativeJsonValue,
            TaskProbe::NativeJsonValue,
            TaskProbe::NativeRetryJson,
        ]
        .into_iter()
        .enumerate()
        {
            let row = sample(&task, "loop-react", &model, port, index + 1, Some(probe)).await;
            eprintln!("small-model JSON value sample: {row}");
        }
    }
}
