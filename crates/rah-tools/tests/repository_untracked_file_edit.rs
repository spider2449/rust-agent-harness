use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use rah_protocol::{ToolContent, ToolInput};
use rah_tools::{RepositoryUntrackedFileEditTool, Tool, ToolContext};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

struct Fixture {
    root: PathBuf,
    git: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "rah-untracked-edit-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(root.join("docs")).unwrap();
        let git = native_git();
        run(&git, &root, &["init", "--quiet"]);
        run(&git, &root, &["config", "user.name", "RAH Test"]);
        run(
            &git,
            &root,
            &["config", "user.email", "rah@example.invalid"],
        );
        fs::write(root.join("sentinel.txt"), "unchanged\n").unwrap();
        run(&git, &root, &["add", "sentinel.txt"]);
        run(&git, &root, &["commit", "--quiet", "-m", "base"]);
        Self { root, git }
    }

    fn tool(&self) -> RepositoryUntrackedFileEditTool {
        RepositoryUntrackedFileEditTool::new(&self.git, &self.root).unwrap()
    }

    async fn edit(&self, path: &str, expected: &str) -> Value {
        let hash = format!("{:x}", Sha256::digest(expected.as_bytes()));
        let output = self
            .tool()
            .execute(
                ToolInput(json!({
                    "path": path, "expected_file_sha256": hash,
                    "expected_file_byte_length": expected.len(),
                    "expected_old_text": "worker_count=44", "replacement_text": "worker_count=4"
                })),
                ToolContext::default(),
            )
            .await
            .unwrap();
        let [ToolContent::Json(value)] = output.content.as_slice() else {
            panic!("JSON result required")
        };
        value.clone()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn native_git() -> PathBuf {
    #[cfg(windows)]
    let (command, argument) = ("where.exe", "git.exe");
    #[cfg(not(windows))]
    let (command, argument) = ("which", "git");
    let output = Command::new(command).arg(argument).output().unwrap();
    fs::canonicalize(
        String::from_utf8(output.stdout)
            .unwrap()
            .lines()
            .next()
            .unwrap(),
    )
    .unwrap()
}

fn run(git: &Path, root: &Path, args: &[&str]) {
    assert!(
        Command::new(git)
            .args(args)
            .current_dir(root)
            .status()
            .unwrap()
            .success(),
        "{args:?}"
    );
}

fn output(git: &Path, root: &Path, args: &[&str]) -> Vec<u8> {
    let result = Command::new(git)
        .args(args)
        .current_dir(root)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    result.stdout
}

#[tokio::test]
async fn corrects_existing_untracked_file_without_staging_or_unrelated_effect() {
    let fixture = Fixture::new();
    let path = fixture.root.join("docs/local-setup.txt");
    fs::write(&path, "worker_count=44\n").unwrap();
    let head = output(&fixture.git, &fixture.root, &["rev-parse", "HEAD"]);
    let index = fs::read(fixture.root.join(".git/index")).unwrap();
    let result = fixture
        .edit("docs/local-setup.txt", "worker_count=44\n")
        .await;
    assert_eq!(
        result,
        json!({"status":"ok","changed":true,"uncertain":false,"reason":"none","path":"docs/local-setup.txt"})
    );
    assert_eq!(fs::read_to_string(&path).unwrap(), "worker_count=4\n");
    assert_eq!(
        output(
            &fixture.git,
            &fixture.root,
            &["ls-files", "--others", "--exclude-standard", "-z"]
        ),
        b"docs/local-setup.txt\0"
    );
    assert_eq!(
        output(
            &fixture.git,
            &fixture.root,
            &["diff", "--cached", "--name-only"]
        ),
        b""
    );
    assert_eq!(
        output(&fixture.git, &fixture.root, &["rev-parse", "HEAD"]),
        head
    );
    assert_eq!(fs::read(fixture.root.join(".git/index")).unwrap(), index);
    assert_eq!(
        fs::read_to_string(fixture.root.join("sentinel.txt")).unwrap(),
        "unchanged\n"
    );
    assert_eq!(fs::read_dir(fixture.root.join("docs")).unwrap().count(), 1);
}

#[tokio::test]
async fn rejects_tracked_ignored_missing_directory_and_stale_targets() {
    let fixture = Fixture::new();
    fs::write(
        fixture.root.join("docs/local-setup.txt"),
        "worker_count=44\n",
    )
    .unwrap();
    let stale = fixture
        .edit("docs/local-setup.txt", "worker_count=44")
        .await;
    assert_eq!(stale["status"], "precondition_failed");
    assert_eq!(stale["reason"], "precondition");
    assert_eq!(
        fs::read_to_string(fixture.root.join("docs/local-setup.txt")).unwrap(),
        "worker_count=44\n"
    );
    run(
        &fixture.git,
        &fixture.root,
        &["add", "docs/local-setup.txt"],
    );
    let staged = fixture
        .edit("docs/local-setup.txt", "worker_count=44\n")
        .await;
    assert_eq!(staged["reason"], "repository_state");
    run(
        &fixture.git,
        &fixture.root,
        &["commit", "--quiet", "-m", "add"],
    );
    let tracked = fixture
        .edit("docs/local-setup.txt", "worker_count=44\n")
        .await;
    assert_eq!(tracked["reason"], "repository_state");
    fs::write(fixture.root.join(".gitignore"), "ignored.txt\n").unwrap();
    fs::write(fixture.root.join("ignored.txt"), "worker_count=44\n").unwrap();
    assert_eq!(
        fixture.edit("ignored.txt", "worker_count=44\n").await["reason"],
        "ignored_target"
    );
    assert_eq!(
        fixture.edit("missing.txt", "worker_count=44\n").await["reason"],
        "missing_target"
    );
    assert_eq!(
        fixture.edit("docs", "worker_count=44\n").await["reason"],
        "non_regular_target"
    );
    let traversal = fixture.tool().execute(ToolInput(json!({
        "path":"../outside.txt", "expected_file_sha256":format!("{:x}", Sha256::digest(b"worker_count=44\n")),
        "expected_file_byte_length":16, "expected_old_text":"worker_count=44", "replacement_text":"worker_count=4"
    })), ToolContext::default()).await;
    assert!(traversal.is_err());
    assert_eq!(
        fs::read_to_string(fixture.root.join("ignored.txt")).unwrap(),
        "worker_count=44\n"
    );
}

#[tokio::test]
async fn rejects_intent_to_add_index_deleted_and_nested_repository() {
    let fixture = Fixture::new();
    let path = fixture.root.join("docs/local-setup.txt");
    fs::write(&path, "worker_count=44\n").unwrap();
    run(
        &fixture.git,
        &fixture.root,
        &["add", "-N", "docs/local-setup.txt"],
    );
    assert_eq!(
        fixture
            .edit("docs/local-setup.txt", "worker_count=44\n")
            .await["reason"],
        "repository_state"
    );
    run(
        &fixture.git,
        &fixture.root,
        &["add", "docs/local-setup.txt"],
    );
    run(
        &fixture.git,
        &fixture.root,
        &["commit", "--quiet", "-m", "tracked"],
    );
    run(
        &fixture.git,
        &fixture.root,
        &["rm", "--cached", "docs/local-setup.txt"],
    );
    assert_eq!(
        fixture
            .edit("docs/local-setup.txt", "worker_count=44\n")
            .await["reason"],
        "repository_state"
    );
    fs::create_dir_all(fixture.root.join("nested")).unwrap();
    run(
        &fixture.git,
        &fixture.root.join("nested"),
        &["init", "--quiet"],
    );
    fs::write(fixture.root.join("nested/new.txt"), "worker_count=44\n").unwrap();
    assert_eq!(
        fixture.edit("nested/new.txt", "worker_count=44\n").await["status"],
        "precondition_failed"
    );
    assert_eq!(
        fs::read_to_string(fixture.root.join("nested/new.txt")).unwrap(),
        "worker_count=44\n"
    );
}

#[tokio::test]
async fn rejects_binary_invalid_utf8_and_ambiguous_match() {
    let fixture = Fixture::new();
    let path = fixture.root.join("docs/local-setup.txt");
    for bytes in [
        b"worker_count=44\0\n".as_slice(),
        b"worker_count=44\xff\n".as_slice(),
    ] {
        fs::write(&path, bytes).unwrap();
        let hash = format!("{:x}", Sha256::digest(bytes));
        let output = fixture.tool().execute(ToolInput(json!({
            "path":"docs/local-setup.txt", "expected_file_sha256":hash,
            "expected_file_byte_length":bytes.len(), "expected_old_text":"worker_count=44", "replacement_text":"worker_count=4"
        })), ToolContext::default()).await.unwrap();
        let [ToolContent::Json(value)] = output.content.as_slice() else {
            panic!("JSON required")
        };
        assert_eq!(value["reason"], "unsupported_text");
        assert_eq!(fs::read(&path).unwrap(), bytes);
    }
    fs::write(&path, "worker_count=44\nworker_count=44\n").unwrap();
    assert_eq!(
        fixture
            .edit("docs/local-setup.txt", "worker_count=44\nworker_count=44\n")
            .await["status"],
        "precondition_failed"
    );
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        "worker_count=44\nworker_count=44\n"
    );
}

#[tokio::test]
async fn linked_worktree_can_edit_only_its_active_untracked_member() {
    let fixture = Fixture::new();
    let linked = fixture
        .root
        .with_extension(format!("linked-{}", uuid::Uuid::new_v4()));
    run(
        &fixture.git,
        &fixture.root,
        &[
            "worktree",
            "add",
            "--quiet",
            "--detach",
            linked.to_str().unwrap(),
        ],
    );
    let linked_file = linked.join("docs/local-setup.txt");
    fs::create_dir_all(linked.join("docs")).unwrap();
    fs::write(&linked_file, "worker_count=44\n").unwrap();
    let active = RepositoryUntrackedFileEditTool::new(&fixture.git, &fixture.root).unwrap();
    let old = b"worker_count=44\n";
    let request = ToolInput(
        json!({"path":"docs/local-setup.txt", "expected_file_sha256":format!("{:x}", Sha256::digest(old)),
        "expected_file_byte_length":old.len(), "expected_old_text":"worker_count=44", "replacement_text":"worker_count=4"}),
    );
    let wrong = active
        .execute(request.clone(), ToolContext::default())
        .await
        .unwrap();
    let [ToolContent::Json(wrong)] = wrong.content.as_slice() else {
        panic!("JSON required")
    };
    assert_eq!(wrong["status"], "precondition_failed");
    let selected = RepositoryUntrackedFileEditTool::new(&fixture.git, &linked).unwrap();
    let result = selected
        .execute(request, ToolContext::default())
        .await
        .unwrap();
    let [ToolContent::Json(result)] = result.content.as_slice() else {
        panic!("JSON required")
    };
    assert_eq!(result["status"], "ok");
    assert_eq!(
        fs::read_to_string(&linked_file).unwrap(),
        "worker_count=4\n"
    );
    assert!(!fixture.root.join("docs/local-setup.txt").exists());
    fs::remove_file(&linked_file).unwrap();
    run(
        &fixture.git,
        &fixture.root,
        &["worktree", "remove", linked.to_str().unwrap()],
    );
}

#[tokio::test]
async fn rejects_symlink_escape_without_touching_external_file() {
    let fixture = Fixture::new();
    let external = fixture
        .root
        .with_extension(format!("external-{}", uuid::Uuid::new_v4()));
    fs::write(&external, "worker_count=44\n").unwrap();
    let link = fixture.root.join("docs/local-setup.txt");
    #[cfg(unix)]
    let linked = std::os::unix::fs::symlink(&external, &link);
    #[cfg(windows)]
    let linked = std::os::windows::fs::symlink_file(&external, &link);
    if linked.is_ok() {
        assert_eq!(
            fixture
                .edit("docs/local-setup.txt", "worker_count=44\n")
                .await["status"],
            "precondition_failed"
        );
        assert_eq!(fs::read_to_string(&external).unwrap(), "worker_count=44\n");
        fs::remove_file(&link).unwrap();
    }
    fs::remove_file(&external).unwrap();
}

#[tokio::test]
async fn equal_replacement_is_verified_noop() {
    let fixture = Fixture::new();
    let path = fixture.root.join("docs/local-setup.txt");
    fs::write(&path, "worker_count=44\n").unwrap();
    let old = b"worker_count=44\n";
    let output = fixture.tool().execute(ToolInput(json!({
        "path":"docs/local-setup.txt", "expected_file_sha256":format!("{:x}", Sha256::digest(old)),
        "expected_file_byte_length":old.len(), "expected_old_text":"worker_count=44", "replacement_text":"worker_count=44"
    })), ToolContext::default()).await.unwrap();
    let [ToolContent::Json(value)] = output.content.as_slice() else {
        panic!("JSON required")
    };
    assert_eq!(value["status"], "ok");
    assert_eq!(value["changed"], false);
    assert_eq!(fs::read(&path).unwrap(), old);
    assert_eq!(fs::read_dir(fixture.root.join("docs")).unwrap().count(), 1);
    let duplicate = fixture.tool().execute(ToolInput(json!({
        "path":"docs/local-setup.txt", "expected_file_sha256":format!("{:x}", Sha256::digest(old)),
        "expected_file_byte_length":old.len(), "replacements":[
            {"expected_old_text":"worker_count=44", "replacement_text":"worker_count=44"},
            {"expected_old_text":"worker_count=44", "replacement_text":"worker_count=44"}
        ]
    })), ToolContext::default()).await.unwrap();
    let [ToolContent::Json(duplicate)] = duplicate.content.as_slice() else {
        panic!("JSON required")
    };
    assert_eq!(duplicate["status"], "precondition_failed");
}

#[cfg(windows)]
#[tokio::test]
async fn rejects_case_alias_of_untracked_windows_path() {
    let fixture = Fixture::new();
    fs::write(
        fixture.root.join("docs/Local-Setup.txt"),
        "worker_count=44\n",
    )
    .unwrap();
    let result = fixture
        .edit("docs/local-setup.txt", "worker_count=44\n")
        .await;
    assert_eq!(result["status"], "precondition_failed");
    assert_eq!(
        fs::read_to_string(fixture.root.join("docs/Local-Setup.txt")).unwrap(),
        "worker_count=44\n"
    );
}
