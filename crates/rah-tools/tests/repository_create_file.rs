use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};

use rah_protocol::{ToolContent, ToolInput};
use rah_tools::{RepositoryFileCreationTool, Tool, ToolContext};
use serde_json::{Value, json};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    root: PathBuf,
    git: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "rah-create-file-{}-{}",
            std::process::id(),
            NEXT_ID.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("src/nested")).unwrap();
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
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
fn native_git() -> PathBuf {
    #[cfg(windows)]
    let command = "where.exe";
    #[cfg(not(windows))]
    let command = "which";
    #[cfg(windows)]
    let argument = "git.exe";
    #[cfg(not(windows))]
    let argument = "git";
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
fn result(tool: &RepositoryFileCreationTool, input: Value) -> Value {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let output = runtime
        .block_on(tool.execute(ToolInput(input), ToolContext::default()))
        .unwrap();
    let [ToolContent::Json(value)] = output.content.as_slice() else {
        panic!("JSON result required")
    };
    value.clone()
}
#[test]
fn creates_exact_untracked_utf8_file_without_git_mutation() {
    let fixture = Fixture::new();
    let tool = RepositoryFileCreationTool::new(&fixture.git, &fixture.root).unwrap();
    let head = fs::read(fixture.root.join(".git/HEAD")).unwrap();
    let index = fs::read(fixture.root.join(".git/index")).unwrap();
    let value = result(
        &tool,
        json!({"path":"src/nested/new.rs","content":"\u{03bb}\n"}),
    );
    assert_eq!(value["status"], "ok");
    assert_eq!(value["length"], 3);
    assert_eq!(
        fs::read(fixture.root.join("src/nested/new.rs")).unwrap(),
        "λ\n".as_bytes()
    );
    assert_eq!(fs::read(fixture.root.join(".git/HEAD")).unwrap(), head);
    assert_eq!(fs::read(fixture.root.join(".git/index")).unwrap(), index);
    assert_eq!(
        fs::read(fixture.root.join("sentinel.txt")).unwrap(),
        b"unchanged\n"
    );
    let output = Command::new(&fixture.git)
        .args(["status", "--porcelain=v1", "--untracked-files=all"])
        .current_dir(&fixture.root)
        .output()
        .unwrap();
    assert_eq!(output.stdout, b"?? src/nested/new.rs\n");
}
#[test]
fn rejects_tracked_indexed_ignored_and_invalid_targets() {
    let fixture = Fixture::new();
    fs::write(fixture.root.join("tracked.rs"), "old").unwrap();
    run(&fixture.git, &fixture.root, &["add", "tracked.rs"]);
    run(
        &fixture.git,
        &fixture.root,
        &["commit", "--quiet", "-m", "tracked"],
    );
    fs::remove_file(fixture.root.join("tracked.rs")).unwrap();
    fs::write(fixture.root.join(".gitignore"), "ignored.rs\n").unwrap();
    let tool = RepositoryFileCreationTool::new(&fixture.git, &fixture.root).unwrap();
    for path in [
        "tracked.rs",
        "ignored.rs",
        "/absolute",
        "../escape",
        "C:drive",
        "//server/share",
        ".git/config",
        "CON",
        "src/",
    ] {
        assert_ne!(
            result(&tool, json!({"path":path,"content":"x"}))["status"],
            "ok",
            "{path}"
        );
    }
    fs::write(fixture.root.join("staged.rs"), "x").unwrap();
    run(&fixture.git, &fixture.root, &["add", "staged.rs"]);
    assert_eq!(
        result(&tool, json!({"path":"staged.rs","content":"x"}))["status"],
        "precondition_failed"
    );
}
#[test]
fn enforces_content_and_serialized_request_bounds() {
    let fixture = Fixture::new();
    let tool = RepositoryFileCreationTool::new(&fixture.git, &fixture.root).unwrap();
    assert_eq!(
        result(&tool, json!({"path":"src/empty","content":""}))["status"],
        "ok"
    );
    assert_eq!(
        result(&tool, json!({"path":"src/one","content":"x"}))["status"],
        "ok"
    );
    assert_eq!(
        result(
            &tool,
            json!({"path":"src/max","content":"x".repeat(256 * 1024)})
        )["status"],
        "ok"
    );
    assert_eq!(
        result(
            &tool,
            json!({"path":"src/over","content":"x".repeat(256 * 1024 + 1)})
        )["status"],
        "invalid_target"
    );
    assert_eq!(
        result(&tool, json!({"path":"src/nul","content":"a\u{0000}"}))["status"],
        "invalid_target"
    );
}
