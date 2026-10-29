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
    // The presets keep one memory file per workspace, named after the
    // sandbox root (the workspace directory).
    let root = std::fs::canonicalize(dir.workspace()).unwrap();
    let root = root.display().to_string();
    let root = root.strip_prefix(r"\\?\").unwrap_or(&root);
    let name = rah_plugin_context_default::workspace_file_name(std::path::Path::new(root));
    std::fs::create_dir_all(dir.0.join("memory")).unwrap();
    std::fs::write(dir.0.join("memory").join(name), "- the user likes tea\n").unwrap();
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

#[tokio::test]
async fn skills_are_listed_and_loaded_through_the_presets() {
    let dir = TempDir::new("context-skills");
    std::fs::create_dir_all(dir.0.join("skills/tea")).unwrap();
    std::fs::write(
        dir.0.join("skills/tea/SKILL.md"),
        "---\ndescription: Brews tea.\n---\nBoil water first.\n",
    )
    .unwrap();
    let kernel = kernel(&headless(&dir)).await;
    let handle = kernel.handle();
    let session = new_session(&handle).await;
    // The listing is one system item before the user message.
    let text = reply(&turn(&handle, &session, "/notool hi").await);
    assert_eq!(text, "[model-mock] you said: hi (1 earlier items)");
    let text = reply(&turn(&handle, &session, r#"/call skills.load {"name":"tea"}"#).await);
    assert_eq!(text, "[model-mock] skills.load returned: Boil water first.");
}

#[tokio::test]
async fn the_instructions_feature_adds_the_workspace_file_on_every_request() {
    let dir = TempDir::new("context-instructions");
    let mut distribution = headless(&dir);
    distribution.enable("instructions").unwrap();
    let kernel = kernel(&distribution).await;
    let handle = kernel.handle();
    let text = reply(&turn(&handle, &new_session(&handle).await, "/notool hi").await);
    assert_eq!(text, "[model-mock] you said: hi (0 earlier items)");
    // Read on every request: a file written now counts at once.
    std::fs::write(dir.workspace().join("AGENTS.md"), "Run the tests.").unwrap();
    let text = reply(&turn(&handle, &new_session(&handle).await, "/notool hi").await);
    assert_eq!(text, "[model-mock] you said: hi (1 earlier items)");
}
