//! Deterministic loopback fixture tests; no network, credentials or model.

use std::sync::{Arc, Mutex};

use futures::StreamExt;
use rah_api::{
    CancellationToken, Kernel, MountOptions, PluginStatus,
    model::{ModelError, ModelHealth, ModelProvider},
};
use rah_protocol::{
    ContextItem, ModelEvent, ModelRequest, ModelUsage, PermissionLevel, ToolDefinition, ToolName,
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
    Json(u16, Value),
    Status(u16, &'static str),
    /// A status with a `Retry-After: 7` header.
    Limited(u16),
    /// A status with this JSON body.
    Body(u16, &'static str),
    Hold,
}

/// Serves scripted replies to sequential connections and captures requests
/// (head and JSON body).
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
                    Reply::Sse(body) => (200, "text/event-stream; charset=utf-8", body),
                    Reply::Json(status, body) => (status, "application/json", body.to_string()),
                    Reply::Status(status, kind) => (status, kind, "{}".to_owned()),
                    Reply::Limited(status) => (
                        status,
                        "application/json\r\nRetry-After: 7",
                        "{}".to_owned(),
                    ),
                    Reply::Body(status, body) => (status, "application/json", body.to_owned()),
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

/// Frames chunks the way Chat Completions servers do, ending with `[DONE]`.
fn frames(chunks: &[Value]) -> String {
    let mut body: String = chunks.iter().map(|v| format!("data: {v}\n\n")).collect();
    body.push_str("data: [DONE]\n\n");
    body
}

fn delta(delta: Value) -> Value {
    json!({"id": "c", "object": "chat.completion.chunk", "choices": [{"index": 0, "delta": delta, "finish_reason": null}]})
}

fn finish(reason: &str) -> Value {
    json!({"id": "c", "object": "chat.completion.chunk", "choices": [{"index": 0, "delta": {}, "finish_reason": reason}]})
}

fn usage() -> Value {
    json!({"id": "c", "choices": [], "usage": {"prompt_tokens": 12, "completion_tokens": 3, "total_tokens": 15, "prompt_tokens_details": {"cached_tokens": 8}}})
}

fn text_reply(text: &str) -> String {
    frames(&[
        delta(json!({"role": "assistant", "content": ""})),
        delta(json!({"content": text})),
        finish("stop"),
        usage(),
    ])
}

fn tool(name: &str) -> ToolDefinition {
    ToolDefinition {
        name: ToolName::new(name),
        description: "a tool".into(),
        input_schema: json!({"type": "object", "properties": {"path": {"type": "string"}}}),
        permission: PermissionLevel::Read,
    }
}

fn request(items: Vec<ContextItem>, tools: Vec<ToolDefinition>) -> ModelRequest {
    ModelRequest {
        model: None,
        items,
        tools,
    }
}

fn user(text: &str) -> Vec<ContextItem> {
    vec![ContextItem::User { text: text.into() }]
}

fn settings(server: &Server, model: Option<&str>) -> Settings {
    let (base_url, loopback) = base_url(&format!("{}/v1", server.origin)).unwrap();
    Settings {
        base_url,
        model: model.map(str::to_owned),
        max_tokens: None,
        temperature: None,
        loopback,
    }
}

fn model(settings: Settings) -> ChatCompletionsModel {
    ChatCompletionsModel::new(KEY.into(), settings).unwrap()
}

async fn collect(
    model: &ChatCompletionsModel,
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
async fn streams_text_and_sends_a_chat_completions_request() {
    let server = Server::new(vec![Reply::Sse(text_reply("hello there"))]).await;
    let mut configured = settings(&server, Some("deepseek-chat"));
    configured.max_tokens = Some(512);
    configured.temperature = Some(0.25);
    let model = model(configured);
    let history = vec![
        ContextItem::System {
            text: "be brief".into(),
        },
        ContextItem::User { text: "hi".into() },
    ];
    let events = collect(&model, request(history, Vec::new())).await.unwrap();
    assert_eq!(text(&events), "hello there");
    assert_eq!(
        events[events.len() - 2],
        ModelEvent::Usage {
            usage: ModelUsage {
                input_tokens: Some(12),
                output_tokens: Some(3),
                cached_input_tokens: Some(8),
            }
        }
    );
    assert_eq!(events.last(), Some(&ModelEvent::Completed));

    let requests = server.requests();
    let (head, body) = &requests[0];
    assert!(head.starts_with("POST /v1/chat/completions "), "{head}");
    assert!(
        head.to_ascii_lowercase().contains(&format!(
            "authorization: bearer {}",
            KEY.to_ascii_lowercase()
        )),
        "{head}"
    );
    assert_eq!(body["model"], "deepseek-chat");
    assert_eq!(body["stream"], true);
    assert_eq!(body["stream_options"], json!({"include_usage": true}));
    assert_eq!(body["max_tokens"], 512);
    assert_eq!(body["temperature"], 0.25);
    assert_eq!(
        body["messages"],
        json!([{"role": "system", "content": "be brief"}, {"role": "user", "content": "hi"}])
    );
    // No tools: the field is left out, since some servers reject `[]`.
    assert!(body.get("tools").is_none(), "{body}");
}

#[tokio::test]
async fn tool_calls_map_back_to_the_original_tool_names() {
    let reply = frames(&[
        delta(json!({"reasoning_content": "Let me look."})),
        delta(json!({"reasoning_content": " More."})),
        delta(json!({"content": "Reading."})),
        delta(
            json!({"tool_calls": [{"index": 0, "id": "call_1", "type": "function", "function": {"name": "fs_read", "arguments": ""}}]}),
        ),
        delta(json!({"tool_calls": [{"index": 0, "function": {"arguments": "{\"path\":"}}]})),
        delta(json!({"tool_calls": [{"index": 0, "function": {"arguments": "\"a.txt\"}"}}]})),
        finish("tool_calls"),
    ]);
    let server = Server::new(vec![Reply::Sse(reply)]).await;
    let model = model(settings(&server, Some("m")));
    let events = collect(&model, request(user("read a.txt"), vec![tool("fs.read")]))
        .await
        .unwrap();
    assert_eq!(
        events,
        [
            ModelEvent::ReasoningStarted,
            ModelEvent::TextDelta {
                text: "Reading.".into()
            },
            ModelEvent::ToolCall {
                call_id: "call_1".into(),
                name: ToolName::new("fs.read"),
                arguments: json!({"path": "a.txt"}),
            },
            ModelEvent::Completed,
        ]
    );
    let body = &server.requests()[0].1;
    assert_eq!(body["tools"][0]["function"]["name"], "fs_read");
    assert_eq!(body["tools"][0]["function"]["description"], "a tool");
}

#[tokio::test]
async fn a_call_to_an_unknown_tool_is_a_protocol_error() {
    let reply = frames(&[
        delta(
            json!({"tool_calls": [{"index": 0, "id": "c", "function": {"name": "nope", "arguments": "{}"}}]}),
        ),
        finish("tool_calls"),
    ]);
    let server = Server::new(vec![Reply::Sse(reply)]).await;
    let model = model(settings(&server, Some("m")));
    let error = collect(&model, request(user("x"), vec![tool("fs.read")]))
        .await
        .unwrap_err();
    assert!(matches!(error, ModelError::Protocol(_)), "{error:?}");
}

#[tokio::test]
async fn the_sole_served_model_is_discovered_once() {
    let catalog = json!({"object": "list", "data": [{"id": "qwen3-8b", "object": "model"}]});
    let server = Server::new(vec![
        Reply::Json(200, catalog),
        Reply::Sse(text_reply("one")),
        Reply::Sse(text_reply("two")),
    ])
    .await;
    let model = model(settings(&server, None));
    for expected in ["one", "two"] {
        let events = collect(&model, request(user("x"), Vec::new()))
            .await
            .unwrap();
        assert_eq!(text(&events), expected);
    }
    let requests = server.requests();
    assert!(
        requests[0].0.starts_with("GET /v1/models "),
        "{}",
        requests[0].0
    );
    assert_eq!(requests[1].1["model"], "qwen3-8b");
    assert_eq!(requests[2].1["model"], "qwen3-8b");

    // A request's own model wins and needs no discovery.
    let server = Server::new(vec![Reply::Sse(text_reply("x"))]).await;
    let model = self::model(settings(&server, None));
    let mut asked = request(user("x"), Vec::new());
    asked.model = Some("chosen".into());
    collect(&model, asked).await.unwrap();
    assert_eq!(server.requests()[0].1["model"], "chosen");
}

#[tokio::test]
async fn several_served_models_need_a_configured_one() {
    let catalog = json!({"data": [{"id": "a"}, {"id": "b"}]});
    let server = Server::new(vec![Reply::Json(200, catalog)]).await;
    let model = model(settings(&server, None));
    let error = collect(&model, request(user("x"), Vec::new()))
        .await
        .unwrap_err();
    assert!(
        matches!(&error, ModelError::Rejected(m) if m.contains("no model is configured")),
        "{error:?}"
    );
}

#[tokio::test]
async fn failures_are_typed_and_carry_no_secrets() {
    let secret =
        json!({"error": {"message": "secret-ish detail", "type": "invalid_request_error"}});
    let replies = vec![
        Reply::Json(401, secret.clone()),
        Reply::Json(400, secret.clone()),
        Reply::Status(200, "application/json"),
        Reply::Sse(frames(std::slice::from_ref(&secret))),
        Reply::Sse(frames(&[delta(json!({"content": "cut"}))])),
        Reply::Sse(
            "data: {\"choices\": [{\"index\": 0, \"delta\": {\"content\": \"torn\"}}]}".into(),
        ),
        Reply::Sse(frames(&[
            delta(json!({"content": "long"})),
            finish("length"),
        ])),
        Reply::Sse(frames(&[finish("content_filter")])),
        Reply::Sse("data: {nope\n\n".into()),
    ];
    let count = replies.len();
    let server = Server::new(replies).await;
    let model = model(settings(&server, Some("m")));
    let mut errors = Vec::new();
    for _ in 0..count {
        errors.push(
            collect(&model, request(user("x"), Vec::new()))
                .await
                .unwrap_err(),
        );
    }
    assert!(matches!(&errors[0], ModelError::Rejected(m) if m.contains("authentication")));
    assert!(matches!(&errors[1], ModelError::Rejected(m) if m.contains("400")));
    assert!(matches!(&errors[2], ModelError::Protocol(_)));
    assert!(matches!(&errors[3], ModelError::Rejected(m) if m.contains("reported a failure")));
    assert!(
        matches!(&errors[4], ModelError::Protocol(_)),
        "{:?}",
        errors[4]
    );
    assert!(
        matches!(&errors[5], ModelError::Protocol(_)),
        "{:?}",
        errors[5]
    );
    assert!(matches!(&errors[6], ModelError::Rejected(m) if m.contains("token limit")));
    assert!(matches!(&errors[7], ModelError::Rejected(m) if m.contains("filtered")));
    assert!(matches!(&errors[8], ModelError::Protocol(_)));
    for error in &errors {
        let text = format!("{error} {error:?}");
        assert!(
            !text.contains(KEY) && !text.contains("secret-ish"),
            "{text}"
        );
    }
}

#[tokio::test]
async fn an_unreachable_server_is_a_transport_error() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);
    let (base_url, loopback) = base_url(&format!("http://{address}/v1")).unwrap();
    let model = model(Settings {
        base_url,
        model: Some("m".into()),
        max_tokens: None,
        temperature: None,
        loopback,
    });
    let error = collect(&model, request(user("x"), Vec::new()))
        .await
        .unwrap_err();
    assert!(matches!(error, ModelError::Transport(_)), "{error:?}");
}

#[tokio::test]
async fn cancellation_ends_an_open_stream() {
    let server = Server::new(vec![Reply::Hold]).await;
    let model = model(settings(&server, Some("m")));
    let cancel = CancellationToken::new();
    let mut stream = model
        .stream(request(user("x"), Vec::new()), cancel.clone())
        .await
        .unwrap();
    cancel.cancel();
    assert_eq!(stream.next().await, Some(Err(ModelError::Cancelled)));
}

#[test]
fn base_urls_need_https_or_a_loopback_address() {
    assert_eq!(
        base_url("https://api.deepseek.com/").unwrap(),
        ("https://api.deepseek.com".to_owned(), false)
    );
    assert_eq!(
        base_url("http://127.0.0.1:11434/v1").unwrap(),
        ("http://127.0.0.1:11434/v1".to_owned(), true)
    );
    assert!(base_url("http://localhost:8080/v1").unwrap().1);
    assert!(base_url("http://[::1]:8080/v1").unwrap().1);
    assert!(base_url("https://gateway.example/openai/v1").is_ok());
    for rejected in [
        "http://api.deepseek.com",
        "http://192.168.1.10:8080/v1",
        "https://user:pw@api.deepseek.com",
        "https://api.deepseek.com/v1?x=1",
        "https://api.deepseek.com/#x",
        "file:///etc/passwd",
        "not a url",
    ] {
        assert!(base_url(rejected).is_err(), "{rejected}");
    }
}

#[test]
fn providers_fill_in_defaults_that_the_configuration_overrides() {
    let resolve = |config: Value| serde_json::from_value::<Config>(config).unwrap().resolve();
    let (settings, key) = resolve(json!({"provider": "deepseek"})).unwrap();
    assert_eq!(settings.base_url, "https://api.deepseek.com");
    assert_eq!(settings.model.as_deref(), Some("deepseek-chat"));
    assert!(!settings.loopback);
    assert_eq!(key, Some(("DEEPSEEK_API_KEY".to_owned(), true)));

    let (settings, key) = resolve(
        json!({"provider": "deepseek", "model": "deepseek-reasoner", "api_key_env": "MY_KEY"}),
    )
    .unwrap();
    assert_eq!(settings.model.as_deref(), Some("deepseek-reasoner"));
    assert_eq!(key, Some(("MY_KEY".to_owned(), true)));

    let (settings, key) = resolve(json!({"provider": "llamacpp"})).unwrap();
    assert_eq!(settings.base_url, "http://127.0.0.1:8080/v1");
    assert!(settings.model.is_none() && settings.loopback);
    assert_eq!(key, Some(("RAH_LLAMA_CPP_API_KEY".to_owned(), false)));

    let (settings, key) =
        resolve(json!({"provider": "ollama", "base_url": "http://127.0.0.1:9999/v1"})).unwrap();
    assert_eq!(settings.base_url, "http://127.0.0.1:9999/v1");
    assert_eq!(key, None);

    let (settings, key) = resolve(json!({"base_url": "https://gateway.example/v1"})).unwrap();
    assert!(settings.model.is_none());
    assert_eq!(key, None);

    for (bad, message) in [
        (json!({}), "needs `base_url` or `provider`"),
        (json!({"provider": "nope"}), "provider must be one of"),
        (
            json!({"base_url": "http://10.0.0.1/v1"}),
            "base_url must be",
        ),
        (
            json!({"provider": "ollama", "max_tokens": 0}),
            "max_tokens must be",
        ),
        (
            json!({"provider": "ollama", "temperature": 3.0}),
            "temperature must be",
        ),
    ] {
        let error = resolve(bad.clone()).err().unwrap();
        assert!(error.contains(message), "{bad}: {error}");
    }
}

#[tokio::test]
async fn plugin_mounts_for_local_servers_and_fails_closed_without_a_required_key() {
    let kernel = Kernel::new();
    let status = kernel
        .mount(
            Arc::new(ChatCompletionsPlugin::new("ollama")),
            MountOptions::with_config(json!({"provider": "ollama"})),
        )
        .await
        .unwrap();
    assert_eq!(status, PluginStatus::Active);

    let status = kernel
        .mount(
            Arc::new(ChatCompletionsPlugin::new("keyed")),
            MountOptions::with_config(
                json!({"provider": "deepseek", "api_key_env": "RAH_TEST_UNSET_KEY_7F3A"}),
            ),
        )
        .await
        .unwrap();
    assert!(
        matches!(&status, PluginStatus::Failed { message } if message.contains("RAH_TEST_UNSET_KEY_7F3A is not set")),
        "{status:?}"
    );
    for (id, config) in [
        ("empty", Value::Null),
        ("typo", json!({"provider": "ollama", "modle": "x"})),
    ] {
        let status = kernel
            .mount(
                Arc::new(ChatCompletionsPlugin::new(id)),
                MountOptions::with_config(config),
            )
            .await
            .unwrap_err();
        assert!(
            matches!(status, rah_api::kernel::KernelError::InvalidConfig { .. }),
            "{status:?}"
        );
    }
}

#[test]
fn a_key_pasted_as_the_variable_name_is_refused_without_echoing_it() {
    let pasted = "sk-abc123-this-is-a-secret-key";
    let error = read_key(pasted, true).unwrap_err();
    assert!(error.contains("NAME of an environment variable"), "{error}");
    assert!(!error.contains(pasted), "{error}");
    let error = read_key(&"a".repeat(80), true).unwrap_err();
    assert!(error.contains("NAME of an environment variable"), "{error}");
}

#[tokio::test]
async fn health_reports_reachable_unauthorized_and_down_servers() {
    let server = Server::new(vec![
        Reply::Json(200, json!({"data": []})),
        Reply::Json(404, json!({})),
        Reply::Json(401, json!({"error": KEY})),
    ])
    .await;
    let up = model(settings(&server, Some("m")));
    let health = || up.health(CancellationToken::new());
    assert_eq!(health().await, ModelHealth::Reachable);
    // A gateway without `/models` is still up.
    assert_eq!(health().await, ModelHealth::Reachable);
    let ModelHealth::Unavailable(reason) = health().await else {
        panic!("a refused key must be unavailable");
    };
    assert!(!reason.contains(KEY), "{reason}");

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);
    let (base_url, loopback) = base_url(&format!("http://{address}/v1")).unwrap();
    let down = model(Settings {
        base_url,
        model: None,
        max_tokens: None,
        temperature: None,
        loopback,
    });
    assert!(matches!(
        down.health(CancellationToken::new()).await,
        ModelHealth::Unavailable(_)
    ));
}

#[tokio::test]
async fn busy_statuses_are_unavailable_and_carry_retry_after() {
    let server = Server::new(vec![
        Reply::Limited(429),
        Reply::Status(502, "application/json"),
        Reply::Status(400, "application/json"),
    ])
    .await;
    let model = model(settings(&server, Some("m")));
    let mut errors = Vec::new();
    for _ in 0..3 {
        errors.push(
            collect(&model, request(user("x"), vec![]))
                .await
                .unwrap_err(),
        );
    }
    let seven = Some(std::time::Duration::from_secs(7));
    assert!(
        matches!(&errors[0], ModelError::Unavailable { message, retry_after } if message.contains("429") && *retry_after == seven)
    );
    assert!(
        matches!(&errors[1], ModelError::Unavailable { message, retry_after } if message.contains("502") && retry_after.is_none())
    );
    assert!(matches!(&errors[2], ModelError::Rejected(_)));
}

#[tokio::test]
async fn a_request_that_does_not_fit_is_a_context_overflow() {
    let server = Server::new(vec![
        Reply::Body(
            400,
            r#"{"error":{"code":"context_length_exceeded","message":"This model's maximum context length is 65536 tokens"}}"#,
        ),
        Reply::Body(
            400,
            r#"{"error":{"type":"exceed_context_size_error","message":"the request exceeds the available context size"}}"#,
        ),
        Reply::Body(422, r#"{"error":{"message":"unknown field"}}"#),
    ])
    .await;
    let model = model(settings(&server, Some("m")));
    let mut errors = Vec::new();
    for _ in 0..3 {
        errors.push(
            collect(&model, request(user("x"), vec![]))
                .await
                .unwrap_err(),
        );
    }
    assert!(matches!(&errors[0], ModelError::ContextOverflow(m) if !m.contains("65536")));
    assert!(matches!(&errors[1], ModelError::ContextOverflow(_)));
    assert!(matches!(&errors[2], ModelError::Rejected(m) if m.contains("422")));
}
