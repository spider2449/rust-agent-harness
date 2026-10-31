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
        stdout.contains("you said: again (5 earlier items)"),
        "{stdout}"
    );
    assert_eq!(jsonl_files(&dir.sessions()), 1);
}

#[test]
fn export_writes_only_the_session_and_runs_no_turn() {
    let dir = TempDir::new("bin-export");
    let run = |args: &[&str]| {
        let output = rah(&dir)
            .args(["--preset", "dev"])
            .args(args)
            .arg("--session-dir")
            .arg(dir.sessions())
            .arg("--root")
            .arg(dir.workspace())
            .output()
            .unwrap();
        (
            output.status.code(),
            String::from_utf8_lossy(&output.stdout).into_owned(),
            String::from_utf8_lossy(&output.stderr).into_owned(),
        )
    };
    let (code, stdout, _) = run(&["--prompt", "hello"]);
    assert_eq!(code, Some(0), "{stdout}");
    let session = stdout
        .lines()
        .find_map(|line| line.strip_prefix("session "))
        .unwrap()
        .to_owned();

    let (code, trace, stderr) = run(&["--session", &session, "--export", "trace"]);
    assert_eq!(code, Some(0), "{trace}\n{stderr}");
    let lines: Vec<&str> = trace.lines().collect();
    assert_eq!(lines.first(), Some(&"[USER] hello"), "{trace}");
    assert!(
        lines.iter().any(|line| line.starts_with("[ACTION] echo(")),
        "{trace}"
    );
    assert!(
        lines.iter().any(|line| line.starts_with("[OBSERVATION] ")),
        "{trace}"
    );
    assert!(
        lines.iter().any(|line| line.starts_with("[ASSISTANT] ")),
        "{trace}"
    );

    let (code, json, _) = run(&["--session", &session, "--export", "json"]);
    assert_eq!(code, Some(0), "{json}");
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["session"]["id"], session.as_str());
    // Exporting ran no turn: the records are those of the one prompt.
    let (_, again, _) = run(&["--session", &session, "--export", "trace"]);
    assert_eq!(again, trace);

    let (code, _, stderr) = run(&["--session", &session, "--export", "pdf"]);
    assert_eq!(code, Some(2), "{stderr}");
    assert!(stderr.contains("unknown export format `pdf`"), "{stderr}");
    let (code, _, stderr) = run(&["--export", "md"]);
    assert_eq!(code, Some(2), "{stderr}");
    assert!(stderr.contains("--session"), "{stderr}");
}

#[test]
fn summary_describes_a_prompt_run() {
    let dir = TempDir::new("bin-summary");
    let summary = dir.0.join("summary.json");
    let output = rah(&dir)
        .args(["--preset", "dev", "--prompt", "hello", "--summary"])
        .arg(&summary)
        .arg("--session-dir")
        .arg(dir.sessions())
        .arg("--root")
        .arg(dir.workspace())
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "{stdout}");
    let session = stdout
        .lines()
        .find_map(|line| line.strip_prefix("session "))
        .unwrap();
    let text = std::fs::read_to_string(&summary).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(parsed["session"], session, "{text}");
    assert_eq!(parsed["outcome"], "completed", "{text}");
    assert!(parsed["error"].is_null(), "{text}");
    assert!(parsed["model_requests"].as_u64().unwrap() >= 2, "{text}");
    assert_eq!(parsed["tools"]["calls"], 1, "{text}");
    assert_eq!(parsed["tools"]["errors"], 0, "{text}");
    assert_eq!(parsed["tools"]["unrecorded"], 0, "{text}");
    assert!(parsed["wall_ms"].is_u64(), "{text}");
    assert!(parsed["usage"]["reports"].is_u64(), "{text}");

    let output = rah(&dir)
        .args(["--preset", "dev", "--summary"])
        .arg(&summary)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("--prompt"));
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

#[test]
fn init_config_writes_a_preset_that_runs_and_never_overwrites() {
    let dir = TempDir::new("bin-init-config");
    let home = dir.0.join("home");
    let output = rah(&dir)
        .args(["--init-config", "--preset", "dev"])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stderr}");
    let path = home.join("rah.toml");
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("use = \"model-mock\""), "{text}");
    assert!(text.contains("# features = ["), "{text}");

    // The written file is what runs without --preset and --config.
    let output = rah(&dir)
        .args(["--prompt", "hello", "--root"])
        .arg(dir.workspace())
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stdout}\n{stderr}");
    assert!(stderr.contains("rah.toml"), "{stderr}");
    assert!(!stderr.contains("UNGUARDED"), "{stderr}");
    assert!(
        stdout.contains("[model-mock] echo returned: hello"),
        "{stdout}"
    );

    // A second run leaves the file alone.
    std::fs::write(&path, "# mine\n").unwrap();
    let output = rah(&dir).arg("--init-config").output().unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(2), "{stderr}");
    assert!(stderr.contains("already exists"), "{stderr}");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "# mine\n");

    // --config names another target; an unknown preset writes nothing.
    let other = dir.0.join("other.toml");
    let output = rah(&dir)
        .arg("--init-config")
        .arg("--config")
        .arg(&other)
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(
        std::fs::read_to_string(&other)
            .unwrap()
            .contains("model-llamacpp")
    );
    let missing = dir.0.join("missing.toml");
    let output = rah(&dir)
        .args(["--init-config", "--preset", "nope", "--config"])
        .arg(&missing)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(!missing.exists());
}

#[test]
fn root_reaches_the_workspace_plugins_that_features_mount() {
    let dir = TempDir::new("bin-root-features");
    let output = rah(&dir)
        .args(["--preset", "dev", "--root", "workspace-x"])
        .args(["--with", "format", "--with", "lsp", "--show-config"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stdout}\n{stderr}");
    for plugin in ["format-on-write", "lsp-diagnostics"] {
        let at = stdout
            .find(&format!("  {plugin}  <- feature"))
            .unwrap_or_else(|| panic!("{plugin} is not mounted: {stdout}"));
        let section = stdout[at..].lines().take(2).collect::<Vec<_>>().join("\n");
        assert!(section.contains("workspace-x"), "{plugin}: {section}");
    }
}

#[test]
fn a_user_file_that_extends_a_preset_runs_with_only_its_changes() {
    let dir = TempDir::new("bin-extends");
    let home = dir.0.join("home");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::write(
        home.join("rah.toml"),
        "extends = \"dev\"\n\n[[plugin]]\nuse = \"model-mock\"\nconfig = { label = \"mine\" }\n",
    )
    .unwrap();
    let output = rah(&dir)
        .args(["--prompt", "hello", "--root"])
        .arg(dir.workspace())
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stdout}\n{stderr}");
    assert!(stdout.contains("[mine] echo returned: hello"), "{stdout}");
    // The preset's policy is still there, so the run is guarded.
    assert!(!stderr.contains("UNGUARDED"), "{stderr}");

    let output = rah(&dir).arg("--show-config").output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "{stdout}");
    assert!(stdout.contains("extends:   preset `dev`"), "{stdout}");
    assert!(
        stdout.contains("model-mock  <- changed by the file"),
        "{stdout}"
    );
}

#[test]
fn check_config_reports_every_problem_and_starts_nothing() {
    let dir = TempDir::new("bin-check-config");
    let output = rah(&dir)
        .args(["--preset", "dev", "--check-config"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "{stdout}");
    assert!(stdout.contains("configuration ok: 17 plugins"), "{stdout}");

    let config = dir.0.join("rah.toml");
    std::fs::write(
        &config,
        r#"
extends = "dev"

[[plugin]]
use = "loop-react"
config = { max_roundz = 4 }

[[plugin]]
use = "process"
config = { program = "rah-no-such-program-x7" }
"#,
    )
    .unwrap();
    let output = rah(&dir)
        .arg("--config")
        .arg(&config)
        .arg("--check-config")
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(2), "{stderr}");
    assert!(stderr.contains("nothing was mounted"), "{stderr}");
    assert!(stderr.contains("loop-react: "), "{stderr}");
    assert!(stderr.contains("max_roundz"), "{stderr}");
    // The process plugin has no id; its program is never started.
    assert!(stderr.contains("needs an `id`"), "{stderr}");
    assert!(!stderr.contains("cannot start"), "{stderr}");

    // A start with the same file stops at the same problems.
    let output = rah(&dir).arg("--config").arg(&config).output().unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(2), "{stderr}");
    assert!(stderr.contains("max_roundz"), "{stderr}");
    assert!(stderr.contains("needs an `id`"), "{stderr}");
}

#[test]
fn show_config_prints_the_plugins_features_and_source() {
    let dir = TempDir::new("bin-show-config");
    let output = rah(&dir)
        .args(["--preset", "dev", "--with", "shell=git", "--show-config"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stdout}\n{stderr}");
    assert!(stdout.contains("source:    --preset `dev`"), "{stdout}");
    assert!(stdout.contains("features:  shell=git"), "{stdout}");
    assert!(
        stdout.contains("tools-shell  <- feature shell=git"),
        "{stdout}"
    );
    assert!(stdout.contains("web ui overrides: none"), "{stdout}");
}
