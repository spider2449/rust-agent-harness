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
    // `rustc` is a rustup shim. Where rustup lives outside the home directory
    // it needs these to find a toolchain; unset ones are skipped.
    let inherit_env = rah_plugin_sandbox_process::DEFAULT_INHERIT_ENV
        .iter()
        .chain(&["RUSTUP_HOME", "CARGO_HOME", "RUSTUP_TOOLCHAIN"])
        .map(|name| format!("{name:?}"))
        .collect::<Vec<_>>()
        .join(", ");
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
config = {{ root = "{workspace}", inherit_env = [{inherit_env}] }}

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

#[test]
fn the_shell_flag_enables_shell_exec_on_a_preset() {
    let dir = TempDir::new("shell-flag");
    let run_flag_with = |flag_args: &[&str], prompt: &str, answer: &str| {
        let mut child = Command::new(env!("CARGO_BIN_EXE_rah"))
            .args(["--preset", "dev", "--prompt", prompt, "--session-dir"])
            .arg(dir.sessions())
            .arg("--root")
            .arg(dir.workspace())
            .args(flag_args)
            .env(rah_cli::DATA_HOME_ENV, dir.0.join("home"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(answer.as_bytes())
            .unwrap();
        child.wait_with_output().unwrap()
    };

    let run_flag = |flag_args: &[&str], answer: &str| run_flag_with(flag_args, CALL, answer);

    // Without the flag the preset has no `shell.exec`.
    let output = run_flag(&[], "y\n");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("[approve]"), "{stdout}");

    // With the flag the policy asks, then the program runs (the rustup shim
    // may fail without a toolchain in the sandbox, which still means it ran).
    let output = run_flag(&["--with", "shell=rustc"], "y\n");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("[approve] shell.exec"), "{stdout}");
    assert!(stdout.contains(r#""exit_code":"#), "{stdout}");

    // A program outside the list never starts.
    let output = run_flag_with(
        &["--with", "shell=rustc"],
        r#"/call shell.exec {"program":"cargo","args":["--version"]}"#,
        "y\n",
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("is not in this tool's allow_programs"),
        "{stdout}"
    );
}

#[test]
fn features_in_the_distribution_file_enable_shell_exec() {
    let dir = TempDir::new("shell-features-file");
    let toml_path = |path: std::path::PathBuf| path.display().to_string().replace('\\', "/");
    let path = dir.0.join("rah.toml");
    std::fs::write(
        &path,
        format!(
            r#"features = ["shell=rustc"]

[[plugin]]
use = "policy-authority"

[[plugin]]
use = "session-jsonl"
config = {{ dir = "{}" }}

[[plugin]]
use = "sandbox-process"
config = {{ root = "{}" }}

[[plugin]]
use = "model-mock"

[[plugin]]
use = "loop-react"

[[plugin]]
use = "frontend-cli"
"#,
            toml_path(dir.sessions()),
            toml_path(dir.workspace())
        ),
    )
    .unwrap();
    let output = run(&path, CALL, Some("y\n"));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("[approve] shell.exec"), "{stdout}");
    assert!(stdout.contains(r#""exit_code":"#), "{stdout}");
}

#[test]
fn list_features_names_every_feature() {
    let output = Command::new(env!("CARGO_BIN_EXE_rah"))
        .arg("--list-features")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "{stdout}");
    for name in ["shell", "web-search", "compact", "telemetry", "plan"] {
        assert!(stdout.contains(name), "{stdout}");
    }
}
