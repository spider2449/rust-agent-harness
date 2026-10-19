use std::process::Stdio;

use rah_api::{
    CancellationToken,
    sandbox::{ExecOutput, ProgramSpec},
};
use rah_protocol::SessionId;

use super::*;

/// Runs specs with `tokio::process` in `root`.
struct TestEnv(PathBuf);

#[async_trait]
impl ExecEnv for TestEnv {
    async fn run(&self, spec: ProgramSpec, _: CancellationToken) -> Result<ExecOutput, ExecError> {
        let cwd = spec
            .cwd
            .map(|cwd| self.0.join(cwd))
            .unwrap_or_else(|| self.0.clone());
        let output = tokio::process::Command::new(&spec.program)
            .args(&spec.args)
            .current_dir(cwd)
            .envs(&spec.env)
            .stdin(Stdio::null())
            .output()
            .await
            .map_err(|error| ExecError::Spawn {
                program: spec.program.clone(),
                message: error.to_string(),
            })?;
        Ok(ExecOutput {
            stdout: output.stdout,
            stderr: output.stderr,
            exit_code: output.status.code(),
            termination: None,
        })
    }
}

struct Repo(PathBuf);

impl Repo {
    fn new(name: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("rah-repo-write-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(path.join("src")).unwrap();
        let repo = Self(std::fs::canonicalize(path).unwrap());
        std::fs::write(repo.0.join("src/lib.rs"), "fn alpha() {}\nfn beta() {}\n").unwrap();
        std::fs::write(repo.0.join("crlf.txt"), "one\r\ntwo\r\n").unwrap();
        let status = std::process::Command::new("git")
            .args(["init", "--quiet"])
            .current_dir(&repo.0)
            .status()
            .unwrap();
        assert!(status.success());
        repo
    }

    fn read(&self, path: &str) -> String {
        std::fs::read_to_string(self.0.join(path)).unwrap()
    }

    async fn tools(&self, config: Value) -> BTreeMap<String, Arc<dyn Tool>> {
        let mut config: RepoWriteConfig = serde_json::from_value(config).unwrap();
        config.root.get_or_insert_with(|| self.0.clone());
        write_tools(Arc::new(TestEnv(self.0.clone())), config)
            .await
            .unwrap()
            .into_iter()
            .map(|tool| (tool.definition().name.to_string(), tool))
            .collect()
    }
}

impl Drop for Repo {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn cx() -> ToolContext {
    ToolContext {
        session: SessionId::new(),
        cancel: CancellationToken::new(),
    }
}

#[tokio::test]
async fn create_file_creates_new_files_only_inside_the_root() {
    let repo = Repo::new("create");
    let tools = repo.tools(json!({})).await;
    let create = &tools["repo.create-file"];
    create
        .call(
            json!({ "path": "docs/new/notes.md", "content": "hello" }),
            cx(),
        )
        .await
        .unwrap();
    assert_eq!(repo.read("docs/new/notes.md"), "hello");
    for path in [
        "src/lib.rs",
        "docs/new/notes.md",
        "../escape.txt",
        ".git/config",
        ".rah/storage/frontend-web/settings-tools",
        ".RAH/x",
        "/abs",
        "a\\b",
    ] {
        assert!(
            matches!(
                create
                    .call(json!({ "path": path, "content": "x" }), cx())
                    .await,
                Err(ToolError::InvalidInput(_))
            ),
            "{path}"
        );
    }
    let tools = repo.tools(json!({ "max_file_bytes": 3 })).await;
    assert!(
        tools["repo.create-file"]
            .call(json!({ "path": "big.txt", "content": "four" }), cx())
            .await
            .is_err()
    );
}

#[tokio::test]
async fn edit_files_is_all_or_nothing() {
    let repo = Repo::new("edit");
    let tools = repo.tools(json!({})).await;
    let edit = &tools["repo.edit-files"];
    edit.call(
        json!({ "edits": [
            { "path": "src/lib.rs", "old": "fn alpha() {}", "new": "fn alpha() { 1 }" },
            { "path": "src/lib.rs", "old": "fn beta() {}", "new": "fn gamma() {}" },
            { "path": "crlf.txt", "old": "one\ntwo", "new": "uno\ndos" }
        ] }),
        cx(),
    )
    .await
    .unwrap();
    assert_eq!(repo.read("src/lib.rs"), "fn alpha() { 1 }\nfn gamma() {}\n");
    assert_eq!(repo.read("crlf.txt"), "uno\r\ndos\r\n");

    // The second edit fails, so the first file is not changed either.
    let error = edit
        .call(
            json!({ "edits": [
                { "path": "src/lib.rs", "old": "gamma", "new": "delta" },
                { "path": "crlf.txt", "old": "missing", "new": "x" }
            ] }),
            cx(),
        )
        .await
        .unwrap_err();
    assert!(
        matches!(&error, ToolError::InvalidInput(m) if m.contains("not found")),
        "{error}"
    );
    assert_eq!(repo.read("src/lib.rs"), "fn alpha() { 1 }\nfn gamma() {}\n");
    std::fs::write(repo.0.join("dup.txt"), "x x").unwrap();
    let error = edit
        .call(
            json!({ "edits": [{ "path": "dup.txt", "old": "x", "new": "y" }] }),
            cx(),
        )
        .await
        .unwrap_err();
    assert!(error.to_string().contains("occurs 2 times"), "{error}");
    assert!(
        edit.call(
            json!({ "edits": [{ "path": "missing.rs", "old": "a", "new": "b" }] }),
            cx()
        )
        .await
        .is_err()
    );
}

#[tokio::test]
async fn patch_applies_checked_diffs_only() {
    let repo = Repo::new("patch");
    let tools = repo.tools(json!({})).await;
    let patch = &tools["repo.patch"];
    let diff = "--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1,2 +1,2 @@\n fn alpha() {}\n-fn beta() {}\n+fn beta() { 2 }\n";
    patch.call(json!({ "patch": diff }), cx()).await.unwrap();
    assert_eq!(repo.read("src/lib.rs"), "fn alpha() {}\nfn beta() { 2 }\n");
    // The same diff no longer applies; nothing changes.
    let error = patch
        .call(json!({ "patch": diff }), cx())
        .await
        .unwrap_err();
    assert!(matches!(error, ToolError::InvalidInput(_)), "{error}");
    let escape = "--- a/../outside.txt\n+++ b/../outside.txt\n@@ -0,0 +1 @@\n+x\n";
    assert!(patch.call(json!({ "patch": escape }), cx()).await.is_err());
    assert!(!repo.0.parent().unwrap().join("outside.txt").exists());
    // Nothing under `.rah/`, also as the new name of a rename.
    for diff in [
        "--- /dev/null
+++ b/.rah/storage/x
@@ -0,0 +1 @@
+x
",
        "diff --git a/src/lib.rs b/.rah/lib.rs
similarity index 100%
rename from src/lib.rs
rename to .rah/lib.rs
",
    ] {
        let error = patch
            .call(json!({ "patch": diff }), cx())
            .await
            .unwrap_err();
        assert!(error.to_string().contains("`.rah/`"), "{error}");
    }
    assert!(!repo.0.join(".rah").exists());
    assert!(repo.0.join("src/lib.rs").exists());

    // A sandbox whose repository is not the configured root is refused.
    let tools = repo
        .tools(json!({ "cwd": "src", "root": repo.0.join("src") }))
        .await;
    let error = tools["repo.patch"]
        .call(json!({ "patch": diff }), cx())
        .await
        .unwrap_err();
    assert!(matches!(error, ToolError::NotStarted(_)), "{error}");
}

#[test]
fn definitions_declare_write() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    let repo = Repo::new("definitions");
    for tool in runtime.block_on(repo.tools(json!({}))).values() {
        assert_eq!(tool.definition().permission, PermissionLevel::Write);
    }
}

#[tokio::test]
async fn concurrent_edits_of_one_file_are_all_applied() {
    let repo = Repo::new("concurrent");
    let lines: Vec<String> = (0..24).map(|n| format!("line {n}")).collect();
    std::fs::write(repo.0.join("many.txt"), lines.join("\n") + "\n").unwrap();
    let tools = repo.tools(json!({})).await;
    let edit = tools["repo.edit-files"].clone();
    let mut tasks = Vec::new();
    for n in 0..24 {
        let edit = edit.clone();
        tasks.push(tokio::spawn(async move {
            edit.call(
                json!({ "edits": [
                    { "path": "many.txt", "old": format!("line {n}\n"), "new": format!("edited {n}\n") }
                ] }),
                cx(),
            )
            .await
        }));
    }
    for task in tasks {
        task.await.unwrap().unwrap();
    }
    let expected: Vec<String> = (0..24).map(|n| format!("edited {n}")).collect();
    assert_eq!(repo.read("many.txt"), expected.join("\n") + "\n");
}

#[tokio::test]
async fn edits_that_name_the_same_files_in_opposite_orders_do_not_wait_for_each_other() {
    let repo = Repo::new("order");
    std::fs::write(repo.0.join("a.txt"), "a0\n<end>\n").unwrap();
    std::fs::write(repo.0.join("b.txt"), "b0\n<end>\n").unwrap();
    let tools = repo.tools(json!({})).await;
    let edit = tools["repo.edit-files"].clone();
    let both = |first: &'static str, second: &'static str, tag: &'static str| {
        let edit = edit.clone();
        tokio::spawn(async move {
            for round in 0..20 {
                let line = format!("{tag}{round}");
                let insert = format!("{line}\n<end>\n");
                edit.call(
                    json!({ "edits": [
                        { "path": first, "old": "<end>\n", "new": insert },
                        { "path": second, "old": "<end>\n", "new": insert },
                    ] }),
                    cx(),
                )
                .await
                .unwrap();
            }
        })
    };
    let (one, two) = (both("a.txt", "b.txt", "x"), both("b.txt", "a.txt", "y"));
    tokio::time::timeout(std::time::Duration::from_secs(30), async {
        one.await.unwrap();
        two.await.unwrap();
    })
    .await
    .expect("the edits deadlocked");
    // Every edit landed: 40 inserted lines in each file, plus its own two.
    assert_eq!(repo.read("a.txt").lines().count(), 2 + 40);
    assert_eq!(repo.read("b.txt").lines().count(), 2 + 40);
}

#[tokio::test]
async fn a_byte_order_mark_is_kept_exactly_once() {
    let repo = Repo::new("bom");
    std::fs::write(repo.0.join("bom.txt"), "\u{feff}first\nsecond\n").unwrap();
    let tools = repo.tools(json!({})).await;
    tools["repo.edit-files"]
        .call(
            json!({ "edits": [
                { "path": "bom.txt", "old": "first", "new": "changed" },
                { "path": "bom.txt", "old": "second", "new": "other" },
            ] }),
            cx(),
        )
        .await
        .unwrap();
    let bytes = std::fs::read(repo.0.join("bom.txt")).unwrap();
    assert_eq!(bytes, "\u{feff}changed\nother\n".as_bytes());
    assert_eq!(
        bytes
            .windows(3)
            .filter(|w| *w == [0xEF, 0xBB, 0xBF])
            .count(),
        1
    );
}

#[tokio::test]
async fn a_patch_waits_for_running_edits() {
    let repo = Repo::new("patch-wait");
    let tools = repo.tools(json!({})).await;
    let edit = tools["repo.edit-files"].clone();
    let patch = tools["repo.patch"].clone();
    let diff = "--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1,2 +1,2 @@\n-fn alpha() {}\n+fn alpha() { 0 }\n fn beta() {}\n";
    let (edited, patched) = tokio::join!(
        edit.call(
            json!({ "edits": [{ "path": "src/lib.rs", "old": "fn beta() {}", "new": "fn beta() { 1 }" }] }),
            cx(),
        ),
        patch.call(json!({ "patch": diff }), cx()),
    );
    // Either order is valid; neither may corrupt the file or fail half way.
    let text = repo.read("src/lib.rs");
    if patched.is_ok() {
        assert!(text.contains("fn alpha() { 0 }"), "{text}");
    }
    if edited.is_ok() {
        assert!(text.contains("fn beta() { 1 }"), "{text}");
    }
    assert!(edited.is_ok() || patched.is_ok());
}

fn invalid(result: Result<ToolOutput, ToolError>) -> bool {
    matches!(result, Err(ToolError::InvalidInput(_)))
}

#[tokio::test]
async fn delete_file_removes_one_regular_file_inside_the_root() {
    let repo = Repo::new("delete");
    let tools = repo.tools(json!({})).await;
    let delete = &tools["repo.delete-file"];
    assert_eq!(
        delete.definition().permission,
        PermissionLevel::Write,
        "deleting a file asks like any write"
    );
    let done = delete
        .call(json!({ "path": "crlf.txt" }), cx())
        .await
        .unwrap();
    assert!(!done.is_error);
    assert!(!repo.0.join("crlf.txt").exists());
    assert!(repo.0.join("src/lib.rs").exists(), "other files are kept");
    for path in [
        "crlf.txt", // already gone
        "src",      // a directory
        "../escape.txt",
        ".git/config",
        ".rah/storage/frontend-web/settings-tools",
        "/abs",
        r"a\b",
    ] {
        assert!(
            invalid(delete.call(json!({ "path": path }), cx()).await),
            "{path}"
        );
    }
    assert!(repo.0.join("src").is_dir());
}

#[cfg(unix)]
#[tokio::test]
async fn delete_and_move_do_not_touch_symbolic_links() {
    let repo = Repo::new("links");
    std::os::unix::fs::symlink("src/lib.rs", repo.0.join("link.rs")).unwrap();
    let tools = repo.tools(json!({})).await;
    assert!(invalid(
        tools["repo.delete-file"]
            .call(json!({ "path": "link.rs" }), cx())
            .await
    ));
    assert!(invalid(
        tools["repo.move-file"]
            .call(json!({ "from": "link.rs", "to": "moved.rs" }), cx())
            .await
    ));
    assert!(repo.0.join("src/lib.rs").exists());
    assert!(std::fs::symlink_metadata(repo.0.join("link.rs")).is_ok());
}

#[tokio::test]
async fn move_file_renames_without_overwriting() {
    let repo = Repo::new("move");
    let tools = repo.tools(json!({})).await;
    let mv = &tools["repo.move-file"];
    mv.call(
        json!({ "from": "crlf.txt", "to": "deep/er/lines.txt" }),
        cx(),
    )
    .await
    .unwrap();
    assert_eq!(repo.read("deep/er/lines.txt"), "one\r\ntwo\r\n");
    assert!(!repo.0.join("crlf.txt").exists());
    for (from, to) in [
        ("src/lib.rs", "deep/er/lines.txt"), // the target exists
        ("src/lib.rs", "src/lib.rs"),        // the same path
        ("missing.txt", "x.txt"),
        ("src", "other"), // a directory
        ("src/lib.rs", "../escape.txt"),
        ("src/lib.rs", ".git/hooks/pre-commit"),
        ("src/lib.rs", ".rah/storage/x"),
        ("../escape.txt", "x.txt"),
    ] {
        assert!(
            invalid(mv.call(json!({ "from": from, "to": to }), cx()).await),
            "{from} -> {to}"
        );
    }
    assert!(
        repo.0.join("src/lib.rs").exists(),
        "a refused move changes nothing"
    );
}

fn git(repo: &Repo, args: &[&str]) -> String {
    let output = std::process::Command::new("git")
        .args(args)
        .current_dir(&repo.0)
        .env(
            "GIT_CONFIG_GLOBAL",
            if cfg!(windows) { "NUL" } else { "/dev/null" },
        )
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .unwrap();
    assert!(output.status.success(), "git {args:?}: {output:?}");
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

fn author() -> Value {
    json!({ "commit": { "name": "Test Author", "email": "test@example.com" } })
}

#[tokio::test]
async fn commit_exists_only_with_an_identity_and_asks_at_execute() {
    let repo = Repo::new("commit-config");
    let tools = repo.tools(json!({})).await;
    assert!(!tools.contains_key("repo.commit"));
    let tools = repo.tools(author()).await;
    assert_eq!(
        tools["repo.commit"].definition().permission,
        PermissionLevel::Execute
    );
    for commit in [
        json!({ "name": "", "email": "a@b.c" }),
        json!({ "name": "A", "email": " " }),
        json!({ "name": "A\nB", "email": "a@b.c" }),
        json!({ "name": "A", "email": "<a@b.c>" }),
    ] {
        let config: RepoWriteConfig = serde_json::from_value(json!({ "commit": commit })).unwrap();
        assert!(
            write_tools(Arc::new(TestEnv(repo.0.clone())), config)
                .await
                .is_err(),
            "{commit}"
        );
    }
}

#[tokio::test]
async fn commit_commits_only_the_named_files_as_the_configured_identity() {
    let repo = Repo::new("commit");
    let tools = repo.tools(author()).await;
    let commit = &tools["repo.commit"];
    std::fs::write(repo.0.join("a.txt"), "a").unwrap();
    std::fs::write(repo.0.join("b.txt"), "b").unwrap();
    // Something staged earlier is not swept into the commit.
    git(&repo, &["add", "--", "b.txt"]);
    let done = commit
        .call(
            json!({ "message": "add a\n\nbody", "paths": ["a.txt"] }),
            cx(),
        )
        .await
        .unwrap();
    assert!(!done.is_error);
    assert_eq!(
        git(&repo, &["log", "-1", "--format=%an|%ae|%cn|%ce|%s"]),
        "Test Author|test@example.com|Test Author|test@example.com|add a"
    );
    assert_eq!(
        git(&repo, &["show", "--name-only", "--format=", "HEAD"]),
        "a.txt"
    );
    assert!(git(&repo, &["status", "--porcelain"]).contains("A  b.txt"));

    // Nothing changed for the path: refused, no empty commit.
    assert!(invalid(
        commit
            .call(json!({ "message": "again", "paths": ["a.txt"] }), cx())
            .await
    ));
    assert_eq!(git(&repo, &["rev-list", "--count", "HEAD"]), "1");

    // A deletion is committed too.
    std::fs::remove_file(repo.0.join("a.txt")).unwrap();
    commit
        .call(json!({ "message": "drop a", "paths": ["a.txt"] }), cx())
        .await
        .unwrap();
    assert_eq!(
        git(&repo, &["show", "--name-status", "--format=", "HEAD"]),
        "D\ta.txt"
    );
}

#[tokio::test]
async fn commit_refuses_patterns_directories_and_unsafe_input() {
    let repo = Repo::new("commit-input");
    let tools = repo.tools(author()).await;
    let commit = &tools["repo.commit"];
    std::fs::write(repo.0.join("a.txt"), "a").unwrap();
    std::fs::write(repo.0.join("b.txt"), "b").unwrap();
    for input in [
        json!({ "message": "x", "paths": ["*.txt"] }), // not a pattern
        json!({ "message": "x", "paths": ["src"] }),   // a directory
        json!({ "message": "x", "paths": [".git/config"] }),
        json!({ "message": "x", "paths": ["../a.txt"] }),
        json!({ "message": "x", "paths": [":(top)a.txt"] }),
        json!({ "message": "x", "paths": [] }),
        json!({ "message": "  ", "paths": ["a.txt"] }),
        json!({ "message": "x", "paths": ["nosuch.txt"] }),
        json!({ "message": "x", "paths": ["a.txt"], "amend": true }),
    ] {
        let result = commit.call(input.clone(), cx()).await;
        assert!(invalid(result), "{input}");
    }
    assert!(
        git(&repo, &["status", "--porcelain"]).contains("?? a.txt"),
        "a refused call stages nothing"
    );
}
