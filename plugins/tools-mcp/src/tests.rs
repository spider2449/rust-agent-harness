//! A scripted MCP server runs in-process behind a fake sandbox whose
//! `spawn` returns in-memory pipes.

use std::sync::{
    Mutex,
    atomic::{AtomicBool, Ordering},
};

use rah_api::{
    Kernel, MountOptions, PluginStatus,
    sandbox::{ExecEnv, ExecError, ExecOutput, ProcessHandle, ProgramSpec, Spawned},
    tools::{Invocation, ToolInvoker},
};
use rah_protocol::{SessionId, ToolCall, ToolCallId, ToolEffect, ToolInput};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use super::*;

/// What the fixture server observed.
#[derive(Default)]
struct Seen {
    cancelled: Mutex<Vec<u64>>,
    pinged_back: AtomicBool,
    spawned: Mutex<Option<SpawnSpec>>,
    terminated: Arc<AtomicBool>,
}

struct FakeSandbox {
    version: &'static str,
    seen: Arc<Seen>,
}

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
        unreachable!("tools-mcp only spawns")
    }

    async fn spawn(&self, spec: SpawnSpec) -> Result<Spawned, ExecError> {
        *self.seen.spawned.lock().unwrap() = Some(spec);
        let (client_in, server_in) = tokio::io::duplex(64 * 1024);
        let (server_out, client_out) = tokio::io::duplex(64 * 1024);
        let stop = CancellationToken::new();
        let done = CancellationToken::new();
        let (version, seen) = (self.version, self.seen.clone());
        let (server_stop, server_done) = (stop.clone(), done.clone());
        tokio::spawn(async move {
            tokio::select! {
                () = serve(server_in, server_out, version, seen) => {}
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
                terminated: self.seen.terminated.clone(),
            }),
        })
    }
}

fn tool(name: &str) -> Value {
    json!({
        "name": name,
        "description": format!("fixture {name}"),
        "inputSchema": { "type": "object" },
        "annotations": { "readOnlyHint": true }
    })
}

/// The scripted server. `echo` pings the client first; `fail` reports a
/// tool error; `slow` never answers; `crash` ends the connection.
async fn serve(
    input: tokio::io::DuplexStream,
    mut output: tokio::io::DuplexStream,
    version: &'static str,
    seen: Arc<Seen>,
) {
    let mut lines = BufReader::new(input).lines();
    let send = async |output: &mut tokio::io::DuplexStream, value: Value| {
        let mut bytes = serde_json::to_vec(&value).unwrap();
        bytes.push(b'\n');
        output.write_all(&bytes).await.unwrap();
    };
    while let Ok(Some(line)) = lines.next_line().await {
        let message: Value = serde_json::from_str(&line).unwrap();
        let id = message.get("id").cloned();
        let method = message.get("method").and_then(Value::as_str).unwrap_or("");
        let result = match method {
            "initialize" => json!({
                "protocolVersion": version,
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "fixture", "version": "1" }
            }),
            "tools/list" if message["params"].get("cursor").is_none() => {
                json!({ "tools": [tool("echo"), tool("fail")], "nextCursor": "page-2" })
            }
            "tools/list" => json!({ "tools": [tool("slow"), tool("crash"), tool("hidden")] }),
            "tools/call" => match message["params"]["name"].as_str().unwrap() {
                "echo" => {
                    send(&mut output, json!({ "jsonrpc": "2.0", "method": "notifications/message", "params": {} })).await;
                    send(
                        &mut output,
                        json!({ "jsonrpc": "2.0", "id": "srv-1", "method": "ping" }),
                    )
                    .await;
                    json!({
                        "content": [
                            { "type": "text", "text": message["params"]["arguments"]["text"] },
                            { "type": "image", "data": "AA==", "mimeType": "image/png" },
                            { "type": "image", "data": "AA==", "mimeType": "image/svg+xml" },
                            { "type": "audio", "data": "AA==", "mimeType": "audio/wav" }
                        ],
                        "structuredContent": { "ok": true }
                    })
                }
                "fail" => {
                    json!({ "content": [{ "type": "text", "text": "disk full" }], "isError": true })
                }
                "slow" => continue,
                _ => return,
            },
            "notifications/cancelled" => {
                let id = message["params"]["requestId"].as_u64().unwrap();
                seen.cancelled.lock().unwrap().push(id);
                continue;
            }
            _ if id == Some(json!("srv-1")) => {
                seen.pinged_back
                    .store(message.get("result") == Some(&json!({})), Ordering::SeqCst);
                continue;
            }
            _ => continue,
        };
        send(
            &mut output,
            json!({ "jsonrpc": "2.0", "id": id, "result": result }),
        )
        .await;
    }
}

struct SandboxPlugin(PluginManifest, Arc<FakeSandbox>);

#[async_trait]
impl Plugin for SandboxPlugin {
    fn manifest(&self) -> &PluginManifest {
        &self.0
    }

    async fn apply(&self, scope: &mut Scope) -> Result<(), PluginError> {
        scope.provide(
            SANDBOX,
            self.1.clone() as Arc<dyn rah_api::sandbox::ExecEnv>,
        )?;
        Ok(())
    }
}

fn config(extra: Value) -> Value {
    let mut config = json!({
        "server": "fixture",
        "program": "fixture-server",
        "args": ["--stdio"],
        "tools": { "echo": "read", "fail": "write", "slow": "write", "crash": "execute" },
        "call_timeout_ms": 300
    });
    for (key, value) in extra.as_object().unwrap() {
        config[key] = value.clone();
    }
    config
}

async fn mount(version: &'static str, config: Value) -> (Kernel, Arc<Seen>, PluginStatus) {
    let kernel = Kernel::new();
    let seen = Arc::new(Seen::default());
    let sandbox = FakeSandbox {
        version,
        seen: seen.clone(),
    };
    kernel
        .mount(
            Arc::new(SandboxPlugin(
                PluginManifest::new("sandbox", "1").provides(["sandbox"]),
                Arc::new(sandbox),
            )),
            MountOptions::default(),
        )
        .await
        .unwrap();
    let status = kernel
        .mount(
            Arc::new(McpToolsPlugin::new("mcp")),
            MountOptions::with_config(config),
        )
        .await;
    // A configuration the check refuses fails like one `apply` refuses.
    let status = match status {
        Ok(status) => status,
        Err(rah_api::kernel::KernelError::InvalidConfig { reason, .. }) => {
            PluginStatus::Failed { message: reason }
        }
        Err(error) => panic!("{error}"),
    };
    (kernel, seen, status)
}

fn invocation(name: &str, input: Value, cancel: CancellationToken) -> Invocation {
    Invocation {
        session: SessionId::new(),
        call: ToolCall {
            id: ToolCallId::new(),
            name: ToolName::new(name),
            input: ToolInput(input),
        },
        cancel,
    }
}

fn text(output: &ToolOutput) -> String {
    match &output.content[0] {
        ToolContent::Text(text) => text.clone(),
        ToolContent::Json(value) => value.to_string(),
        other => panic!("unexpected content: {other:?}"),
    }
}

#[tokio::test]
async fn exposes_only_listed_tools_with_host_permissions() {
    let (kernel, seen, status) = mount(MCP_PROTOCOL_VERSION, config(json!({}))).await;
    assert_eq!(status, PluginStatus::Active);
    let spec = seen.spawned.lock().unwrap().clone().unwrap();
    assert_eq!(
        (spec.program.as_str(), spec.args.as_slice()),
        ("fixture-server", ["--stdio".to_owned()].as_slice())
    );
    let definitions = ToolInvoker::new(kernel.handle()).definitions();
    let names: Vec<(&str, PermissionLevel)> = definitions
        .iter()
        .map(|d| (d.name.as_str(), d.permission))
        .collect();
    assert_eq!(
        names,
        [
            ("mcp.fixture.crash", PermissionLevel::Execute),
            ("mcp.fixture.echo", PermissionLevel::Read),
            ("mcp.fixture.fail", PermissionLevel::Write),
            ("mcp.fixture.slow", PermissionLevel::Write),
        ]
    );
    assert_eq!(definitions[1].description, "fixture echo");
}

#[tokio::test]
async fn calls_map_results_and_answer_server_requests() {
    let (kernel, seen, _) = mount("2025-03-26", config(json!({}))).await;
    let invoker = ToolInvoker::new(kernel.handle());
    let echo = invoker
        .invoke(
            invocation(
                "mcp.fixture.echo",
                json!({ "text": "hi" }),
                CancellationToken::new(),
            ),
            || {},
        )
        .await;
    assert!(!echo.output.is_error, "{:?}", echo.output);
    assert_eq!(
        echo.output.content,
        vec![
            ToolContent::Text("hi".into()),
            ToolContent::Image(rah_protocol::ImageContent {
                media_type: "image/png".into(),
                data: "AA==".into()
            }),
            ToolContent::Text("[image left out: unsupported image type `image/svg+xml`]".into()),
            ToolContent::Json(json!({ "type": "audio", "data": "AA==", "mimeType": "audio/wav" })),
            ToolContent::Json(json!({ "ok": true })),
        ]
    );
    assert_eq!(echo.effect, ToolEffect::None);
    assert!(seen.pinged_back.load(Ordering::SeqCst));

    let fail = invoker
        .invoke(
            invocation("mcp.fixture.fail", json!({}), CancellationToken::new()),
            || {},
        )
        .await;
    assert!(fail.output.is_error);
    assert_eq!(text(&fail.output), "disk full");
    assert_eq!(fail.effect, ToolEffect::Completed);

    let invalid = invoker
        .invoke(
            invocation("mcp.fixture.fail", json!([1]), CancellationToken::new()),
            || {},
        )
        .await;
    assert_eq!(invalid.effect, ToolEffect::None);
    assert!(text(&invalid.output).contains("JSON object"));
}

#[tokio::test]
async fn timeouts_and_cancellation_notify_the_server_and_stay_uncertain() {
    let (kernel, seen, _) = mount(MCP_PROTOCOL_VERSION, config(json!({}))).await;
    let invoker = ToolInvoker::new(kernel.handle());
    let timed_out = invoker
        .invoke(
            invocation("mcp.fixture.slow", json!({}), CancellationToken::new()),
            || {},
        )
        .await;
    assert_eq!(timed_out.effect, ToolEffect::Uncertain);
    assert!(
        text(&timed_out.output).contains("not replayed"),
        "{}",
        text(&timed_out.output)
    );

    let cancel = CancellationToken::new();
    let trigger = cancel.clone();
    let (cancelled, ()) = tokio::join!(
        invoker.invoke(invocation("mcp.fixture.slow", json!({}), cancel), || {}),
        async {
            tokio::time::sleep(Duration::from_millis(50)).await;
            trigger.cancel();
        }
    );
    assert_eq!(cancelled.effect, ToolEffect::Uncertain);
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(seen.cancelled.lock().unwrap().len(), 2);
}

#[tokio::test]
async fn busy_and_crashed_servers() {
    let (kernel, _, _) = mount(
        MCP_PROTOCOL_VERSION,
        config(json!({ "max_outstanding": 1 })),
    )
    .await;
    let invoker = ToolInvoker::new(kernel.handle());
    let (slow, busy) = tokio::join!(
        invoker.invoke(
            invocation("mcp.fixture.slow", json!({}), CancellationToken::new()),
            || {}
        ),
        async {
            tokio::time::sleep(Duration::from_millis(50)).await;
            invoker
                .invoke(
                    invocation(
                        "mcp.fixture.echo",
                        json!({ "text": "x" }),
                        CancellationToken::new(),
                    ),
                    || {},
                )
                .await
        }
    );
    assert_eq!(slow.effect, ToolEffect::Uncertain);
    assert!(
        text(&busy.output).contains("not started"),
        "{}",
        text(&busy.output)
    );
    // The timed-out request keeps its slot until the actor has retired it.
    tokio::time::sleep(Duration::from_millis(50)).await;

    let crash = invoker
        .invoke(
            invocation("mcp.fixture.crash", json!({}), CancellationToken::new()),
            || {},
        )
        .await;
    assert_eq!(crash.effect, ToolEffect::Uncertain);
    assert!(
        text(&crash.output).contains("disconnected"),
        "{}",
        text(&crash.output)
    );
    let after = invoker
        .invoke(
            invocation(
                "mcp.fixture.echo",
                json!({ "text": "x" }),
                CancellationToken::new(),
            ),
            || {},
        )
        .await;
    assert_eq!(after.effect, ToolEffect::None);
    assert!(
        text(&after.output).contains("not started"),
        "{}",
        text(&after.output)
    );
}

#[tokio::test]
async fn unmounting_terminates_the_server() {
    let (kernel, seen, _) = mount(MCP_PROTOCOL_VERSION, config(json!({}))).await;
    assert!(!seen.terminated.load(Ordering::SeqCst));
    kernel.unmount(&PluginId::new("mcp")).await.unwrap();
    assert!(seen.terminated.load(Ordering::SeqCst));
}

#[tokio::test]
async fn handshake_and_configuration_failures_fail_the_plugin() {
    let cases = [
        (
            "1999-01-01",
            config(json!({})),
            "unsupported protocol version",
        ),
        (
            MCP_PROTOCOL_VERSION,
            config(json!({ "tools": { "missing": "read" } })),
            "does not offer",
        ),
        (
            MCP_PROTOCOL_VERSION,
            config(json!({ "tools": {} })),
            "at least one",
        ),
        (
            MCP_PROTOCOL_VERSION,
            config(json!({ "tools": { "bad.name": "read" } })),
            "[A-Za-z0-9_-]",
        ),
        (
            MCP_PROTOCOL_VERSION,
            config(json!({ "server": "Bad" })),
            "`server`",
        ),
        (
            MCP_PROTOCOL_VERSION,
            config(json!({ "call_timeout_ms": 0 })),
            "greater than zero",
        ),
    ];
    for (version, config, expected) in cases {
        let (_kernel, seen, status) = mount(version, config.clone()).await;
        match status {
            PluginStatus::Failed { message } => {
                assert!(message.contains(expected), "{config}: {message}")
            }
            other => panic!("{config}: {other:?}"),
        }
        if seen.spawned.lock().unwrap().is_some() {
            assert!(seen.terminated.load(Ordering::SeqCst), "{config}");
        }
    }
}
