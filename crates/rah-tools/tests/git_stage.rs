use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
};

use rah_protocol::{PermissionLevel, ToolContent, ToolInput};
use rah_tools::{GitStageTool, Tool, ToolContext, ToolError};
use serde_json::{Value, json};

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new(label: &str) -> Self {
        let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("rah-git-stage-{label}-{}-{id}", std::process::id()));
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
    let output = Command::new("where.exe")
        .arg("git.exe")
        .output()
        .expect("Git must be available for deterministic local Git tests");
    assert!(output.status.success(), "Git must be available for tests");
    let path = String::from_utf8(output.stdout).unwrap();
    fs::canonicalize(path.lines().next().unwrap()).unwrap()
}

#[cfg(not(windows))]
fn native_git() -> PathBuf {
    let output = Command::new("which")
        .arg("git")
        .output()
        .expect("Git must be available for deterministic local Git tests");
    assert!(output.status.success(), "Git must be available for tests");
    let path = String::from_utf8(output.stdout).unwrap();
    fs::canonicalize(path.trim()).unwrap()
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

fn tool(git: &Path, root: &Path) -> GitStageTool {
    GitStageTool::new(git, root, "release-artifact", root.join("target.txt")).unwrap()
}

fn content(output: &rah_protocol::ToolOutput) -> &Value {
    let [ToolContent::Json(content)] = output.content.as_slice() else {
        panic!("result should have one JSON content item")
    };
    content
}

fn index(root: &Path) -> Vec<u8> {
    let output = Command::new(native_git())
        .args(["ls-files", "-s", "-z"])
        .current_dir(root)
        .output()
        .unwrap();
    assert!(output.status.success());
    output.stdout
}

fn head(root: &Path) -> Vec<u8> {
    let output = Command::new(native_git())
        .args(["rev-parse", "HEAD"])
        .current_dir(root)
        .output()
        .unwrap();
    assert!(output.status.success());
    output.stdout
}

fn refs(root: &Path) -> Vec<u8> {
    let output = Command::new(native_git())
        .args(["for-each-ref", "--format=%(refname)%00%(objectname)%00"])
        .current_dir(root)
        .output()
        .unwrap();
    assert!(output.status.success());
    output.stdout
}

fn index_entry(index: &[u8], path: &[u8]) -> Vec<u8> {
    index
        .split(|byte| *byte == 0)
        .find(|record| record.ends_with(path))
        .expect("requested index entry should exist")
        .to_vec()
}

fn index_matches_worktree(root: &Path, target: &str) -> bool {
    Command::new(native_git())
        .args(["diff-files", "--quiet", "--", target])
        .current_dir(root)
        .status()
        .unwrap()
        .success()
}

#[tokio::test]
async fn dirty_tracked_file_stages_only_the_host_owned_target() {
    let base = TestDirectory::new("dirty");
    let root = base.repository();
    let executable = native_git();
    fs::write(root.join("target.txt"), "changed\n").unwrap();
    fs::write(root.join("other.txt"), "unrelated staged change\n").unwrap();
    git(&root, &["add", "--", "other.txt"]);
    fs::write(root.join("other.txt"), "unrelated worktree change\n").unwrap();
    let before_head = head(&root);
    let before_refs = refs(&root);
    let before_other = fs::read(root.join("other.txt")).unwrap();
    let before_target = fs::read(root.join("target.txt")).unwrap();
    let before_other_index = index_entry(&index(&root), b"other.txt");
    let output = tool(&executable, &root)
        .execute(ToolInput(json!({})), ToolContext::default())
        .await
        .unwrap();
    assert!(!output.is_error, "{output:?}");
    assert_eq!(content(&output)["status"], "ok");
    assert_eq!(content(&output)["staged"], true);
    assert_eq!(content(&output)["no_op"], false);
    assert_eq!(head(&root), before_head);
    assert_eq!(refs(&root), before_refs);
    assert_eq!(fs::read(root.join("other.txt")).unwrap(), before_other);
    assert_eq!(fs::read(root.join("target.txt")).unwrap(), before_target);
    assert_eq!(index_entry(&index(&root), b"other.txt"), before_other_index);
    assert!(index_matches_worktree(&root, "target.txt"));
}

#[tokio::test]
async fn already_staged_and_clean_targets_are_deterministic_no_ops() {
    let base = TestDirectory::new("no-op");
    let root = base.repository();
    let executable = native_git();
    fs::write(root.join("target.txt"), "changed\n").unwrap();
    git(&root, &["add", "--", "target.txt"]);
    let staged_before = index(&root);
    let output = tool(&executable, &root)
        .execute(ToolInput(json!({})), ToolContext::default())
        .await
        .unwrap();
    assert!(!output.is_error);
    assert_eq!(content(&output)["no_op"], true);
    assert_eq!(index(&root), staged_before);
    assert!(index_matches_worktree(&root, "target.txt"));

    git(&root, &["reset", "--quiet", "HEAD", "--", "target.txt"]);
    fs::write(root.join("target.txt"), "initial\n").unwrap();
    let clean_before = index(&root);
    let output = tool(&executable, &root)
        .execute(ToolInput(json!({})), ToolContext::default())
        .await
        .unwrap();
    assert!(!output.is_error);
    assert_eq!(content(&output)["no_op"], true);
    assert_eq!(index(&root), clean_before);
    assert!(index_matches_worktree(&root, "target.txt"));
}

#[tokio::test]
async fn input_and_untracked_or_directory_targets_are_rejected() {
    let base = TestDirectory::new("rejections");
    let root = base.repository();
    let executable = native_git();
    let stage = tool(&executable, &root);
    assert_eq!(stage.definition().permission, PermissionLevel::Execute);
    assert_eq!(
        stage.definition().input_schema,
        json!({"type":"object","properties":{},"additionalProperties":false})
    );
    assert!(matches!(
        stage
            .execute(
                ToolInput(json!({"path":"target.txt"})),
                ToolContext::default()
            )
            .await,
        Err(ToolError::InvalidInput { .. })
    ));
    fs::write(root.join("untracked.txt"), "new\n").unwrap();
    let untracked =
        GitStageTool::new(&executable, &root, "untracked", root.join("untracked.txt")).unwrap();
    assert!(
        untracked
            .execute(ToolInput(json!({})), ToolContext::default())
            .await
            .is_err()
    );
    fs::create_dir(root.join("directory")).unwrap();
    assert!(GitStageTool::new(&executable, &root, "directory", root.join("directory")).is_err());
}

#[tokio::test]
async fn root_and_executable_identity_revalidation_fails_closed() {
    let base = TestDirectory::new("identity");
    let root = base.repository();
    let executable = native_git();
    let stage = tool(&executable, &root);
    let old = base.0.join("old-repository");
    fs::rename(&root, &old).unwrap();
    fs::create_dir(&root).unwrap();
    fs::write(root.join("target.txt"), "initial\n").unwrap();
    fs::create_dir(root.join(".git")).unwrap();
    assert!(
        stage
            .execute(ToolInput(json!({})), ToolContext::default())
            .await
            .is_err()
    );
}

#[tokio::test]
async fn native_git_executable_identity_is_revalidated_before_any_git_command() {
    let base = TestDirectory::new("executable-identity");
    let root = base.repository();
    let native = native_git();
    let copied = base.0.join(native.file_name().unwrap());
    fs::copy(&native, &copied).unwrap();
    let stage = tool(&copied, &root);
    let mut replacement = fs::read(&copied).unwrap();
    replacement.push(0);
    fs::write(&copied, replacement).unwrap();

    let error = stage
        .execute(ToolInput(json!({})), ToolContext::default())
        .await
        .expect_err("changed native Git executable must be rejected before execution");
    assert!(
        error
            .to_string()
            .contains("configured executable identity changed")
    );
    assert_eq!(fs::read(root.join("target.txt")).unwrap(), b"initial\n");
}

#[tokio::test]
async fn same_repository_calls_serialize_to_one_stage_and_one_no_op() {
    let base = TestDirectory::new("lease");
    let root = base.repository();
    let executable = native_git();
    fs::write(root.join("target.txt"), "changed\n").unwrap();
    let first = std::sync::Arc::new(tool(&executable, &root));
    let second = std::sync::Arc::new(tool(&executable, &root));
    let first_task = tokio::spawn({
        let first = first.clone();
        async move {
            first
                .execute(ToolInput(json!({})), ToolContext::default())
                .await
                .unwrap()
        }
    });
    let second_task = tokio::spawn({
        let second = second.clone();
        async move {
            second
                .execute(ToolInput(json!({})), ToolContext::default())
                .await
                .unwrap()
        }
    });
    let results = [first_task.await.unwrap(), second_task.await.unwrap()];
    assert_eq!(
        results
            .iter()
            .filter(|output| content(output)["staged"] == true)
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
    let git = native_git();
    fs::remove_file(root.join("target.txt")).unwrap();
    symlink(root.join("other.txt"), root.join("target.txt")).unwrap();
    assert!(GitStageTool::new(git, root.join("."), "link", root.join("target.txt")).is_err());
}
