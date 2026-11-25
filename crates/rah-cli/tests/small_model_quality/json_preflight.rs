//! Preflight for isolated single-writer fixtures. The planned file and mutable
//! field come from the test configuration, not tool arguments or answer data.

use std::{borrow::Cow, path::PathBuf};

use rah_api::{
    Plugin, PluginError, PluginManifest, Scope, async_trait,
    hooks::{ToolBeforeInvoke, ToolDecision},
};
use rah_protocol::{ToolContent, ToolOutput};
use serde_json::Value;

const PREFIX: &str = "JSON preflight:";

pub(super) struct FixtureJsonGuard {
    manifest: PluginManifest,
    workspace: PathBuf,
    file: &'static str,
    pointer: &'static str,
}

impl FixtureJsonGuard {
    pub(super) fn new(workspace: PathBuf, task: &str) -> Self {
        let (file, pointer) = target(task);
        Self {
            manifest: PluginManifest::new("fixture-json-preflight", "1"),
            workspace,
            file,
            pointer,
        }
    }
}

pub(super) fn target(task: &str) -> (&'static str, &'static str) {
    match task {
        "batch-limit" => ("worker.json", "/batch/max_items"),
        "stale-edit" => ("queue.json", "/producer/limit"),
        _ => super::json_value_generalization::target(task)
            .unwrap_or_else(|| panic!("missing JSON fixture configuration for {task}")),
    }
}

#[async_trait]
impl Plugin for FixtureJsonGuard {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    async fn apply(&self, scope: &mut Scope) -> Result<(), PluginError> {
        let file = self.file;
        let full_path = self.workspace.join(file);
        let pointer = self.pointer;
        scope.hook_bail::<ToolBeforeInvoke, _, _>(5, move |request| {
            let full_path = full_path.clone();
            async move {
                if request.call.name.as_str() != "repo.edit-files" {
                    return None;
                }
                let check = async {
                    let Some([edit]) = request.call.input.0["edits"].as_array().map(Vec::as_slice)
                    else {
                        return Err("expected exactly one edit");
                    };
                    if edit["path"].as_str() != Some(file) {
                        return Err("the edit must target the configured file");
                    }
                    let (Some(old), Some(new)) = (edit["old"].as_str(), edit["new"].as_str())
                    else {
                        return Err("old and new must be strings");
                    };
                    if old.is_empty() {
                        return Err("old must be nonempty");
                    }
                    let current = tokio::fs::read_to_string(&full_path)
                        .await
                        .map_err(|_| "could not read the fixture")?;
                    validate(&current, old, new, pointer)
                }
                .await;
                check.err().map(|reason| ToolDecision::Deny {
                    reason: format!("{PREFIX} {reason}."),
                })
            }
        });
        Ok(())
    }
}

fn validate(current: &str, old: &str, new: &str, pointer: &str) -> Result<(), &'static str> {
    let (old, new) = if !current.contains(old)
        && current.contains("\r\n")
        && old.contains('\n')
        && !old.contains("\r\n")
    {
        (
            Cow::Owned(old.replace('\n', "\r\n")),
            Cow::Owned(new.replace('\n', "\r\n")),
        )
    } else {
        (Cow::Borrowed(old), Cow::Borrowed(new))
    };
    if current.matches(old.as_ref()).count() != 1 {
        // Preserve repo.edit-files' own no-write error and recovery signal.
        return Ok(());
    }
    let proposal = current.replacen(old.as_ref(), new.as_ref(), 1);
    let original: Value =
        serde_json::from_str(current).map_err(|_| "the original file is invalid JSON")?;
    let mut candidate: Value =
        serde_json::from_str(&proposal).map_err(|_| "the replacement would leave invalid JSON")?;
    let before = original
        .pointer(pointer)
        .ok_or("the configured field is missing")?;
    let after = candidate
        .pointer_mut(pointer)
        .ok_or("the replacement would remove the configured field")?;
    *after = before.clone();
    if candidate != original {
        return Err("the replacement would change a field outside the configured scope");
    }
    Ok(())
}

pub(super) fn was_denied(output: &ToolOutput) -> bool {
    output.is_error
        && output.content.iter().any(|part| {
            matches!(part,
        ToolContent::Text(text) if text.contains(PREFIX))
        })
}

#[tokio::test]
async fn invalid_json_and_changes_outside_the_field_never_start_the_tool() {
    use crate::common::{TempDir, headless, kernel, new_session};
    use rah_api::{
        CancellationToken, MountOptions,
        tools::{Invocation, ToolInvoker},
    };
    use rah_protocol::{ToolCall, ToolCallId, ToolEffect, ToolInput, ToolName};
    use serde_json::json;
    use std::{fs, sync::Arc};

    let dir = TempDir::new("small-model-json-preflight");
    let path = dir.workspace().join("queue.json");
    let original = "{\"consumer\":{\"limit\":4},\r\n\"producer\":{\"limit\":4}}\r\n";
    fs::write(&path, original).unwrap();
    let mut distribution = headless(&dir);
    distribution.set_config("policy-authority", "approve", json!(["repo.edit-files"]));
    let kernel = kernel(&distribution).await;
    kernel
        .mount(
            Arc::new(FixtureJsonGuard::new(dir.workspace(), "stale-edit")),
            MountOptions::default(),
        )
        .await
        .unwrap();
    let handle = kernel.handle();
    let session = new_session(&handle).await;
    let invoker = ToolInvoker::new(handle);
    let edit = |old, new| {
        invoker.invoke(
            Invocation {
                session: session.clone(),
                cancel: CancellationToken::new(),
                call: ToolCall {
                    id: ToolCallId::new(),
                    name: ToolName::new("repo.edit-files"),
                    input: ToolInput(json!({"edits":[{"path":"queue.json","old":old,"new":new}]})),
                },
            },
            || {},
        )
    };
    let lf = original.replace("\r\n", "\n");
    for (new, diagnostic) in [
        (
            "{\"consumer\":{\"limit\":4},\n\"producer\":{\"limit\":8},}\n",
            "invalid JSON",
        ),
        (
            "{\"consumer\":{\"limit\":5},\n\"producer\":{\"limit\":8}}\n",
            "outside the configured scope",
        ),
        (
            "{\"consumer\":{\"limit\":4}}\n",
            "remove the configured field",
        ),
    ] {
        let outcome = edit(lf.as_str(), new).await;
        assert!(
            !outcome.authorized
                && outcome.effect == ToolEffect::None
                && was_denied(&outcome.output)
        );
        assert!(
            outcome
                .output
                .content
                .iter()
                .any(|part| matches!(part, ToolContent::Text(text) if text.contains(diagnostic)))
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), original);
    }
    let missing = edit("stale substring", "different").await;
    assert!(
        missing.authorized
            && missing.effect == ToolEffect::None
            && super::old_was_not_found(&missing.output)
    );
    assert_eq!(fs::read_to_string(&path).unwrap(), original);
    let valid = lf.replace("\"producer\":{\"limit\":4}", "\"producer\":{\"limit\":8}");
    let outcome = edit(lf.as_str(), valid.as_str()).await;
    assert!(outcome.authorized && !outcome.output.is_error);
    assert_eq!(
        fs::read_to_string(path).unwrap(),
        valid.replace('\n', "\r\n")
    );
    kernel.shutdown().await.unwrap();
}

#[tokio::test]
#[ignore = "requires an isolated llama.cpp with Qwen3-Coder template and the native tool choice proxy"]
async fn live_small_model_json_preflight() {
    use super::{TaskProbe, live_settings, sample, tasks};
    use serde_json::json;

    let (model, run_label, port) = live_settings();
    eprintln!(
        "small-model JSON preflight configuration: {}",
        json!({"model":model,"run":run_label,"port":port,"set":"development"})
    );
    for task in tasks()
        .into_iter()
        .filter(|task| matches!(task.id.as_str(), "batch-limit" | "stale-edit"))
    {
        for (index, guard) in [false, true, true, false].into_iter().enumerate() {
            let probe = if guard {
                TaskProbe::NativeRetryJson
            } else {
                TaskProbe::NativeRetry { restrict_old: true }
            };
            let result = sample(&task, "loop-react", &model, port, index + 1, Some(probe)).await;
            eprintln!("small-model JSON preflight sample: {result}");
        }
    }
}
