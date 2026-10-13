//! `context-compact` in a distribution: it runs before `context-default`,
//! summarizes old turns through the mounted model and leaves the session
//! log unchanged.

mod common;

use common::{TempDir, headless, kernel, new_session, turn};
use rah_api::{
    hooks::ModelBeforeRequest,
    session::{SESSION, rebuild_context},
};
use rah_cli::PluginSpec;
use rah_protocol::{ContextItem, ModelRequest};
use serde_json::json;

#[tokio::test]
async fn long_sessions_are_summarized_before_the_window_applies() {
    let dir = TempDir::new("compact");
    let mut distribution = headless(&dir);
    distribution.plugin.push(PluginSpec {
        kind: "context-compact".into(),
        id: None,
        config: json!({
            "trigger_chars": 600,
            "keep_chars": 200,
            "dir": dir.0.join("compact").display().to_string(),
        }),
        bind: Default::default(),
    });
    let kernel = kernel(&distribution).await;
    let handle = kernel.handle();
    let session = new_session(&handle).await;
    for index in 0..4 {
        turn(
            &handle,
            &session,
            &format!("turn {index} {}", "x".repeat(150)),
        )
        .await;
    }
    let records = handle.get(SESSION).unwrap().read(&session).await.unwrap();
    let mut items = rebuild_context(&records);
    let logged = items.len();
    items.push(ContextItem::User {
        text: "next".into(),
    });
    let shaped = handle
        .waterfall::<ModelBeforeRequest>(ModelRequest {
            items,
            ..ModelRequest::default()
        })
        .await
        .unwrap()
        .items;

    let summaries: Vec<&str> = shaped
        .iter()
        .filter_map(|item| match item {
            ContextItem::System { text } if text.starts_with("Summary of the ") => {
                Some(text.as_str())
            }
            _ => None,
        })
        .collect();
    assert_eq!(summaries.len(), 1, "{shaped:#?}");
    // The summary came from the mounted model (model-mock).
    assert!(summaries[0].contains("[model-mock]"), "{}", summaries[0]);
    // context-default saw the compacted history: nothing left to omit.
    assert!(
        !shaped.iter().any(|item| matches!(
            item,
            ContextItem::System { text } if text.contains("earlier conversation items are omitted")
        )),
        "{shaped:#?}"
    );
    assert!(shaped.len() < logged + 1, "{shaped:#?}");
    assert_eq!(
        shaped.last(),
        Some(&ContextItem::User {
            text: "next".into()
        })
    );
    // The log itself is unchanged.
    let after = handle.get(SESSION).unwrap().read(&session).await.unwrap();
    assert_eq!(after.len(), records.len());
    // The summary was kept on disk.
    let cached = std::fs::read_dir(dir.0.join("compact")).unwrap().count();
    assert!(cached >= 1);
}
