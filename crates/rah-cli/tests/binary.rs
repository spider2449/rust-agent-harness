//! Runs the `rah` binary itself.

mod common;

use std::process::{Command, Output, Stdio};

use common::TempDir;
use rah_cli::DATA_HOME_ENV;

/// The `rah` binary with its data home in `dir`, never the user's.
fn rah(dir: &TempDir) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_rah"));
    command.stdin(Stdio::null()).env(DATA_HOME_ENV, dir.home());
    command
}

/// `rah run <prompt>` with the mock model, rooted in the workspace.
fn run_mock(dir: &TempDir, prompt: &str, extra: &[&str]) -> Output {
    rah(dir)
        .args(["--model", "mock", "run", prompt])
        .args(extra)
        .arg("--root")
        .arg(dir.workspace())
        .output()
        .unwrap()
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn session_of(stdout: &str) -> String {
    stdout
        .lines()
        .find_map(|line| line.strip_prefix("session "))
        .unwrap()
        .to_owned()
}

fn write_home_config(dir: &TempDir, text: &str) {
    std::fs::create_dir_all(dir.home()).unwrap();
    std::fs::write(dir.home().join("rah.toml"), text).unwrap();
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
fn run_is_guarded_and_resumes_its_session() {
    let dir = TempDir::new("bin-dev");
    let output = run_mock(&dir, "hello", &[]);
    let stdout = text(&output.stdout);
    let stderr = text(&output.stderr);
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
    let session = session_of(&stdout);
    let output = run_mock(&dir, "/notool again", &["--session", &session]);
    let stdout = text(&output.stdout);
    assert!(output.status.success(), "{stdout}");
    assert!(
        stdout.contains("you said: again (6 earlier items)"),
        "{stdout}"
    );
    assert_eq!(jsonl_files(&dir.sessions()), 1);
}

#[test]
fn sessions_list_and_export_need_no_model_and_run_no_turn() {
    let dir = TempDir::new("bin-export");
    let output = run_mock(&dir, "hello", &[]);
    let stdout = text(&output.stdout);
    assert!(output.status.success(), "{stdout}");
    let session = session_of(&stdout);
    // The configuration names a model whose key is not set; reading sessions
    // does not start it.
    write_home_config(&dir, "model = \"anthropic/claude-x\"\n");
    let sessions = |args: &[&str]| {
        let output = rah(&dir)
            .env_remove("ANTHROPIC_API_KEY")
            .arg("sessions")
            .args(args)
            .output()
            .unwrap();
        (
            output.status.code(),
            text(&output.stdout),
            text(&output.stderr),
        )
    };

    let (code, list, stderr) = sessions(&["list"]);
    assert_eq!(code, Some(0), "{list}\n{stderr}");
    assert!(list.starts_with(&session), "{list}");
    assert!(list.contains("hello"), "{list}");
    assert!(!stderr.contains("not active"), "{stderr}");

    let (code, trace, stderr) = sessions(&["export", &session, "--format", "trace"]);
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

    let (code, json, _) = sessions(&["export", &session, "--format", "json"]);
    assert_eq!(code, Some(0), "{json}");
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["session"]["id"], session.as_str());
    // Exporting ran no turn: the records are those of the one prompt.
    let (_, again, _) = sessions(&["export", &session, "--format", "trace"]);
    assert_eq!(again, trace);
    let (code, markdown, _) = sessions(&["export", &session]);
    assert_eq!(code, Some(0));
    assert!(markdown.starts_with("# "), "{markdown}");

    let (code, _, stderr) = sessions(&["export", &session, "--format", "pdf"]);
    assert_eq!(code, Some(2), "{stderr}");
    assert!(stderr.contains("unknown export format `pdf`"), "{stderr}");
}

#[test]
fn summary_describes_a_run() {
    let dir = TempDir::new("bin-summary");
    let summary = dir.0.join("summary.json");
    let summary_arg = summary.display().to_string();
    let output = run_mock(&dir, "hello", &["--summary", &summary_arg]);
    let stdout = text(&output.stdout);
    assert!(output.status.success(), "{stdout}");
    let session = session_of(&stdout);
    let text = std::fs::read_to_string(&summary).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(parsed["session"], session.as_str(), "{text}");
    assert_eq!(parsed["outcome"], "completed", "{text}");
    assert!(parsed["error"].is_null(), "{text}");
    assert!(parsed["model_requests"].as_u64().unwrap() >= 2, "{text}");
    assert_eq!(parsed["tools"]["calls"], 1, "{text}");
    assert_eq!(parsed["tools"]["errors"], 0, "{text}");
    assert_eq!(parsed["tools"]["unrecorded"], 0, "{text}");
    assert!(parsed["wall_ms"].is_u64(), "{text}");
    assert!(parsed["usage"]["reports"].is_u64(), "{text}");
}

#[test]
fn a_composition_without_policy_warns_that_it_is_unguarded() {
    let dir = TempDir::new("bin-minimal");
    write_home_config(&dir, "extends = \"minimal\"\n");
    let output = rah(&dir)
        .args(["run", "hello", "--root"])
        .arg(dir.workspace())
        .output()
        .unwrap();
    let stdout = text(&output.stdout);
    let stderr = text(&output.stderr);
    assert!(output.status.success(), "{stdout}\n{stderr}");
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
    let stderr = text(&output.stderr);
    assert_eq!(output.status.code(), Some(2), "{stderr}");
    assert!(stderr.contains("refusing to run unguarded"), "{stderr}");
}

#[test]
fn config_files_compose_plugins_and_unknown_plugins_fail() {
    let dir = TempDir::new("bin-config");
    let sessions = dir
        .0
        .join("elsewhere")
        .display()
        .to_string()
        .replace('\\', "/");
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
    let stdout = text(&output.stdout);
    assert!(output.status.success(), "{stdout}");
    assert!(
        stdout.contains("[custom] you said: no tools here"),
        "{stdout}"
    );
    assert_eq!(jsonl_files(&dir.0.join("elsewhere")), 1);

    std::fs::write(&config, "[[plugin]]\nuse = \"model-gpt9\"\n").unwrap();
    let output = rah(&dir).arg("--config").arg(&config).output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(text(&output.stderr).contains("unknown plugin `model-gpt9`"));

    let output = rah(&dir)
        .args(["--model", "nope/x", "run", "hi"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(text(&output.stderr).contains("unknown model provider `nope`"));
}

#[test]
fn data_lives_in_the_data_home_whatever_the_current_directory() {
    let dir = TempDir::new("bin-data-home");
    let output = rah(&dir)
        .args(["--model", "mock", "run", "hello", "--root"])
        .arg(dir.workspace())
        .current_dir(dir.workspace())
        .output()
        .unwrap();
    let stderr = text(&output.stderr);
    assert!(output.status.success(), "{stderr}");
    assert_eq!(jsonl_files(&dir.sessions()), 1);
    assert!(!dir.workspace().join(".rah").exists());

    // `--data-dir` moves it; a relative one is relative to where `rah` runs.
    let output = rah(&dir)
        .args(["--model", "mock", "run", "hello", "--data-dir", "data"])
        .args(["--root", "."])
        .current_dir(dir.workspace())
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(
        jsonl_files(&dir.workspace().join("data").join("sessions")),
        1
    );
    assert_eq!(jsonl_files(&dir.sessions()), 1);
}

#[test]
fn rah_toml_in_the_data_home_runs_without_config() {
    let dir = TempDir::new("bin-user-config");
    write_home_config(
        &dir,
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
    );
    let output = rah(&dir).current_dir(dir.workspace()).output().unwrap();
    let stdout = text(&output.stdout);
    let stderr = text(&output.stderr);
    assert!(output.status.success(), "{stdout}\n{stderr}");
    assert!(stdout.contains("[mine] you said: hello"), "{stdout}");
    assert!(stderr.contains("rah.toml"), "{stderr}");
    assert_eq!(jsonl_files(&dir.sessions()), 1);

    // `--config` wins over it.
    let other = dir.0.join("other.toml");
    std::fs::write(&other, "model = \"mock\"\n").unwrap();
    let output = rah(&dir)
        .arg("--config")
        .arg(&other)
        .args(["run", "/notool hello", "--root", "."])
        .current_dir(dir.workspace())
        .output()
        .unwrap();
    let stdout = text(&output.stdout);
    assert!(output.status.success(), "{stdout}");
    assert!(!stdout.contains("[mine]"), "{stdout}");
}

#[test]
fn config_init_writes_user_keys_that_run_and_never_overwrites() {
    let dir = TempDir::new("bin-init-config");
    let output = rah(&dir)
        .args([
            "config",
            "init",
            "--model",
            "mock",
            "--permissions",
            "auto-edit",
        ])
        .args(["--with", "todo"])
        .output()
        .unwrap();
    let stderr = text(&output.stderr);
    assert!(output.status.success(), "{stderr}");
    let path = dir.home().join("rah.toml");
    let written = std::fs::read_to_string(&path).unwrap();
    assert!(written.contains("model = \"mock\""), "{written}");
    assert!(written.contains("permissions = \"auto-edit\""), "{written}");
    assert!(written.contains("features = [\"todo\"]"), "{written}");
    assert!(!written.contains("[[plugin]]"), "{written}");

    // The written file is what runs, and `config show` describes it.
    let output = rah(&dir)
        .args(["run", "hello", "--root"])
        .arg(dir.workspace())
        .output()
        .unwrap();
    let stdout = text(&output.stdout);
    let stderr = text(&output.stderr);
    assert!(output.status.success(), "{stdout}\n{stderr}");
    assert!(stderr.contains("rah.toml"), "{stderr}");
    assert!(!stderr.contains("UNGUARDED"), "{stderr}");
    assert!(
        stdout.contains("[model-mock] echo returned: hello"),
        "{stdout}"
    );
    let output = rah(&dir).args(["config", "show"]).output().unwrap();
    let shown = text(&output.stdout);
    assert!(output.status.success(), "{shown}");
    assert!(shown.contains("model:       mock"), "{shown}");
    assert!(shown.contains("permissions: auto-edit"), "{shown}");
    assert!(shown.contains("(file)"), "{shown}");
    assert!(shown.contains("features:    todo"), "{shown}");

    // A second run leaves the file alone.
    std::fs::write(&path, "# mine\n").unwrap();
    let output = rah(&dir)
        .args(["config", "init", "--model", "mock"])
        .output()
        .unwrap();
    let stderr = text(&output.stderr);
    assert_eq!(output.status.code(), Some(2), "{stderr}");
    assert!(stderr.contains("already exists"), "{stderr}");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "# mine\n");

    // --config names another target; a bad model or none writes nothing.
    let other = dir.0.join("other.toml");
    let output = rah(&dir)
        .args(["config", "init", "--model", "deepseek", "--config"])
        .arg(&other)
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(
        std::fs::read_to_string(&other)
            .unwrap()
            .contains("deepseek")
    );
    let missing = dir.0.join("missing.toml");
    for args in [&["--model", "nope/x"][..], &[]] {
        let output = rah(&dir)
            .args(["config", "init"])
            .args(args)
            .arg("--config")
            .arg(&missing)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        assert!(!missing.exists());
    }
}

#[test]
fn config_edit_runs_the_editor_on_the_file() {
    let dir = TempDir::new("bin-config-edit");
    let output = rah(&dir).args(["config", "edit"]).output().unwrap();
    let stderr = text(&output.stderr);
    assert_eq!(output.status.code(), Some(2), "{stderr}");
    assert!(stderr.contains("does not exist"), "{stderr}");

    // The "editor" is `rah config check --config`, so the file's path is its
    // last argument and its exit code says whether the file was checked.
    write_home_config(&dir, "model = \"mock\"\n");
    let editor = format!("{} config check --config", env!("CARGO_BIN_EXE_rah"));
    let output = rah(&dir)
        .args(["config", "edit"])
        .env("VISUAL", &editor)
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", text(&output.stderr));
    assert!(text(&output.stdout).contains("configuration ok"));
}

#[test]
fn root_reaches_the_workspace_plugins_that_features_mount() {
    let dir = TempDir::new("bin-root-features");
    let output = rah(&dir)
        .args(["--model", "mock", "--root", "workspace-x"])
        .args([
            "--with",
            "format",
            "--with",
            "lsp",
            "config",
            "show",
            "--plugins",
        ])
        .output()
        .unwrap();
    let stdout = text(&output.stdout);
    let stderr = text(&output.stderr);
    assert!(output.status.success(), "{stdout}\n{stderr}");
    for plugin in ["format-on-write", "lsp-diagnostics"] {
        let at = stdout
            .find(&format!("  {plugin}  <- feature"))
            .unwrap_or_else(|| panic!("{plugin} is not mounted: {stdout}"));
        let section = stdout[at..].lines().take(2).collect::<Vec<_>>().join("\n");
        assert!(section.contains("workspace-x"), "{plugin}: {section}");
    }
    assert!(stdout.contains("(command line)"), "{stdout}");
}

#[test]
fn a_user_file_that_extends_a_preset_runs_with_only_its_changes() {
    let dir = TempDir::new("bin-extends");
    write_home_config(
        &dir,
        "extends = \"dev\"\n\n[[plugin]]\nuse = \"model-mock\"\nconfig = { label = \"mine\" }\n",
    );
    let output = rah(&dir)
        .args(["run", "hello", "--root"])
        .arg(dir.workspace())
        .output()
        .unwrap();
    let stdout = text(&output.stdout);
    let stderr = text(&output.stderr);
    assert!(output.status.success(), "{stdout}\n{stderr}");
    assert!(stdout.contains("[mine] echo returned: hello"), "{stdout}");
    // The preset's policy is still there, so the run is guarded.
    assert!(!stderr.contains("UNGUARDED"), "{stderr}");

    let output = rah(&dir)
        .args(["config", "show", "--plugins"])
        .output()
        .unwrap();
    let stdout = text(&output.stdout);
    assert!(output.status.success(), "{stdout}");
    assert!(stdout.contains("extends:     preset `dev`"), "{stdout}");
    assert!(
        stdout.contains("model-mock  <- changed by the file"),
        "{stdout}"
    );
}

#[test]
fn config_check_reports_every_problem_and_starts_nothing() {
    let dir = TempDir::new("bin-check-config");
    let output = rah(&dir)
        .args(["--model", "mock", "config", "check"])
        .output()
        .unwrap();
    let stdout = text(&output.stdout);
    assert!(output.status.success(), "{stdout}");
    assert!(stdout.contains("configuration ok: 18 plugins"), "{stdout}");

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
        .args(["config", "check"])
        .output()
        .unwrap();
    let stderr = text(&output.stderr);
    assert_eq!(output.status.code(), Some(2), "{stderr}");
    assert!(stderr.contains("nothing was mounted"), "{stderr}");
    assert!(stderr.contains("loop-react: "), "{stderr}");
    assert!(stderr.contains("max_roundz"), "{stderr}");
    // The process plugin has no id; its program is never started.
    assert!(stderr.contains("needs an `id`"), "{stderr}");
    assert!(!stderr.contains("cannot start"), "{stderr}");

    // A start with the same file stops at the same problems.
    let output = rah(&dir).arg("--config").arg(&config).output().unwrap();
    let stderr = text(&output.stderr);
    assert_eq!(output.status.code(), Some(2), "{stderr}");
    assert!(stderr.contains("max_roundz"), "{stderr}");
    assert!(stderr.contains("needs an `id`"), "{stderr}");
}

#[test]
fn config_show_is_short_unless_plugins_are_asked_for() {
    let dir = TempDir::new("bin-show-config");
    write_home_config(&dir, "extends = \"minimal\"\n");
    let show = |extra: &[&str]| {
        let output = rah(&dir)
            // `minimal` has no `tools-shell`, so the feature mounts it.
            .args(["--with", "shell=git", "config", "show"])
            .args(extra)
            .output()
            .unwrap();
        assert!(output.status.success(), "{}", text(&output.stderr));
        text(&output.stdout)
    };
    let short = show(&[]);
    assert!(short.contains("rah.toml"), "{short}");
    assert!(short.contains("features:    shell=git"), "{short}");
    assert!(short.contains("UNGUARDED"), "{short}");
    assert!(short.contains("web ui overrides: none"), "{short}");
    assert!(!short.contains("tools-shell"), "{short}");
    let long = show(&["--plugins"]);
    assert!(long.contains("tools-shell  <- feature shell=git"), "{long}");
}

#[test]
fn features_lists_the_opt_in_capabilities() {
    let dir = TempDir::new("bin-features");
    let output = rah(&dir).arg("features").output().unwrap();
    let stdout = text(&output.stdout);
    assert!(output.status.success(), "{stdout}");
    assert!(stdout.contains("web-search"), "{stdout}");
}

#[test]
fn a_file_with_user_keys_runs_the_model_and_permissions_it_names() {
    let dir = TempDir::new("bin-user-keys");
    write_home_config(&dir, "model = \"mock\"\npermissions = \"full\"\n");
    let output = rah(&dir)
        .args(["run", "hello", "--root"])
        .arg(dir.workspace())
        .output()
        .unwrap();
    let stdout = text(&output.stdout);
    let stderr = text(&output.stderr);
    assert!(output.status.success(), "{stdout}\n{stderr}");
    assert!(
        stdout.contains("[model-mock] echo returned: hello"),
        "{stdout}"
    );
    assert!(stderr.contains("WARNING: permissions `full`"), "{stderr}");
    assert!(!stderr.contains("UNGUARDED"), "{stderr}");

    // `--permissions` overrides the file for one run.
    let output = rah(&dir)
        .args(["--permissions", "ask", "run", "hello", "--root"])
        .arg(dir.workspace())
        .output()
        .unwrap();
    let stderr = text(&output.stderr);
    assert!(output.status.success(), "{stderr}");
    assert!(!stderr.contains("permissions `full`"), "{stderr}");
}

#[test]
fn a_file_with_user_keys_but_no_model_asks_for_one() {
    let dir = TempDir::new("bin-user-no-model");
    write_home_config(&dir, "permissions = \"ask\"\n");
    let output = rah(&dir).args(["run", "hello"]).output().unwrap();
    let stderr = text(&output.stderr);
    assert_eq!(output.status.code(), Some(2), "{stderr}");
    assert!(stderr.contains("no model is configured"), "{stderr}");

    // `--model` supplies it.
    let output = run_mock(&dir, "hello", &[]);
    let stdout = text(&output.stdout);
    assert!(output.status.success(), "{stdout}");
    assert!(
        stdout.contains("[model-mock] echo returned: hello"),
        "{stdout}"
    );

    let output = rah(&dir)
        .args(["--model", "mock", "--permissions", "yolo", "run", "hello"])
        .output()
        .unwrap();
    let stderr = text(&output.stderr);
    assert_eq!(output.status.code(), Some(2), "{stderr}");
    assert!(
        stderr.contains("--permissions: unknown permissions `yolo`"),
        "{stderr}"
    );
}

#[test]
fn a_first_start_without_a_terminal_explains_how_to_choose_a_model() {
    let dir = TempDir::new("bin-first-start");
    for args in [&["run", "hello"][..], &[], &["web"]] {
        let output = rah(&dir).args(args).output().unwrap();
        let stderr = text(&output.stderr);
        assert_eq!(output.status.code(), Some(2), "{args:?}: {stderr}");
        assert!(stderr.contains("rah config init"), "{stderr}");
        assert!(stderr.contains("--model"), "{stderr}");
        // No model was guessed, so none was started.
        assert!(!stderr.contains("llama"), "{stderr}");
    }
    assert!(!dir.home().join("rah.toml").exists());

    // Nothing to answer with: init asks for --model instead.
    let output = rah(&dir).args(["config", "init"]).output().unwrap();
    let stderr = text(&output.stderr);
    assert_eq!(output.status.code(), Some(2), "{stderr}");
    assert!(
        stderr.contains("needs --model when it cannot ask"),
        "{stderr}"
    );

    // What runs can still be shown, and sessions read.
    let output = rah(&dir).args(["config", "show"]).output().unwrap();
    let shown = text(&output.stdout);
    assert!(output.status.success(), "{shown}");
    assert!(shown.contains("model:       none"), "{shown}");
    let output = rah(&dir).args(["sessions", "list"]).output().unwrap();
    assert!(output.status.success(), "{}", text(&output.stderr));
    assert!(text(&output.stdout).contains("no sessions"));
}
