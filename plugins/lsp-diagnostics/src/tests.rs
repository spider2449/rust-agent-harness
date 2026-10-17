//! A scripted language server runs in-process behind a fake sandbox whose
//! `spawn` returns in-memory pipes. The plugin is mounted in a real kernel and
//! driven through the `tool.after_invoke` hook.

use std::{
    path::PathBuf,
    sync::{
        Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};

use rah_api::{
    CancellationToken, Kernel, KernelHandle, MountOptions, PluginStatus,
    kernel::KernelError,
    sandbox::{ExecEnv, ExecError, ExecOutput, ProcessHandle, ProgramSpec, SpawnSpec, Spawned},
};
use rah_protocol::{
    SessionId, ToolCall, ToolCallId, ToolDefinition, ToolInput, ToolName, ToolOutput,
};
use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::*;
use crate::client::{next_frame, uri_key};
use crate::server::path_to_uri;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    /// Publishes after every open and change.
    Normal,
    /// Never publishes.
    Silent,
    /// Publishes a stale version first, then two partial results.
    Staged,
    /// Ends the connection when it is asked to initialize.
    Crash,
}

/// What the fixture server observed.
struct Seen {
    mode: Mode,
    messages: Mutex<Vec<(String, Value)>>,
    spawns: AtomicUsize,
    spawned: Mutex<Option<SpawnSpec>>,
    terminated: Arc<AtomicBool>,
    configuration: Mutex<Option<Value>>,
}

impl Seen {
    fn new(mode: Mode) -> Arc<Self> {
        Arc::new(Self {
            mode,
            messages: Mutex::new(Vec::new()),
            spawns: AtomicUsize::new(0),
            spawned: Mutex::new(None),
            terminated: Arc::new(AtomicBool::new(false)),
            configuration: Mutex::new(None),
        })
    }

    fn methods(&self) -> Vec<String> {
        self.messages
            .lock()
            .unwrap()
            .iter()
            .map(|(method, _)| method.clone())
            .collect()
    }

    fn params(&self, method: &str) -> Vec<Value> {
        self.messages
            .lock()
            .unwrap()
            .iter()
            .filter(|(name, _)| name == method)
            .map(|(_, params)| params.clone())
            .collect()
    }
}

struct FakeSandbox(Arc<Seen>);

struct FakeProcess {
    stop: CancellationToken,
    done: CancellationToken,
    terminated: Arc<AtomicBool>,
}

#[async_trait]
impl ProcessHandle for FakeProcess {
    async fn wait(&self) -> Option<i32> {
        self.done.cancelled().await;
        None
    }

    fn terminate(&self) {
        self.terminated.store(true, Ordering::SeqCst);
        self.stop.cancel();
    }

    fn stderr(&self) -> String {
        "fixture stderr".into()
    }
}

impl Drop for FakeProcess {
    fn drop(&mut self) {
        self.terminate();
    }
}

#[async_trait]
impl ExecEnv for FakeSandbox {
    async fn run(&self, _: ProgramSpec, _: CancellationToken) -> Result<ExecOutput, ExecError> {
        unreachable!("lsp-diagnostics only spawns")
    }

    async fn spawn(&self, spec: SpawnSpec) -> Result<Spawned, ExecError> {
        self.0.spawns.fetch_add(1, Ordering::SeqCst);
        *self.0.spawned.lock().unwrap() = Some(spec);
        let (client_in, server_in) = tokio::io::duplex(256 * 1024);
        let (server_out, client_out) = tokio::io::duplex(256 * 1024);
        let stop = CancellationToken::new();
        let done = CancellationToken::new();
        let (server_stop, server_done, seen) = (stop.clone(), done.clone(), self.0.clone());
        tokio::spawn(async move {
            tokio::select! {
                () = serve(server_in, server_out, seen) => {}
                () = server_stop.cancelled() => {}
            }
            server_done.cancel();
        });
        Ok(Spawned {
            stdin: Box::new(client_in),
            stdout: Box::new(client_out),
            process: Box::new(FakeProcess {
                stop,
                done,
                terminated: self.0.terminated.clone(),
            }),
        })
    }
}

async fn send(output: &mut tokio::io::DuplexStream, value: Value) {
    let body = serde_json::to_vec(&value).unwrap();
    let mut bytes = format!("Content-Length: {}\r\n\r\n", body.len()).into_bytes();
    bytes.extend_from_slice(&body);
    output.write_all(&bytes).await.unwrap();
}

fn diagnostic(line: u64, severity: u64, message: &str) -> Value {
    json!({
        "range": {
            "start": { "line": line, "character": 4 },
            "end": { "line": line, "character": 9 }
        },
        "severity": severity,
        "code": "E1",
        "source": "fixture",
        "message": message,
    })
}

/// One diagnostic per line holding `ERROR` or `WARN`.
fn diagnose(text: &str) -> Vec<Value> {
    text.lines()
        .enumerate()
        .filter_map(|(line, content)| {
            if content.contains("ERROR") {
                Some(diagnostic(line as u64, 1, "bad ERROR token"))
            } else if content.contains("WARN") {
                Some(diagnostic(line as u64, 2, "suspicious WARN token"))
            } else {
                None
            }
        })
        .collect()
}

async fn serve(
    mut input: tokio::io::DuplexStream,
    mut output: tokio::io::DuplexStream,
    seen: Arc<Seen>,
) {
    let mut buffer: Vec<u8> = Vec::new();
    let mut chunk = vec![0_u8; 16 * 1024];
    loop {
        while let Some(body) = next_frame(&mut buffer).unwrap() {
            let message: Value = serde_json::from_slice(&body).unwrap();
            let id = message.get("id").cloned();
            let method = message
                .get("method")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned();
            let params = message.get("params").cloned().unwrap_or(Value::Null);
            if method.is_empty() {
                if id == Some(json!("cfg")) {
                    *seen.configuration.lock().unwrap() = message.get("result").cloned();
                }
                continue;
            }
            seen.messages
                .lock()
                .unwrap()
                .push((method.clone(), params.clone()));
            match method.as_str() {
                "initialize" => {
                    if seen.mode == Mode::Crash {
                        return;
                    }
                    send(
                        &mut output,
                        json!({ "jsonrpc": "2.0", "id": id, "result": { "capabilities": {} } }),
                    )
                    .await;
                    send(
                        &mut output,
                        json!({
                            "jsonrpc": "2.0", "id": "cfg", "method": "workspace/configuration",
                            "params": { "items": [{ "section": "a" }, { "section": "b" }] }
                        }),
                    )
                    .await;
                }
                "textDocument/didOpen" | "textDocument/didChange" => {
                    let document = &params["textDocument"];
                    let uri = document["uri"].clone();
                    let version = document["version"].clone();
                    let text = params["textDocument"]["text"]
                        .as_str()
                        .or_else(|| params["contentChanges"][0]["text"].as_str())
                        .unwrap_or("")
                        .to_owned();
                    let publish = |version: Value, diagnostics: Vec<Value>| {
                        json!({
                            "jsonrpc": "2.0",
                            "method": "textDocument/publishDiagnostics",
                            "params": { "uri": uri, "version": version, "diagnostics": diagnostics },
                        })
                    };
                    match seen.mode {
                        Mode::Silent | Mode::Crash => {}
                        Mode::Normal => {
                            send(&mut output, publish(version, diagnose(&text))).await;
                        }
                        Mode::Staged => {
                            send(
                                &mut output,
                                publish(json!(0), vec![diagnostic(0, 1, "stale result")]),
                            )
                            .await;
                            let all = diagnose(&text);
                            send(&mut output, publish(version.clone(), all[..1].to_vec())).await;
                            tokio::time::sleep(Duration::from_millis(60)).await;
                            send(&mut output, publish(version, all)).await;
                        }
                    }
                }
                _ => {}
            }
        }
        match input.read(&mut chunk).await {
            Ok(0) | Err(_) => return,
            Ok(count) => buffer.extend_from_slice(&chunk[..count]),
        }
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
    seen: Arc<Seen>,
    root: PathBuf,
    status: PluginStatus,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn config(root: &Path, extra: Value) -> Value {
    let mut config = json!({
        "root": root,
        "defaults": false,
        "servers": { "fixture": { "command": ["fixture-lsp", "--stdio"], "extensions": [".rs", ".TS"] } },
        "wait_ms": 1500,
    });
    for (key, value) in extra.as_object().unwrap() {
        config[key] = value.clone();
    }
    config
}

async fn mount(mode: Mode, extra: Value) -> Fixture {
    let root = std::env::temp_dir().join(format!("rah-lsp-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(root.join("src")).unwrap();
    let kernel = Kernel::new();
    let seen = Seen::new(mode);
    kernel
        .mount(
            Arc::new(SandboxPlugin(
                PluginManifest::new("sandbox", "1").provides(["sandbox"]),
                Arc::new(FakeSandbox(seen.clone())),
            )),
            MountOptions::default(),
        )
        .await
        .unwrap();
    let status = match kernel
        .mount(
            Arc::new(LspDiagnosticsPlugin::new("lsp")),
            MountOptions::with_config(config(&root, extra)),
        )
        .await
    {
        Ok(status) => status,
        Err(KernelError::InvalidConfig { reason, .. }) => PluginStatus::Failed { message: reason },
        Err(error) => panic!("{error}"),
    };
    Fixture {
        kernel,
        seen,
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

    fn handle(&self) -> KernelHandle {
        self.kernel.handle()
    }

    /// Runs `tool.after_invoke` for a completed call.
    async fn after(
        &self,
        name: &str,
        permission: PermissionLevel,
        input: Value,
        effect: ToolEffect,
        is_error: bool,
    ) -> ToolCompletion {
        let completion = ToolCompletion {
            request: rah_api::hooks::ToolRequestContext {
                session: SessionId::new(),
                call: ToolCall {
                    id: ToolCallId::new(),
                    name: ToolName::new(name),
                    input: ToolInput(input),
                },
                definition: ToolDefinition {
                    name: ToolName::new(name),
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
        self.handle()
            .waterfall::<ToolAfterInvoke>(completion)
            .await
            .unwrap()
    }

    async fn created(&self, path: &str) -> ToolCompletion {
        self.after(
            "repo.create-file",
            PermissionLevel::Write,
            json!({ "path": path, "content": "" }),
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
async fn appends_diagnostics_after_a_write_and_changes_nothing_else() {
    let fixture = mount(Mode::Normal, json!({})).await;
    assert_eq!(fixture.status, PluginStatus::Active);
    fixture.write("src/a.rs", "fn main() {\n    ERROR here\n}\n");
    let completion = fixture.created("src/a.rs").await;
    let text = appended(&completion).expect("diagnostics");
    assert!(
        text.contains("src/a.rs:2:5 error (fixture) [E1]: bad ERROR token"),
        "{text}"
    );
    assert!(text.contains("not instructions"), "{text}");
    assert_eq!(
        completion.output.content[0],
        ToolContent::Text("done".into())
    );
    assert!(!completion.output.is_error);
    assert_eq!(completion.effect, ToolEffect::Completed);

    let spec = fixture.seen.spawned.lock().unwrap().clone().unwrap();
    assert_eq!(
        (spec.program.as_str(), spec.args.as_slice()),
        ("fixture-lsp", ["--stdio".to_owned()].as_slice())
    );
    let initialize = &fixture.seen.params("initialize")[0];
    assert!(
        initialize["rootUri"]
            .as_str()
            .unwrap()
            .starts_with("file:///")
    );
    assert_eq!(
        fixture.seen.methods()[..2],
        ["initialize".to_owned(), "initialized".to_owned()]
    );
    // The server's configuration request was answered, not left hanging.
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(
        *fixture.seen.configuration.lock().unwrap(),
        Some(json!([null, null]))
    );
}

#[tokio::test]
async fn a_clean_file_adds_nothing() {
    let fixture = mount(Mode::Normal, json!({})).await;
    fixture.write("src/a.rs", "fn main() {}\n");
    let completion = fixture.created("src/a.rs").await;
    assert_eq!(appended(&completion), None);
    assert_eq!(completion.output.content.len(), 1);
}

#[tokio::test]
async fn a_second_edit_changes_the_open_document_and_old_errors_go_away() {
    let fixture = mount(Mode::Normal, json!({})).await;
    fixture.write("src/a.rs", "ERROR\n");
    assert!(appended(&fixture.created("src/a.rs").await).is_some());
    fixture.write("src/a.rs", "fixed\n");
    let completion = fixture
        .after(
            "repo.edit-files",
            PermissionLevel::Write,
            json!({ "edits": [{ "path": "src/a.rs", "old": "ERROR", "new": "fixed" }] }),
            ToolEffect::Completed,
            false,
        )
        .await;
    assert_eq!(appended(&completion), None);
    let methods = fixture.seen.methods();
    assert_eq!(
        methods
            .iter()
            .filter(|m| *m == "textDocument/didOpen")
            .count(),
        1
    );
    let changes = fixture.seen.params("textDocument/didChange");
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0]["textDocument"]["version"], 2);
    assert_eq!(changes[0]["contentChanges"][0]["text"], "fixed\n");
    assert_eq!(fixture.seen.params("textDocument/didSave").len(), 2);
    assert_eq!(fixture.seen.spawns.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn only_completed_successful_writes_are_checked() {
    let fixture = mount(Mode::Normal, json!({})).await;
    fixture.write("src/a.rs", "ERROR\n");
    let input = json!({ "path": "src/a.rs" });
    for (permission, effect, is_error) in [
        (PermissionLevel::Read, ToolEffect::None, false),
        (PermissionLevel::Execute, ToolEffect::Completed, false),
        (PermissionLevel::Write, ToolEffect::Uncertain, false),
        (PermissionLevel::Write, ToolEffect::Completed, true),
        (PermissionLevel::Write, ToolEffect::None, false),
    ] {
        let completion = fixture
            .after("tool", permission, input.clone(), effect, is_error)
            .await;
        assert_eq!(
            appended(&completion),
            None,
            "{permission:?} {effect:?} {is_error}"
        );
    }
    assert_eq!(fixture.seen.spawns.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn paths_outside_the_root_or_without_a_server_are_ignored() {
    let fixture = mount(Mode::Normal, json!({})).await;
    fixture.write("src/notes.txt", "ERROR\n");
    std::fs::write(
        fixture.root.parent().unwrap().join("outside-lsp.rs"),
        "ERROR\n",
    )
    .unwrap();
    for path in [
        "../outside-lsp.rs",
        "src/../../outside-lsp.rs",
        "src/notes.txt",
        "src/missing.rs",
        "src",
        "",
    ] {
        let completion = fixture.created(path).await;
        assert_eq!(appended(&completion), None, "{path}");
    }
    let absolute = fixture.root.join("src/notes.txt");
    let completion = fixture.created(&absolute.to_string_lossy()).await;
    assert_eq!(appended(&completion), None);
    assert_eq!(fixture.seen.spawns.load(Ordering::SeqCst), 0);
    let _ = std::fs::remove_file(fixture.root.parent().unwrap().join("outside-lsp.rs"));
}

#[tokio::test]
async fn extensions_match_without_regard_to_case() {
    let fixture = mount(Mode::Normal, json!({})).await;
    fixture.write("web/app.ts", "ERROR\n");
    assert!(appended(&fixture.created("web/app.ts").await).is_some());
    fixture.write("web/UP.RS", "ERROR\n");
    assert!(appended(&fixture.created("web/UP.RS").await).is_some());
}

#[tokio::test]
async fn severity_and_the_limit_bound_what_is_shown() {
    let fixture = mount(
        Mode::Normal,
        json!({ "severity": "warning", "max_diagnostics": 2 }),
    )
    .await;
    fixture.write("src/a.rs", "WARN\nERROR\nWARN\nERROR\n");
    let text = appended(&fixture.created("src/a.rs").await).unwrap();
    // Errors first, then warnings, cut at the limit.
    let lines: Vec<&str> = text.lines().collect();
    assert!(lines[1].contains("src/a.rs:2:5 error"), "{text}");
    assert!(lines[2].contains("src/a.rs:4:5 error"), "{text}");
    assert!(lines[3].contains("(2 more not shown)"), "{text}");
    assert_eq!(lines.len(), 4);

    let errors_only = mount(Mode::Normal, json!({})).await;
    errors_only.write("src/a.rs", "WARN\n");
    assert_eq!(appended(&errors_only.created("src/a.rs").await), None);
}

#[tokio::test]
async fn staged_publications_are_merged_and_stale_ones_ignored() {
    let fixture = mount(Mode::Staged, json!({})).await;
    fixture.write("src/a.rs", "ERROR one\nERROR two\n");
    let text = appended(&fixture.created("src/a.rs").await).unwrap();
    assert!(
        text.contains("src/a.rs:1:5") && text.contains("src/a.rs:2:5"),
        "{text}"
    );
    assert!(!text.contains("stale"), "{text}");
}

#[tokio::test]
async fn a_server_that_stays_silent_is_reported_not_waited_on() {
    let fixture = mount(Mode::Silent, json!({ "wait_ms": 200 })).await;
    fixture.write("src/a.rs", "ERROR\n");
    let started = std::time::Instant::now();
    let text = appended(&fixture.created("src/a.rs").await).unwrap();
    assert!(
        text.contains("src/a.rs: `fixture` did not report within 200 ms"),
        "{text}"
    );
    assert!(started.elapsed() < Duration::from_secs(5));
}

#[tokio::test]
async fn a_server_that_cannot_start_is_noted_once_and_not_retried_soon() {
    let fixture = mount(Mode::Crash, json!({ "start_timeout_ms": 500 })).await;
    fixture.write("src/a.rs", "ERROR\n");
    let first = appended(&fixture.created("src/a.rs").await).unwrap();
    assert!(first.contains("src/a.rs: `fixture`:"), "{first}");
    let second = appended(&fixture.created("src/a.rs").await).unwrap();
    assert!(second.contains("src/a.rs: `fixture`:"), "{second}");
    assert_eq!(fixture.seen.spawns.load(Ordering::SeqCst), 1);
    // The edit itself was reported as it was.
    assert_eq!(
        fixture.created("src/a.rs").await.effect,
        ToolEffect::Completed
    );
}

#[tokio::test]
async fn a_default_server_that_is_not_installed_stays_quiet() {
    struct Refusing;
    #[async_trait]
    impl ExecEnv for Refusing {
        async fn run(&self, _: ProgramSpec, _: CancellationToken) -> Result<ExecOutput, ExecError> {
            unreachable!()
        }
        async fn spawn(&self, spec: SpawnSpec) -> Result<Spawned, ExecError> {
            Err(ExecError::Spawn {
                program: spec.program,
                message: "not found".into(),
            })
        }
    }
    let root = std::env::temp_dir().join(format!("rah-lsp-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("a.rs"), "ERROR\n").unwrap();
    struct Refuser(PluginManifest);
    #[async_trait]
    impl Plugin for Refuser {
        fn manifest(&self) -> &PluginManifest {
            &self.0
        }
        async fn apply(&self, scope: &mut Scope) -> Result<(), PluginError> {
            scope.provide(SANDBOX, Arc::new(Refusing) as Arc<dyn ExecEnv>)?;
            Ok(())
        }
    }
    let kernel2 = Kernel::new();
    kernel2
        .mount(
            Arc::new(Refuser(
                PluginManifest::new("sandbox", "1").provides(["sandbox"]),
            )),
            MountOptions::default(),
        )
        .await
        .unwrap();
    kernel2
        .mount(
            Arc::new(LspDiagnosticsPlugin::new("lsp")),
            MountOptions::with_config(json!({ "root": root })),
        )
        .await
        .unwrap();
    let completion = ToolCompletion {
        request: rah_api::hooks::ToolRequestContext {
            session: SessionId::new(),
            call: ToolCall {
                id: ToolCallId::new(),
                name: ToolName::new("repo.create-file"),
                input: ToolInput(json!({ "path": "a.rs" })),
            },
            definition: ToolDefinition {
                name: ToolName::new("repo.create-file"),
                description: String::new(),
                input_schema: json!({}),
                permission: PermissionLevel::Write,
            },
        },
        output: ToolOutput {
            content: vec![ToolContent::Text("done".into())],
            is_error: false,
        },
        effect: ToolEffect::Completed,
    };
    let after = kernel2
        .handle()
        .waterfall::<ToolAfterInvoke>(completion.clone())
        .await
        .unwrap();
    assert_eq!(after, completion);
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn unmounting_ends_the_server() {
    let fixture = mount(Mode::Normal, json!({})).await;
    fixture.write("src/a.rs", "fine\n");
    fixture.created("src/a.rs").await;
    assert!(!fixture.seen.terminated.load(Ordering::SeqCst));
    fixture.kernel.unmount(&"lsp".into()).await.unwrap();
    for _ in 0..50 {
        if fixture.seen.terminated.load(Ordering::SeqCst) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("the server was not terminated");
}

#[tokio::test]
async fn invalid_configuration_is_refused() {
    for (extra, expected) in [
        (json!({ "severity": "loud" }), "severity"),
        (
            json!({ "servers": { "x": { "extensions": [".x"] } } }),
            "needs a `command`",
        ),
        (
            json!({ "servers": { "x": { "command": ["x"] } } }),
            "needs `extensions`",
        ),
        (
            json!({ "servers": { "x": { "command": ["x"], "extensions": ["rs"] } } }),
            "must start with a dot",
        ),
        (
            json!({ "servers": { "x": { "command": [""], "extensions": [".x"] } } }),
            "needs a `command`",
        ),
        (
            json!({ "servers": { "x": { "command": ["x"], "extensions": [".x"], "initialization": 1 } } }),
            "must be an object",
        ),
        (json!({ "colour": true }), "colour"),
    ] {
        let fixture = mount(Mode::Normal, extra).await;
        match &fixture.status {
            PluginStatus::Failed { message } => assert!(message.contains(expected), "{message}"),
            other => panic!("expected a failure containing `{expected}`, got {other:?}"),
        }
    }
}

#[test]
fn default_servers_can_be_overridden_and_disabled() {
    let configured = serde_json::from_value(json!({
        "rust-analyzer": { "command": ["ra", "--flag"], "initialization": { "check": true } },
        "gopls": { "disabled": true },
        "typescript": { "disabled": true },
        "pyright": { "disabled": true },
        "clangd": { "disabled": true },
    }))
    .unwrap();
    let servers = server::build(true, Some(&configured)).unwrap();
    assert_eq!(servers.len(), 1);
    assert_eq!(servers[0].name, "rust-analyzer");
    assert!(servers[0].explicit);
    assert_eq!(server::build(true, None).unwrap().len(), 5);
    assert!(server::build(false, None).unwrap().is_empty());
}

#[test]
fn the_config_example_is_a_valid_configuration() {
    let value: Value = serde_json::from_str(LspDiagnosticsPlugin::CONFIG_EXAMPLE).unwrap();
    assert!(parse_config(&value).is_ok());
}

#[test]
fn paths_come_from_the_arguments_of_the_write_tools() {
    assert_eq!(
        changed_paths(&json!({ "path": "a.rs", "content": "x" })),
        ["a.rs"]
    );
    assert_eq!(
        changed_paths(
            &json!({ "edits": [{ "path": "a.rs" }, { "path": "b.rs" }, { "path": "a.rs" }] })
        ),
        ["a.rs", "b.rs"]
    );
    let patch = "diff --git a/src/x.rs b/src/x.rs\n--- a/src/x.rs\n+++ b/src/x.rs\t2026\n@@ -1 +1 @@\n-a\n+b\n\
                 --- a/gone.rs\n+++ /dev/null\n@@ -1 +0,0 @@\n-a\n";
    assert_eq!(changed_paths(&json!({ "patch": patch })), ["src/x.rs"]);
    assert!(changed_paths(&json!({ "path": 3, "edits": "no" })).is_empty());
}

#[test]
fn messages_are_one_bounded_line() {
    assert_eq!(flatten("a\nb\tc\u{7}d   e", 100), "a b c d e");
    assert_eq!(flatten(&"x".repeat(50), 10), "xxxxxxxxxx...");
}

#[test]
fn uris_survive_the_spellings_servers_use() {
    assert_eq!(
        path_to_uri(Path::new("/home/a b/c.rs")),
        "file:///home/a%20b/c.rs"
    );
    assert_eq!(
        path_to_uri(Path::new(r"\\?\C:\Users\a\x.rs")),
        "file:///C:/Users/a/x.rs"
    );
    assert_eq!(
        uri_key("file:///c%3A/Users/a/x.rs"),
        uri_key("file:///C:/Users/a/x.rs")
    );
    assert_eq!(uri_key("file:///home/a%20b/c.rs"), "/home/a b/c.rs");
    assert_ne!(uri_key("file:///home/A.rs"), uri_key("file:///home/a.rs"));
}
