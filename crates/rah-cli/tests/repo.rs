//! P4 acceptance for `tools-repo`: read-only repository tools run through
//! `sandbox-process` in the guarded presets.

mod common;

use std::process::{Command, Stdio};

use common::{TempDir, headless, kernel, new_session, reply, turn};

fn git(dir: &std::path::Path, args: &[&str]) {
    let status = Command::new("git")
        .args([
            "-c",
            "user.name=RAH Test",
            "-c",
            "user.email=rah@example.invalid",
        ])
        .args(args)
        .current_dir(dir)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env(
            "GIT_CONFIG_GLOBAL",
            if cfg!(windows) { "NUL" } else { "/dev/null" },
        )
        .stdout(Stdio::null())
        .status()
        .unwrap();
    assert!(status.success(), "git {args:?}");
}

#[tokio::test]
async fn repo_tools_observe_the_workspace_repository() {
    let dir = TempDir::new("repo-tools");
    let workspace = dir.workspace();
    std::fs::write(workspace.join("README.md"), "plugin kernel\n").unwrap();
    git(&workspace, &["init", "--quiet", "--initial-branch=main"]);
    git(&workspace, &["add", "README.md"]);
    git(&workspace, &["commit", "--quiet", "-m", "init"]);
    std::fs::write(workspace.join("README.md"), "plugin kernel\nedited\n").unwrap();

    let kernel = kernel(&headless(&dir)).await;
    let handle = kernel.handle();
    let session = new_session(&handle).await;

    let listed = reply(&turn(&handle, &session, "/call repo.list {}").await);
    assert!(
        listed.contains(r#"{"kind":"file","name":"README.md"}"#),
        "{listed}"
    );
    let status = reply(&turn(&handle, &session, "/call repo.status {}").await);
    assert!(status.contains(r#""path":"README.md""#), "{status}");
    assert!(status.contains(r#""worktree":"M""#), "{status}");
    let diff = reply(&turn(&handle, &session, "/call repo.diff {}").await);
    assert!(diff.contains("+edited"), "{diff}");
    let unsafe_path = reply(&turn(&handle, &session, r#"/call repo.list {"path":"../x"}"#).await);
    assert!(
        unsafe_path.contains("not a safe repository-relative path"),
        "{unsafe_path}"
    );
}

#[tokio::test]
async fn repo_tools_report_a_workspace_that_is_not_a_repository() {
    let dir = TempDir::new("repo-none");
    let kernel = kernel(&headless(&dir)).await;
    let handle = kernel.handle();
    let session = new_session(&handle).await;
    let text = reply(&turn(&handle, &session, "/call repo.status {}").await);
    assert!(text.contains("repo.status returned:"), "{text}");
    assert!(text.contains("git"), "{text}");
}

#[tokio::test]
async fn undo_recognizes_a_mounted_formatters_write() {
    use serde_json::json;

    let dir = TempDir::new("repo-format-undo");
    let workspace = dir.workspace();
    git(&workspace, &["init", "--quiet"]);
    git(&workspace, &["config", "core.autocrlf", "false"]);
    std::fs::write(
        workspace.join("format.diff"),
        "--- a/note.txt\n+++ b/note.txt\n@@ -1 +1 @@\n-raw\n+formatted\n",
    )
    .unwrap();
    let mut distribution = headless(&dir);
    distribution.enable("format").unwrap();
    distribution.set_config("format-on-write", "root", json!(workspace));
    distribution.set_config("format-on-write", "formatters", json!({
        "test": {"command":["git","apply","--include=$FILE","format.diff"],"extensions":[".txt"]}
    }));
    distribution.set_config("policy-authority", "approve", json!(["repo.create-file"]));
    let kernel = kernel(&distribution).await;
    let handle = kernel.handle();
    let session = new_session(&handle).await;
    let answer = reply(
        &turn(
            &handle,
            &session,
            r#"/call repo.create-file {"path":"note.txt","content":"raw\n"}"#,
        )
        .await,
    );
    assert!(answer.contains("Formatted by"), "{answer}");
    let file = workspace.join("note.txt");
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "formatted\n");
    rah_api::undo::undo_newest(&handle, &session)
        .await
        .unwrap()
        .unwrap();
    assert!(!file.exists());
}

#[tokio::test]
async fn repo_write_tools_need_approval_in_the_presets() {
    let dir = TempDir::new("repo-write-ask");
    let kernel = kernel(&headless(&dir)).await;
    let handle = kernel.handle();
    let session = new_session(&handle).await;
    let text = reply(
        &turn(
            &handle,
            &session,
            r#"/call repo.create-file {"path":"new.txt","content":"x"}"#,
        )
        .await,
    );
    assert!(
        text.contains("requires `write` permission") || text.contains("no approver"),
        "{text}"
    );
    assert!(!dir.workspace().join("new.txt").exists());
}
