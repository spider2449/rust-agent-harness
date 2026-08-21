use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use rah_protocol::{PermissionLevel, ToolContent, ToolInput};
use rah_sandbox::OutputLimits;
use rah_tools::{
    HostArgumentPolicy, HostExecutionPolicy, HostExecutionTool, Tool, ToolContext, ToolError,
};
use serde_json::{Value, json};

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new(label: &str) -> Self {
        let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("rah-execute-{label}-{}-{id}", std::process::id()));
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

fn exact_policy(root: &Path, args: &[&str]) -> HostExecutionPolicy {
    HostExecutionPolicy::new(
        fixture(),
        HostArgumentPolicy::Exact(args.iter().map(|value| (*value).to_owned()).collect()),
        root,
        ".",
    )
    .expect("fixture policy should be valid")
}

fn text_tool(root: &Path) -> HostExecutionTool {
    let policy = HostExecutionPolicy::new(
        fixture(),
        HostArgumentPolicy::Text {
            prefix: vec!["echo".to_owned()],
            max_bytes: 64 * 1024,
        },
        root,
        ".",
    )
    .expect("fixture policy should be valid");
    HostExecutionTool::new(
        "process.test.echo",
        "Echoes one literal text argument through the deterministic fixture.",
        policy,
    )
}

fn exact_tool(root: &Path, args: &[&str]) -> HostExecutionTool {
    HostExecutionTool::new(
        "process.test.fixture",
        "Runs one exact deterministic fixture operation.",
        exact_policy(root, args),
    )
}

async fn run(
    tool: &HostExecutionTool,
    input: Value,
) -> Result<rah_protocol::ToolOutput, ToolError> {
    tool.execute(ToolInput(input), ToolContext::default()).await
}

fn json_content(output: &rah_protocol::ToolOutput) -> &Value {
    let [ToolContent::Json(value)] = output.content.as_slice() else {
        panic!("process output should contain one JSON value")
    };
    value
}

#[tokio::test]
async fn canonical_fixture_and_typed_literal_argument_are_enforced() {
    let root = TestDirectory::new("canonical");
    let tool = text_tool(&root.0);
    let metacharacters = "RAH_EXECUTE_OK & | ; $() ` > < \"quoted\"";

    let output = run(&tool, json!({"text": metacharacters}))
        .await
        .expect("configured fixture should run");

    assert_eq!(tool.definition().permission, PermissionLevel::Execute);
    assert_eq!(json_content(&output)["stdout"], metacharacters);
    assert_eq!(json_content(&output)["status"], "exited");
    assert!(!output.is_error);
    assert!(!root.0.join("quoted").exists());
}

#[tokio::test]
async fn audited_echo_keeps_process_evidence_out_of_tool_output() {
    let root = TestDirectory::new("audited-echo");
    let audit_file = root.0.join("audit.txt");
    let count_file = root.0.join("count.txt");
    let policy = HostExecutionPolicy::new(
        fixture(),
        HostArgumentPolicy::Text {
            prefix: vec![
                "audited-echo".to_owned(),
                audit_file.to_string_lossy().into_owned(),
                count_file.to_string_lossy().into_owned(),
            ],
            max_bytes: 64 * 1024,
        },
        &root.0,
        ".",
    )
    .expect("audited fixture policy should be valid");
    let tool = HostExecutionTool::new(
        "process.test.audited_echo",
        "Echoes literal text and records host-only process evidence.",
        policy,
    );

    let output = run(&tool, json!({"text": "AUDIT_OK"}))
        .await
        .expect("audited echo should run");
    let content = json_content(&output);
    assert_eq!(content["stdout"], "AUDIT_OK");
    assert_eq!(content["stderr"], "");
    assert!(!content.to_string().contains("execution_count"));

    let audit = fs::read_to_string(audit_file).expect("host audit should exist");
    assert!(audit.contains("execution_count=1\n"));
    assert!(audit.contains("input=AUDIT_OK\n"));
    assert!(audit.contains("stdin_bytes=0\n"));
    assert_eq!(fs::read_to_string(count_file).unwrap(), "1");
}

#[tokio::test]
async fn model_cannot_replace_program_argv_cwd_environment_or_timeout() {
    let root = TestDirectory::new("model-fields");
    let tool = text_tool(&root.0);

    for forbidden in [
        json!({"text": "ok", "program": "alternate.exe"}),
        json!({"text": "ok", "args": ["exit", "9"]}),
        json!({"text": "ok", "cwd": ".."}),
        json!({"text": "ok", "environment": {"PATH": "attacker"}}),
        json!({"text": "ok", "timeout_ms": 999999}),
        json!({"text": "ok", "command": "cmd.exe /c echo unsafe"}),
    ] {
        assert!(matches!(
            run(&tool, forbidden).await,
            Err(ToolError::InvalidInput { .. })
        ));
    }
}

#[test]
fn relative_path_lookup_and_script_associations_fail_closed() {
    let root = TestDirectory::new("executable-deny");
    let relative = fixture()
        .file_name()
        .expect("fixture has a file name")
        .to_owned();
    let error = HostExecutionPolicy::new(
        relative,
        HostArgumentPolicy::Exact(vec!["echo".to_owned(), "ok".to_owned()]),
        &root.0,
        ".",
    )
    .expect_err("relative executable must not use PATH");
    assert!(error.to_string().contains("PATH lookup is disabled"));

    #[cfg(windows)]
    for extension in ["cmd", "bat", "ps1"] {
        let script = root.0.join(format!("replacement.{extension}"));
        fs::write(&script, "echo unsafe").expect("script fixture should be written");
        let error =
            HostExecutionPolicy::new(script, HostArgumentPolicy::Exact(Vec::new()), &root.0, ".")
                .expect_err("script association must not be invoked");
        assert!(error.to_string().contains("native .exe"));
    }
}

#[tokio::test]
async fn cwd_is_host_fixed_canonical_and_not_repository_inherited() {
    let root = TestDirectory::new("cwd");
    let isolated = root.0.join("isolated");
    fs::create_dir(&isolated).expect("isolated cwd should be created");
    let policy = HostExecutionPolicy::new(
        fixture(),
        HostArgumentPolicy::Exact(vec!["cwd".to_owned()]),
        &root.0,
        "isolated",
    )
    .expect("fixed cwd policy should be valid");
    let tool = HostExecutionTool::new("process.test.cwd", "Audits fixed cwd.", policy);

    let output = run(&tool, json!({})).await.expect("cwd audit should run");
    let observed = fs::canonicalize(json_content(&output)["stdout"].as_str().unwrap())
        .expect("fixture cwd should canonicalize");
    assert_eq!(observed, fs::canonicalize(&isolated).unwrap());
    assert_ne!(observed, fs::canonicalize(".").unwrap());

    for denied in [PathBuf::from(".."), root.0.join("..")] {
        assert!(
            HostExecutionPolicy::new(
                fixture(),
                HostArgumentPolicy::Exact(vec!["cwd".to_owned()]),
                &root.0,
                denied,
            )
            .is_err()
        );
    }
}

#[tokio::test]
async fn child_environment_is_cleared_and_only_host_values_are_visible() {
    let root = TestDirectory::new("environment");
    let tool = exact_tool(&root.0, &["audit-env"]);
    let output = run(&tool, json!({}))
        .await
        .expect("empty environment audit should run");
    let environment = json_content(&output)["stdout"].as_str().unwrap();
    for name in [
        "PATH=",
        "SystemRoot=",
        "AWS_SECRET_ACCESS_KEY=",
        "OPENAI_API_KEY=",
        "HTTP_PROXY=",
        "HTTPS_PROXY=",
        "SSH_AUTH_SOCK=",
    ] {
        assert!(!environment.contains(name), "unexpected variable {name}");
    }

    let mut allowed = BTreeMap::new();
    allowed.insert(
        OsString::from("RAH_FIXED_ALLOWED"),
        OsString::from("visible"),
    );
    #[cfg(windows)]
    allowed.insert(OsString::from("SystemRoot"), OsString::from(r"C:\Windows"));
    let policy = exact_policy(&root.0, &["audit-env"])
        .with_environment(allowed)
        .expect("trusted environment should be valid");
    let tool = HostExecutionTool::new("process.test.env", "Audits exact environment.", policy);
    let output = run(&tool, json!({})).await.unwrap();
    let environment = json_content(&output)["stdout"].as_str().unwrap();
    assert!(environment.contains("RAH_FIXED_ALLOWED=visible"));
    #[cfg(windows)]
    assert!(environment.contains(r"SystemRoot=C:\Windows"));
}

#[tokio::test]
async fn recommended_stream_bounds_succeed_at_the_limit_without_deadlock() {
    let root = TestDirectory::new("stream-limits");
    for (stdout_bytes, stderr_bytes) in [(256 * 1024, 0), (0, 256 * 1024)] {
        let stdout = stdout_bytes.to_string();
        let stderr = stderr_bytes.to_string();
        let tool = exact_tool(&root.0, &["streams", &stdout, &stderr]);
        let output = run(&tool, json!({})).await.expect("limit should succeed");
        assert!(!output.is_error);
        assert_eq!(json_content(&output)["retained_stdout_bytes"], stdout_bytes);
        assert_eq!(json_content(&output)["retained_stderr_bytes"], stderr_bytes);
        assert!(serde_json::to_vec(&output).unwrap().len() <= 768 * 1024);
    }

    let tool = exact_tool(&root.0, &["streams", "65536", "65536"]);
    let output = run(&tool, json!({}))
        .await
        .expect("simultaneous streams should drain");
    assert_eq!(json_content(&output)["retained_stdout_bytes"], 65536);
    assert_eq!(json_content(&output)["retained_stderr_bytes"], 65536);
}

#[tokio::test]
async fn stdout_stderr_and_combined_overflow_are_terminal_and_bounded() {
    let root = TestDirectory::new("overflow");
    let cases = [
        (
            "1025",
            "0",
            "stdout",
            OutputLimits {
                stdout_bytes: 1024,
                stderr_bytes: 1024,
                combined_bytes: 2048,
            },
        ),
        (
            "0",
            "1025",
            "stderr",
            OutputLimits {
                stdout_bytes: 1024,
                stderr_bytes: 1024,
                combined_bytes: 2048,
            },
        ),
        (
            "800",
            "800",
            "combined",
            OutputLimits {
                stdout_bytes: 1024,
                stderr_bytes: 1024,
                combined_bytes: 1500,
            },
        ),
    ];

    for (stdout, stderr, expected_overflow, limits) in cases {
        let policy = exact_policy(&root.0, &["streams", stdout, stderr])
            .with_output_limits(limits)
            .unwrap();
        let tool = HostExecutionTool::new("process.test.overflow", "Tests output bounds.", policy);
        let output = run(&tool, json!({}))
            .await
            .expect("overflow is a tool result");
        let content = json_content(&output);
        assert!(output.is_error);
        assert_eq!(content["status"], "output_limit_exceeded");
        assert_eq!(content["output_overflow"], expected_overflow);
        assert_eq!(content["termination_attempted"], true);
        assert!(content["retained_stdout_bytes"].as_u64().unwrap() <= limits.stdout_bytes as u64);
        assert!(content["retained_stderr_bytes"].as_u64().unwrap() <= limits.stderr_bytes as u64);
        assert!(
            content["retained_stdout_bytes"].as_u64().unwrap()
                + content["retained_stderr_bytes"].as_u64().unwrap()
                <= limits.combined_bytes as u64
        );
    }
}

#[tokio::test]
async fn serialized_tool_output_limit_is_enforced() {
    let root = TestDirectory::new("serialized-limit");
    let policy = exact_policy(&root.0, &["streams", "1024", "0"])
        .with_serialized_output_limit(512)
        .unwrap();
    let tool = HostExecutionTool::new("process.test.serialized", "Tests serialization.", policy);
    let output = run(&tool, json!({})).await.unwrap();

    assert!(output.is_error);
    assert_eq!(
        json_content(&output)["status"],
        "serialized_output_limit_exceeded"
    );
    assert!(serde_json::to_vec(&output).unwrap().len() <= 512);
}

#[tokio::test]
async fn normal_nonzero_stdin_timeout_and_no_retry_are_explicit() {
    let root = TestDirectory::new("outcomes");

    let normal = run(&exact_tool(&root.0, &["echo", "ok"]), json!({}))
        .await
        .unwrap();
    assert_eq!(json_content(&normal)["exit_code"], 0);

    let nonzero = run(&exact_tool(&root.0, &["exit", "7"]), json!({}))
        .await
        .unwrap();
    assert!(nonzero.is_error);
    assert_eq!(json_content(&nonzero)["status"], "nonzero_exit");
    assert_eq!(json_content(&nonzero)["exit_code"], 7);

    let stdin = run(&exact_tool(&root.0, &["stdin-eof"]), json!({}))
        .await
        .unwrap();
    assert_eq!(json_content(&stdin)["stdout"], "0");

    let count_file = root.0.join("count.txt");
    let count = count_file.to_string_lossy().into_owned();
    let policy = exact_policy(&root.0, &["count-delay", &count, "2000"])
        .with_timeout(Duration::from_millis(100))
        .unwrap();
    let tool = HostExecutionTool::new("process.test.timeout", "Tests timeout.", policy);
    let timeout = run(&tool, json!({})).await.unwrap();
    assert!(timeout.is_error);
    assert_eq!(json_content(&timeout)["status"], "timeout");
    assert_eq!(json_content(&timeout)["timed_out"], true);
    assert_eq!(json_content(&timeout)["termination_attempted"], true);
    assert_eq!(fs::read_to_string(count_file).unwrap(), "1");
}

#[tokio::test]
async fn aborting_execution_drops_ownership_and_terminates_the_direct_child() {
    let root = TestDirectory::new("abort-child");
    let pid_file = root.0.join("pid.txt");
    let pid_path = pid_file.to_string_lossy().into_owned();
    let tool = Arc::new(exact_tool(&root.0, &["pid-delay", &pid_path, "30000"]));
    let task = tokio::spawn({
        let tool = Arc::clone(&tool);
        async move { run(&tool, json!({})).await }
    });
    wait_for_file(&pid_file).await;
    let pid = fs::read_to_string(&pid_file)
        .unwrap()
        .parse::<u32>()
        .unwrap();

    task.abort();
    assert!(task.await.is_err());
    wait_for_process_exit(pid).await;
}

#[cfg(any(unix, windows))]
#[tokio::test]
async fn platform_supervisor_terminates_fixture_descendant_best_effort() {
    let root = TestDirectory::new("abort-tree");
    let marker = root.0.join("marker.txt");
    let child_pid_file = root.0.join("child-pid.txt");
    let marker_arg = marker.to_string_lossy().into_owned();
    let child_pid_arg = child_pid_file.to_string_lossy().into_owned();
    let tool = Arc::new(exact_tool(
        &root.0,
        &["spawn-child", &marker_arg, &child_pid_arg, "2000"],
    ));
    let task = tokio::spawn({
        let tool = Arc::clone(&tool);
        async move { run(&tool, json!({})).await }
    });
    wait_for_file(&child_pid_file).await;
    let child_pid = fs::read_to_string(&child_pid_file)
        .unwrap()
        .parse::<u32>()
        .unwrap();

    task.abort();
    assert!(task.await.is_err());
    wait_for_process_exit(child_pid).await;
    tokio::time::sleep(Duration::from_millis(2200)).await;
    assert!(
        !marker.exists(),
        "terminated descendant must not finish work"
    );
}

async fn wait_for_file(path: &Path) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !path.exists() {
        assert!(
            Instant::now() < deadline,
            "fixture audit file was not created"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

async fn wait_for_process_exit(pid: u32) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while process_is_running(pid) {
        assert!(
            Instant::now() < deadline,
            "process {pid} was not terminated"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

#[cfg(windows)]
fn process_is_running(pid: u32) -> bool {
    use windows_sys::Win32::{
        Foundation::{CloseHandle, STILL_ACTIVE},
        System::Threading::{GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION},
    };

    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if process.is_null() {
        return false;
    }
    let mut exit_code = 0;
    let queried = unsafe { GetExitCodeProcess(process, &mut exit_code) };
    unsafe { CloseHandle(process) };
    queried != 0 && exit_code == STILL_ACTIVE as u32
}

#[cfg(unix)]
fn process_is_running(pid: u32) -> bool {
    let result = unsafe { libc::kill(pid as i32, 0) };
    result == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}
