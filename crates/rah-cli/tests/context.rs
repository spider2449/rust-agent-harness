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
    assert_eq!(text, "[model-mock] you said: again (3 earlier items)");
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
    assert_eq!(text, "[model-mock] you said: hi (3 earlier items)");
    // Without an approver, saving a note (`write`, Ask under the preset
    // policy) is denied.
    let text = reply(&turn(&handle, &session, r#"/call memory.remember {"note":"x"}"#).await);
    assert!(text.contains("no approver"), "{text}");
}

#[tokio::test]
async fn skills_are_listed_and_loaded_through_the_presets() {
    let dir = TempDir::new("context-skills");
    let kernel = kernel(&headless(&dir)).await;
    let handle = kernel.handle();
    let session = new_session(&handle).await;
    let run = |args: &[&str]| {
        std::process::Command::new(env!("CARGO_BIN_EXE_rah"))
            .current_dir(&dir.0)
            .env("RAH_HOME", &dir.0)
            .arg("skills")
            .args(args)
            .output()
            .unwrap()
    };
    std::fs::write(
        dir.0.join("tea.md"),
        "---\ndescription: Brews tea.\n---\nBoil water first.\n",
    )
    .unwrap();
    let added = run(&["add", "tea", "--file", "tea.md"]);
    assert!(
        added.status.success(),
        "{}",
        String::from_utf8_lossy(&added.stderr)
    );
    assert!(!run(&["add", "tea"]).status.success());
    assert!(!run(&["delete", "../skills/tea"]).status.success());
    // The listing is one system item before the user message.
    let text = reply(&turn(&handle, &session, "/notool hi").await);
    assert_eq!(text, "[model-mock] you said: hi (3 earlier items)");
    let text = reply(&turn(&handle, &session, r#"/call skills.load {"name":"tea"}"#).await);
    assert_eq!(text, "[model-mock] skills.load returned: Boil water first.");
    let extra = dir.0.join("skills/tea/notes.md");
    std::fs::write(&extra, "keep").unwrap();
    assert!(run(&["delete", "tea"]).status.success());
    assert_eq!(std::fs::read_to_string(extra).unwrap(), "keep");
    let text = reply(&turn(&handle, &session, r#"/call skills.load {"name":"tea"}"#).await);
    assert!(text.contains("there is no skill `tea`"), "{text}");
    assert!(!run(&["delete", "tea"]).status.success());
    assert!(run(&["add", "tea"]).status.success());
}

#[test]
fn skill_commands_respect_configured_directory_and_size_limit() {
    let dir = TempDir::new("skills-configuration");
    let config = dir.0.join("rah.toml");
    std::fs::write(
        &config,
        "[[plugin]]\nuse = \"skills-fs\"\n[plugin.config]\ndir = \"custom\"\nmax_skill_bytes = 6\n",
    )
    .unwrap();
    let source = dir.0.join("source.md");
    let run = |args: &[&str]| {
        std::process::Command::new(env!("CARGO_BIN_EXE_rah"))
            .current_dir(&dir.0)
            .arg("--data-dir")
            .arg(dir.home())
            .arg("--config")
            .arg(&config)
            .arg("skills")
            .args(args)
            .output()
            .unwrap()
    };
    std::fs::write(&source, "1234567").unwrap();
    assert!(!run(&["add", "one", "--file", "source.md"]).status.success());
    std::fs::write(&source, "123456").unwrap();
    let added = run(&["add", "one", "--file", "source.md"]);
    assert!(
        added.status.success(),
        "{}",
        String::from_utf8_lossy(&added.stderr)
    );
    let installed = dir.home().join("custom/one");
    assert_eq!(
        std::fs::read(installed.join("SKILL.md")).unwrap(),
        b"123456"
    );
    assert!(run(&["delete", "one"]).status.success());
    assert!(!installed.exists());
}

#[tokio::test]
async fn the_instructions_feature_adds_the_workspace_file_on_every_request() {
    let dir = TempDir::new("context-instructions");
    let mut distribution = headless(&dir);
    distribution.enable("instructions").unwrap();
    let kernel = kernel(&distribution).await;
    let handle = kernel.handle();
    let text = reply(&turn(&handle, &new_session(&handle).await, "/notool hi").await);
    assert_eq!(text, "[model-mock] you said: hi (2 earlier items)");
    // Read on every request: a file written now counts at once.
    std::fs::write(dir.workspace().join("AGENTS.md"), "Run the tests.").unwrap();
    let text = reply(&turn(&handle, &new_session(&handle).await, "/notool hi").await);
    assert_eq!(text, "[model-mock] you said: hi (3 earlier items)");
}
