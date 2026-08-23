use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
};

use rah_protocol::{PermissionLevel, ToolContent, ToolInput, ToolName};
use rah_tools::{
    REPOSITORY_DIFF_TOOL_NAME, RepositoryDiffTool, Tool, ToolContext, ToolError, ToolRegistry,
};
use serde_json::{Value, json};

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

type RepositorySnapshot = (Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>);

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new(label: &str) -> Self {
        let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "rah-repository-diff-{label}-{}-{id}",
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

async fn observe(git: &Path, root: &Path) -> Value {
    let tool = RepositoryDiffTool::new(git, root).unwrap();
    let output = tool
        .execute(ToolInput(json!({})), ToolContext::default())
        .await
        .unwrap();
    assert!(!output.is_error);
    let [ToolContent::Json(value)] = output.content.as_slice() else {
        panic!("expected JSON")
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
async fn closed_schema_registers_and_rejects_model_selected_process_fields() {
    let (_base, git, root) = repository("schema");
    let tool = RepositoryDiffTool::new(&git, &root).unwrap();
    let definition = tool.definition();
    assert_eq!(definition.name, ToolName::new(REPOSITORY_DIFF_TOOL_NAME));
    assert_eq!(definition.permission, PermissionLevel::Execute);
    assert_eq!(
        definition.input_schema,
        json!({"type":"object","properties":{},"additionalProperties":false})
    );
    let mut registry = ToolRegistry::new();
    registry.register(std::sync::Arc::new(tool)).unwrap();
    assert_eq!(registry.definitions(), vec![definition]);
    let tool = RepositoryDiffTool::new(&git, &root).unwrap();
    for input in [
        json!({"cached":true}),
        json!({"pathspec":"*"}),
        json!({"context":999}),
        json!({"renames":true}),
        json!({"binary":true}),
        json!({"textconv":true}),
        json!({"cwd":"."}),
        json!({"env":{"GIT_EXTERNAL_DIFF":"bad"}}),
    ] {
        assert!(matches!(
            tool.execute(ToolInput(input), ToolContext::default()).await,
            Err(ToolError::InvalidInput { .. })
        ));
    }
}

#[tokio::test]
async fn worktree_to_index_excludes_staged_only_and_untracked_and_is_read_only() {
    let (_base, git, root) = repository("semantics");
    fs::write(root.join("tracked.txt"), "staged\r\n").unwrap();
    run(&git, &root, &["add", "--", "tracked.txt"]);
    assert!(
        files(&observe(&git, &root).await).is_empty(),
        "staged-only content is excluded"
    );
    fs::write(root.join("tracked.txt"), "staged then worktree\r\n").unwrap();
    fs::remove_file(root.join("delete.txt")).unwrap();
    fs::write(root.join("space name.txt"), "changed\n").unwrap();
    fs::write(root.join("unicode-檔案.txt"), "changed\n").unwrap();
    fs::write(root.join("untracked.bin"), [0, 9, 0, 8]).unwrap();
    let before = snapshot(&git, &root);
    let result = observe(&git, &root).await;
    assert_eq!(
        snapshot(&git, &root),
        before,
        "diff must make no intentional repository mutation"
    );
    assert_eq!(result["status"], "ok");
    assert_eq!(result["consistency"], "best_effort");
    assert_eq!(result["comparison"], "worktree_to_index");
    assert_eq!(result["base"], "index");
    let entries = files(&result);
    assert_eq!(file(&entries, "tracked.txt")["added_lines"], 1);
    assert_eq!(file(&entries, "tracked.txt")["deleted_lines"], 1);
    assert_eq!(file(&entries, "delete.txt")["change_kind"], "deleted");
    assert_eq!(
        file(&entries, "space name.txt")["new_path"]["encoding"],
        "utf8"
    );
    assert_eq!(
        file(&entries, "unicode-檔案.txt")["new_path"]["encoding"],
        "utf8"
    );
    assert!(
        !entries
            .iter()
            .any(|entry| entry.to_string().contains("untracked.bin"))
    );
    assert!(
        !serde_json::to_string(&result)
            .unwrap()
            .contains(root.to_string_lossy().as_ref())
    );
}

#[tokio::test]
async fn binary_and_hostile_external_helpers_never_surface_or_execute() {
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
    fs::write(root.join("tracked.txt"), "changed\n").unwrap();
    fs::write(root.join("binary.bin"), [0, 9, 0, 8, 0]).unwrap();
    let result = observe(&git, &root).await;
    assert!(
        !marker.exists(),
        "fixed diff flags and cleared environment must suppress helpers"
    );
    let entries = files(&result);
    let binary = file(&entries, "binary.bin");
    assert_eq!(binary["binary"], true);
    assert!(binary["patch"].is_null());
    assert!(
        !serde_json::to_string(binary).unwrap().contains("CQ"),
        "binary bytes must not be surfaced"
    );
    assert!(file(&entries, "tracked.txt")["patch"].is_object());
}

#[tokio::test]
async fn conflict_is_rejected_without_partial_patch() {
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
    let tool = RepositoryDiffTool::new(&git, &root).unwrap();
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
async fn unix_mode_symlink_case_and_invalid_utf8_paths_are_observed_without_dereference() {
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
    run(&git, &root, &["commit", "--quiet", "-m", "unix fixtures"]);
    fs::set_permissions(root.join("run.sh"), fs::Permissions::from_mode(0o644)).unwrap();
    fs::remove_file(root.join("link.txt")).unwrap();
    symlink("/outside-target", root.join("link.txt")).unwrap();
    fs::write(root.join("Case.txt"), "upper changed\n").unwrap();
    fs::write(root.join("case.txt"), "lower changed\n").unwrap();
    fs::write(root.join(&invalid), "changed\n").unwrap();
    fs::write(root.join("tab\tname\nnext"), "changed\n").unwrap();
    let result = observe(&git, &root).await;
    let entries = files(&result);
    assert_eq!(file(&entries, "run.sh")["old_mode"], "100755");
    assert_eq!(file(&entries, "run.sh")["new_mode"], "100644");
    assert_eq!(file(&entries, "link.txt")["change_kind"], "modified");
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
