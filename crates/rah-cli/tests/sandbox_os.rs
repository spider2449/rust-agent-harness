//! G4 acceptance for `sandbox-os`: `shell.exec` over the confined sandbox
//! writes inside the workspace but not outside it. Off Linux the plugin
//! refuses to start, so the shell tool never runs unconfined.

mod common;

use std::{
    io::Write,
    path::Path,
    process::{Command, Output, Stdio},
};

use common::TempDir;

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
use = "sandbox-os"
config = {{ root = "{workspace}" }}

[[plugin]]
use = "tools-shell"
config = {{ allow_programs = ["touch"] }}

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

/// Runs `touch <target>` through `shell.exec`, approving the call.
fn touch(config: &Path, target: &Path) -> Output {
    let arguments = serde_json::json!({ "program": "touch", "args": [target] });
    let prompt = format!("/call shell.exec {arguments}");
    let mut child = Command::new(env!("CARGO_BIN_EXE_rah"))
        .arg("--config")
        .arg(config)
        .args(["--prompt", &prompt])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"y\n").unwrap();
    child.wait_with_output().unwrap()
}

#[cfg(target_os = "linux")]
#[test]
fn shell_exec_cannot_write_outside_the_workspace() {
    let dir = TempDir::new("sandbox-os");
    let config = config(&dir);
    let inside = dir.workspace().join("inside.txt");
    let output = touch(&config, &inside);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if stderr.contains("landlock") && !stdout.contains("[tool") {
        eprintln!("skipped: this kernel cannot run sandbox-os\n{stderr}");
        return;
    }
    assert!(stdout.contains(r#""exit_code":0"#), "{stdout}\n{stderr}");
    assert!(inside.exists());

    let outside = dir.0.join("outside.txt");
    let output = touch(&config, &outside);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("[tool error]"), "{stdout}");
    assert!(stdout.contains(r#""exit_code":1"#), "{stdout}");
    assert!(stdout.contains("Permission denied"), "{stdout}");
    assert!(!outside.exists());
}

#[cfg(not(target_os = "linux"))]
#[test]
fn sandbox_os_refuses_to_start_and_the_shell_never_runs() {
    let dir = TempDir::new("sandbox-os-unsupported");
    let inside = dir.workspace().join("inside.txt");
    let output = touch(&config(&dir), &inside);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let text = format!("{stdout}\n{stderr}");
    assert!(text.contains("sandbox-os supports only Linux"), "{text}");
    assert!(!stdout.contains("[tool"), "{text}");
    assert!(!inside.exists());
}
