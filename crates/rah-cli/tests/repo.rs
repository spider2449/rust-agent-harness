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
