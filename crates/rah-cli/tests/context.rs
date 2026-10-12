//! P5 acceptance for `context-default` in the presets: the history window and
//! memory notes shape what the model sees, while the session log keeps
//! everything.

mod common;

use common::{TempDir, entries, headless, kernel, new_session, reply, turn};
use serde_json::json;

#[tokio::test]
async fn the_window_limits_the_model_context_but_not_the_log() {
    let dir = TempDir::new("context-window");
    let mut distribution = headless(&dir);
    distribution.set_config("context-default", "max_items", json!(2));
    let kernel = kernel(&distribution).await;
    let handle = kernel.handle();
    let session = new_session(&handle).await;
    turn(&handle, &session, "hello").await;
    // The mock reports how many items precede the newest one: just the
    // omission note.
    let text = reply(&turn(&handle, &session, "/notool again").await);
    assert_eq!(text, "[model-mock] you said: again (1 earlier items)");
    assert_eq!(entries(&handle, &session).await.len(), 8);
}

#[tokio::test]
async fn memory_notes_are_part_of_every_request() {
    let dir = TempDir::new("context-memory");
    std::fs::write(dir.0.join("memory.md"), "- the user likes tea\n").unwrap();
    let kernel = kernel(&headless(&dir)).await;
    let handle = kernel.handle();
    let session = new_session(&handle).await;
    let text = reply(&turn(&handle, &session, "/notool hi").await);
    assert_eq!(text, "[model-mock] you said: hi (1 earlier items)");
    // Without an approver, saving a note (`write`, Ask under the preset
    // policy) is denied.
    let text = reply(&turn(&handle, &session, r#"/call memory.remember {"note":"x"}"#).await);
    assert!(text.contains("no approver"), "{text}");
}
