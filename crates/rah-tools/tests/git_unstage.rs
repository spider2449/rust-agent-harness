use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
};

use rah_protocol::{PermissionLevel, ToolContent, ToolInput};
use rah_tools::{GitUnstageTool, Tool, ToolContext, ToolError};
use serde_json::{Value, json};

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new(label: &str) -> Self {
        let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "rah-git-unstage-{label}-{}-{id}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn repository(&self) -> PathBuf {
        let root = self.0.join("repository");
        fs::create_dir(&root).unwrap();
        git(&root, &["init", "--quiet"]);
        fs::write(root.join("target.txt"), "initial\n").unwrap();
        fs::write(root.join("other.txt"), "other\n").unwrap();
        git(&root, &["add", "--", "target.txt", "other.txt"]);
        git(
            &root,
            &[
                "-c",
                "user.name=RAH Test",
                "-c",
                "user.email=rah@example.invalid",
                "commit",
                "--quiet",
                "-m",
                "initial",
            ],
        );
        root
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[cfg(windows)]
fn native_git() -> PathBuf {
    let output = Command::new("where.exe").arg("git.exe").output().unwrap();
    assert!(output.status.success());
    fs::canonicalize(
        String::from_utf8(output.stdout)
            .unwrap()
            .lines()
            .next()
            .unwrap(),
    )
    .unwrap()
}

#[cfg(not(windows))]
fn native_git() -> PathBuf {
    let output = Command::new("which").arg("git").output().unwrap();
    assert!(output.status.success());
    fs::canonicalize(String::from_utf8(output.stdout).unwrap().trim()).unwrap()
}

fn git(cwd: &Path, args: &[&str]) {
    let status = Command::new(native_git())
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .env_clear()
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", null_device())
        .status()
        .unwrap();
    assert!(status.success(), "Git setup failed: {args:?}");
}

#[cfg(windows)]
fn null_device() -> &'static str {
    "NUL"
}

#[cfg(not(windows))]
fn null_device() -> &'static str {
    "/dev/null"
}

fn tool(git: &Path, root: &Path) -> GitUnstageTool {
    GitUnstageTool::new(git, root, "release-artifact", root.join("target.txt")).unwrap()
}

fn content(output: &rah_protocol::ToolOutput) -> &Value {
    let [ToolContent::Json(content)] = output.content.as_slice() else {
        panic!("result should have one JSON content item")
    };
    content
}

fn git_output(root: &Path, args: &[&str]) -> Vec<u8> {
    let output = Command::new(native_git())
        .args(args)
        .current_dir(root)
        .output()
        .unwrap();
    assert!(output.status.success(), "Git observation failed: {args:?}");
    output.stdout
}

fn index(root: &Path) -> Vec<u8> {
    git_output(root, &["ls-files", "-s", "-z"])
}

fn index_entry(index: &[u8], path: &[u8]) -> Vec<u8> {
    index
        .split(|byte| *byte == 0)
        .find(|record| record.ends_with(path))
        .expect("requested index entry should exist")
        .to_vec()
}

#[tokio::test]
async fn staged_tracked_target_unstages_and_preserves_every_other_observed_plane() {
    let base = TestDirectory::new("success");
    let root = base.repository();
    let executable = native_git();
    fs::write(root.join("target.txt"), "staged target\n").unwrap();
    git(&root, &["add", "--", "target.txt"]);
    fs::write(root.join("other.txt"), "staged other\n").unwrap();
    git(&root, &["add", "--", "other.txt"]);
    fs::write(root.join("other.txt"), "dirty other\n").unwrap();
    let target_worktree = fs::read(root.join("target.txt")).unwrap();
    let other_worktree = fs::read(root.join("other.txt")).unwrap();
    let other_index = index_entry(&index(&root), b"other.txt");
    let before_head = git_output(&root, &["rev-parse", "HEAD"]);
    let before_refs = git_output(
        &root,
        &["for-each-ref", "--format=%(refname)%00%(objectname)%00"],
    );

    let output = tool(&executable, &root)
        .execute(ToolInput(json!({})), ToolContext::default())
        .await
        .unwrap();

    assert!(!output.is_error, "{output:?}");
    assert_eq!(content(&output)["status"], "ok");
    assert_eq!(content(&output)["unstaged"], true);
    assert_eq!(fs::read(root.join("target.txt")).unwrap(), target_worktree);
    assert_eq!(fs::read(root.join("other.txt")).unwrap(), other_worktree);
    assert_eq!(index_entry(&index(&root), b"other.txt"), other_index);
    assert_eq!(git_output(&root, &["rev-parse", "HEAD"]), before_head);
    assert_eq!(
        git_output(
            &root,
            &["for-each-ref", "--format=%(refname)%00%(objectname)%00"]
        ),
        before_refs
    );
    assert_ne!(
        index_entry(&index(&root), b"target.txt"),
        b"100644 0000000000000000000000000000000000000000 0\ttarget.txt"
    );
    assert!(
        Command::new(native_git())
            .args(["diff", "--cached", "--quiet", "--", "target.txt"])
            .current_dir(&root)
            .status()
            .unwrap()
            .success()
    );
}

#[tokio::test]
async fn staged_and_unstaged_target_preserves_current_worktree_bytes() {
    let base = TestDirectory::new("staged-and-unstaged");
    let root = base.repository();
    fs::write(root.join("target.txt"), "staged\n").unwrap();
    git(&root, &["add", "--", "target.txt"]);
    fs::write(root.join("target.txt"), "unstaged\n").unwrap();
    let bytes = fs::read(root.join("target.txt")).unwrap();

    let output = tool(&native_git(), &root)
        .execute(ToolInput(json!({})), ToolContext::default())
        .await
        .unwrap();

    assert!(!output.is_error);
    assert_eq!(content(&output)["unstaged"], true);
    assert_eq!(fs::read(root.join("target.txt")).unwrap(), bytes);
    assert!(
        Command::new(native_git())
            .args(["diff", "--cached", "--quiet", "--", "target.txt"])
            .current_dir(&root)
            .status()
            .unwrap()
            .success()
    );
}

#[tokio::test]
async fn already_unstaged_dirty_and_clean_targets_are_verified_no_ops() {
    let base = TestDirectory::new("no-op");
    let root = base.repository();
    fs::write(root.join("target.txt"), "dirty\n").unwrap();
    let dirty_before = fs::read(root.join("target.txt")).unwrap();
    let output = tool(&native_git(), &root)
        .execute(ToolInput(json!({})), ToolContext::default())
        .await
        .unwrap();
    assert!(!output.is_error);
    assert_eq!(content(&output)["no_op"], true);
    assert_eq!(fs::read(root.join("target.txt")).unwrap(), dirty_before);

    fs::write(root.join("target.txt"), "initial\n").unwrap();
    let output = tool(&native_git(), &root)
        .execute(ToolInput(json!({})), ToolContext::default())
        .await
        .unwrap();
    assert!(!output.is_error);
    assert_eq!(content(&output)["no_op"], true);
}

#[tokio::test]
async fn empty_schema_and_host_target_validation_fail_closed() {
    let base = TestDirectory::new("rejections");
    let root = base.repository();
    let executable = native_git();
    let unstage = tool(&executable, &root);
    assert_eq!(unstage.definition().permission, PermissionLevel::Execute);
    assert_eq!(
        unstage.definition().input_schema,
        json!({"type":"object","properties":{},"additionalProperties":false})
    );
    assert!(matches!(
        unstage
            .execute(
                ToolInput(json!({"path":"other.txt"})),
                ToolContext::default()
            )
            .await,
        Err(ToolError::InvalidInput { .. })
    ));
    assert!(
        !unstage
            .execute(ToolInput(json!({})), ToolContext::default())
            .await
            .unwrap()
            .is_error
    );

    fs::write(root.join("untracked.txt"), "new\n").unwrap();
    let untracked =
        GitUnstageTool::new(&executable, &root, "untracked", root.join("untracked.txt")).unwrap();
    assert!(
        untracked
            .execute(ToolInput(json!({})), ToolContext::default())
            .await
            .is_err()
    );
    fs::create_dir(root.join("directory")).unwrap();
    assert!(GitUnstageTool::new(&executable, &root, "directory", root.join("directory")).is_err());
}

#[tokio::test]
async fn repository_and_executable_identity_changes_are_rejected_before_mutation() {
    let base = TestDirectory::new("identity");
    let root = base.repository();
    let executable = native_git();
    let unstage = tool(&executable, &root);
    let old = base.0.join("old-repository");
    fs::rename(&root, &old).unwrap();
    fs::create_dir(&root).unwrap();
    fs::create_dir(root.join(".git")).unwrap();
    fs::write(root.join("target.txt"), "initial\n").unwrap();
    assert!(
        unstage
            .execute(ToolInput(json!({})), ToolContext::default())
            .await
            .is_err()
    );

    let second = TestDirectory::new("executable");
    let second_root = second.repository();
    let copied = second.0.join(executable.file_name().unwrap());
    fs::copy(&executable, &copied).unwrap();
    let unstage = tool(&copied, &second_root);
    let mut replacement = fs::read(&copied).unwrap();
    replacement.push(0);
    fs::write(&copied, replacement).unwrap();
    assert!(
        unstage
            .execute(ToolInput(json!({})), ToolContext::default())
            .await
            .is_err()
    );
}

#[tokio::test]
async fn same_repository_calls_serialize_to_one_unstage_and_one_no_op() {
    let base = TestDirectory::new("lease");
    let root = base.repository();
    fs::write(root.join("target.txt"), "staged\n").unwrap();
    git(&root, &["add", "--", "target.txt"]);
    let first = std::sync::Arc::new(tool(&native_git(), &root));
    let second = std::sync::Arc::new(tool(&native_git(), &root));
    let first_task = tokio::spawn({
        let tool = first.clone();
        async move {
            tool.execute(ToolInput(json!({})), ToolContext::default())
                .await
                .unwrap()
        }
    });
    let second_task = tokio::spawn({
        let tool = second.clone();
        async move {
            tool.execute(ToolInput(json!({})), ToolContext::default())
                .await
                .unwrap()
        }
    });
    let results = [first_task.await.unwrap(), second_task.await.unwrap()];
    assert_eq!(
        results
            .iter()
            .filter(|output| content(output)["unstaged"] == true)
            .count(),
        1
    );
    assert_eq!(
        results
            .iter()
            .filter(|output| content(output)["no_op"] == true)
            .count(),
        1
    );
}

#[cfg(unix)]
#[test]
fn symbolic_link_target_is_rejected() {
    use std::os::unix::fs::symlink;
    let base = TestDirectory::new("symlink");
    let root = base.repository();
    fs::remove_file(root.join("target.txt")).unwrap();
    symlink(root.join("other.txt"), root.join("target.txt")).unwrap();
    assert!(
        GitUnstageTool::new(
            native_git(),
            root.join("."),
            "link",
            root.join("target.txt")
        )
        .is_err()
    );
}
