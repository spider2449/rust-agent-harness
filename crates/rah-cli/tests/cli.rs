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
    assert_eq!(String::from_utf8_lossy(&output.stdout), "echo\n");
    assert!(output.stderr.is_empty());
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
