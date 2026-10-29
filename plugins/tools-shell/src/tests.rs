use std::sync::Mutex;

use rah_api::{
    CancellationToken, Kernel, MountOptions, PluginStatus,
    tools::{Invocation, ToolInvoker},
};
use rah_protocol::{SessionId, ToolCall, ToolCallId, ToolEffect, ToolInput};

use super::*;

/// Records specs and answers with a fixed result.
struct FakeEnv {
    specs: Mutex<Vec<ProgramSpec>>,
    result: Result<ExecOutput, ExecError>,
}

impl FakeEnv {
    fn new(result: Result<ExecOutput, ExecError>) -> Arc<Self> {
        Arc::new(Self {
            specs: Mutex::new(Vec::new()),
            result,
        })
    }

    fn specs(&self) -> Vec<ProgramSpec> {
        self.specs.lock().unwrap().clone()
    }
}

#[async_trait]
impl ExecEnv for FakeEnv {
    async fn run(
        &self,
        spec: ProgramSpec,
        _cancel: CancellationToken,
    ) -> Result<ExecOutput, ExecError> {
        self.specs.lock().unwrap().push(spec);
        self.result.clone()
    }
}

fn exited(code: i32) -> Result<ExecOutput, ExecError> {
    Ok(ExecOutput {
        stdout: b"out".to_vec(),
        stderr: b"err".to_vec(),
        exit_code: Some(code),
        termination: None,
    })
}

fn cx() -> ToolContext {
    ToolContext {
        session: SessionId::new(),
        call: rah_protocol::ToolCallId::new(),
        cancel: CancellationToken::new(),
    }
}

fn tool(env: &Arc<FakeEnv>, config: Value) -> ShellExec {
    ShellExec::new(env.clone(), serde_json::from_value(config).unwrap()).unwrap()
}

#[tokio::test]
async fn passes_a_validated_spec_and_reports_the_exit_code() {
    let env = FakeEnv::new(exited(0));
    let shell = tool(&env, json!({ "max_output_bytes": 100 }));
    let output = shell
        .call(
            json!({ "program": "git", "args": ["status", "--short"], "cwd": "sub", "timeout_ms": 500 }),
            cx(),
        )
        .await
        .unwrap();
    assert!(!output.is_error);
    assert_eq!(
        output.content,
        vec![ToolContent::Json(
            json!({ "exit_code": 0, "stdout": "out", "stderr": "err" })
        )]
    );
    let spec = &env.specs()[0];
    assert_eq!(spec.program, "git");
    assert_eq!(spec.args, ["status", "--short"]);
    assert_eq!(spec.cwd, Some(PathBuf::from("sub")));
    assert_eq!(spec.timeout, Duration::from_millis(500));
    assert_eq!(spec.limits.stdout_bytes, 100);
    assert!(spec.env.is_empty());

    let env = FakeEnv::new(exited(2));
    let output = tool(&env, json!({}))
        .call(json!({ "program": "false" }), cx())
        .await
        .unwrap();
    assert!(output.is_error);
    assert_eq!(
        env.specs()[0].timeout,
        Duration::from_millis(DEFAULT_TIMEOUT_MS)
    );
}

#[tokio::test]
async fn invalid_input_is_rejected_before_the_sandbox() {
    let env = FakeEnv::new(exited(0));
    let shell = tool(
        &env,
        json!({ "allow_programs": ["git"], "default_timeout_ms": 500, "max_timeout_ms": 1000 }),
    );
    for input in [
        json!({ "program": "rm", "args": ["-rf", "."] }),
        json!({ "program": "git", "shell": true }),
        json!({ "program": "git", "timeout_ms": 0 }),
        json!({ "program": "git", "timeout_ms": 1001 }),
        json!({ "args": ["x"] }),
        json!("git status"),
    ] {
        assert!(
            matches!(
                shell.call(input.clone(), cx()).await,
                Err(ToolError::InvalidInput(_))
            ),
            "{input}"
        );
    }
    assert!(env.specs().is_empty());
}

#[test]
fn the_definition_names_its_use_and_the_allowed_programs() {
    let env = FakeEnv::new(exited(0));
    let definition = tool(&env, json!({ "allow_programs": ["git", "cargo"] })).definition();
    assert!(
        definition.description.contains("command-line program"),
        "{}",
        definition.description
    );
    assert!(definition.description.contains("`git pull`"));
    assert!(
        definition
            .description
            .contains("Allowed programs: `git`, `cargo`.")
    );
    assert_eq!(
        definition.input_schema["properties"]["timeout_ms"]["maximum"],
        json!(DEFAULT_MAX_TIMEOUT_MS)
    );

    let open = tool(&env, json!({})).definition();
    assert!(!open.description.contains("Allowed programs"));
    let closed = tool(&env, json!({ "allow_programs": [] })).definition();
    assert!(closed.description.contains("No program is allowed"));
}

#[test]
fn invalid_configuration_is_rejected() {
    let env = FakeEnv::new(exited(0));
    for config in [
        json!({ "default_timeout_ms": 0 }),
        json!({ "default_timeout_ms": 10, "max_timeout_ms": 5 }),
        json!({ "max_output_bytes": 0 }),
    ] {
        let config: ShellConfig = serde_json::from_value(config).unwrap();
        assert!(ShellExec::new(env.clone(), config).is_err());
    }
}

struct Provider(PluginManifest, Arc<FakeEnv>);

#[async_trait]
impl Plugin for Provider {
    fn manifest(&self) -> &PluginManifest {
        &self.0
    }

    async fn apply(&self, scope: &mut Scope) -> Result<(), PluginError> {
        scope.provide(SANDBOX, self.1.clone() as Arc<dyn ExecEnv>)?;
        Ok(())
    }
}

async fn effect_of(result: Result<ExecOutput, ExecError>) -> (ToolEffect, String) {
    let kernel = Kernel::new();
    let shell = Arc::new(ShellToolsPlugin::new("tools-shell"));
    let status = kernel.mount(shell, MountOptions::default()).await.unwrap();
    assert!(matches!(status, PluginStatus::Pending { .. }), "{status:?}");
    let sandbox = Provider(
        PluginManifest::new("sandbox", "1").provides(["sandbox"]),
        FakeEnv::new(result),
    );
    kernel
        .mount(Arc::new(sandbox), MountOptions::default())
        .await
        .unwrap();
    let outcome = ToolInvoker::new(kernel.handle())
        .invoke(
            Invocation {
                session: SessionId::new(),
                call: ToolCall {
                    id: ToolCallId::new(),
                    name: ToolName::new("shell.exec"),
                    input: ToolInput(json!({ "program": "x" })),
                },
                cancel: CancellationToken::new(),
            },
            || {},
        )
        .await;
    let text = match &outcome.output.content[0] {
        ToolContent::Text(text) => text.clone(),
        ToolContent::Json(value) => value.to_string(),
        other => panic!("unexpected content: {other:?}"),
    };
    (outcome.effect, text)
}

#[tokio::test]
async fn effects_follow_what_the_sandbox_reports() {
    assert_eq!(effect_of(exited(1)).await.0, ToolEffect::Completed);

    let (effect, text) = effect_of(Ok(ExecOutput {
        stdout: b"partial".to_vec(),
        stderr: Vec::new(),
        exit_code: None,
        termination: Some(Termination::TimedOut),
    }))
    .await;
    assert_eq!(effect, ToolEffect::Uncertain);
    assert!(
        text.contains("timed out") && text.contains("partial"),
        "{text}"
    );

    let spawn = ExecError::Spawn {
        program: "x".into(),
        message: "not found".into(),
    };
    assert_eq!(effect_of(Err(spawn)).await.0, ToolEffect::None);
    let supervision = ExecError::Supervision("lost".into());
    assert_eq!(effect_of(Err(supervision)).await.0, ToolEffect::Uncertain);
}
