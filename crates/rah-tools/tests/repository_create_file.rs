use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};

use rah_protocol::{ToolContent, ToolInput};
use rah_tools::{
    RepositoryCreateFilePreparationError, RepositoryCreateFilePreparationRequest,
    RepositoryCreateFilePreparer, RepositoryFileCreationTool, Tool, ToolContext,
};
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
fn run_input(git: &Path, root: &Path, args: &[&str], input: &[u8]) {
    use std::io::Write as _;
    let mut child = Command::new(git)
        .args(args)
        .current_dir(root)
        .stdin(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(input).unwrap();
    assert!(child.wait().unwrap().success(), "{args:?}");
}
fn git_hash(git: &Path, root: &Path, bytes: &[u8]) -> String {
    use std::io::Write as _;
    let mut child = Command::new(git)
        .args(["hash-object", "-w", "--stdin"])
        .current_dir(root)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(bytes).unwrap();
    String::from_utf8(child.wait_with_output().unwrap().stdout)
        .unwrap()
        .trim()
        .to_owned()
}
fn snapshot(fixture: &Fixture) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let refs = Command::new(&fixture.git)
        .args(["for-each-ref", "--format=%(refname)%00%(objectname)%00"])
        .current_dir(&fixture.root)
        .output()
        .unwrap()
        .stdout;
    (
        fs::read(fixture.root.join(".git/HEAD")).unwrap(),
        fs::read(fixture.root.join(".git/index")).unwrap(),
        refs,
    )
}
fn assert_snapshot(fixture: &Fixture, before: &(Vec<u8>, Vec<u8>, Vec<u8>)) {
    assert_eq!(fs::read(fixture.root.join(".git/HEAD")).unwrap(), before.0);
    assert_eq!(fs::read(fixture.root.join(".git/index")).unwrap(), before.1);
    assert_eq!(
        Command::new(&fixture.git)
            .args(["for-each-ref", "--format=%(refname)%00%(objectname)%00"])
            .current_dir(&fixture.root)
            .output()
            .unwrap()
            .stdout,
        before.2
    );
}

fn prepare(
    preparer: &RepositoryCreateFilePreparer,
    path: &str,
    content: &str,
) -> Result<rah_tools::RepositoryCreateFilePreparation, RepositoryCreateFilePreparationError> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(preparer.prepare(RepositoryCreateFilePreparationRequest {
        path: path.to_owned(),
        content: content.to_owned(),
    }))
}

fn revalidate(
    preparer: &RepositoryCreateFilePreparer,
    preparation: &rah_tools::RepositoryCreateFilePreparation,
) -> Result<(), RepositoryCreateFilePreparationError> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(preparer.revalidate(preparation))
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
        r"\\server\share",
        r"\\?\C:\verbatim",
        r"\\.\NUL",
        "file:stream",
        ".git/config",
        "CON",
        "COM1.txt",
        "src//repeated",
        "src\\backslash",
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
    fs::create_dir(fixture.root.join("directory-target")).unwrap();
    fs::write(fixture.root.join("regular-parent"), b"x").unwrap();
    for path in ["directory-target", "regular-parent/child"] {
        assert_eq!(
            result(&tool, json!({"path":path,"content":"x"}))["status"],
            "precondition_failed"
        );
    }
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

#[test]
fn preparation_is_complete_exact_and_zero_effect() {
    let fixture = Fixture::new();
    let before = snapshot(&fixture);
    let preparer = RepositoryCreateFilePreparer::new(&fixture.git, &fixture.root).unwrap();
    let content = "\u{feff}first\r\nsecond\n";
    let preparation = prepare(&preparer, "src/nested/review.rs", content).unwrap();

    assert_eq!(
        preparation.tool_input().0,
        json!({"path":"src/nested/review.rs","content":content})
    );
    assert_eq!(preparation.review().operation(), "repo.create-file");
    assert_eq!(preparation.review().path(), "src/nested/review.rs");
    assert_eq!(preparation.review().parent_path(), "src/nested");
    assert_eq!(
        preparation.review().content_escaped(),
        "\\u{feff}first\\r\\nsecond\\n"
    );
    assert_eq!(preparation.content_byte_length(), content.len());
    assert_eq!(
        preparation.content_sha256(),
        "0017207417bf647819a800ac3f04185412ca8e51e52b7ae00e6f544f7f9cd265"
    );
    assert!(!fixture.root.join("src/nested/review.rs").exists());
    assert_snapshot(&fixture, &before);
    let serialized = serde_json::to_vec(preparation.review()).unwrap();
    assert!(serialized.len() <= 256 * 1024);
    assert!(
        !String::from_utf8(serialized)
            .unwrap()
            .contains(fixture.root.to_string_lossy().as_ref())
    );
}

#[test]
fn preparation_supports_root_empty_content_and_has_deterministic_review() {
    let fixture = Fixture::new();
    let preparer = RepositoryCreateFilePreparer::new(&fixture.git, &fixture.root).unwrap();
    let first = prepare(&preparer, "root-empty.txt", "").unwrap();
    let second = prepare(&preparer, "root-empty.txt", "").unwrap();
    assert_eq!(first.review(), second.review());
    assert_eq!(first.review_identity(), second.review_identity());
    assert_eq!(
        first.tool_input().0,
        json!({"path":"root-empty.txt","content":""})
    );
    assert_eq!(first.content_byte_length(), 0);
    assert!(revalidate(&preparer, &first).is_ok());
}

#[test]
fn preparation_review_and_state_are_bounded_and_private() {
    let fixture = Fixture::new();
    let preparer = RepositoryCreateFilePreparer::new(&fixture.git, &fixture.root).unwrap();
    let oversized = "x".repeat(256 * 1024);
    assert!(matches!(
        prepare(&preparer, "src/too-large-review.rs", &oversized),
        Err(RepositoryCreateFilePreparationError::ReviewTooLarge)
    ));
    let preparation = prepare(&preparer, "src/private.rs", "private-content").unwrap();
    let debug = format!("{preparation:?}");
    assert!(!debug.contains("private-content"));
    assert!(!debug.contains(fixture.root.to_string_lossy().as_ref()));
    assert!(
        !String::from_utf8(serde_json::to_vec(preparation.review()).unwrap())
            .unwrap()
            .contains(fixture.root.to_string_lossy().as_ref())
    );
}

#[test]
fn preparation_revalidation_fails_on_target_parent_git_ignore_and_preparer_drift() {
    let fixture = Fixture::new();
    let preparer = RepositoryCreateFilePreparer::new(&fixture.git, &fixture.root).unwrap();
    let target = prepare(&preparer, "src/appears.rs", "x").unwrap();
    fs::write(fixture.root.join("src/appears.rs"), b"external").unwrap();
    assert_eq!(
        revalidate(&preparer, &target),
        Err(RepositoryCreateFilePreparationError::Stale)
    );

    let parent = fixture.root.join("src/nested");
    let moved = fixture.root.join("src/nested-moved");
    fs::rename(&parent, &moved).unwrap();
    fs::create_dir(&parent).unwrap();
    let parent_preparation = prepare(&preparer, "src/nested/parent.rs", "x").unwrap();
    fs::rename(&parent, fixture.root.join("src/nested-replaced")).unwrap();
    fs::create_dir(&parent).unwrap();
    assert_eq!(
        revalidate(&preparer, &parent_preparation),
        Err(RepositoryCreateFilePreparationError::Stale)
    );

    let git_preparation = prepare(&preparer, "src/git-drift.rs", "x").unwrap();
    fs::write(fixture.root.join("git-drift-sentinel"), b"drift").unwrap();
    run(&fixture.git, &fixture.root, &["add", "git-drift-sentinel"]);
    run(
        &fixture.git,
        &fixture.root,
        &["commit", "--quiet", "-m", "drift"],
    );
    assert_eq!(
        revalidate(&preparer, &git_preparation),
        Err(RepositoryCreateFilePreparationError::Stale)
    );

    let ignored_preparation = prepare(&preparer, "src/ignored-drift.rs", "x").unwrap();
    fs::write(fixture.root.join(".gitignore"), "src/ignored-drift.rs\n").unwrap();
    assert_eq!(
        revalidate(&preparer, &ignored_preparation),
        Err(RepositoryCreateFilePreparationError::Stale)
    );

    let other = RepositoryCreateFilePreparer::new(&fixture.git, &fixture.root).unwrap();
    let fresh = prepare(&preparer, "src/other-preparer.rs", "x").unwrap();
    assert_eq!(
        revalidate(&other, &fresh),
        Err(RepositoryCreateFilePreparationError::Stale)
    );
}

#[test]
fn rejects_noncanonical_schema_without_leaking_host_paths() {
    let fixture = Fixture::new();
    let tool = RepositoryFileCreationTool::new(&fixture.git, &fixture.root).unwrap();
    let root = fixture.root.to_string_lossy().into_owned();
    assert_eq!(
        tool.definition().permission,
        rah_protocol::PermissionLevel::Execute
    );
    assert_eq!(
        tool.definition().input_schema,
        json!({"type":"object","additionalProperties":false,"required":["path","content"],"properties":{"path":{"type":"string"},"content":{"type":"string"}}})
    );
    for input in [
        json!({"path":"src/missing-content"}),
        json!({"content":"x"}),
        json!({"path":"src/extra","content":"x","repository":"workspace"}),
        json!({"path":["src/alternate"],"content":"x"}),
    ] {
        let value = result(&tool, input);
        assert_eq!(value, json!({"status":"invalid_target"}));
        assert!(!value.to_string().contains(&root));
    }
}

#[test]
fn preserves_unrelated_dirty_worktree_content() {
    let fixture = Fixture::new();
    fs::write(
        fixture.root.join("sentinel.txt"),
        b"user-owned dirty change\n",
    )
    .unwrap();
    let tool = RepositoryFileCreationTool::new(&fixture.git, &fixture.root).unwrap();

    assert_eq!(
        result(&tool, json!({"path":"src/dirty-safe.rs","content":"new"}))["status"],
        "ok"
    );
    assert_eq!(
        fs::read(fixture.root.join("sentinel.txt")).unwrap(),
        b"user-owned dirty change\n"
    );
    let output = Command::new(&fixture.git)
        .args(["status", "--porcelain=v1", "--untracked-files=all"])
        .current_dir(&fixture.root)
        .output()
        .unwrap();
    assert_eq!(output.stdout, b" M sentinel.txt\n?? src/dirty-safe.rs\n");
}

#[test]
fn rejects_real_submodule_sparse_intent_and_conflict_index_targets() {
    let fixture = Fixture::new();
    let child = fixture.root.parent().unwrap().join("rah-create-file-child");
    let _ = fs::remove_dir_all(&child);
    fs::create_dir(&child).unwrap();
    run(&fixture.git, &child, &["init", "--quiet"]);
    run(&fixture.git, &child, &["config", "user.name", "RAH Test"]);
    run(
        &fixture.git,
        &child,
        &["config", "user.email", "rah@example.invalid"],
    );
    fs::write(child.join("base"), "base").unwrap();
    run(&fixture.git, &child, &["add", "base"]);
    run(&fixture.git, &child, &["commit", "--quiet", "-m", "base"]);
    run(
        &fixture.git,
        &fixture.root,
        &[
            "-c",
            "protocol.file.allow=always",
            "submodule",
            "add",
            "--quiet",
            child.to_str().unwrap(),
            "submodule_dir",
        ],
    );
    let tool = RepositoryFileCreationTool::new(&fixture.git, &fixture.root).unwrap();
    let before = snapshot(&fixture);
    assert_eq!(
        result(&tool, json!({"path":"submodule_dir/new.rs","content":"x"}))["status"],
        "precondition_failed"
    );
    assert!(!fixture.root.join("submodule_dir/new.rs").exists());
    assert_snapshot(&fixture, &before);
    run(
        &fixture.git,
        &fixture.root,
        &["sparse-checkout", "init", "--cone"],
    );
    let before = snapshot(&fixture);
    assert_eq!(
        result(&tool, json!({"path":"src/sparse.rs","content":"x"}))["status"],
        "precondition_failed"
    );
    assert!(!fixture.root.join("src/sparse.rs").exists());
    assert_snapshot(&fixture, &before);
    run(&fixture.git, &fixture.root, &["sparse-checkout", "disable"]);
    fs::write(fixture.root.join("intent.rs"), "x").unwrap();
    run(&fixture.git, &fixture.root, &["add", "-N", "intent.rs"]);
    fs::remove_file(fixture.root.join("intent.rs")).unwrap();
    let before = snapshot(&fixture);
    assert_eq!(
        result(&tool, json!({"path":"intent.rs","content":"x"}))["status"],
        "precondition_failed"
    );
    assert_snapshot(&fixture, &before);
    let hash = git_hash(&fixture.git, &fixture.root, b"conflict");
    for stage in 1..=3 {
        let path = format!("conflict-{stage}.rs");
        let record = format!("100644 {hash} {stage}\t{path}\0");
        run_input(
            &fixture.git,
            &fixture.root,
            &["update-index", "-z", "--index-info"],
            record.as_bytes(),
        );
        let before = snapshot(&fixture);
        assert_eq!(
            result(&tool, json!({"path":path,"content":"x"}))["status"],
            "precondition_failed"
        );
        assert!(!fixture.root.join(path).exists());
        assert_snapshot(&fixture, &before);
    }
    let _ = fs::remove_dir_all(&child);
}

#[test]
fn measures_exact_serialized_request_bytes_including_path_and_escapes() {
    let fixture = Fixture::new();
    let tool = RepositoryFileCreationTool::new(&fixture.git, &fixture.root).unwrap();
    let path = "src/request-bound";
    let mut content = "\n".repeat(160 * 1024);
    while serde_json::to_vec(&json!({"path":path,"content":content}))
        .unwrap()
        .len()
        > 320 * 1024
    {
        content.pop();
    }
    while serde_json::to_vec(&json!({"path":path,"content":content}))
        .unwrap()
        .len()
        < 320 * 1024
    {
        content.push('x');
    }
    assert_eq!(
        serde_json::to_vec(&json!({"path":path,"content":content}))
            .unwrap()
            .len(),
        320 * 1024
    );
    assert_eq!(
        result(&tool, json!({"path":path,"content":content}))["status"],
        "ok"
    );
    let over_path = "src/request-boundx";
    assert_eq!(
        serde_json::to_vec(&json!({"path":over_path,"content":content}))
            .unwrap()
            .len(),
        320 * 1024 + 1
    );
    assert_eq!(
        result(&tool, json!({"path":over_path,"content":content}))["status"],
        "invalid_target"
    );
    assert_eq!(
        result(
            &tool,
            json!({"path":"src/escaped","content":"\n".repeat(200 * 1024)})
        )["status"],
        "invalid_target"
    );
}
