//! A fake sandbox runs scripted "formatters" in-process. The plugin is
//! mounted in a real kernel and driven through the `tool.after_invoke` hook.

use std::sync::atomic::{AtomicUsize, Ordering};

use rah_api::{
    Kernel, MountOptions, PluginStatus,
    kernel::KernelError,
    sandbox::{ExecOutput, SpawnSpec, Spawned, Termination},
};
use rah_protocol::{
    SessionId, ToolCall, ToolCallId, ToolDefinition, ToolInput, ToolName, ToolOutput,
};
use serde_json::json;

use super::*;

/// Formatters by program name:
/// - `trim`: removes trailing spaces from every line;
/// - `broken`: exits with 1 and says why;
/// - `slow`: is terminated for taking too long;
/// - anything else is not installed.
struct FakeSandbox {
    root: PathBuf,
    calls: Mutex<Vec<ProgramSpec>>,
    runs: AtomicUsize,
}

#[async_trait]
impl ExecEnv for FakeSandbox {
    async fn run(&self, spec: ProgramSpec, _: CancellationToken) -> Result<ExecOutput, ExecError> {
        self.runs.fetch_add(1, Ordering::SeqCst);
        self.calls.lock().unwrap().push(spec.clone());
        let done = |code: i32, stderr: &str| ExecOutput {
            stdout: Vec::new(),
            stderr: stderr.as_bytes().to_vec(),
            exit_code: Some(code),
            termination: None,
        };
        match spec.program.as_str() {
            "trim" => {
                let file = self.root.join(spec.args.last().unwrap());
                let text = std::fs::read_to_string(&file).unwrap();
                let trimmed: Vec<&str> = text.lines().map(str::trim_end).collect();
                std::fs::write(file, trimmed.join("\n") + "\n").unwrap();
                Ok(done(0, ""))
            }
            "broken" => Ok(done(1, "line one\nline\ttwo")),
            "slow" => Ok(ExecOutput {
                termination: Some(Termination::TimedOut),
                ..done(0, "")
            }),
            other => Err(ExecError::Spawn {
                program: other.to_owned(),
                message: "not found".into(),
            }),
        }
    }

    async fn spawn(&self, _: SpawnSpec) -> Result<Spawned, ExecError> {
        unreachable!("format-on-write only runs programs")
    }
}

struct SandboxPlugin(PluginManifest, Arc<FakeSandbox>);

#[async_trait]
impl Plugin for SandboxPlugin {
    fn manifest(&self) -> &PluginManifest {
        &self.0
    }

    async fn apply(&self, scope: &mut Scope) -> Result<(), PluginError> {
        scope.provide(SANDBOX, self.1.clone() as Arc<dyn ExecEnv>)?;
        Ok(())
    }
}

struct Fixture {
    kernel: Kernel,
    sandbox: Arc<FakeSandbox>,
    root: PathBuf,
    status: PluginStatus,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

async fn mount(extra: Value) -> Fixture {
    let root = std::env::temp_dir().join(format!("rah-format-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(root.join("src")).unwrap();
    let root = std::fs::canonicalize(&root).unwrap();
    let sandbox = Arc::new(FakeSandbox {
        root: root.clone(),
        calls: Mutex::new(Vec::new()),
        runs: AtomicUsize::new(0),
    });
    let kernel = Kernel::new();
    kernel
        .mount(
            Arc::new(SandboxPlugin(
                PluginManifest::new("sandbox", "1").provides(["sandbox"]),
                sandbox.clone(),
            )),
            MountOptions::default(),
        )
        .await
        .unwrap();
    let mut config = json!({
        "root": root,
        "defaults": false,
        "formatters": { "trim": { "command": ["trim", "$FILE"], "extensions": [".txt", ".MD"] } },
    });
    for (key, value) in extra.as_object().unwrap() {
        if key == "formatters" {
            for (name, entry) in value.as_object().unwrap() {
                config["formatters"][name] = entry.clone();
            }
        } else {
            config[key] = value.clone();
        }
    }
    let status = match kernel
        .mount(
            Arc::new(FormatOnWritePlugin::new("format")),
            MountOptions::with_config(config),
        )
        .await
    {
        Ok(status) => status,
        Err(KernelError::InvalidConfig { reason, .. }) => PluginStatus::Failed { message: reason },
        Err(error) => panic!("{error}"),
    };
    Fixture {
        kernel,
        sandbox,
        root,
        status,
    }
}

impl Fixture {
    fn write(&self, relative: &str, text: &str) {
        let path = self.root.join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    fn read(&self, relative: &str) -> String {
        std::fs::read_to_string(self.root.join(relative)).unwrap()
    }

    async fn after(
        &self,
        input: Value,
        permission: PermissionLevel,
        effect: ToolEffect,
        is_error: bool,
    ) -> ToolCompletion {
        let completion = ToolCompletion {
            request: rah_api::hooks::ToolRequestContext {
                session: SessionId::new(),
                call: ToolCall {
                    id: ToolCallId::new(),
                    name: ToolName::new("repo.create-file"),
                    input: ToolInput(input),
                },
                definition: ToolDefinition {
                    name: ToolName::new("repo.create-file"),
                    description: String::new(),
                    input_schema: json!({}),
                    permission,
                },
            },
            output: ToolOutput {
                content: vec![ToolContent::Text("done".into())],
                is_error,
            },
            effect,
        };
        self.kernel
            .handle()
            .waterfall::<ToolAfterInvoke>(completion)
            .await
            .unwrap()
    }

    async fn created(&self, path: &str) -> ToolCompletion {
        self.after(
            json!({ "path": path, "content": "" }),
            PermissionLevel::Write,
            ToolEffect::Completed,
            false,
        )
        .await
    }
}

fn appended(completion: &ToolCompletion) -> Option<String> {
    match completion.output.content.get(1) {
        Some(ToolContent::Text(text)) => Some(text.clone()),
        Some(other) => panic!("unexpected content: {other:?}"),
        None => None,
    }
}

#[tokio::test]
async fn a_file_the_formatter_changed_is_reported_and_nothing_else_changes() {
    let fixture = mount(json!({})).await;
    assert_eq!(fixture.status, PluginStatus::Active);
    fixture.write("notes.txt", "one   \ntwo\n");
    let completion = fixture.created("notes.txt").await;
    assert_eq!(fixture.read("notes.txt"), "one\ntwo\n");
    let text = appended(&completion).expect("a note");
    assert!(text.contains("notes.txt (trim)"), "{text}");
    assert!(text.contains("read it again"), "{text}");
    assert_eq!(
        completion.output.content[0],
        ToolContent::Text("done".into())
    );
    assert!(!completion.output.is_error);
    assert_eq!(completion.effect, ToolEffect::Completed);
    let spec = fixture.sandbox.calls.lock().unwrap()[0].clone();
    assert_eq!(
        (spec.program.as_str(), spec.args.as_slice()),
        ("trim", ["notes.txt".to_owned()].as_slice())
    );
}

#[tokio::test]
async fn a_file_that_was_already_formatted_adds_nothing() {
    let fixture = mount(json!({})).await;
    fixture.write("src/clean.txt", "one\ntwo\n");
    let completion = fixture.created("src/clean.txt").await;
    assert_eq!(appended(&completion), None);
    assert_eq!(fixture.sandbox.runs.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn only_completed_successful_writes_are_formatted() {
    let fixture = mount(json!({})).await;
    fixture.write("notes.txt", "one   \n");
    for (permission, effect, is_error) in [
        (PermissionLevel::Read, ToolEffect::None, false),
        (PermissionLevel::Execute, ToolEffect::Completed, false),
        (PermissionLevel::Write, ToolEffect::Uncertain, false),
        (PermissionLevel::Write, ToolEffect::Completed, true),
        (PermissionLevel::Write, ToolEffect::None, false),
    ] {
        let completion = fixture
            .after(json!({ "path": "notes.txt" }), permission, effect, is_error)
            .await;
        assert_eq!(appended(&completion), None, "{permission:?} {effect:?}");
    }
    assert_eq!(fixture.sandbox.runs.load(Ordering::SeqCst), 0);
    assert_eq!(fixture.read("notes.txt"), "one   \n");
}

#[tokio::test]
async fn paths_outside_the_root_or_without_a_formatter_are_ignored() {
    let fixture = mount(json!({})).await;
    fixture.write("code.rs", "x   \n");
    let outside = fixture.root.parent().unwrap().join("outside-format.txt");
    std::fs::write(&outside, "x   \n").unwrap();
    for path in [
        "../outside-format.txt",
        "src/../../outside-format.txt",
        "code.rs",
        "missing.txt",
        "src",
        "",
    ] {
        assert_eq!(appended(&fixture.created(path).await), None, "{path}");
    }
    assert_eq!(fixture.sandbox.runs.load(Ordering::SeqCst), 0);
    assert_eq!(std::fs::read_to_string(&outside).unwrap(), "x   \n");
    let _ = std::fs::remove_file(outside);
}

#[tokio::test]
async fn an_edit_and_a_patch_name_their_files_too() {
    let fixture = mount(json!({})).await;
    fixture.write("a.txt", "a   \n");
    fixture.write("b.txt", "b   \n");
    let edit = fixture
        .after(
            json!({ "edits": [{ "path": "a.txt" }, { "path": "b.txt" }] }),
            PermissionLevel::Write,
            ToolEffect::Completed,
            false,
        )
        .await;
    let text = appended(&edit).unwrap();
    assert!(
        text.contains("a.txt (trim)") && text.contains("b.txt (trim)"),
        "{text}"
    );
    fixture.write("a.txt", "again   \n");
    let patch = fixture
        .after(
            json!({ "patch": "--- a/a.txt\n+++ b/a.txt\n@@ -1 +1 @@\n-x\n+again   \n" }),
            PermissionLevel::Write,
            ToolEffect::Completed,
            false,
        )
        .await;
    assert!(appended(&patch).unwrap().contains("a.txt (trim)"));
}

#[tokio::test]
async fn a_failing_formatter_is_reported_when_the_user_named_it() {
    let fixture = mount(json!({
        "formatters": { "broken": { "command": ["broken", "$FILE"], "extensions": [".bad"] } },
    }))
    .await;
    fixture.write("x.bad", "x\n");
    let text = appended(&fixture.created("x.bad").await).unwrap();
    assert!(
        text.contains("x.bad: `broken` failed: exit code 1: line one line two"),
        "{text}"
    );
    assert!(!text.contains('\t'));
    assert_eq!(fixture.read("x.bad"), "x\n");
}

#[tokio::test]
async fn a_formatter_that_is_missing_is_noted_once_and_not_run_again_soon() {
    let fixture = mount(json!({
        "formatters": { "gone": { "command": ["gone", "$FILE"], "extensions": [".gone"] } },
    }))
    .await;
    fixture.write("x.gone", "x\n");
    let first = appended(&fixture.created("x.gone").await).unwrap();
    assert!(first.contains("`gone` failed"), "{first}");
    assert_eq!(appended(&fixture.created("x.gone").await), None);
    assert_eq!(fixture.sandbox.runs.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn a_slow_formatter_is_reported_as_unfinished() {
    let fixture = mount(json!({
        "formatters": { "slow": { "command": ["slow", "$FILE"], "extensions": [".slow"] } },
        "timeout_ms": 50,
    }))
    .await;
    fixture.write("x.slow", "x\n");
    let text = appended(&fixture.created("x.slow").await).unwrap();
    assert!(text.contains("did not finish in time"), "{text}");
}

#[tokio::test]
async fn a_default_formatter_that_is_not_installed_stays_quiet() {
    let fixture = mount(json!({ "defaults": true })).await;
    fixture.write("main.rs", "fn main(){}\n");
    fixture.write("main.go", "package main\n");
    assert_eq!(appended(&fixture.created("main.rs").await), None);
    assert_eq!(appended(&fixture.created("main.go").await), None);
}

#[tokio::test]
async fn project_formatters_apply_only_where_the_project_configures_them() {
    let fixture = mount(json!({ "defaults": true })).await;
    fixture.write("tool/a.py", "x = 1\n");
    fixture.write("c/a.c", "int x;\n");
    // Neither a Python formatter nor clang-format is configured: nothing runs.
    fixture.created("tool/a.py").await;
    fixture.created("c/a.c").await;
    assert_eq!(fixture.sandbox.runs.load(Ordering::SeqCst), 0);

    fixture.write("pyproject.toml", "[tool.black]\nline-length = 88\n");
    fixture.created("tool/a.py").await;
    assert_eq!(fixture.sandbox.calls.lock().unwrap()[0].program, "black");

    fixture.write("tool/ruff.toml", "");
    fixture.created("tool/a.py").await;
    assert_eq!(fixture.sandbox.calls.lock().unwrap()[1].program, "ruff");

    fixture.write(".clang-format", "BasedOnStyle: LLVM\n");
    fixture.created("c/a.c").await;
    assert_eq!(
        fixture.sandbox.calls.lock().unwrap()[2].program,
        "clang-format"
    );
}

#[tokio::test]
async fn a_configuration_above_the_root_does_not_count() {
    let fixture = mount(json!({ "defaults": true })).await;
    let above = fixture.root.parent().unwrap().join(".clang-format");
    std::fs::write(&above, "x").unwrap();
    fixture.write("c/a.c", "int x;\n");
    fixture.created("c/a.c").await;
    let runs = fixture.sandbox.runs.load(Ordering::SeqCst);
    let _ = std::fs::remove_file(above);
    assert_eq!(runs, 0);
}

#[tokio::test]
async fn invalid_configuration_is_refused() {
    for (extra, expected) in [
        (
            json!({ "formatters": { "x": { "extensions": [".x"] } } }),
            "needs a `command`",
        ),
        (
            json!({ "formatters": { "x": { "command": ["x", "$FILE"] } } }),
            "needs `extensions`",
        ),
        (
            json!({ "formatters": { "x": { "command": ["x"], "extensions": [".x"] } } }),
            "must contain `$FILE`",
        ),
        (
            json!({ "formatters": { "x": { "command": ["x", "$FILE"], "extensions": ["x"] } } }),
            "must start with a dot",
        ),
        (
            json!({ "formatters": { "x": {
                "command": ["x", "$FILE"], "extensions": [".x"], "requires": ["a/b"]
            } } }),
            "without a path",
        ),
        (json!({ "colour": true }), "colour"),
    ] {
        let fixture = mount(extra).await;
        match &fixture.status {
            PluginStatus::Failed { message } => assert!(message.contains(expected), "{message}"),
            other => panic!("expected a failure containing `{expected}`, got {other:?}"),
        }
    }
}

#[test]
fn defaults_can_be_overridden_and_disabled() {
    let configured = serde_json::from_value(json!({
        "rustfmt": { "command": ["rustfmt", "--edition", "2021", "$FILE"] },
        "gofmt": { "disabled": true },
        "ruff": { "disabled": true },
        "black": { "disabled": true },
        "clang-format": { "disabled": true },
    }))
    .unwrap();
    let formatters = build(true, Some(&configured)).unwrap();
    assert_eq!(formatters.len(), 1);
    assert_eq!(formatters[0].command[2], "2021");
    assert!(formatters[0].explicit);
    assert_eq!(build(true, None).unwrap().len(), 5);
    assert!(build(false, None).unwrap().is_empty());
}

#[test]
fn the_config_example_is_a_valid_configuration() {
    let value: Value = serde_json::from_str(FormatOnWritePlugin::CONFIG_EXAMPLE).unwrap();
    assert!(parse_config(&value).is_ok());
}
