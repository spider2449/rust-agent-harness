use std::process::Command;

fn rah() -> Command {
    Command::new(env!("CARGO_BIN_EXE_rah"))
}

#[test]
fn doctor_reports_healthy_configuration() {
    let output = rah().arg("doctor").output().expect("rah should launch");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "RAH doctor: ok\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn tools_lists_deterministic_registry() {
    let output = rah().arg("tools").output().expect("rah should launch");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "echo\nfs.read\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn run_reads_workspace_manifest_through_fs_tool() {
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("CLI crate is nested under workspace/crates");
    let output = rah()
        .current_dir(workspace)
        .env("RUST_LOG", "rah=debug")
        .args([
            "run",
            "read Cargo.toml and report the workspace package information",
        ])
        .output()
        .expect("rah should launch");
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        format!(
            "workspace version {}, edition 2024\n",
            env!("CARGO_PKG_VERSION")
        )
    );
    assert!(stderr.contains("tool_name=fs.read"), "stderr: {stderr}");
    assert!(
        stderr.contains("tool execution finished"),
        "stderr: {stderr}"
    );
}

#[test]
fn run_streams_deterministic_text() {
    let output = rah()
        .args(["run", "hello from rah"])
        .output()
        .expect("rah should launch");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "hello from rah\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn invalid_command_writes_stderr_and_fails() {
    let output = rah().arg("unknown").output().expect("rah should launch");

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(!output.stderr.is_empty());
}

#[test]
fn debug_logging_contains_runtime_correlation_fields() {
    let output = rah()
        .env("RUST_LOG", "rah=debug")
        .args(["run", "trace me"])
        .output()
        .expect("rah should launch");
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(output.status.success());
    assert!(
        stderr.contains("session_id="),
        "expected tracing on stderr; stdout: {}; stderr: {stderr}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(stderr.contains("request_id="));
    assert!(stderr.contains("model_request_id="));
    assert!(stderr.contains("tool_call_id="));
}
