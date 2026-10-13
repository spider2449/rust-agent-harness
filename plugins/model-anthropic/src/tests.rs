//! Deterministic loopback fixture tests; no network, credentials or model.

use std::sync::{Arc, Mutex};

use futures::StreamExt;
use rah_api::{
    CancellationToken, Kernel, MountOptions, PluginStatus,
    model::{ModelError, ModelProvider},
};
use rah_protocol::{
    ContextItem, ModelEvent, ModelRequest, PermissionLevel, ToolDefinition, ToolName,
};
use serde_json::{Value, json};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

use super::*;

const KEY: &str = "FIXTURE-KEY-DO-NOT-EXPOSE";

enum Reply {
    Sse(String),
    Status(u16, &'static str),
    Hold,
}

/// Serves scripted replies to sequential connections and captures requests.
struct Server {
    origin: String,
    requests: Arc<Mutex<Vec<(String, Value)>>>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl Server {
    async fn new(replies: Vec<Reply>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let requests = Arc::new(Mutex::new(Vec::new()));
        let captured = requests.clone();
        let task = tokio::spawn(async move {
            for reply in replies {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut bytes = Vec::new();
                let header_end = loop {
                    let mut chunk = [0; 4096];
                    let n = socket.read(&mut chunk).await.unwrap();
                    if n == 0 {
                        return;
                    }
                    bytes.extend_from_slice(&chunk[..n]);
                    if let Some(i) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                        break i + 4;
                    }
                };
                let head = String::from_utf8(bytes[..header_end].to_vec()).unwrap();
                let length = head
                    .lines()
                    .find_map(|l| {
                        l.to_ascii_lowercase()
                            .strip_prefix("content-length: ")
                            .map(|n| n.parse::<usize>().unwrap())
                    })
                    .unwrap_or(0);
                while bytes.len() < header_end + length {
                    let mut chunk = [0; 4096];
                    let n = socket.read(&mut chunk).await.unwrap();
                    bytes.extend_from_slice(&chunk[..n]);
                }
                let body = serde_json::from_slice(&bytes[header_end..]).unwrap_or(Value::Null);
                captured.lock().unwrap().push((head, body));
                let (status, kind, body) = match reply {
                    Reply::Sse(body) => (200, "text/event-stream", body),
                    Reply::Status(status, kind) => (status, kind, "{}".to_owned()),
                    Reply::Hold => {
                        let _ = socket
                            .write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\n\r\n")
                            .await;
                        let mut byte = [0];
                        let _ = socket.read(&mut byte).await;
                        continue;
                    }
                };
                let header = format!(
                    "HTTP/1.1 {status} Fixture\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                if socket.write_all(header.as_bytes()).await.is_err() {
                    continue;
                }
                for chunk in body.as_bytes().chunks(7) {
                    if socket.write_all(chunk).await.is_err() {
                        break;
                    }
                    tokio::task::yield_now().await;
                }
            }
        });
        Self {
            origin,
            requests,
            task,
        }
    }

    fn requests(&self) -> Vec<(String, Value)> {
        self.requests.lock().unwrap().clone()
    }
}

/// Frames events the way the Messages API does: a named event per frame.
fn frames(events: &[Value]) -> String {
    events
        .iter()
        .map(|v| format!("event: {}\ndata: {v}\n\n", v["type"].as_str().unwrap()))
        .collect()
}

fn message_start() -> Value {
    json!({"type":"message_start","message":{"id":"msg_1","type":"message","role":"assistant","content":[],"model":"claude-test","stop_reason":null,"usage":{"input_tokens":3,"output_tokens":1}}})
}

fn stop(reason: &str) -> Vec<Value> {
    vec![
        json!({"type":"message_delta","delta":{"stop_reason":reason,"stop_sequence":null},"usage":{"output_tokens":5}}),
        json!({"type":"message_stop"}),
    ]
}

fn text_events(text: &str) -> Vec<Value> {
    let mut events = vec![
        message_start(),
        json!({"type":"content_block_start","index":0,"content_block":{"type":"thinking","thinking":"","signature":""}}),
        json!({"type":"content_block_delta","index":0,"delta":{"type":"signature_delta","signature":"sig"}}),
        json!({"type":"content_block_stop","index":0}),
        json!({"type":"ping"}),
        json!({"type":"content_block_start","index":1,"content_block":{"type":"text","text":""}}),
    ];
    for piece in text.split_inclusive(' ') {
        events.push(json!({"type":"content_block_delta","index":1,"delta":{"type":"text_delta","text":piece}}));
    }
    events.push(json!({"type":"content_block_stop","index":1}));
    events.extend(stop("end_turn"));
    events
}

fn tool_events(name: &str, partial_json: &[&str]) -> Vec<Value> {
    let mut events = vec![
        message_start(),
        json!({"type":"content_block_start","index":0,"content_block":{"type":"tool_use","id":"toolu_1","name":name,"input":{}}}),
    ];
    for piece in partial_json {
        events.push(json!({"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":piece}}));
    }
    events.push(json!({"type":"content_block_stop","index":0}));
    events.extend(stop("tool_use"));
    events
}

fn tool(name: &str) -> ToolDefinition {
    ToolDefinition {
        name: ToolName::new(name),
        description: "test".into(),
        input_schema: json!({"type":"object"}),
        permission: PermissionLevel::Read,
    }
}

fn request(items: Vec<ContextItem>) -> ModelRequest {
    ModelRequest {
        model: None,
        items,
        tools: vec![tool("fs.read")],
    }
}

fn user(text: &str) -> Vec<ContextItem> {
    vec![ContextItem::User { text: text.into() }]
}

fn settings(model: &str) -> Settings {
    Settings {
        model: model.into(),
        max_tokens: DEFAULT_MAX_TOKENS,
        effort: None,
        refusal_fallback: true,
    }
}

fn anthropic(server: &Server, settings: Settings) -> AnthropicModel {
    let endpoint = anthropic_endpoint(Some(&format!("{}/v1/messages", server.origin))).unwrap();
    AnthropicModel::new(endpoint, KEY.into(), settings).unwrap()
}

async fn collect(
    model: &AnthropicModel,
    request: ModelRequest,
) -> Result<Vec<ModelEvent>, ModelError> {
    let mut stream = model.stream(request, CancellationToken::new()).await?;
    let mut events = Vec::new();
    while let Some(event) = stream.next().await {
        events.push(event?);
    }
    Ok(events)
}

fn text(events: &[ModelEvent]) -> String {
    events
        .iter()
        .filter_map(|e| match e {
            ModelEvent::TextDelta { text } => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn streams_text_and_sends_a_messages_request() {
    let server = Server::new(vec![Reply::Sse(frames(&text_events("hello there")))]).await;
    let mut configured = settings("claude-test");
    configured.effort = Some("high".into());
    let mut history = vec![ContextItem::System {
        text: "be brief".into(),
    }];
    history.extend(user("hi"));
    let events = collect(&anthropic(&server, configured), request(history))
        .await
        .unwrap();
    assert_eq!(events[0], ModelEvent::ReasoningStarted);
    assert_eq!(text(&events), "hello there");
    assert_eq!(events.last(), Some(&ModelEvent::Completed));

    let (head, body) = &server.requests()[0];
    let head = head.to_ascii_lowercase();
    assert!(head.starts_with("post /v1/messages http/1.1"));
    assert!(head.contains(&format!("x-api-key: {}", KEY.to_ascii_lowercase())));
    assert!(head.contains("anthropic-version: 2023-06-01"));
    assert!(!head.contains("authorization:"));
    // Not a fallback model: no beta header and no `fallbacks`.
    assert!(!head.contains("anthropic-beta"));
    assert!(body.get("fallbacks").is_none());
    assert_eq!(body["model"], "claude-test");
    assert_eq!(body["max_tokens"], DEFAULT_MAX_TOKENS);
    assert_eq!(body["stream"], true);
    assert_eq!(body["cache_control"], json!({"type":"ephemeral"}));
    assert_eq!(body["output_config"], json!({"effort":"high"}));
    assert_eq!(body["system"], "be brief");
    assert_eq!(
        body["messages"],
        json!([{"role":"user","content":[{"type":"text","text":"hi"}]}])
    );
    assert_eq!(body["tools"][0]["name"], "fs_read");
    assert!(body.get("thinking").is_none());
}

#[tokio::test]
async fn refusal_fallback_is_requested_for_supported_models_unless_disabled() {
    let server = Server::new(vec![
        Reply::Sse(frames(&text_events("a"))),
        Reply::Sse(frames(&text_events("b"))),
    ])
    .await;
    collect(
        &anthropic(&server, settings(DEFAULT_MODEL)),
        request(user("x")),
    )
    .await
    .unwrap();
    let mut disabled = settings(DEFAULT_MODEL);
    disabled.refusal_fallback = false;
    collect(&anthropic(&server, disabled), request(user("x")))
        .await
        .unwrap();
    let requests = server.requests();
    assert!(
        requests[0]
            .0
            .to_ascii_lowercase()
            .contains("anthropic-beta: server-side-fallback-2026-07-01")
    );
    assert_eq!(requests[0].1["fallbacks"], "default");
    assert!(
        !requests[1]
            .0
            .to_ascii_lowercase()
            .contains("anthropic-beta")
    );
    assert!(requests[1].1.get("fallbacks").is_none());
}

#[tokio::test]
async fn tool_use_maps_back_to_the_original_tool_name() {
    let server = Server::new(vec![
        Reply::Sse(frames(&tool_events(
            "fs_read",
            &["{\"pa", "th\":\"a.txt\"}"],
        ))),
        Reply::Sse(frames(&tool_events("fs_read", &[]))),
    ])
    .await;
    let model = anthropic(&server, settings("claude-test"));
    let events = collect(&model, request(user("read"))).await.unwrap();
    assert_eq!(
        events,
        [
            ModelEvent::ToolCall {
                call_id: "toolu_1".into(),
                name: ToolName::new("fs.read"),
                arguments: json!({"path":"a.txt"}),
            },
            ModelEvent::Completed
        ]
    );
    // No input deltas: the start block's input is the complete input.
    let events = collect(&model, request(user("read"))).await.unwrap();
    assert!(
        matches!(&events[0], ModelEvent::ToolCall { arguments, .. } if *arguments == json!({}))
    );
}

#[tokio::test]
async fn failures_are_typed_and_carry_no_secrets() {
    let truncated = frames(&text_events("cut")[..6]);
    let api_error = frames(&[
        message_start(),
        json!({"type":"error","error":{"type":"invalid_request_error","message":"secret-ish detail"}}),
    ]);
    let overloaded = frames(&[
        json!({"type":"error","error":{"type":"overloaded_error","message":"secret-ish detail"}}),
    ]);
    let unknown_tool = frames(&tool_events("not_offered", &["{}"]));
    let bad_arguments = frames(&tool_events("fs_read", &["[1]"]));
    let mut max_tokens = text_events("long");
    let len = max_tokens.len();
    max_tokens[len - 2]["delta"]["stop_reason"] = json!("max_tokens");
    let mut refusal = text_events("no");
    refusal[len - 2]["delta"]["stop_reason"] = json!("refusal");
    let server = Server::new(vec![
        Reply::Status(401, "application/json"),
        Reply::Status(400, "application/json"),
        Reply::Status(529, "application/json"),
        Reply::Status(200, "application/json"),
        Reply::Sse(truncated),
        Reply::Sse(api_error),
        Reply::Sse(overloaded),
        Reply::Sse(unknown_tool),
        Reply::Sse(bad_arguments),
        Reply::Sse(frames(&max_tokens)),
        Reply::Sse(frames(&refusal)),
    ])
    .await;
    let model = anthropic(&server, settings("claude-test"));
    let mut errors = Vec::new();
    for _ in 0..11 {
        errors.push(collect(&model, request(user("x"))).await.unwrap_err());
    }
    assert!(matches!(&errors[0], ModelError::Rejected(m) if m.contains("authentication")));
    assert!(matches!(&errors[1], ModelError::Rejected(m) if m.contains("400")));
    assert!(matches!(&errors[2], ModelError::Transport(m) if m.contains("overloaded")));
    assert!(matches!(errors[3], ModelError::Protocol(_)));
    assert!(matches!(errors[4], ModelError::Protocol(_)));
    assert!(matches!(errors[5], ModelError::Rejected(_)));
    assert!(matches!(&errors[6], ModelError::Transport(m) if m.contains("overloaded")));
    assert!(matches!(errors[7], ModelError::Protocol(_)));
    assert!(matches!(errors[8], ModelError::Protocol(_)));
    assert!(matches!(&errors[9], ModelError::Rejected(m) if m.contains("token limit")));
    assert!(matches!(&errors[10], ModelError::Rejected(m) if m.contains("declined")));
    for error in &errors {
        let text = format!("{error} {error:?}");
        assert!(
            !text.contains(KEY) && !text.contains("secret-ish"),
            "{text}"
        );
    }
}

#[tokio::test]
async fn cancellation_ends_an_open_stream() {
    let server = Server::new(vec![Reply::Hold]).await;
    let model = anthropic(&server, settings("claude-test"));
    let cancel = CancellationToken::new();
    let mut stream = model
        .stream(request(user("x")), cancel.clone())
        .await
        .unwrap();
    cancel.cancel();
    assert_eq!(stream.next().await, Some(Err(ModelError::Cancelled)));
}

#[test]
fn endpoints_are_bounded_to_official_or_loopback_origins() {
    assert_eq!(
        anthropic_endpoint(None).unwrap(),
        "https://api.anthropic.com/v1/messages"
    );
    assert!(anthropic_endpoint(Some("http://127.0.0.1:9/v1/messages")).is_ok());
    assert!(anthropic_endpoint(Some("http://[::1]:9/v1/messages")).is_ok());
    for rejected in [
        "https://evil.example/v1/messages",
        "http://api.anthropic.com/v1/messages",
        "http://user:pw@127.0.0.1/v1/messages",
        "http://127.0.0.1/v1/messages?x=1",
        "file:///etc/passwd",
    ] {
        assert!(anthropic_endpoint(Some(rejected)).is_err(), "{rejected}");
    }
}

#[tokio::test]
async fn plugin_fails_closed_on_missing_credentials_and_bad_configuration() {
    let kernel = Kernel::new();
    let status = kernel
        .mount(
            Arc::new(AnthropicPlugin::new("anthropic")),
            MountOptions::with_config(json!({"api_key_env":"RAH_TEST_UNSET_KEY_9C1D"})),
        )
        .await
        .unwrap();
    assert!(
        matches!(&status, PluginStatus::Failed { message } if message.contains("RAH_TEST_UNSET_KEY_9C1D is not set")),
        "{status:?}"
    );
    let status = kernel
        .mount(
            Arc::new(AnthropicPlugin::new("typo")),
            MountOptions::with_config(json!({"modle":"x"})),
        )
        .await
        .unwrap();
    assert!(matches!(status, PluginStatus::Failed { .. }));
    for (bad, message) in [
        (json!({"effort": "extreme"}), "effort must be"),
        (json!({"max_tokens": 0}), "max_tokens must be"),
    ] {
        let error = serde_json::from_value::<Config>(bad)
            .unwrap()
            .settings()
            .err()
            .unwrap();
        assert!(error.contains(message), "{error}");
    }
    let defaults = Config::default().settings().unwrap();
    assert_eq!(defaults.model, DEFAULT_MODEL);
    assert_eq!(defaults.max_tokens, DEFAULT_MAX_TOKENS);
    assert!(defaults.refusal_fallback && defaults.effort.is_none());
}
