//! Test-workspace write policy. The caller supplies writable fixture paths
//! from outside the model conversation.

use std::collections::BTreeSet;
use std::sync::Arc;

use rah_api::{
    Plugin, PluginError, PluginManifest, Scope, async_trait,
    hooks::{ToolBeforeInvoke, ToolDecision},
};
use serde_json::Value;

pub(super) struct FixtureWriteGuard {
    manifest: PluginManifest,
    writable: Arc<BTreeSet<String>>,
}

impl FixtureWriteGuard {
    pub(super) fn new(paths: impl IntoIterator<Item = String>) -> Self {
        Self {
            manifest: PluginManifest::new("fixture-write-guard", "1"),
            writable: Arc::new(paths.into_iter().collect()),
        }
    }
}

fn allowed_batch(writable: &BTreeSet<String>, input: &Value) -> bool {
    input
        .get("edits")
        .and_then(Value::as_array)
        .is_some_and(|edits| {
            !edits.is_empty()
                && edits.iter().all(|edit| {
                    edit.get("path")
                        .and_then(Value::as_str)
                        .is_some_and(|path| writable.contains(path))
                })
        })
}

#[async_trait]
impl Plugin for FixtureWriteGuard {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    async fn apply(&self, scope: &mut Scope) -> Result<(), PluginError> {
        let writable = self.writable.clone();
        scope.hook_bail::<ToolBeforeInvoke, _, _>(10, move |request| {
            let denied = request.call.name.as_str() == "repo.edit-files"
                && !allowed_batch(&writable, &request.call.input.0);
            async move {
                denied.then(|| ToolDecision::Deny {
                    reason: "The edit batch includes a path outside the fixture write allowlist."
                        .into(),
                })
            }
        });
        Ok(())
    }
}

#[tokio::test]
async fn a_protected_check_denies_the_entire_batch_before_writing() {
    use crate::common::{TempDir, headless, kernel, new_session};
    use rah_api::{
        CancellationToken, MountOptions,
        tools::{Invocation, ToolInvoker},
    };
    use rah_protocol::{ToolCall, ToolCallId, ToolContent, ToolInput, ToolName};
    use serde_json::json;
    use std::fs;

    let dir = TempDir::new("small-model-write-guard");
    let root = dir.workspace();
    fs::write(root.join("settings.json"), "old value").unwrap();
    fs::write(root.join("checks.json"), "expected value").unwrap();
    let mut distribution = headless(&dir);
    distribution.set_config("policy-authority", "approve", json!(["repo.edit-files"]));
    let kernel = kernel(&distribution).await;
    kernel
        .mount(
            Arc::new(FixtureWriteGuard::new(["settings.json".into()])),
            MountOptions::default(),
        )
        .await
        .unwrap();
    let handle = kernel.handle();
    let session = new_session(&handle).await;
    let invoker = ToolInvoker::new(handle);
    let call = |edits| {
        invoker.invoke(
            Invocation {
                session: session.clone(),
                cancel: CancellationToken::new(),
                call: ToolCall {
                    id: ToolCallId::new(),
                    name: ToolName::new("repo.edit-files"),
                    input: ToolInput(json!({"edits":edits})),
                },
            },
            || {},
        )
    };
    let permitted = json!({"path":"settings.json","old":"old value","new":"new value"});
    for denied_path in [
        "checks.json",
        "./checks.json",
        "CHECKS.JSON",
        "settings.json/../checks.json",
    ] {
        let denied = json!({"path":denied_path,"old":"expected value","new":"fake"});
        let result = call(json!([permitted, denied])).await;
        assert!(result.output.is_error && !result.authorized);
        assert!(result.output.content.iter().any(|part| matches!(part,
            ToolContent::Text(text) if text.contains("outside the fixture write allowlist"))));
        assert_eq!(
            fs::read_to_string(root.join("settings.json")).unwrap(),
            "old value"
        );
        assert_eq!(
            fs::read_to_string(root.join("checks.json")).unwrap(),
            "expected value"
        );
    }
    let result = call(json!([permitted])).await;
    assert!(result.authorized && !result.output.is_error);
    assert_eq!(
        fs::read_to_string(root.join("settings.json")).unwrap(),
        "new value"
    );
    kernel.shutdown().await.unwrap();
}
