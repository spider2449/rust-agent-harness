use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};

use rah_protocol::ToolInput;
use rah_tools::{
    FsReadTool, RepositoryDiffStagedTool, RepositoryDiffTool, RepositoryFileInfoTool,
    RepositoryStatusTool, Tool, ToolContext,
};
use serde_json::json;

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    root: PathBuf,
    git: PathBuf,
}

impl Fixture {
    fn nested_repository() -> Self {
        let root = std::env::temp_dir().join(format!(
            "rah-task-312-nested-{}-{}",
            std::process::id(),
            NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("nested")).unwrap();
        let git = native_git();
        run(&git, &root, &["init", "--quiet"]);
        run(&git, &root, &["config", "user.name", "RAH Task 312"]);
        run(
            &git,
            &root,
            &["config", "user.email", "rah-task-312@example.invalid"],
        );
        fs::write(root.join("nested/secret.txt"), b"nested secret\n").unwrap();
        run(&git, &root, &["add", "nested/secret.txt"]);
        run(&git, &root, &["commit", "--quiet", "-m", "base"]);
        run(&git, &root.join("nested"), &["init", "--quiet"]);
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
    let output = Command::new("where.exe").arg("git.exe").output().unwrap();
    #[cfg(not(windows))]
    let output = Command::new("which").arg("git").output().unwrap();
    fs::canonicalize(
        String::from_utf8(output.stdout)
            .unwrap()
            .lines()
            .next()
            .unwrap(),
    )
    .unwrap()
}

fn run(git: &Path, cwd: &Path, args: &[&str]) {
    assert!(
        Command::new(git)
            .args(args)
            .current_dir(cwd)
            .status()
            .unwrap()
            .success(),
        "git {args:?}"
    );
}

async fn rejected(tool: &dyn Tool, input: serde_json::Value) {
    assert!(
        tool.execute(ToolInput(input), ToolContext::default())
            .await
            .is_err(),
        "nested repository operation unexpectedly succeeded"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn real_repo_a_and_nested_repo_b_are_rejected_before_observation() {
    let fixture = Fixture::nested_repository();

    let reader = FsReadTool::new_repository(&fixture.root, 1024).unwrap();
    rejected(&reader, json!({"path":"nested/secret.txt"})).await;

    let file_info = RepositoryFileInfoTool::new(&fixture.git, &fixture.root).unwrap();
    rejected(&file_info, json!({"path":"nested/secret.txt"})).await;

    let status = RepositoryStatusTool::new(&fixture.git, &fixture.root).unwrap();
    rejected(&status, json!({})).await;

    let diff = RepositoryDiffTool::new(&fixture.git, &fixture.root).unwrap();
    rejected(&diff, json!({})).await;

    let diff_staged = RepositoryDiffStagedTool::new(&fixture.git, &fixture.root).unwrap();
    rejected(&diff_staged, json!({})).await;
}
