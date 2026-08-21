use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use rah_protocol::{PermissionLevel, ToolContent, ToolInput, ToolName};
use rah_tools::{GIT_STATUS_TOOL_NAME, GitStatusTool, Tool, ToolContext, ToolError, ToolRegistry};
use serde_json::{Value, json};

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new(label: &str) -> Self {
        let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "rah-git-status-{label}-{}-{id}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir(&path).expect("test directory should be created");
        Self(path)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        for _ in 0..5 {
            if fs::remove_dir_all(&self.0).is_ok() || !self.0.exists() {
                return;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_rah_execute_fixture"))
}

fn copied_fixture(root: &Path) -> PathBuf {
    let extension = fixture()
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| format!(".{value}"))
        .unwrap_or_default();
    let copy = root.join(format!("configured-git{extension}"));
    fs::copy(fixture(), &copy).expect("fixture executable should be copied");
    copy
}

fn create_repository(root: &Path, name: &str) -> PathBuf {
    let repository = root.join(name);
    fs::create_dir(&repository).expect("repository root should be created");
    create_dot_git_directory(&repository);
    repository
}

fn create_dot_git_directory(repository: &Path) {
    let dot_git = repository.join(".git");
    fs::create_dir(&dot_git).expect(".git directory should be created");
    fs::write(dot_git.join("HEAD"), "ref: refs/heads/main\n").expect("HEAD should be created");
}

fn json_content(output: &rah_protocol::ToolOutput) -> &Value {
    let [ToolContent::Json(value)] = output.content.as_slice() else {
        panic!("Git status output should contain one JSON value")
    };
    value
}

#[tokio::test]
async fn definition_input_and_exact_process_are_capability_specific() {
    let root = TestDirectory::new("definition");
    let repository = create_repository(&root.0, "repository");
    let tool = GitStatusTool::new(fixture(), &repository).expect("Git policy should be valid");
    let definition = tool.definition();

    assert_eq!(definition.name, ToolName::new(GIT_STATUS_TOOL_NAME));
    assert_eq!(definition.permission, PermissionLevel::Execute);
    assert_eq!(
        definition.input_schema,
        json!({
            "type": "object",
            "properties": {},
            "additionalProperties": false
        })
    );
    let mut registry = ToolRegistry::new();
    registry
        .register(Arc::new(
            GitStatusTool::new(fixture(), &repository).expect("Git policy should be valid"),
        ))
        .expect("Git status capability should register");
    assert_eq!(registry.definitions(), vec![definition]);
    assert!(
        registry
            .definitions()
            .iter()
            .all(|registered| registered.name.as_str() != "shell.exec")
    );

    for forbidden in [
        json!({"executable": "git"}),
        json!({"program": "git"}),
        json!({"command": "git status"}),
        json!({"args": ["status"]}),
        json!({"argv": ["status"]}),
        json!({"cwd": "."}),
        json!({"repo": "."}),
        json!({"repository": "."}),
        json!({"path": ".."}),
        json!({"env": {"GIT_DIR": "elsewhere"}}),
        json!({"environment": {"PATH": "attacker"}}),
        json!({"timeout": 60}),
        json!({"shell": true}),
    ] {
        assert!(matches!(
            tool.execute(ToolInput(forbidden), ToolContext::default())
                .await,
            Err(ToolError::InvalidInput { .. })
        ));
    }

    for command in [
        "add",
        "commit",
        "checkout",
        "switch",
        "restore",
        "reset",
        "clean",
        "stash",
        "merge",
        "rebase",
        "pull",
        "push",
        "fetch",
        "clone",
        "config",
        "remote",
        "tag",
        "worktree",
        "submodule",
    ] {
        assert!(matches!(
            tool.execute(
                ToolInput(json!({"args": [command]})),
                ToolContext::default()
            )
            .await,
            Err(ToolError::InvalidInput { .. })
        ));
    }

    let output = tool
        .execute(ToolInput(json!({})), ToolContext::default())
        .await
        .expect("exact Git status capability should execute");
    let content = json_content(&output);
    assert!(!output.is_error);
    assert_eq!(content["status"], "exited");
    assert_eq!(content["exit_code"], 0);
    assert_eq!(content["timed_out"], false);
    assert_eq!(content["termination_attempted"], false);
    assert_eq!(content["stdout"], " M tracked.txt\n?? untracked.txt\n");
    assert_eq!(content["stderr"], "");
    assert!(serde_json::to_vec(&output).unwrap().len() <= 768 * 1024);

    let audit = fs::read_to_string(repository.join(".rah-git-status-audit"))
        .expect("fixture audit should exist");
    assert!(audit.contains("execution_count=1\n"));
    assert!(audit.contains("argv=status|--porcelain=v1\n"));
    assert!(audit.contains("stdin_bytes=0\n"));
    assert!(audit.contains(&format!(
        "executable={}\n",
        fs::canonicalize(fixture()).unwrap().display()
    )));
    assert!(audit.contains(&format!(
        "cwd={}\n",
        fs::canonicalize(&repository).unwrap().display()
    )));
    #[cfg(windows)]
    assert!(audit.contains("process_in_job=true\n"));
    let environment = audit
        .lines()
        .find_map(|line| line.strip_prefix("environment="))
        .expect("environment audit should exist");
    assert_eq!(
        environment.split('|').count(),
        9,
        "only the fixed Git status environment should be present"
    );
    for expected in [
        "GIT_CONFIG_NOSYSTEM=1",
        "GIT_CONFIG_COUNT=2",
        "GIT_CONFIG_KEY_0=core.fsmonitor",
        "GIT_CONFIG_VALUE_0=false",
        "GIT_CONFIG_KEY_1=core.untrackedCache",
        "GIT_CONFIG_VALUE_1=false",
        "GIT_OPTIONAL_LOCKS=0",
        "GIT_TERMINAL_PROMPT=0",
    ] {
        assert!(environment.contains(expected), "missing {expected}");
    }
    for forbidden in [
        "PATH=",
        "HOME=",
        "USERPROFILE=",
        "GIT_DIR=",
        "GIT_WORK_TREE=",
        "GIT_INDEX_FILE=",
        "GIT_OBJECT_DIRECTORY=",
        "GIT_ALTERNATE_OBJECT_DIRECTORIES=",
        "SSH_AUTH_SOCK=",
        "HTTP_PROXY=",
        "OPENAI_API_KEY=",
    ] {
        assert!(!environment.contains(forbidden), "unexpected {forbidden}");
    }
}

#[test]
fn executable_and_repository_setup_must_be_host_owned_and_absolute() {
    let root = TestDirectory::new("setup-policy");
    let repository = create_repository(&root.0, "repository");
    let fixture_path = fixture();
    let relative_executable = fixture_path.file_name().unwrap();
    let error = GitStatusTool::new(relative_executable, &repository)
        .err()
        .expect("relative executable must be rejected");
    assert!(error.to_string().contains("PATH lookup is disabled"));

    let error = GitStatusTool::new(fixture(), "relative-repository")
        .err()
        .expect("relative repository must be rejected");
    assert!(error.to_string().contains("absolute path"));

    let not_repository = root.0.join("not-repository");
    fs::create_dir(&not_repository).unwrap();
    let error = GitStatusTool::new(fixture(), not_repository)
        .err()
        .expect("directory without .git must be rejected");
    assert!(error.to_string().contains("repository policy"));
}

#[tokio::test]
async fn executable_and_repository_identities_are_revalidated_before_spawn() {
    let root = TestDirectory::new("identity");
    let executable = copied_fixture(&root.0);
    let repository = create_repository(&root.0, "executable-repository");
    let tool = GitStatusTool::new(&executable, &repository).unwrap();
    let mut replacement = fs::read(&executable).unwrap();
    replacement.push(0);
    fs::write(&executable, replacement).unwrap();
    let error = tool
        .execute(ToolInput(json!({})), ToolContext::default())
        .await
        .expect_err("changed executable identity must fail before spawn");
    assert!(error.to_string().contains("executable identity changed"));

    let repository = create_repository(&root.0, "root-repository");
    let tool = GitStatusTool::new(fixture(), &repository).unwrap();
    let original = root.0.join("original-repository");
    fs::rename(&repository, &original).unwrap();
    fs::create_dir(&repository).unwrap();
    create_dot_git_directory(&repository);
    let error = tool
        .execute(ToolInput(json!({})), ToolContext::default())
        .await
        .expect_err("replaced repository root must fail before spawn");
    assert!(
        error
            .to_string()
            .contains("repository root identity changed")
    );

    let repository = create_repository(&root.0, "metadata-repository");
    let tool = GitStatusTool::new(fixture(), &repository).unwrap();
    fs::rename(repository.join(".git"), repository.join(".git-original")).unwrap();
    create_dot_git_directory(&repository);
    let error = tool
        .execute(ToolInput(json!({})), ToolContext::default())
        .await
        .expect_err("replaced repository metadata must fail before spawn");
    assert!(
        error
            .to_string()
            .contains("repository metadata identity changed")
    );
}

#[tokio::test]
async fn worktree_style_git_file_is_supported_and_revalidated() {
    let root = TestDirectory::new("git-file");
    let repository = root.0.join("repository");
    fs::create_dir(&repository).unwrap();
    fs::write(repository.join(".git"), "gitdir: ../metadata\n").unwrap();
    let tool = GitStatusTool::new(fixture(), &repository).unwrap();
    fs::write(repository.join(".git"), "gitdir: ../replacement\n").unwrap();
    let error = tool
        .execute(ToolInput(json!({})), ToolContext::default())
        .await
        .expect_err("changed .git file must fail before spawn");
    assert!(
        error
            .to_string()
            .contains("repository metadata identity changed")
    );
}

#[tokio::test]
#[ignore = "requires RAH_GIT_STATUS_EXECUTABLE to name an absolute native Git executable"]
async fn explicitly_configured_local_git_reports_temporary_repository_status() {
    let configured = std::env::var_os("RAH_GIT_STATUS_EXECUTABLE")
        .expect("RAH_GIT_STATUS_EXECUTABLE must be set for this opt-in test");
    let git = fs::canonicalize(configured).expect("configured Git should canonicalize");
    #[cfg(windows)]
    assert_eq!(
        git.file_name().and_then(|name| name.to_str()),
        Some("git.exe"),
        "the Windows opt-in test requires native git.exe"
    );
    let root = TestDirectory::new("local");
    run_setup(&git, &root.0, &["init", "--quiet"]);
    fs::write(root.0.join("tracked.txt"), "initial\n").unwrap();
    run_setup(&git, &root.0, &["add", "tracked.txt"]);
    run_setup(
        &git,
        &root.0,
        &[
            "-c",
            "user.name=RAH Test",
            "-c",
            "user.email=rah@example.invalid",
            "commit",
            "--quiet",
            "-m",
            "fixture",
        ],
    );
    fs::write(root.0.join("tracked.txt"), "modified\n").unwrap();
    fs::write(root.0.join("untracked.txt"), "untracked\n").unwrap();

    let tool = GitStatusTool::new(&git, &root.0).expect("local Git policy should be valid");
    let output = tool
        .execute(ToolInput(json!({})), ToolContext::default())
        .await
        .expect("configured local Git should execute directly");
    let content = json_content(&output);
    assert!(!output.is_error, "unexpected Git result: {content}");
    assert_eq!(content["exit_code"], 0);
    assert_eq!(content["stderr"], "");
    let lines = content["stdout"]
        .as_str()
        .unwrap()
        .lines()
        .collect::<Vec<_>>();
    assert!(lines.contains(&" M tracked.txt"));
    assert!(lines.contains(&"?? untracked.txt"));
}

fn run_setup(git: &Path, cwd: &Path, args: &[&str]) {
    let status = Command::new(git)
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .status()
        .expect("test harness Git setup should start");
    assert!(status.success(), "test harness Git setup failed: {args:?}");
}
