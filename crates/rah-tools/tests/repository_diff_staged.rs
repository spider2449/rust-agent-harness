use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
};

use rah_protocol::{PermissionLevel, ToolContent, ToolInput, ToolName};
use rah_tools::{
    REPOSITORY_DIFF_STAGED_TOOL_NAME, RepositoryDiffStagedTool, Tool, ToolContext, ToolError,
    ToolRegistry,
};
use serde_json::{Value, json};

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

type RepositorySnapshot = (Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>);

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new(label: &str) -> Self {
        let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "rah-repository-diff-staged-{label}-{}-{id}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn native_git() -> PathBuf {
    #[cfg(windows)]
    let output = Command::new("where.exe").arg("git.exe").output().unwrap();
    #[cfg(not(windows))]
    let output = Command::new("which").arg("git").output().unwrap();
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

fn run(git: &Path, root: &Path, args: &[&str]) {
    assert!(
        Command::new(git)
            .args(args)
            .current_dir(root)
            .status()
            .unwrap()
            .success(),
        "Git fixture command failed: {args:?}"
    );
}

fn repository(label: &str) -> (TestDirectory, PathBuf, PathBuf) {
    let base = TestDirectory::new(label);
    let root = base.0.join("repository");
    fs::create_dir(&root).unwrap();
    let git = native_git();
    run(&git, &root, &["init", "--quiet"]);
    run(&git, &root, &["config", "user.name", "RAH Test"]);
    run(
        &git,
        &root,
        &["config", "user.email", "rah@example.invalid"],
    );
    run(&git, &root, &["config", "core.autocrlf", "false"]);
    run(&git, &root, &["config", "core.filemode", "true"]);
    for (name, contents) in [
        ("tracked.txt", b"initial\r\n".as_slice()),
        ("delete.txt", b"delete\n".as_slice()),
        ("binary.bin", &[0, 1, 0, 2][..]),
        ("space name.txt", b"space\n".as_slice()),
        ("unicode-檔案.txt", b"unicode\n".as_slice()),
    ] {
        fs::write(root.join(name), contents).unwrap();
    }
    run(&git, &root, &["add", "."]);
    run(&git, &root, &["commit", "--quiet", "-m", "initial"]);
    (base, git, root)
}

fn snapshot(root: &Path) -> RepositorySnapshot {
    (
        fs::read(root.join(".git/HEAD")).unwrap(),
        fs::read(root.join(".git/index")).unwrap(),
        fs::read(root.join("tracked.txt")).unwrap_or_default(),
        fs::read(root.join("untracked.bin")).unwrap_or_default(),
        Command::new(native_git())
            .args(["show-ref", "--head"])
            .current_dir(root)
            .output()
            .unwrap()
            .stdout,
    )
}

async fn observe(git: &Path, root: &Path) -> Value {
    let output = RepositoryDiffStagedTool::new(git, root)
        .unwrap()
        .execute(ToolInput(json!({})), ToolContext::default())
        .await
        .unwrap();
    let [ToolContent::Json(value)] = output.content.as_slice() else {
        panic!("expected JSON");
    };
    value.clone()
}

fn files(value: &Value) -> Vec<&Value> {
    value["files"].as_array().unwrap().iter().collect()
}

fn file<'a>(files: &'a [&Value], path: &str) -> &'a Value {
    files
        .iter()
        .copied()
        .find(|entry| entry["old_path"]["value"] == path || entry["new_path"]["value"] == path)
        .unwrap_or_else(|| panic!("missing diff entry for {path}"))
}

#[tokio::test]
async fn closed_schema_registers_and_rejects_model_selected_baselines() {
    let (_base, git, root) = repository("schema");
    let tool = RepositoryDiffStagedTool::new(&git, &root).unwrap();
    let definition = tool.definition();
    assert_eq!(
        definition.name,
        ToolName::new(REPOSITORY_DIFF_STAGED_TOOL_NAME)
    );
    assert_eq!(definition.permission, PermissionLevel::Execute);
    assert_eq!(
        definition.input_schema,
        json!({"type":"object","properties":{},"additionalProperties":false})
    );
    let mut registry = ToolRegistry::new();
    registry.register(std::sync::Arc::new(tool)).unwrap();
    assert_eq!(registry.definitions(), vec![definition]);
    let tool = RepositoryDiffStagedTool::new(&git, &root).unwrap();
    for input in [
        json!({"cached":false}),
        json!({"base":"HEAD~1"}),
        json!({"ref":"other"}),
        json!({"args":["--cached"]}),
        json!({"textconv":true}),
        json!({"renames":true}),
    ] {
        assert!(matches!(
            tool.execute(ToolInput(input), ToolContext::default()).await,
            Err(ToolError::InvalidInput { .. })
        ));
    }
}

#[tokio::test]
async fn index_vs_head_includes_only_staged_changes_and_is_read_only() {
    let (_base, git, root) = repository("semantics");
    fs::write(root.join("tracked.txt"), "staged\r\n").unwrap();
    fs::write(root.join("added.txt"), "added\n").unwrap();
    fs::remove_file(root.join("delete.txt")).unwrap();
    fs::write(root.join("binary.bin"), [0, 8, 0, 9]).unwrap();
    run(&git, &root, &["add", "-A"]);
    fs::write(root.join("tracked.txt"), "staged then unstaged\r\n").unwrap();
    fs::write(root.join("untracked.bin"), [0, 7, 0, 6]).unwrap();
    let before = snapshot(&root);
    let result = observe(&git, &root).await;
    assert_eq!(
        snapshot(&root),
        before,
        "no intentional repository mutation"
    );
    assert_eq!(result["status"], "ok");
    assert_eq!(result["consistency"], "best_effort");
    assert_eq!(result["comparison"], "index_to_head");
    assert_eq!(result["base"], "head");
    let entries = files(&result);
    assert_eq!(file(&entries, "tracked.txt")["added_lines"], 1);
    assert_eq!(file(&entries, "tracked.txt")["deleted_lines"], 1);
    assert_eq!(file(&entries, "added.txt")["change_kind"], "added");
    assert_eq!(file(&entries, "delete.txt")["change_kind"], "deleted");
    assert_eq!(file(&entries, "binary.bin")["binary"], true);
    assert!(file(&entries, "binary.bin")["patch"].is_null());
    assert!(
        !entries
            .iter()
            .any(|entry| entry.to_string().contains("untracked.bin"))
    );
    assert!(
        !serde_json::to_string(&result)
            .unwrap()
            .contains("staged then unstaged")
    );
    assert!(
        !serde_json::to_string(&result)
            .unwrap()
            .contains(root.to_string_lossy().as_ref())
    );
}

#[tokio::test]
async fn unborn_head_uses_git_selected_empty_tree_and_empty_index_is_empty() {
    let base = TestDirectory::new("unborn");
    let root = base.0.join("repository");
    fs::create_dir(&root).unwrap();
    let git = native_git();
    run(&git, &root, &["init", "--quiet"]);
    fs::write(root.join("first.txt"), "first\n").unwrap();
    run(&git, &root, &["add", "--", "first.txt"]);
    let before = snapshot(&root);
    let staged = observe(&git, &root).await;
    assert_eq!(snapshot(&root), before);
    assert_eq!(staged["comparison"], "index_to_head");
    assert_eq!(staged["base"], "empty_tree");
    assert_eq!(file(&files(&staged), "first.txt")["change_kind"], "added");
    run(&git, &root, &["rm", "--cached", "--quiet", "first.txt"]);
    assert!(files(&observe(&git, &root).await).is_empty());
}

#[tokio::test]
async fn unborn_sha256_repository_uses_git_selected_empty_tree() {
    let base = TestDirectory::new("unborn-sha256");
    let root = base.0.join("repository");
    fs::create_dir(&root).unwrap();
    let git = native_git();
    run(&git, &root, &["init", "--quiet", "--object-format=sha256"]);
    fs::write(root.join("first.txt"), "first\n").unwrap();
    run(&git, &root, &["add", "--", "first.txt"]);
    let result = observe(&git, &root).await;
    assert_eq!(result["base"], "empty_tree");
    assert_eq!(file(&files(&result), "first.txt")["change_kind"], "added");
}

#[tokio::test]
async fn intent_to_add_follows_native_cached_semantics_without_reading_worktree_content() {
    let (_base, git, root) = repository("intent-to-add");
    fs::write(root.join("intent.txt"), "not staged content\n").unwrap();
    run(&git, &root, &["add", "-N", "--", "intent.txt"]);
    let native = Command::new(&git)
        .args([
            "diff",
            "--cached",
            "--raw",
            "-z",
            "--no-renames",
            "--no-ext-diff",
            "--no-textconv",
            "--ignore-submodules=all",
        ])
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(native.status.success());
    let result = observe(&git, &root).await;
    assert_eq!(files(&result).is_empty(), native.stdout.is_empty());
    assert!(
        !serde_json::to_string(&result)
            .unwrap()
            .contains("not staged content")
    );
}

#[tokio::test]
async fn hostile_helpers_conflicts_bounds_and_lease_errors_fail_closed() {
    let (_base, git, root) = repository("hardening");
    let marker = root.join("helper-ran");
    let command = if cfg!(windows) {
        format!("cmd /c type nul > \"{}\"", marker.display())
    } else {
        format!("sh -c 'touch \"{}\"'", marker.display())
    };
    run(&git, &root, &["config", "diff.external", &command]);
    run(&git, &root, &["config", "diff.evil.textconv", &command]);
    fs::write(root.join(".gitattributes"), "*.txt diff=evil\n").unwrap();
    fs::write(root.join("tracked.txt"), "staged\n").unwrap();
    run(&git, &root, &["add", ".gitattributes", "tracked.txt"]);
    assert!(!marker.exists());
    let _ = observe(&git, &root).await;
    assert!(!marker.exists(), "--cached must retain helper suppression");

    let one = hash_blob(&git, &root, b"one\n");
    let two = hash_blob(&git, &root, b"two\n");
    run_input(
        &git,
        &root,
        &["update-index", "--index-info"],
        format!("100644 {one} 1\tconflict.txt\n100644 {two} 2\tconflict.txt\n").as_bytes(),
    );
    let tool = RepositoryDiffStagedTool::new(&git, &root).unwrap();
    assert!(matches!(
        tool.execute(ToolInput(json!({})), ToolContext::default())
            .await,
        Err(ToolError::Execution { .. })
    ));
    run(&git, &root, &["reset", "--quiet"]);
    assert!(
        observe(&git, &root).await["files"].is_array(),
        "error released the lease"
    );
}

#[tokio::test]
async fn staged_file_count_bounds_fail_closed() {
    let (_base, git, root) = repository("bounds");
    for index in 0..257 {
        fs::write(root.join(format!("bounded-{index}.txt")), "x\n").unwrap();
    }
    run(&git, &root, &["add", "."]);
    let tool = RepositoryDiffStagedTool::new(&git, &root).unwrap();
    assert!(matches!(
        tool.execute(ToolInput(json!({})), ToolContext::default())
            .await,
        Err(ToolError::Execution { .. })
    ));
}

fn run_input(git: &Path, root: &Path, args: &[&str], input: &[u8]) {
    let mut child = Command::new(git)
        .args(args)
        .current_dir(root)
        .stdin(Stdio::piped())
        .spawn()
        .unwrap();
    use std::io::Write;
    child.stdin.take().unwrap().write_all(input).unwrap();
    assert!(child.wait().unwrap().success());
}

fn hash_blob(git: &Path, root: &Path, bytes: &[u8]) -> String {
    let mut child = Command::new(git)
        .args(["hash-object", "-w", "--stdin"])
        .current_dir(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    use std::io::Write;
    child.stdin.take().unwrap().write_all(bytes).unwrap();
    String::from_utf8(child.wait_with_output().unwrap().stdout)
        .unwrap()
        .trim()
        .to_owned()
}

#[cfg(unix)]
#[tokio::test]
async fn unix_mode_symlink_and_hostile_paths_are_staged_without_dereference() {
    use std::os::unix::{
        ffi::OsStringExt,
        fs::{PermissionsExt, symlink},
    };
    let (_base, git, root) = repository("unix");
    fs::write(root.join("run.sh"), "#!/bin/sh\n").unwrap();
    fs::set_permissions(root.join("run.sh"), fs::Permissions::from_mode(0o755)).unwrap();
    symlink("tracked.txt", root.join("link.txt")).unwrap();
    fs::write(root.join("Case.txt"), "upper\n").unwrap();
    fs::write(root.join("case.txt"), "lower\n").unwrap();
    let invalid = PathBuf::from(std::ffi::OsString::from_vec(b"invalid-\xff".to_vec()));
    fs::write(root.join(&invalid), "initial\n").unwrap();
    fs::write(root.join("tab\tname\nnext"), "initial\n").unwrap();
    run(&git, &root, &["add", "."]);
    let result = observe(&git, &root).await;
    let entries = files(&result);
    assert_eq!(file(&entries, "run.sh")["new_mode"], "100755");
    assert_eq!(file(&entries, "link.txt")["new_mode"], "120000");
    assert!(
        entries
            .iter()
            .any(|entry| entry["new_path"]["value"] == "Case.txt")
    );
    assert!(
        entries
            .iter()
            .any(|entry| entry["new_path"]["value"] == "case.txt")
    );
    assert!(
        entries
            .iter()
            .any(|entry| entry["new_path"] == json!({"encoding":"base64","value":"aW52YWxpZC3/"}))
    );
    assert!(
        entries
            .iter()
            .any(|entry| entry["new_path"]["value"] == "tab\tname\nnext")
    );
}
