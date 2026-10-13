//! A scripted Streamable HTTP MCP server on a loopback port.

use std::collections::HashMap;

use rah_api::{
    Kernel, MountOptions, PluginStatus,
    tools::{Invocation, ToolInvoker},
};
use rah_protocol::{SessionId, ToolCall, ToolCallId, ToolEffect, ToolInput};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

use super::*;

const TOKEN_VARIABLE: &str = "RAH_TEST_MCP_HTTP_TOKEN";
const TOKEN: &str = "fixture-token-do-not-expose";

/// One request the fixture received.
#[derive(Clone, Debug)]
struct Request {
    method: String,
    headers: HashMap<String, String>,
    body: Value,
}

impl Request {
    fn rpc(&self) -> &str {
        self.body["method"].as_str().unwrap_or_default()
    }
}

enum Reply {
    Json(Value, Option<&'static str>),
    Accepted,
    Status(u16),
    /// SSE events, then the connection closes.
    Events(Vec<Value>),
    /// Never answers.
    Hold,
}

fn tool(name: &str) -> Value {
    json!({ "name": name, "description": format!("{name} tool"), "inputSchema": { "type": "object" } })
}

/// The fixture's behavior for one request.
fn reply(request: &Request, version: &str) -> Reply {
    let id = request.body["id"].clone();
    let result = |result: Value| json!({ "jsonrpc": "2.0", "id": id, "result": result });
    if request.method == "DELETE" {
        return Reply::Status(200);
    }
    if request.body.get("result").is_some() {
        // The client's answer to the fixture's `ping`.
        return Reply::Accepted;
    }
    match request.rpc() {
        "initialize" => Reply::Json(
            result(json!({
                "protocolVersion": version,
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "fixture", "version": "1" }
            })),
            Some("session-1"),
        ),
        "notifications/initialized" | "notifications/cancelled" => Reply::Accepted,
        "tools/list" => Reply::Json(
            result(
                json!({ "tools": [tool("echo"), tool("cut"), tool("slow"), tool("gone"), tool("broken")] }),
            ),
            None,
        ),
        "tools/call" => match request.body["params"]["name"].as_str().unwrap_or_default() {
            "echo" => Reply::Events(vec![
                json!({ "jsonrpc": "2.0", "id": "srv-1", "method": "ping" }),
                json!({ "jsonrpc": "2.0", "method": "notifications/progress", "params": {} }),
                result(json!({
                    "content": [
                        { "type": "text", "text": request.body["params"]["arguments"]["text"] },
                        { "type": "image", "data": "AA==", "mimeType": "image/png" }
                    ]
                })),
            ]),
            "cut" => Reply::Events(vec![
                json!({ "jsonrpc": "2.0", "method": "notifications/progress", "params": {} }),
            ]),
            "slow" => Reply::Hold,
            "gone" => Reply::Status(404),
            _ => Reply::Status(500),
        },
        _ => Reply::Status(400),
    }
}

struct Server {
    origin: String,
    requests: Arc<std::sync::Mutex<Vec<Request>>>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl Server {
    async fn new(version: &'static str) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let requests = Arc::new(std::sync::Mutex::new(Vec::new()));
        let captured = requests.clone();
        let task = tokio::spawn(async move {
            let mut connections = tokio::task::JoinSet::new();
            loop {
                let (mut socket, _) = listener.accept().await.unwrap();
                let captured = captured.clone();
                connections.spawn(async move {
                    let Some(request) = read_request(&mut socket).await else {
                        return;
                    };
                    captured.lock().unwrap().push(request.clone());
                    let (status, headers, body) = match reply(&request, version) {
                        Reply::Json(body, session) => (
                            200,
                            match session {
                                Some(id) => format!(
                                    "Content-Type: application/json\r\nMcp-Session-Id: {id}\r\n"
                                ),
                                None => "Content-Type: application/json\r\n".into(),
                            },
                            body.to_string(),
                        ),
                        Reply::Accepted => (202, String::new(), String::new()),
                        Reply::Status(status) => (status, String::new(), String::new()),
                        Reply::Events(events) => (
                            200,
                            "Content-Type: text/event-stream\r\n".into(),
                            events
                                .iter()
                                .map(|event| format!("event: message\r\ndata: {event}\r\n\r\n"))
                                .collect(),
                        ),
                        Reply::Hold => {
                            tokio::time::sleep(Duration::from_secs(30)).await;
                            return;
                        }
                    };
                    let head = format!(
                        "HTTP/1.1 {status} X\r\n{headers}Content-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    let _ = socket.write_all(head.as_bytes()).await;
                    let _ = socket.write_all(body.as_bytes()).await;
                    let _ = socket.shutdown().await;
                });
            }
        });
        Self {
            origin,
            requests,
            task,
        }
    }

    fn requests(&self) -> Vec<Request> {
        self.requests.lock().unwrap().clone()
    }
}

async fn read_request(socket: &mut tokio::net::TcpStream) -> Option<Request> {
    let mut bytes = Vec::new();
    let end = loop {
        let mut chunk = [0; 4096];
        let n = socket.read(&mut chunk).await.ok()?;
        if n == 0 {
            return None;
        }
        bytes.extend_from_slice(&chunk[..n]);
        if let Some(i) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
            break i + 4;
        }
    };
    let head = String::from_utf8(bytes[..end].to_vec()).ok()?;
    let mut lines = head.lines();
    let method = lines.next()?.split(' ').next()?.to_owned();
    let headers: HashMap<String, String> = lines
        .filter_map(|line| line.split_once(": "))
        .map(|(name, value)| (name.to_ascii_lowercase(), value.to_owned()))
        .collect();
    let length: usize = headers
        .get("content-length")
        .and_then(|n| n.parse().ok())
        .unwrap_or(0);
    while bytes.len() < end + length {
        let mut chunk = [0; 4096];
        let n = socket.read(&mut chunk).await.ok()?;
        if n == 0 {
            return None;
        }
        bytes.extend_from_slice(&chunk[..n]);
    }
    let body = serde_json::from_slice(&bytes[end..end + length]).unwrap_or(Value::Null);
    Some(Request {
        method,
        headers,
        body,
    })
}

fn config(url: &str, extra: Value) -> Value {
    let mut config = json!({
        "server": "web",
        "url": url,
        "tools": { "echo": "read", "cut": "write", "slow": "write", "gone": "write", "broken": "read" },
        "call_timeout_ms": 300
    });
    for (key, value) in extra.as_object().unwrap() {
        config[key] = value.clone();
    }
    config
}

async fn mount(config: Value) -> (Kernel, PluginStatus) {
    let kernel = Kernel::new();
    let status = kernel
        .mount(
            Arc::new(McpHttpToolsPlugin::new("mcp-http")),
            MountOptions::with_config(config),
        )
        .await
        .unwrap();
    (kernel, status)
}

async fn call(kernel: &Kernel, name: &str, input: Value) -> (ToolOutput, ToolEffect) {
    let outcome = ToolInvoker::new(kernel.handle())
        .invoke(
            Invocation {
                session: SessionId::new(),
                call: ToolCall {
                    id: ToolCallId::new(),
                    name: ToolName::new(name),
                    input: ToolInput(input),
                },
                cancel: CancellationToken::new(),
            },
            || {},
        )
        .await;
    (outcome.output, outcome.effect)
}

fn first_text(output: &ToolOutput) -> String {
    match &output.content[0] {
        ToolContent::Text(text) => text.clone(),
        other => panic!("unexpected content: {other:?}"),
    }
}

#[test]
fn endpoints_are_https_or_loopback_http() {
    for url in [
        "https://mcp.example.com/mcp",
        "https://203.0.113.5:8443/v1/mcp?team=a",
        "http://127.0.0.1:9/mcp",
        "http://localhost:9/mcp",
        "http://[::1]:9/mcp",
    ] {
        assert!(http::endpoint(url).is_ok(), "{url}");
    }
    for (url, expected) in [
        ("http://192.168.1.5/mcp", "loopback"),
        ("http://mcp.example.com/mcp", "loopback"),
        ("ftp://127.0.0.1/mcp", "`https`"),
        ("https://user:secret@mcp.example.com/mcp", "user name"),
        ("https://mcp.example.com/mcp#x", "fragment"),
        ("not a url", "valid URL"),
    ] {
        let error = http::endpoint(url).unwrap_err();
        assert!(error.contains(expected), "{url}: {error}");
    }
}

#[tokio::test]
async fn calls_carry_the_token_session_and_version_and_answer_server_requests() {
    let server = Server::new(MCP_PROTOCOL_VERSION).await;
    // SAFETY: only this test reads this variable, and nothing else in this
    // test process sets or reads environment variables concurrently with it.
    unsafe { std::env::set_var(TOKEN_VARIABLE, TOKEN) };
    let url = format!("{}/mcp", server.origin);
    let (kernel, status) = mount(config(&url, json!({ "bearer_token_env": TOKEN_VARIABLE }))).await;
    assert_eq!(status, PluginStatus::Active);

    let (output, effect) = call(&kernel, "mcp.web.echo", json!({ "text": "hi" })).await;
    assert!(!output.is_error, "{output:?}");
    assert_eq!(
        output.content,
        vec![
            ToolContent::Text("hi".into()),
            ToolContent::Image(rah_protocol::ImageContent {
                media_type: "image/png".into(),
                data: "AA==".into()
            }),
        ]
    );
    assert_eq!(effect, ToolEffect::None);

    let requests = server.requests();
    let rpc: Vec<&str> = requests.iter().map(Request::rpc).collect();
    assert_eq!(
        rpc,
        [
            "initialize",
            "notifications/initialized",
            "tools/list",
            "tools/call",
            ""
        ],
        "the last request answers the ping"
    );
    assert!(requests.iter().all(|request| request.method == "POST"
        && request.headers["authorization"] == format!("Bearer {TOKEN}")
        && request.headers["accept"] == "application/json, text/event-stream"));
    assert!(!requests[0].headers.contains_key("mcp-session-id"));
    for request in &requests[1..] {
        assert_eq!(request.headers["mcp-session-id"], "session-1");
        assert_eq!(
            request.headers["mcp-protocol-version"],
            MCP_PROTOCOL_VERSION
        );
    }
    assert_eq!(
        requests[4].body,
        json!({ "jsonrpc": "2.0", "id": "srv-1", "result": {} })
    );

    // Unmounting ends the session.
    kernel.unmount(&PluginId::new("mcp-http")).await.unwrap();
    let last = server.requests().pop().unwrap();
    assert_eq!(last.method, "DELETE");
    assert_eq!(last.headers["mcp-session-id"], "session-1");
}

#[tokio::test]
async fn lost_streams_timeouts_and_errors_are_not_replayed() {
    let server = Server::new("2025-03-26").await;
    let (kernel, status) = mount(config(&format!("{}/mcp", server.origin), json!({}))).await;
    assert_eq!(status, PluginStatus::Active);

    // A stream that ends without the response may have run.
    let (output, effect) = call(&kernel, "mcp.web.cut", json!({})).await;
    assert!(output.is_error);
    assert_eq!(effect, ToolEffect::Uncertain);
    assert!(first_text(&output).contains("not replayed"), "{output:?}");

    // A timeout tells the server and stays uncertain.
    let (output, effect) = call(&kernel, "mcp.web.slow", json!({})).await;
    assert_eq!(effect, ToolEffect::Uncertain);
    assert!(first_text(&output).contains("timed out"), "{output:?}");
    tokio::time::sleep(Duration::from_millis(200)).await;
    let cancelled = server
        .requests()
        .into_iter()
        .find(|request| request.rpc() == "notifications/cancelled")
        .expect("a cancellation notice");
    assert_eq!(cancelled.body["params"]["requestId"], json!(4));

    // An error status is an error with the usual effect.
    let (output, effect) = call(&kernel, "mcp.web.broken", json!({})).await;
    assert!(first_text(&output).contains("HTTP 500"), "{output:?}");
    assert_eq!(effect, ToolEffect::None);

    // An ended session ran nothing, and later calls fail at once.
    let (output, effect) = call(&kernel, "mcp.web.gone", json!({})).await;
    assert!(
        first_text(&output).contains("ended the session"),
        "{output:?}"
    );
    assert_eq!(effect, ToolEffect::None);
    let before = server.requests().len();
    let (output, effect) = call(&kernel, "mcp.web.echo", json!({ "text": "x" })).await;
    assert!(
        first_text(&output).contains("ended the session"),
        "{output:?}"
    );
    assert_eq!(effect, ToolEffect::None);
    assert_eq!(server.requests().len(), before, "nothing was sent");
}

#[tokio::test]
async fn startup_failures_fail_the_plugin() {
    let server = Server::new("2024-11-05").await;
    let url = format!("{}/mcp", server.origin);
    // Bind and drop a listener for a port that refuses connections.
    let closed = {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        format!("http://{}/mcp", listener.local_addr().unwrap())
    };
    let cases = [
        (config(&url, json!({})), "no Streamable HTTP"),
        (config(&closed, json!({})), "cannot connect"),
        (config("http://192.0.2.1/mcp", json!({})), "loopback"),
        (
            config(
                &url,
                json!({ "bearer_token_env": "RAH_TEST_MCP_HTTP_UNSET" }),
            ),
            "holds no token",
        ),
        (config(&url, json!({ "program": "x" })), "unknown field"),
    ];
    for (config, expected) in cases {
        let (_kernel, status) = mount(config.clone()).await;
        match status {
            PluginStatus::Failed { message } => {
                assert!(message.contains(expected), "{config}: {message}");
                assert!(!message.contains(TOKEN), "{message}");
            }
            other => panic!("{config}: {other:?}"),
        }
    }
}
