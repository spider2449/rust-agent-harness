//! A request the model's context window cannot hold is sent again with the
//! old history summarized (ADR 0052): the turn goes on, and the session log
//! keeps the whole conversation.

mod common;

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU32, Ordering},
};

use common::{TempDir, entries, headless, kernel, new_session, reply, turn};
use rah_api::{
    CancellationToken, Plugin, PluginError, PluginManifest, Scope, async_trait,
    hooks::{ModelBeforeRequest, ModelOverflow},
    kernel::MountOptions,
    model::{MODEL, ModelError, ModelProvider, ModelStream, answer_side_question},
    session::SESSION,
};
use rah_protocol::{
    AgentEvent, ContextItem, ModelEvent, ModelRequest, PermissionLevel, RequestId, SessionEntry,
    ToolDefinition, ToolName,
};
use serde_json::json;

/// A model whose window holds `window` characters of conversation.
struct SmallWindow {
    window: usize,
    sizes: Mutex<Vec<usize>>,
    side_history_counts: Mutex<Vec<usize>>,
    overflows: AtomicU32,
}

fn chars(request: &ModelRequest) -> usize {
    request
        .items
        .iter()
        .map(|item| match item {
            ContextItem::System { text }
            | ContextItem::User { text }
            | ContextItem::UserGuidance { text }
            | ContextItem::Assistant { text } => text.len(),
            _ => 0,
        })
        .sum()
}

#[async_trait]
impl ModelProvider for SmallWindow {
    async fn stream(
        &self,
        request: ModelRequest,
        _cancel: CancellationToken,
    ) -> Result<ModelStream, ModelError> {
        if request.items.iter().any(|item| matches!(item, ContextItem::System { text } if text.starts_with("Answer the side question"))) {
            assert!(request.tools.is_empty());
            let history_count = request.items.iter().filter(
                |item| matches!(item, ContextItem::User { text } if text.starts_with('x'))
            ).count();
            self.side_history_counts.lock().unwrap().push(history_count);
        }
        let size = chars(&request);
        self.sizes.lock().unwrap().push(size);
        if size > self.window {
            self.overflows.fetch_add(1, Ordering::SeqCst);
            return Err(ModelError::ContextOverflow("too long".into()));
        }
        Ok(Box::pin(futures::stream::iter(vec![
            Ok(ModelEvent::TextDelta { text: "ok".into() }),
            Ok(ModelEvent::Completed),
        ])))
    }
}

struct SmallWindowPlugin(PluginManifest, Arc<SmallWindow>);

#[async_trait]
impl Plugin for SmallWindowPlugin {
    fn manifest(&self) -> &PluginManifest {
        &self.0
    }

    async fn apply(&self, scope: &mut Scope) -> Result<(), PluginError> {
        scope.provide(MODEL, self.1.clone() as Arc<dyn ModelProvider>)?;
        Ok(())
    }
}

async fn setup(name: &str, with_compact: bool) -> (rah_api::Kernel, TempDir, Arc<SmallWindow>) {
    let dir = TempDir::new(name);
    let mut distribution = headless(&dir);
    // The window sizes below count only the conversation.
    distribution.set_config("context-default", "system", json!(""));
    distribution.set_config("context-default", "environment", json!(false));
    if with_compact {
        distribution.enable("compact").unwrap();
        distribution.set_config("context-compact", "trigger_chars", json!(1_000_000));
        distribution.set_config("context-compact", "keep_chars", json!(1000));
    }
    let kernel = kernel(&distribution).await;
    kernel.unmount(&"model-mock".into()).await.unwrap();
    let model = Arc::new(SmallWindow {
        window: 4000,
        sizes: Mutex::new(Vec::new()),
        side_history_counts: Mutex::new(Vec::new()),
        overflows: AtomicU32::new(0),
    });
    kernel
        .mount(
            Arc::new(SmallWindowPlugin(
                PluginManifest::new("small-window", "1").provides(["model"]),
                model.clone(),
            )),
            MountOptions::default(),
        )
        .await
        .unwrap();
    (kernel, dir, model)
}

#[tokio::test]
async fn an_overflow_compacts_the_history_and_the_turn_goes_on() {
    let (kernel, _dir, model) = setup("overflow-compact", true).await;
    let handle = kernel.handle();
    let session = new_session(&handle).await;
    let long = "a".repeat(1500);
    for round in 0..2 {
        let events = turn(&handle, &session, &format!("{round} {long}")).await;
        assert!(
            matches!(events.last(), Some(AgentEvent::Completed { .. })),
            "{events:?} sizes {:?}",
            model.sizes.lock().unwrap()
        );
        assert!(reply(&events).contains("ok"));
        assert!(
            !events
                .iter()
                .any(|event| matches!(event, AgentEvent::ModelRetrying { .. })),
            "round {round} fit the window"
        );
    }
    // The third message no longer fits next to the first two.
    let events = turn(&handle, &session, &format!("2 {long}")).await;
    assert!(
        matches!(events.last(), Some(AgentEvent::Completed { .. })),
        "{events:?}"
    );
    let notices: Vec<&String> = events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::ModelRetrying { reason, .. } => Some(reason),
            _ => None,
        })
        .collect();
    assert_eq!(notices.len(), 1, "{events:?}");
    assert!(notices[0].contains("context window"), "{}", notices[0]);
    assert_eq!(model.overflows.load(Ordering::SeqCst), 1);
    // The log still holds all three messages.
    let log = entries(&handle, &session).await;
    assert_eq!(
        log.iter().filter(|entry| **entry == "turn-started").count(),
        3
    );
}

#[tokio::test]
async fn without_compaction_the_overflow_ends_the_turn_with_a_clear_error() {
    let (kernel, _dir, model) = setup("overflow-plain", false).await;
    let handle = kernel.handle();
    let session = new_session(&handle).await;
    let huge = "b".repeat(5000);
    let events = turn(&handle, &session, &huge).await;
    match events.last() {
        Some(AgentEvent::Failed { message, .. }) => {
            assert!(message.contains("context window"), "{message}");
        }
        other => panic!("expected a failure, got {other:?}"),
    }
    assert_eq!(model.overflows.load(Ordering::SeqCst), 1);
}

struct AddTool(PluginManifest);

#[async_trait]
impl Plugin for AddTool {
    fn manifest(&self) -> &PluginManifest {
        &self.0
    }

    async fn apply(&self, scope: &mut Scope) -> Result<(), PluginError> {
        async fn add_tool(mut request: ModelRequest) -> ModelRequest {
            request.tools.push(ToolDefinition {
                name: ToolName::new("echo"),
                description: "test".into(),
                input_schema: json!({"type": "object"}),
                permission: PermissionLevel::None,
            });
            request
        }
        scope.hook_waterfall::<ModelBeforeRequest, _, _>(100, add_tool);
        scope.hook_waterfall::<ModelOverflow, _, _>(100, add_tool);
        Ok(())
    }
}

#[tokio::test]
async fn side_questions_use_the_history_window_and_recover_from_model_overflow_without_tools() {
    let (kernel, _dir, model) = setup("side-question-overflow", true).await;
    kernel
        .update_config(
            &"context-default".into(),
            json!({ "system": "", "environment": false, "max_items": 5 }),
        )
        .await
        .unwrap();
    kernel
        .update_config(
            &"context-compact".into(),
            json!({ "trigger_chars": 1000000, "keep_chars": 1000, "chunk_chars": 2500 }),
        )
        .await
        .unwrap();
    kernel
        .mount(
            Arc::new(AddTool(PluginManifest::new("test-add-tool", "1"))),
            MountOptions::default(),
        )
        .await
        .unwrap();
    let handle = kernel.handle();
    let session = new_session(&handle).await;
    let log = handle.get(SESSION).unwrap();
    for _ in 0..3 {
        let turn = RequestId::new();
        log.append(
            &session,
            SessionEntry::TurnStarted {
                turn: turn.clone(),
                input: "x".repeat(2100),
            },
        )
        .await
        .unwrap();
        log.append(
            &session,
            SessionEntry::AssistantMessage {
                turn,
                text: "answer".into(),
            },
        )
        .await
        .unwrap();
    }
    let before = log.read(&session).await.unwrap();
    assert_eq!(
        answer_side_question(&handle, &session, "recap", CancellationToken::new())
            .await
            .unwrap(),
        "ok"
    );
    assert_eq!(*model.side_history_counts.lock().unwrap(), [2, 0]);
    assert_eq!(model.overflows.load(Ordering::SeqCst), 1);
    assert_eq!(log.read(&session).await.unwrap(), before);
}
