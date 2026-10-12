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
