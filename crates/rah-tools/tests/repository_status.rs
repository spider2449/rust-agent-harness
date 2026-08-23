use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
};

use rah_protocol::{PermissionLevel, ToolContent, ToolInput, ToolName};
use rah_tools::{
    REPOSITORY_STATUS_TOOL_NAME, RepositoryStatusTool, Tool, ToolContext, ToolError, ToolRegistry,
};
use serde_json::{Value, json};

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

type RepositorySnapshot = (Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>);

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new(label: &str) -> Self {
        let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "rah-repository-status-{label}-{}-{id}",
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
    let path = String::from_utf8(output.stdout).unwrap();
    fs::canonicalize(path.lines().next().unwrap()).unwrap()
}

fn run(git: &Path, root: &Path, args: &[&str]) {
    let status = Command::new(git)
        .args(args)
        .current_dir(root)
        .status()
        .unwrap();
    assert!(status.success(), "Git fixture command failed: {args:?}");
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
    for (name, contents) in [
        ("tracked.txt", b"initial\r\n".as_slice()),
        ("delete-staged.txt", b"delete staged\n".as_slice()),
        ("delete-unstaged.txt", b"delete unstaged\n".as_slice()),
        ("binary.bin", &[0, 1, 0, 2][..]),
        ("space name.txt", b"space\n".as_slice()),
        ("unicode-檔案.txt", b"unicode\n".as_slice()),
    ] {
        fs::write(root.join(name), contents).unwrap();
    }
    fs::write(root.join(".gitignore"), "ignored.txt\n").unwrap();
    run(&git, &root, &["add", "."]);
    run(&git, &root, &["commit", "--quiet", "-m", "initial"]);
    (base, git, root)
}

async fn observe(git: &Path, root: &Path) -> Value {
    let tool = RepositoryStatusTool::new(git, root).unwrap();
    let output = tool
        .execute(ToolInput(json!({})), ToolContext::default())
        .await
        .unwrap();
    assert!(!output.is_error);
    let [ToolContent::Json(value)] = output.content.as_slice() else {
        panic!("expected JSON output")
    };
    value.clone()
}

fn entries(value: &Value) -> BTreeMap<String, &Value> {
    value["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| (entry["path"]["value"].as_str().unwrap().to_owned(), entry))
        .collect()
}

fn snapshot(git: &Path, root: &Path) -> RepositorySnapshot {
    let refs = Command::new(git)
        .args(["show-ref", "--head"])
        .current_dir(root)
        .output()
        .unwrap()
        .stdout;
    (
        fs::read(root.join(".git/HEAD")).unwrap(),
        fs::read(root.join(".git/index")).unwrap(),
        refs,
        fs::read(root.join("tracked.txt")).unwrap(),
        fs::read(root.join("untracked.bin")).unwrap(),
    )
}

#[tokio::test]
async fn closed_schema_registers_and_rejects_model_selected_process_fields() {
    let (_base, git, root) = repository("schema");
    let tool = RepositoryStatusTool::new(&git, &root).unwrap();
    let definition = tool.definition();
    assert_eq!(definition.name, ToolName::new(REPOSITORY_STATUS_TOOL_NAME));
    assert_eq!(definition.permission, PermissionLevel::Execute);
    assert_eq!(
        definition.input_schema,
        json!({"type":"object","properties":{},"additionalProperties":false})
    );
    let mut registry = ToolRegistry::new();
    registry.register(std::sync::Arc::new(tool)).unwrap();
    assert_eq!(registry.definitions(), vec![definition]);
    let tool = RepositoryStatusTool::new(&git, &root).unwrap();
    for input in [
        json!({"args":["status"]}),
        json!({"pathspec":"*"}),
        json!({"ignored":true}),
        json!({"renames":true}),
        json!({"branch":true}),
        json!({"cwd":"."}),
        json!({"env":{"GIT_DIR":"elsewhere"}}),
    ] {
        assert!(matches!(
            tool.execute(ToolInput(input), ToolContext::default()).await,
            Err(ToolError::InvalidInput { .. })
        ));
    }
}

#[tokio::test]
async fn mixed_states_untracked_normal_and_read_only_invariant_are_normalized() {
    let (_base, git, root) = repository("states");
    fs::write(root.join("tracked.txt"), "staged\r\n").unwrap();
    run(&git, &root, &["add", "--", "tracked.txt"]);
    fs::write(root.join("tracked.txt"), "staged and worktree\r\n").unwrap();
    fs::remove_file(root.join("delete-staged.txt")).unwrap();
    run(&git, &root, &["add", "--", "delete-staged.txt"]);
    fs::remove_file(root.join("delete-unstaged.txt")).unwrap();
    fs::write(root.join("binary.bin"), [0, 9, 0, 8]).unwrap();
    fs::write(root.join("space name.txt"), "space modified\n").unwrap();
    fs::write(root.join("unicode-檔案.txt"), "unicode modified\n").unwrap();
    fs::write(root.join("untracked.bin"), [0, 1, 0, 3]).unwrap();
    fs::create_dir(root.join("untracked-dir")).unwrap();
    fs::write(root.join("untracked-dir/child.txt"), "child\n").unwrap();
    fs::write(root.join("ignored.txt"), "ignored\n").unwrap();
    let before = snapshot(&git, &root);

    let result = observe(&git, &root).await;
    let after = snapshot(&git, &root);
    assert_eq!(
        after, before,
        "status must make no intentional repository mutation"
    );
    assert_eq!(result["status"], "ok");
    assert_eq!(result["consistency"], "best_effort");
    assert_eq!(result["sparse_index_flags"], "not_enumerated");
    let entries = entries(&result);
    assert_eq!(entries["tracked.txt"]["index_state"], "modified");
    assert_eq!(entries["tracked.txt"]["worktree_state"], "modified");
    assert_eq!(entries["delete-staged.txt"]["index_state"], "deleted");
    assert_eq!(entries["delete-staged.txt"]["worktree_state"], "unmodified");
    assert_eq!(entries["delete-unstaged.txt"]["index_state"], "unmodified");
    assert_eq!(entries["delete-unstaged.txt"]["worktree_state"], "deleted");
    assert_eq!(entries["binary.bin"]["index_state"], "unmodified");
    assert_eq!(entries["binary.bin"]["worktree_state"], "modified");
    assert_eq!(entries["space name.txt"]["path"]["encoding"], "utf8");
    assert_eq!(entries["unicode-檔案.txt"]["path"]["encoding"], "utf8");
    assert_eq!(entries["untracked.bin"]["tracked"], false);
    assert_eq!(entries["untracked.bin"]["index_state"], "untracked");
    assert!(entries.contains_key("untracked-dir/"));
    assert!(!entries.contains_key("untracked-dir/child.txt"));
    assert!(!entries.contains_key("ignored.txt"));
    assert!(
        !serde_json::to_string(&result)
            .unwrap()
            .contains(root.to_string_lossy().as_ref()),
        "model-visible result must not disclose host paths"
    );
}

#[tokio::test]
async fn conflict_is_distinct_from_ordinary_modified_state() {
    let (_base, git, root) = repository("conflict");
    let one = hash_blob(&git, &root, b"one\n");
    let two = hash_blob(&git, &root, b"two\n");
    let three = hash_blob(&git, &root, b"three\n");
    let index_info = format!(
        "100644 {one} 1\tconflict.txt\n100644 {two} 2\tconflict.txt\n100644 {three} 3\tconflict.txt\n"
    );
    run_input(
        &git,
        &root,
        &["update-index", "--index-info"],
        index_info.as_bytes(),
    );
    fs::write(root.join("conflict.txt"), "worktree\n").unwrap();
    let result = observe(&git, &root).await;
    let entry = entries(&result)["conflict.txt"];
    assert_eq!(entry["index_state"], "unmerged");
    assert_eq!(entry["worktree_state"], "unmerged");
    assert_eq!(entry["conflict_state"], "both_modified");
    assert_eq!(entry["stages"].as_array().unwrap().len(), 3);
}

#[tokio::test]
async fn hostile_fsmonitor_and_diff_configuration_do_not_run_helpers() {
    let (_base, git, root) = repository("hardening");
    let marker = root.join("helper-ran");
    let command = if cfg!(windows) {
        format!("cmd /c type nul > \"{}\"", marker.display())
    } else {
        format!("sh -c 'touch \"{}\"'", marker.display())
    };
    run(&git, &root, &["config", "core.fsmonitor", &command]);
    run(&git, &root, &["config", "diff.external", &command]);
    let result = observe(&git, &root).await;
    assert_eq!(result["status"], "ok");
    assert!(
        !marker.exists(),
        "fixed status hardening must suppress helpers"
    );
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
async fn unix_paths_modes_symlinks_and_invalid_utf8_are_byte_safe() {
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
    run(
        &git,
        &root,
        &["add", "--", "run.sh", "link.txt", "Case.txt", "case.txt"],
    );
    run(&git, &root, &["commit", "--quiet", "-m", "unix paths"]);
    fs::set_permissions(root.join("run.sh"), fs::Permissions::from_mode(0o644)).unwrap();
    fs::remove_file(root.join("link.txt")).unwrap();
    symlink("binary.bin", root.join("link.txt")).unwrap();
    let invalid = PathBuf::from(std::ffi::OsString::from_vec(b"invalid-\xff".to_vec()));
    fs::write(root.join(&invalid), "invalid\n").unwrap();
    fs::write(root.join("tab\tname\nnext"), "special\n").unwrap();
    let result = observe(&git, &root).await;
    let entries = result["entries"].as_array().unwrap();
    assert!(
        entries
            .iter()
            .any(|entry| entry["path"] == json!({"encoding":"base64","value":"aW52YWxpZC3/"}))
    );
    assert!(
        entries
            .iter()
            .any(|entry| entry["path"]["value"] == "tab\tname\nnext")
    );
    assert!(
        entries
            .iter()
            .any(|entry| entry["path"]["value"] == "Case.txt")
    );
    assert!(
        entries
            .iter()
            .any(|entry| entry["path"]["value"] == "case.txt")
    );
    assert!(
        entries
            .iter()
            .any(|entry| entry["path"]["value"] == "run.sh")
    );
    assert!(
        entries
            .iter()
            .any(|entry| entry["path"]["value"] == "link.txt")
    );
    let run = entries
        .iter()
        .find(|entry| entry["path"]["value"] == "run.sh")
        .unwrap();
    assert_eq!(run["head_mode"], "100755");
    assert_eq!(run["worktree_mode"], "100644");
    assert_eq!(run["worktree_state"], "type_changed");
}
