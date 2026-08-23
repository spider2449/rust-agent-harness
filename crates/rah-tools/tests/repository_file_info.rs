use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
};

use rah_protocol::{ToolContent, ToolInput};
use rah_tools::{RepositoryFileInfoTool, Tool, ToolContext, ToolError};
use serde_json::{Value, json};

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new(label: &str) -> Self {
        let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "rah-repository-file-info-{label}-{}-{id}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir(&path).expect("test directory should be created");
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
    assert!(
        output.status.success(),
        "native Git must be available for fixture tests"
    );
    let path = String::from_utf8(output.stdout).unwrap();
    fs::canonicalize(path.lines().next().unwrap()).unwrap()
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
    // Preserve the CRLF fixture byte-for-byte. The observer intentionally
    // honors repository-local conversion semantics but not user/system config.
    run(&git, &root, &["config", "core.autocrlf", "false"]);
    fs::write(root.join("tracked.txt"), "initial\r\n").unwrap();
    fs::write(root.join("unicode-檔案.txt"), "unicode\n").unwrap();
    run(
        &git,
        &root,
        &["add", "--", "tracked.txt", "unicode-檔案.txt"],
    );
    run(&git, &root, &["commit", "--quiet", "-m", "initial"]);
    (base, git, root)
}

fn run(git: &Path, root: &Path, args: &[&str]) {
    let status = Command::new(git)
        .args(args)
        .current_dir(root)
        .status()
        .unwrap();
    assert!(status.success(), "Git fixture command failed: {args:?}");
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
    assert!(
        child.wait().unwrap().success(),
        "Git fixture command failed: {args:?}"
    );
}

async fn observe(git: &Path, root: &Path, path: &str) -> Value {
    let tool = RepositoryFileInfoTool::new(git, root).unwrap();
    let output = tool
        .execute(ToolInput(json!({"path": path})), ToolContext::default())
        .await
        .unwrap();
    assert!(!output.is_error);
    let [ToolContent::Json(value)] = output.content.as_slice() else {
        panic!("expected JSON")
    };
    value.clone()
}

fn state(root: &Path) -> (Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>) {
    let dot_git = root.join(".git");
    (
        fs::read(dot_git.join("HEAD")).unwrap(),
        fs::read(dot_git.join("index")).unwrap(),
        fs::read(root.join("tracked.txt")).unwrap(),
        fs::read_dir(root)
            .unwrap()
            .filter_map(Result::ok)
            .flat_map(|entry| entry.file_name().into_string().ok())
            .collect::<Vec<_>>()
            .join("|")
            .into_bytes(),
    )
}

#[tokio::test]
async fn normal_modified_staged_untracked_missing_unicode_and_read_only_states_are_normalized() {
    let (_base, git, root) = repository("normal");
    let before = state(&root);
    let clean = observe(&git, &root, "tracked.txt").await;
    assert_eq!(clean["status"], "ok");
    assert_eq!(clean["consistency"], "best_effort");
    assert_eq!(clean["index"]["tracked"], true);
    assert_eq!(clean["index"]["entries"][0]["stage"], 0);
    assert_eq!(clean["worktree_modified_vs_index"], false);
    assert_eq!(clean["content"]["byte_length"], 9);
    assert_eq!(
        clean["path"],
        json!({"encoding":"utf8","value":"tracked.txt"})
    );
    assert_eq!(
        state(&root),
        before,
        "observer must not intentionally alter fixture state"
    );

    fs::write(root.join("tracked.txt"), "worktree\r\n").unwrap();
    let modified = observe(&git, &root, "tracked.txt").await;
    assert_eq!(modified["staged_vs_head"], false);
    assert_eq!(modified["worktree_modified_vs_index"], true);

    run(&git, &root, &["add", "--", "tracked.txt"]);
    let staged = observe(&git, &root, "tracked.txt").await;
    assert_eq!(staged["staged_vs_head"], true);
    assert_eq!(staged["worktree_modified_vs_index"], false);

    fs::write(root.join("untracked.bin"), [0_u8, 1, 2, 3]).unwrap();
    let untracked = observe(&git, &root, "untracked.bin").await;
    assert_eq!(untracked["index"]["tracked"], false);
    assert_eq!(untracked["sparse_state"], "not_tracked");
    assert_eq!(untracked["worktree"]["present"], true);
    assert!(
        untracked.get("content").is_some(),
        "binary bytes remain structurally observable"
    );

    let missing = observe(&git, &root, "missing.txt").await;
    assert_eq!(missing["head"]["present"], false);
    assert_eq!(missing["worktree"]["present"], false);
    let unicode = observe(&git, &root, "unicode-檔案.txt").await;
    assert_eq!(unicode["path"]["value"], "unicode-檔案.txt");
}

#[tokio::test]
async fn unborn_index_only_conflict_and_sparse_states_are_closed() {
    let base = TestDirectory::new("states");
    let root = base.0.join("repository");
    fs::create_dir(&root).unwrap();
    let git = native_git();
    run(&git, &root, &["init", "--quiet"]);
    fs::write(root.join("new.txt"), "new\n").unwrap();
    run(&git, &root, &["add", "--", "new.txt"]);
    let unborn = observe(&git, &root, "new.txt").await;
    assert_eq!(unborn["head"]["present"], false);
    assert_eq!(unborn["staged_vs_head"], true);

    fs::write(root.join("index-only.txt"), "index\n").unwrap();
    run(&git, &root, &["add", "--", "index-only.txt"]);
    fs::remove_file(root.join("index-only.txt")).unwrap();
    let index_only = observe(&git, &root, "index-only.txt").await;
    assert_eq!(index_only["index"]["tracked"], true);
    assert_eq!(index_only["worktree"]["present"], false);

    run(&git, &root, &["update-index", "--skip-worktree", "new.txt"]);
    let skip_present = observe(&git, &root, "new.txt").await;
    assert_eq!(skip_present["sparse_state"], "skip_worktree_present");
    fs::remove_file(root.join("new.txt")).unwrap();
    let omitted = observe(&git, &root, "new.txt").await;
    assert_eq!(omitted["sparse_state"], "sparse_omitted");

    let one = hash_blob(&git, &root, b"one\n");
    let two = hash_blob(&git, &root, b"two\n");
    let three = hash_blob(&git, &root, b"three\n");
    let conflict = format!(
        "100644 {one} 1\tconflict.txt\n100644 {two} 2\tconflict.txt\n100644 {three} 3\tconflict.txt\n"
    );
    run_input(
        &git,
        &root,
        &["update-index", "--index-info"],
        conflict.as_bytes(),
    );
    let conflict = observe(&git, &root, "conflict.txt").await;
    assert_eq!(conflict["index"]["conflicted"], true);
    assert_eq!(conflict["index"]["entries"].as_array().unwrap().len(), 3);
    assert_eq!(conflict["sparse_state"], "conflicted");
    assert!(conflict["staged_vs_head"].is_null());
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
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

#[tokio::test]
async fn hostile_environment_and_local_configuration_do_not_run_helpers() {
    let (_base, git, root) = repository("hardening");
    let marker = root.join("helper-ran");
    let marker_command = if cfg!(windows) {
        format!("cmd /c type nul > {}", marker.display())
    } else {
        format!("sh -c 'touch {}'", marker.display())
    };
    run(&git, &root, &["config", "diff.external", &marker_command]);
    let result = observe(&git, &root, "tracked.txt").await;
    assert_eq!(result["status"], "ok");
    assert!(
        !marker.exists(),
        "fixed observer commands must not run inherited helpers"
    );
}

#[test]
fn schema_rejects_non_path_input() {
    let root = TestDirectory::new("input");
    let repository = root.0.join("repository");
    fs::create_dir(&repository).unwrap();
    let git = native_git();
    run(&git, &repository, &["init", "--quiet"]);
    let tool = RepositoryFileInfoTool::new(&git, &repository).unwrap();
    for input in [
        json!({}),
        json!({"path":"../x"}),
        json!({"path":".git/config"}),
        json!({"path":"x", "args":["status"]}),
        json!({"path": 1}),
    ] {
        let error =
            futures::executor::block_on(tool.execute(ToolInput(input), ToolContext::default()))
                .unwrap_err();
        assert!(matches!(error, ToolError::InvalidInput { .. }));
    }
}

#[cfg(unix)]
#[tokio::test]
async fn unix_symlink_executable_and_case_sensitive_paths_are_semantic() {
    use std::os::unix::fs::{PermissionsExt, symlink};
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
    let executable = observe(&git, &root, "run.sh").await;
    assert_eq!(executable["index"]["entries"][0]["mode"], "100755");
    assert_eq!(executable["index"]["entries"][0]["executable"], true);
    let link = observe(&git, &root, "link.txt").await;
    assert_eq!(link["worktree"]["kind"], "symlink");
    assert!(
        link.get("content").is_none(),
        "observer must not follow symlink targets"
    );
    assert_ne!(
        observe(&git, &root, "Case.txt").await["content"],
        observe(&git, &root, "case.txt").await["content"]
    );
}
