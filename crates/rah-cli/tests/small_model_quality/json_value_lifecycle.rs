use std::sync::Arc;

use rah_api::{
    CancellationToken, Kernel, Plugin, PluginError, PluginManifest, Scope, async_trait,
    hooks::ToolBeforeInvoke,
    tools::{Invocation, ToolInvoker},
    undo::{UndoError, changes, undo_newest},
};
use rah_protocol::{SessionId, ToolCall, ToolCallId, ToolEffect, ToolInput, ToolName};
use serde_json::{Value, json};

use super::{FixtureJsonValue, NAME};
use crate::common::{TempDir, headless, kernel, new_session, turn};

const FILE: &str = "queue.json";
const EDIT: &str = "repo.edit-files";
const ORIGINAL: &str = "{\"consumer\":{\"limit\":4},\"producer\":{\"limit\":4}}\r\n";
const PRODUCER_EDIT: &str = r#"/call fixture.set-json-value {"value":8}"#;

async fn setup(dir: &TempDir, approvals: &[&str]) -> Kernel {
    let workspace = dir.workspace();
    std::fs::write(workspace.join(FILE), ORIGINAL).unwrap();
    let mut distribution = headless(dir);
    distribution.set_config(
        "policy-authority",
        "tools",
        json!({NAME:"write",EDIT:"write"}),
    );
    distribution.set_config("policy-authority", "approve", json!(approvals));
    let kernel = kernel(&distribution).await;
    kernel
        .mount(
            Arc::new(FixtureJsonValue::new(workspace, "stale-edit")),
            Default::default(),
        )
        .await
        .unwrap();
    kernel
}

fn invocation(session: &SessionId, name: &str, input: Value) -> Invocation {
    Invocation {
        session: session.clone(),
        cancel: CancellationToken::new(),
        call: ToolCall {
            id: ToolCallId::new(),
            name: ToolName::new(name),
            input: ToolInput(input),
        },
    }
}

#[tokio::test]
async fn typed_edit_has_one_session_undo_and_restores_original_bytes() {
    let dir = TempDir::new("json-value-undo");
    let file = dir.workspace().join(FILE);
    let kernel = setup(&dir, &[NAME, EDIT]).await;
    let handle = kernel.handle();
    let session = new_session(&handle).await;

    turn(&handle, &session, PRODUCER_EDIT).await;
    let edited: Value = serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
    assert_eq!(
        edited,
        json!({"consumer":{"limit":4},"producer":{"limit":8}})
    );
    let recorded = changes(&handle, &session);
    assert_eq!(recorded.len(), 1);
    assert_eq!(recorded[0].tool, EDIT);
    assert_eq!(recorded[0].files, [FILE]);
    let undo = undo_newest(&handle, &session).await.unwrap().unwrap();
    assert_eq!(undo.change, recorded[0]);
    assert_eq!(undo.restored, [FILE]);
    assert_eq!(std::fs::read(&file).unwrap(), ORIGINAL.as_bytes());
    assert_eq!(
        undo_newest(&handle, &session).await,
        Some(Err(UndoError::Nothing))
    );

    let session = new_session(&handle).await;
    turn(&handle, &session, PRODUCER_EDIT).await;
    assert_eq!(changes(&handle, &session).len(), 1);
    let other = new_session(&handle).await;
    turn(
        &handle,
        &other,
        r#"/call repo.edit-files {"edits":[{"path":"queue.json","old":"\"consumer\":{\"limit\":4}","new":"\"consumer\":{\"limit\":9}"}]}"#,
    )
    .await;
    let newer = std::fs::read(&file).unwrap();
    let edited: Value = serde_json::from_slice(&newer).unwrap();
    assert_eq!(
        edited,
        json!({"consumer":{"limit":9},"producer":{"limit":8}})
    );
    assert_eq!(
        undo_newest(&handle, &session).await,
        Some(Err(UndoError::Changed {
            tool: EDIT.into(),
            files: vec![FILE.into()]
        }))
    );
    assert_eq!(std::fs::read(&file).unwrap(), newer);
    assert_eq!(changes(&handle, &other).len(), 1);
    kernel.shutdown().await.unwrap();
}

#[tokio::test]
async fn repo_write_approval_does_not_authorize_the_typed_tool() {
    let dir = TempDir::new("json-value-outer-denied");
    let kernel = setup(&dir, &[EDIT]).await;
    let handle = kernel.handle();
    let session = new_session(&handle).await;
    let result = ToolInvoker::new(handle.clone())
        .invoke(invocation(&session, NAME, json!({"value":8})), || {})
        .await;
    assert!(result.output.is_error && !result.authorized, "{result:?}");
    assert_eq!(result.effect, ToolEffect::None);
    assert!(changes(&handle, &session).is_empty());
    assert_eq!(
        std::fs::read(dir.workspace().join(FILE)).unwrap(),
        ORIGINAL.as_bytes()
    );
    kernel.shutdown().await.unwrap();
}

struct CompetingEdit {
    manifest: PluginManifest,
    session: SessionId,
    old: &'static str,
    new: &'static str,
}

#[async_trait]
impl Plugin for CompetingEdit {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    async fn apply(&self, scope: &mut Scope) -> Result<(), PluginError> {
        let invoker = ToolInvoker::new(scope.kernel());
        let session = self.session.clone();
        let (old, new) = (self.old, self.new);
        scope.hook_bail::<ToolBeforeInvoke, _, _>(5, move |request| {
            let invoker = invoker.clone();
            let session = session.clone();
            async move {
                // Pause the typed write after it reads the snapshot. The shorter
                // `old` of the second session's edit bypasses this hook branch.
                if request.call.name.as_str() == EDIT
                    && request.call.input.0["edits"][0]["old"].as_str() == Some(ORIGINAL)
                {
                    let result = invoker
                        .invoke(
                            invocation(
                                &session,
                                EDIT,
                                json!({"edits":[{"path":FILE,"old":old,"new":new}]}),
                            ),
                            || {},
                        )
                        .await;
                    assert!(result.authorized && !result.output.is_error, "{result:?}");
                    assert_eq!(result.effect, ToolEffect::Completed);
                }
                None
            }
        });
        Ok(())
    }
}

#[tokio::test]
async fn repo_substring_match_rejects_overlapping_changes_but_accepts_appended_whitespace() {
    for (old, new, typed_write_succeeds) in [
        (
            r#""consumer":{"limit":4}"#,
            r#""consumer":{"limit":9}"#,
            false,
        ),
        ("}}\r\n", "}}\r\n ", true),
    ] {
        let dir = TempDir::new("json-value-competing-edit");
        let file = dir.workspace().join(FILE);
        let kernel = setup(&dir, &[NAME, EDIT]).await;
        let handle = kernel.handle();
        let session = new_session(&handle).await;
        let other = new_session(&handle).await;
        kernel
            .mount(
                Arc::new(CompetingEdit {
                    manifest: PluginManifest::new("fixture-competing-edit", "1"),
                    session: other.clone(),
                    old,
                    new,
                }),
                Default::default(),
            )
            .await
            .unwrap();

        let result = ToolInvoker::new(handle.clone())
            .invoke(invocation(&session, NAME, json!({"value":8})), || {})
            .await;
        assert!(result.authorized, "{result:?}");
        assert_eq!(result.output.is_error, !typed_write_succeeds, "{result:?}");
        assert_eq!(changes(&handle, &other).len(), 1);
        let competing_bytes = ORIGINAL.replace(old, new);
        if typed_write_succeeds {
            assert_eq!(result.effect, ToolEffect::Completed);
            assert_eq!(changes(&handle, &session).len(), 1);
            let expected =
                competing_bytes.replace("\"producer\":{\"limit\":4}", "\"producer\":{\"limit\":8}");
            assert_eq!(std::fs::read(&file).unwrap(), expected.as_bytes());
            undo_newest(&handle, &session).await.unwrap().unwrap();
        } else {
            assert_eq!(result.effect, ToolEffect::None);
            assert!(changes(&handle, &session).is_empty());
        }
        assert_eq!(std::fs::read(&file).unwrap(), competing_bytes.as_bytes());
        kernel.shutdown().await.unwrap();
    }
}
