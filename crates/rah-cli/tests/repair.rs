//! `tool-input-repair` in the presets (ADR 0049): a model that sends a number
//! as a string still gets its tool call through, and the policy judges the
//! repaired input.

mod common;

use common::{TempDir, headless, kernel, new_session, reply, turn};

#[tokio::test]
async fn a_numeric_string_reaches_the_tool_as_a_number() {
    let dir = TempDir::new("repair");
    let workspace = dir.workspace();
    std::fs::create_dir_all(workspace.join("a/b")).unwrap();
    std::fs::write(workspace.join("a/b/deep.txt"), "x").unwrap();

    let kernel = kernel(&headless(&dir)).await;
    let handle = kernel.handle();
    let session = new_session(&handle).await;

    let listed = reply(&turn(&handle, &session, r#"/call fs.list {"depth":"3"}"#).await);
    assert!(listed.contains("deep.txt"), "{listed}");
    assert!(!listed.contains("invalid input"), "{listed}");
    // A value that is not a number is still reported by the tool.
    let refused = reply(&turn(&handle, &session, r#"/call fs.list {"depth":"deep"}"#).await);
    assert!(refused.contains("invalid input"), "{refused}");
}
