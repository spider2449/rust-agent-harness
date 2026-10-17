//! Deterministic loopback fixture tests; no network, credentials or model.

use std::sync::{Arc, Mutex};

use futures::StreamExt;
use rah_api::{
    CancellationToken, Kernel, MountOptions, PluginStatus,
    model::{ModelError, ModelProvider},
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
    Json(u16, Value),
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
                    Reply::Json(status, value) => (status, "application/json", value.to_string()),
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

fn frames(events: &[Value]) -> String {
    events.iter().map(|v| format!("data: {v}\n\n")).collect()
}

fn completed(output: Vec<Value>) -> Value {
    json!({"type":"response.completed","response":{"id":"r","status":"completed","output":output}})
}

fn text_events(text: &str) -> Vec<Value> {
    let item = json!({"type":"message","id":"m","role":"assistant","content":[{"type":"output_text","text":text}]});
    let mut events = vec![json!({"type":"response.created"})];
    for piece in text.split_inclusive(' ') {
        events.push(json!({"type":"response.output_text.delta","delta":piece}));
    }
    events.push(completed(vec![item]));
    events
}

fn function_events(name: &str, arguments: &str, with_indices: bool) -> Vec<Value> {
    let item = json!({"type":"function_call","id":"fc_1","call_id":"call_1","name":name,"arguments":arguments});
    let mut added = item.clone();
    added["arguments"] = json!("");
    let mut events = vec![
        json!({"type":"response.output_item.added","output_index":0,"item":added}),
        json!({"type":"response.function_call_arguments.delta","output_index":0,"item_id":"fc_1","delta":arguments}),
        json!({"type":"response.function_call_arguments.done","output_index":0,"item_id":"fc_1","arguments":arguments}),
        json!({"type":"response.output_item.done","output_index":0,"item":item}),
        completed(vec![item.clone()]),
    ];
    if !with_indices {
        events.retain(|e| e["type"] != "response.function_call_arguments.done");
        for event in &mut events {
            if let Some(map) = event.as_object_mut() {
                map.remove("output_index");
            }
        }
    }
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

fn openai(server: &Server) -> ResponsesModel {
    let endpoint = openai_endpoint(Some(&format!("{}/v1/responses", server.origin))).unwrap();
    ResponsesModel::openai(endpoint, KEY.into(), "gpt-test".into()).unwrap()
}

async fn collect(
    model: &ResponsesModel,
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
async fn openai_streams_text_and_sends_a_bounded_request() {
    let server = Server::new(vec![Reply::Sse(frames(&text_events("hello there")))]).await;
    let events = collect(&openai(&server), request(user("hi")))
        .await
        .unwrap();
    assert_eq!(text(&events), "hello there");
    assert_eq!(events.last(), Some(&ModelEvent::Completed));

    let (head, body) = &server.requests()[0];
    assert!(head.starts_with("POST /v1/responses HTTP/1.1"));
    assert!(head.to_ascii_lowercase().contains(&format!(
        "authorization: bearer {}",
        KEY.to_ascii_lowercase()
    )));
    assert_eq!(body["model"], "gpt-test");
    assert_eq!(body["store"], false);
    assert_eq!(body["stream"], true);
    assert_eq!(body["input"], json!([{"role":"user","content":"hi"}]));
    assert_eq!(body["tools"][0]["name"], "fs_read");
}

#[tokio::test]
async fn reported_usage_is_sent_before_completed() {
    let mut events = text_events("hi");
    let last = events.len() - 1;
    events[last]["response"]["usage"] = json!({
        "input_tokens": 120,
        "input_tokens_details": { "cached_tokens": 100 },
        "output_tokens": 7,
        "output_tokens_details": { "reasoning_tokens": 2 },
        "total_tokens": 127
    });
    let server = Server::new(vec![
        Reply::Sse(frames(&events)),
        Reply::Sse(frames(&text_events("hi"))),
    ])
    .await;
    let model = openai(&server);
    let events = collect(&model, request(user("hi"))).await.unwrap();
    assert_eq!(
        events[events.len() - 2..],
        [
            ModelEvent::Usage {
                usage: ModelUsage {
                    input_tokens: Some(120),
                    output_tokens: Some(7),
                    cached_input_tokens: Some(100),
                },
            },
            ModelEvent::Completed
        ]
    );
    // A response without usage sends none.
    let events = collect(&model, request(user("hi"))).await.unwrap();
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, ModelEvent::Usage { .. }))
    );
}

#[tokio::test]
async fn function_calls_map_back_to_the_original_tool_name() {
    let server = Server::new(vec![Reply::Sse(frames(&function_events(
        "fs_read",
        "{\"path\":\"a.txt\"}",
        true,
    )))])
    .await;
    let events = collect(&openai(&server), request(user("read")))
        .await
        .unwrap();
    assert_eq!(
        events,
        [
            ModelEvent::ToolCall {
                call_id: "call_1".into(),
                name: ToolName::new("fs.read"),
                arguments: json!({"path":"a.txt"}),
            },
            ModelEvent::Completed
        ]
    );
}

#[tokio::test]
async fn failures_are_typed_and_carry_no_secrets() {
    let truncated = frames(&text_events("cut")[..2]);
    let api_error = frames(&[json!({"type":"error","message":"secret-ish detail"})]);
    let unknown_tool = frames(&function_events("not_offered", "{}", true));
    let server = Server::new(vec![
        Reply::Status(401, "application/json"),
        Reply::Status(500, "application/json"),
        Reply::Status(200, "application/json"),
        Reply::Sse(truncated),
        Reply::Sse(api_error),
        Reply::Sse(unknown_tool),
    ])
    .await;
    let model = openai(&server);
    let mut errors = Vec::new();
    for _ in 0..6 {
        errors.push(collect(&model, request(user("x"))).await.unwrap_err());
    }
    assert!(matches!(&errors[0], ModelError::Rejected(m) if m.contains("authentication")));
    assert!(matches!(&errors[1], ModelError::Rejected(m) if m.contains("500")));
    assert!(matches!(errors[2], ModelError::Protocol(_)));
    assert!(matches!(errors[3], ModelError::Protocol(_)));
    assert!(matches!(errors[4], ModelError::Rejected(_)));
    assert!(matches!(errors[5], ModelError::Protocol(_)));
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
    let model = openai(&server);
    let cancel = CancellationToken::new();
    let mut stream = model
        .stream(request(user("x")), cancel.clone())
        .await
        .unwrap();
    cancel.cancel();
    assert_eq!(stream.next().await, Some(Err(ModelError::Cancelled)));
}

fn llama_discovery(models: &[&str]) -> Vec<Reply> {
    vec![
        Reply::Json(200, json!({"status":"ok"})),
        Reply::Json(
            200,
            json!({"data": models.iter().map(|id| json!({"id": id})).collect::<Vec<_>>()}),
        ),
    ]
}

#[tokio::test]
async fn llamacpp_discovers_lazily_normalizes_events_and_marks_assistant_messages() {
    let mut replies = vec![Reply::Json(503, json!({"error":"loading"}))];
    replies.extend(llama_discovery(&["qwen"]));
    replies.push(Reply::Sse(frames(&function_events(
        "fs_read",
        "{\"path\":\"b\"}",
        false,
    ))));
    replies.push(Reply::Sse(frames(&text_events("done"))));
    let server = Server::new(replies).await;
    let model = ResponsesModel::llamacpp(
        llamacpp_origin(&server.origin).unwrap(),
        String::new(),
        None,
    )
    .unwrap();

    // The server is still loading: a transport error, and nothing is cached.
    let error = collect(&model, request(user("x"))).await.unwrap_err();
    assert!(
        matches!(&error, ModelError::Transport(m) if m.contains("loading")),
        "{error}"
    );

    let events = collect(&model, request(user("read b"))).await.unwrap();
    assert!(matches!(&events[0], ModelEvent::ToolCall { name, .. } if name.as_str() == "fs.read"));

    let history = vec![
        ContextItem::User { text: "a".into() },
        ContextItem::Assistant { text: "b".into() },
        ContextItem::User { text: "c".into() },
    ];
    let events = collect(&model, request(history)).await.unwrap();
    assert_eq!(text(&events), "done");

    let requests = server.requests();
    assert!(requests[1].0.starts_with("GET /health"));
    assert!(requests[2].0.starts_with("GET /v1/models"));
    assert_eq!(requests[3].1["model"], "qwen");
    // Discovery is cached: the fourth request is the next Responses call.
    assert!(requests[4].0.starts_with("POST /v1/responses"));
    assert_eq!(requests[4].1["input"][1]["type"], "message");
}

#[tokio::test]
async fn llamacpp_requires_an_explicit_model_when_several_are_served() {
    let mut replies = llama_discovery(&["a", "b"]);
    replies.extend(llama_discovery(&["a", "b"]));
    let server = Server::new(replies).await;
    let origin = llamacpp_origin(&server.origin).unwrap();
    let ambiguous = ResponsesModel::llamacpp(origin.clone(), String::new(), None).unwrap();
    assert!(matches!(
        collect(&ambiguous, request(user("x"))).await,
        Err(ModelError::Rejected(_))
    ));
    let missing = ResponsesModel::llamacpp(origin, String::new(), Some("c".into())).unwrap();
    assert!(matches!(
        collect(&missing, request(user("x"))).await,
        Err(ModelError::Rejected(m)) if m.contains("not served")
    ));
}

#[tokio::test]
async fn llamacpp_sends_images_only_to_a_model_listed_with_image_input() {
    let screenshot = vec![
        ContextItem::User {
            text: "look".into(),
        },
        ContextItem::ToolCall {
            call_id: "c1".into(),
            name: ToolName::new("shot"),
            arguments: json!({}),
        },
        ContextItem::ToolResult {
            call_id: "c1".into(),
            output: rah_protocol::ToolOutput {
                content: vec![rah_protocol::ToolContent::Image(
                    rah_protocol::ImageContent {
                        media_type: "image/png".into(),
                        data: "AAAA".into(),
                    },
                )],
                is_error: false,
            },
        },
    ];
    for (vision, expected) in [
        (
            true,
            json!([
                {"type": "input_image", "image_url": "data:image/png;base64,AAAA"}
            ]),
        ),
        (false, json!("[image/png image, 1 KiB]")),
    ] {
        let modalities = if vision {
            json!(["text", "image"])
        } else {
            json!(["text"])
        };
        let server = Server::new(vec![
            Reply::Json(200, json!({"status":"ok"})),
            Reply::Json(
                200,
                json!({"data": [{"id": "m", "architecture": {"input_modalities": modalities}}]}),
            ),
            Reply::Sse(frames(&text_events("seen"))),
        ])
        .await;
        let model = ResponsesModel::llamacpp(
            llamacpp_origin(&server.origin).unwrap(),
            String::new(),
            None,
        )
        .unwrap();
        let events = collect(&model, request(screenshot.clone())).await.unwrap();
        assert_eq!(text(&events), "seen");
        let requests = server.requests();
        assert_eq!(
            requests[2].1["input"][2]["output"], expected,
            "vision {vision}"
        );
    }
}

#[test]
fn endpoints_are_bounded_to_official_or_loopback_origins() {
    assert_eq!(
        openai_endpoint(None).unwrap(),
        "https://api.openai.com/v1/responses"
    );
    assert!(openai_endpoint(Some("http://127.0.0.1:9/v1/responses")).is_ok());
    assert!(openai_endpoint(Some("http://[::1]:9/v1/responses")).is_ok());
    for rejected in [
        "https://evil.example/v1/responses",
        "http://user:pw@127.0.0.1/v1/responses",
        "http://127.0.0.1/v1/responses?x=1",
        "file:///etc/passwd",
    ] {
        assert!(openai_endpoint(Some(rejected)).is_err(), "{rejected}");
    }
    assert_eq!(
        llamacpp_origin("http://localhost:8080/").unwrap(),
        "http://localhost:8080"
    );
    for rejected in [
        "http://10.0.0.2:8080",
        "http://127.0.0.1:8080/v1",
        "ftp://127.0.0.1",
    ] {
        assert!(llamacpp_origin(rejected).is_err(), "{rejected}");
    }
}

#[tokio::test]
async fn plugins_fail_closed_on_missing_credentials_and_bad_configuration() {
    let kernel = Kernel::new();
    let status = kernel
        .mount(
            Arc::new(OpenAiResponsesPlugin::new("openai")),
            MountOptions::with_config(json!({"model":"m","api_key_env":"RAH_TEST_UNSET_KEY_7F3A"})),
        )
        .await
        .unwrap();
    assert!(
        matches!(&status, PluginStatus::Failed { message } if message.contains("RAH_TEST_UNSET_KEY_7F3A is not set")),
        "{status:?}"
    );
    let status = kernel
        .mount(
            Arc::new(OpenAiResponsesPlugin::new("no-model")),
            MountOptions::default(),
        )
        .await
        .unwrap_err();
    assert!(status.to_string().contains("configuration needs `model`"));
    let status = kernel
        .mount(
            Arc::new(LlamaCppPlugin::new("remote-llama")),
            MountOptions::with_config(json!({"endpoint":"http://192.168.1.5:8080"})),
        )
        .await
        .unwrap_err();
    assert!(status.to_string().contains("loopback"), "{status}");
    let status = kernel
        .mount(
            Arc::new(LlamaCppPlugin::new("llama")),
            MountOptions::default(),
        )
        .await
        .unwrap();
    assert_eq!(status, PluginStatus::Active);
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
