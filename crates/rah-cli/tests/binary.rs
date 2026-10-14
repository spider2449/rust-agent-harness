//! Runs the `rah` binary itself.

mod common;

use std::process::{Command, Stdio};

use common::TempDir;
use rah_cli::DATA_HOME_ENV;

/// The `rah` binary with its data home in `dir`, never the user's.
fn rah(dir: &TempDir) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_rah"));
    command
        .stdin(Stdio::null())
        .env(DATA_HOME_ENV, dir.0.join("home"));
    command
}

fn jsonl_files(dir: &std::path::Path) -> usize {
    std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .filter(|e| e.path().extension().is_some_and(|x| x == "jsonl"))
                .count()
        })
        .unwrap_or(0)
}

#[test]
fn dev_preset_runs_a_guarded_prompt_and_resumes_its_session() {
    let dir = TempDir::new("bin-dev");
    let output = rah(&dir)
        .args(["--preset", "dev", "--prompt", "hello", "--session-dir"])
        .arg(dir.sessions())
        .arg("--root")
        .arg(dir.workspace())
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(stdout.contains("[tool] echo"), "{stdout}");
    assert!(
        stdout.contains("[model-mock] echo returned: hello"),
        "{stdout}"
    );
    assert!(!stderr.contains("UNGUARDED"), "{stderr}");
    assert_eq!(jsonl_files(&dir.sessions()), 1);

    // Resume the same session from a new process.
    let session = stdout
        .lines()
        .find_map(|line| line.strip_prefix("session "))
        .unwrap()
        .to_owned();
    let output = rah(&dir)
        .args([
            "--preset",
            "dev",
            "--prompt",
            "/notool again",
            "--session",
            &session,
            "--session-dir",
        ])
        .arg(dir.sessions())
        .arg("--root")
        .arg(dir.workspace())
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "{stdout}");
    assert!(
        stdout.contains("you said: again (4 earlier items)"),
        "{stdout}"
    );
    assert_eq!(jsonl_files(&dir.sessions()), 1);
}

#[test]
fn minimal_preset_warns_that_it_is_unguarded() {
    let dir = TempDir::new("bin-minimal");
    let output = rah(&dir)
        .args(["--preset", "minimal", "--prompt", "hello", "--session-dir"])
        .arg(dir.sessions())
        .arg("--root")
        .arg(dir.workspace())
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "{stdout}
{stderr}"
    );
    assert!(stderr.contains("UNGUARDED"), "{stderr}");
    assert!(stdout.contains("echo returned: hello"), "{stdout}");
}

#[test]
fn an_invalid_policy_refuses_to_start() {
    let dir = TempDir::new("bin-bad-policy");
    let config = dir.0.join("rah.toml");
    std::fs::write(
        &config,
        r#"
[[plugin]]
use = "policy-authority"
config = { tools = { "a*b" = "read" } }

[[plugin]]
use = "frontend-cli"
config = { prompt = "hello" }
"#,
    )
    .unwrap();
    let output = rah(&dir).arg("--config").arg(&config).output().unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(2), "{stderr}");
    assert!(stderr.contains("refusing to run unguarded"), "{stderr}");
}

#[test]
fn config_files_compose_plugins_and_unknown_plugins_fail() {
    let dir = TempDir::new("bin-config");
    let sessions = dir.sessions().display().to_string().replace('\\', "/");
    let config = dir.0.join("rah.toml");
    std::fs::write(
        &config,
        format!(
            r#"
[[plugin]]
use = "session-jsonl"
config = {{ dir = "{sessions}" }}

[[plugin]]
use = "model-mock"
id = "custom-model"
config = {{ label = "custom" }}

[[plugin]]
use = "loop-react"

[[plugin]]
use = "frontend-cli"
config = {{ prompt = "no tools here" }}
"#
        ),
    )
    .unwrap();
    let output = rah(&dir).arg("--config").arg(&config).output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "{stdout}");
    assert!(
        stdout.contains("[custom] you said: no tools here"),
        "{stdout}"
    );

    std::fs::write(&config, "[[plugin]]\nuse = \"model-gpt9\"\n").unwrap();
    let output = rah(&dir).arg("--config").arg(&config).output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("unknown plugin `model-gpt9`"));

    let output = rah(&dir).args(["--preset", "nope"]).output().unwrap();
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn data_lives_in_the_data_home_whatever_the_current_directory() {
    let dir = TempDir::new("bin-data-home");
    let output = rah(&dir)
        .args(["--preset", "dev", "--prompt", "hello", "--root"])
        .arg(dir.workspace())
        .current_dir(dir.workspace())
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stderr}");
    assert_eq!(jsonl_files(&dir.0.join("home").join("sessions")), 1);
    assert!(!dir.workspace().join(".rah").exists());

    // `--data-dir` moves it; a relative one is relative to where `rah` runs.
    let output = rah(&dir)
        .args(["--preset", "dev", "--prompt", "hello", "--data-dir", "data"])
        .args(["--root", "."])
        .current_dir(dir.workspace())
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(
        jsonl_files(&dir.workspace().join("data").join("sessions")),
        1
    );
    assert_eq!(jsonl_files(&dir.0.join("home").join("sessions")), 1);
}

#[test]
fn rah_toml_in_the_data_home_runs_without_preset_or_config() {
    let dir = TempDir::new("bin-user-config");
    let home = dir.0.join("home");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::write(
        home.join("rah.toml"),
        r#"
[[plugin]]
use = "session-jsonl"

[[plugin]]
use = "model-mock"
config = { label = "mine" }

[[plugin]]
use = "loop-react"

[[plugin]]
use = "frontend-cli"
config = { prompt = "/notool hello" }
"#,
    )
    .unwrap();
    let output = rah(&dir).current_dir(dir.workspace()).output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stdout}\n{stderr}");
    assert!(stdout.contains("[mine] you said: hello"), "{stdout}");
    assert!(stderr.contains("rah.toml"), "{stderr}");
    assert_eq!(jsonl_files(&home.join("sessions")), 1);

    // An explicit preset wins over it.
    let output = rah(&dir)
        .args([
            "--preset",
            "dev",
            "--prompt",
            "/notool hello",
            "--root",
            ".",
        ])
        .current_dir(dir.workspace())
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "{stdout}");
    assert!(!stdout.contains("[mine]"), "{stdout}");
}
