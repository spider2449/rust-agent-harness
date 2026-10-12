//! P4 acceptance for `sandbox-process` and `tools-shell`: `shell.exec` runs
//! only after the policy's Ask is confirmed on the terminal.

mod common;

use std::{
    io::Write,
    process::{Command, Output, Stdio},
};

use common::TempDir;

const CALL: &str = r#"/call shell.exec {"program":"rustc","args":["--version"]}"#;

fn config(dir: &TempDir) -> std::path::PathBuf {
    let toml_path = |path: std::path::PathBuf| path.display().to_string().replace('\\', "/");
    let sessions = toml_path(dir.sessions());
    let workspace = toml_path(dir.workspace());
    let path = dir.0.join("rah.toml");
    std::fs::write(
        &path,
        format!(
            r#"
[[plugin]]
use = "policy-authority"
config = {{ tools = {{ "shell.exec" = "execute" }} }}

[[plugin]]
use = "session-jsonl"
config = {{ dir = "{sessions}" }}

[[plugin]]
use = "sandbox-process"
config = {{ root = "{workspace}" }}

[[plugin]]
use = "tools-shell"
config = {{ allow_programs = ["rustc"] }}

[[plugin]]
use = "model-mock"

[[plugin]]
use = "loop-react"

[[plugin]]
use = "frontend-cli"
"#
        ),
    )
    .unwrap();
    path
}

/// Runs one prompt with `answer` (if any) on stdin.
fn run(config: &std::path::Path, prompt: &str, answer: Option<&str>) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_rah"))
        .arg("--config")
        .arg(config)
        .args(["--prompt", prompt])
        .stdin(if answer.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    if let Some(answer) = answer {
        child
            .stdin
            .take()
            .unwrap()
            .write_all(answer.as_bytes())
            .unwrap();
    }
    child.wait_with_output().unwrap()
}

fn session_log(dir: &TempDir) -> String {
    std::fs::read_dir(dir.sessions())
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| std::fs::read_to_string(entry.path()).unwrap())
        .collect()
}

#[test]
fn shell_exec_runs_only_after_terminal_approval() {
    let dir = TempDir::new("shell-approved");
    let output = run(&config(&dir), CALL, Some("y\n"));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stdout}\n{stderr}");
    assert!(!stderr.contains("UNGUARDED"), "{stderr}");
    assert!(
        stdout.contains("[approve] shell.exec") && stdout.contains("requires `execute` permission"),
        "{stdout}"
    );
    assert!(stdout.contains("[tool result]"), "{stdout}");
    assert!(stdout.contains(r#""exit_code":0"#), "{stdout}");
    assert!(stdout.contains("rustc "), "{stdout}");
    assert!(session_log(&dir).contains(r#""effect":"completed""#));
}

#[test]
fn a_refused_or_unanswered_approval_denies_the_call() {
    for answer in [Some("n\n"), None] {
        let dir = TempDir::new(if answer.is_some() {
            "shell-no"
        } else {
            "shell-eof"
        });
        let output = run(&config(&dir), CALL, answer);
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("[denied]"), "{stdout}");
        assert!(stdout.contains("denied: rejected by the user"), "{stdout}");
        assert!(!stdout.contains("rustc "), "{stdout}");
        let log = session_log(&dir);
        assert!(!log.contains(r#""effect":"completed""#), "{log}");
    }
}

#[test]
fn programs_outside_allow_programs_never_start() {
    let dir = TempDir::new("shell-not-allowed");
    let prompt = r#"/call shell.exec {"program":"cargo","args":["--version"]}"#;
    let output = run(&config(&dir), prompt, Some("y\n"));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("program `cargo` is not in this tool's allow_programs"),
        "{stdout}"
    );
    assert!(session_log(&dir).contains(r#""effect":"none""#));
}
