//! Runs real Git in temporary repositories through a minimal test sandbox.

use std::{
    path::Path,
    process::Stdio,
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

use rah_api::{
    CancellationToken,
    sandbox::{ExecError, ExecOutput, ProgramSpec},
};
use rah_protocol::SessionId;

use super::*;

/// Runs specs with `tokio::process` in `root`, with a cleared environment.
struct TestEnv {
    root: PathBuf,
    runs: AtomicUsize,
    last_env: Mutex<BTreeMap<String, String>>,
}

#[async_trait]
impl ExecEnv for TestEnv {
    async fn run(
        &self,
        spec: ProgramSpec,
        _cancel: CancellationToken,
    ) -> Result<ExecOutput, ExecError> {
        self.runs.fetch_add(1, Ordering::SeqCst);
        *self.last_env.lock().unwrap() = spec.env.clone();
        let cwd = spec
            .cwd
            .map(|cwd| self.root.join(cwd))
            .unwrap_or_else(|| self.root.clone());
        let mut command = tokio::process::Command::new(&spec.program);
        command
            .args(&spec.args)
            .current_dir(cwd)
            .env_clear()
            .envs(inherited())
            .envs(&spec.env)
            .stdin(Stdio::null());
        let output = command.output().await.map_err(|error| ExecError::Spawn {
            program: spec.program.clone(),
            message: error.to_string(),
        })?;
        let mut stdout = output.stdout;
        let termination = (stdout.len() > spec.limits.stdout_bytes).then(|| {
            stdout.truncate(spec.limits.stdout_bytes);
            rah_api::sandbox::Termination::OutputLimit
        });
        Ok(ExecOutput {
            stdout,
            stderr: output.stderr,
            exit_code: if termination.is_some() {
                None
            } else {
                output.status.code()
            },
            termination,
        })
    }
}

fn inherited() -> Vec<(String, String)> {
    ["PATH", "PATHEXT", "SYSTEMROOT", "WINDIR", "TEMP", "TMP"]
        .into_iter()
        .filter_map(|name| Some((name.to_owned(), std::env::var(name).ok()?)))
        .collect()
}

struct Repo(PathBuf);

impl Repo {
    /// A repository on branch `main` with `README.md` and `src/lib.rs`
    /// committed, then: `src/lib.rs` modified, `docs/new.md` staged and
    /// `notes.txt` untracked.
    fn new(name: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("rah-tools-repo-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(path.join("src")).unwrap();
        std::fs::create_dir_all(path.join("docs")).unwrap();
        let repo = Self(std::fs::canonicalize(path).unwrap());
        repo.write("README.md", "# Fixture\nplugin kernel\n");
        repo.write("src/lib.rs", "fn alpha() {}\n");
        repo.git(&["init", "--quiet", "--initial-branch=main"]);
        repo.git(&["add", "."]);
        repo.git(&["commit", "--quiet", "-m", "fixture"]);
        repo.write("src/lib.rs", "fn alpha() {}\nfn beta() {}\n");
        repo.write("docs/new.md", "staged text\n");
        repo.git(&["add", "docs/new.md"]);
        repo.write("notes.txt", "untracked\n");
        repo
    }

    fn write(&self, path: &str, text: &str) {
        std::fs::write(self.0.join(path), text).unwrap();
    }

    fn git(&self, args: &[&str]) {
        let status = std::process::Command::new("git")
            .args([
                "-c",
                "user.name=RAH Test",
                "-c",
                "user.email=rah@example.invalid",
            ])
            .args(args)
            .current_dir(&self.0)
            .envs(git::hardened_env())
            .stdout(Stdio::null())
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?}");
    }

    fn tools(&self, config: Value) -> (Arc<TestEnv>, BTreeMap<String, Arc<dyn Tool>>) {
        let env = Arc::new(TestEnv {
            root: self.0.clone(),
            runs: AtomicUsize::new(0),
            last_env: Mutex::new(BTreeMap::new()),
        });
        let tools = repo_tools(env.clone(), serde_json::from_value(config).unwrap())
            .unwrap()
            .into_iter()
            .map(|tool| (tool.definition().name.to_string(), tool))
            .collect();
        (env, tools)
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

async fn call(
    tools: &BTreeMap<String, Arc<dyn Tool>>,
    name: &str,
    input: Value,
) -> Result<Value, ToolError> {
    let output = tools[name].call(input, cx()).await?;
    assert!(!output.is_error);
    Ok(match output.content.into_iter().next().unwrap() {
        ToolContent::Json(value) => value,
        ToolContent::Text(text) => Value::String(text),
    })
}

#[tokio::test]
async fn status_reports_branch_and_changed_paths() {
    let repo = Repo::new("status");
    let (env, tools) = repo.tools(json!({}));
    let status = call(&tools, "repo.status", json!({})).await.unwrap();
    assert!(
        status["branch"].as_str().unwrap().starts_with("main"),
        "{status}"
    );
    let entries = status["entries"].as_array().unwrap();
    let find = |path: &str| entries.iter().find(|e| e["path"] == path).cloned();
    assert_eq!(find("src/lib.rs").unwrap()["worktree"], "M");
    assert_eq!(find("docs/new.md").unwrap()["index"], "A");
    assert_eq!(find("notes.txt").unwrap()["index"], "?");
    assert_eq!(status["truncated"], false);
    let hardened = env.last_env.lock().unwrap().clone();
    assert_eq!(hardened["GIT_LITERAL_PATHSPECS"], "1");
    assert_eq!(hardened["GIT_CONFIG_NOSYSTEM"], "1");
}

#[tokio::test]
async fn diff_shows_unstaged_staged_and_per_path_changes() {
    let repo = Repo::new("diff");
    let (_, tools) = repo.tools(json!({}));
    let unstaged = call(&tools, "repo.diff", json!({})).await.unwrap();
    assert!(
        unstaged.as_str().unwrap().contains("+fn beta() {}"),
        "{unstaged}"
    );
    assert!(!unstaged.as_str().unwrap().contains("staged text"));
    let staged = call(&tools, "repo.diff", json!({ "staged": true }))
        .await
        .unwrap();
    assert!(
        staged.as_str().unwrap().contains("+staged text"),
        "{staged}"
    );
    let none = call(&tools, "repo.diff", json!({ "path": "README.md" }))
        .await
        .unwrap();
    assert_eq!(none, "no differences");

    let (_, tools) = repo.tools(json!({ "max_output_bytes": 40 }));
    let truncated = call(&tools, "repo.diff", json!({})).await.unwrap();
    assert!(
        truncated.as_str().unwrap().ends_with("[diff truncated]"),
        "{truncated}"
    );
}

#[tokio::test]
async fn list_shows_tracked_direct_children_with_literal_paths() {
    let repo = Repo::new("list");
    let (_, tools) = repo.tools(json!({}));
    let root = call(&tools, "repo.list", json!({})).await.unwrap();
    assert_eq!(
        root["entries"],
        json!([
            { "name": "README.md", "kind": "file" },
            { "name": "docs", "kind": "dir" },
            { "name": "src", "kind": "dir" }
        ])
    );
    let src = call(&tools, "repo.list", json!({ "path": "src" }))
        .await
        .unwrap();
    assert_eq!(
        src["entries"],
        json!([{ "name": "lib.rs", "kind": "file" }])
    );
    // Pathspecs are literal: `s*` is not a glob for `src`.
    for path in ["missing", "s*"] {
        assert!(matches!(
            call(&tools, "repo.list", json!({ "path": path })).await,
            Err(ToolError::InvalidInput(_))
        ));
    }
    let (_, tools) = repo.tools(json!({ "max_results": 1 }));
    let limited = call(&tools, "repo.list", json!({})).await.unwrap();
    assert_eq!(limited["entries"].as_array().unwrap().len(), 1);
    assert_eq!(limited["truncated"], true);
}

#[tokio::test]
async fn search_finds_paths_and_literal_text() {
    let repo = Repo::new("search");
    let (_, tools) = repo.tools(json!({}));
    let paths = call(
        &tools,
        "repo.search",
        json!({ "mode": "path", "query": "LIB" }),
    )
    .await
    .unwrap();
    assert_eq!(paths["paths"], json!(["src/lib.rs"]));
    let text = call(
        &tools,
        "repo.search",
        json!({ "mode": "text", "query": "beta()" }),
    )
    .await
    .unwrap();
    assert_eq!(
        text["matches"],
        json!([{ "path": "src/lib.rs", "line": 2, "text": "fn beta() {}" }])
    );
    // A query that looks like an option is searched literally.
    let none = call(
        &tools,
        "repo.search",
        json!({ "mode": "text", "query": "--version" }),
    )
    .await
    .unwrap();
    assert_eq!(none["matches"], json!([]));
    let scoped = call(
        &tools,
        "repo.search",
        json!({ "mode": "text", "query": "kernel", "path_prefix": "src" }),
    )
    .await
    .unwrap();
    assert_eq!(scoped["matches"], json!([]));
}

#[tokio::test]
async fn unsafe_input_is_rejected_before_git_runs() {
    let repo = Repo::new("unsafe");
    let (env, tools) = repo.tools(json!({}));
    for path in [
        "../x",
        "/etc",
        "a\\b",
        ".git/config",
        "src/.GIT",
        "-p",
        "a//b",
        "a/",
        "./a",
        "C:x",
        "",
    ] {
        for (tool, input) in [
            ("repo.list", json!({ "path": path })),
            ("repo.diff", json!({ "path": path })),
            (
                "repo.search",
                json!({ "mode": "path", "query": "x", "path_prefix": path }),
            ),
        ] {
            assert!(
                matches!(
                    call(&tools, tool, input).await,
                    Err(ToolError::InvalidInput(_))
                ),
                "{tool} {path}"
            );
        }
    }
    for input in [
        json!({ "mode": "text", "query": "" }),
        json!({ "mode": "text", "query": "a\nb" }),
        json!({ "mode": "regex", "query": "a" }),
        json!({ "mode": "text", "query": "a", "extra": 1 }),
    ] {
        assert!(call(&tools, "repo.search", input).await.is_err());
    }
    assert!(
        call(&tools, "repo.status", json!({ "x": 1 }))
            .await
            .is_err()
    );
    assert_eq!(env.runs.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn directories_below_the_top_level_or_outside_a_repository_are_refused() {
    let repo = Repo::new("toplevel");
    let (_, tools) = repo.tools(json!({ "cwd": "src" }));
    let error = call(&tools, "repo.status", json!({})).await.unwrap_err();
    assert!(error.to_string().contains("not the top level"), "{error}");

    let plain = std::env::temp_dir().join(format!("rah-tools-repo-plain-{}", std::process::id()));
    std::fs::create_dir_all(&plain).unwrap();
    let not_repo = Repo(std::fs::canonicalize(&plain).unwrap());
    let (_, tools) = not_repo.tools(json!({}));
    assert!(matches!(
        call(&tools, "repo.list", json!({})).await,
        Err(ToolError::Execution(_))
    ));
}

#[test]
fn definitions_are_read_only_and_closed() {
    let env: Arc<dyn ExecEnv> = Arc::new(TestEnv {
        root: Path::new(".").to_path_buf(),
        runs: AtomicUsize::new(0),
        last_env: Mutex::new(BTreeMap::new()),
    });
    let tools = repo_tools(env.clone(), RepoConfig::default()).unwrap();
    let names: Vec<String> = tools
        .iter()
        .map(|t| t.definition().name.to_string())
        .collect();
    assert_eq!(
        names,
        ["repo.status", "repo.diff", "repo.list", "repo.search"]
    );
    for tool in &tools {
        let definition = tool.definition();
        assert_eq!(definition.permission, PermissionLevel::Read);
        assert_eq!(definition.input_schema["additionalProperties"], false);
    }
    let zero: RepoConfig = serde_json::from_value(json!({ "max_results": 0 })).unwrap();
    assert!(repo_tools(env, zero).is_err());
}
