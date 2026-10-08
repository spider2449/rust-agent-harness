use super::*;
use rah_protocol::{
    AgentInput, AgentOptions, AgentRequest, PermissionLevel, RequestId, ToolContent,
    ToolDefinition, ToolOutput,
};
use rah_runtime::experimental_host::HostToolScope;
use rah_tools::{Tool, ToolContext, ToolError, ToolRegistry};
use std::{collections::VecDeque, error::Error, sync::atomic::AtomicUsize};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::Notify,
};

const SECRET: &str = "TASK507-FAKE-KEY-DO-NOT-EXPOSE";
struct Script {
    status: u16,
    chunks: Vec<Vec<u8>>,
    hold: bool,
}
impl Script {
    fn events(events: Vec<Value>) -> Self {
        let body = events
            .into_iter()
            .map(|v| {
                format!(
                    "event: {}\r\ndata: {v}\r\n\r\n",
                    v["type"].as_str().unwrap()
                )
            })
            .collect::<String>();
        Self {
            status: 200,
            chunks: body.as_bytes().chunks(3).map(<[u8]>::to_vec).collect(),
            hold: false,
        }
    }
    fn text(text: &str) -> Self {
        Self::events(text_events(text))
    }
}
struct Fixture {
    endpoint: String,
    requests: Arc<Mutex<Vec<Value>>>,
    arrived: Arc<Notify>,
    task: JoinHandle<()>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl Fixture {
    async fn new(scripts: Vec<Script>) -> Self {
        Self::scripted(scripts, None).await
    }
    async fn scripted(scripts: Vec<Script>, gate: Option<Arc<Notify>>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}/v1/responses", listener.local_addr().unwrap());
        let requests = Arc::new(Mutex::new(Vec::new()));
        let captured = requests.clone();
        let arrived = Arc::new(Notify::new());
        let notify = arrived.clone();
        let task = tokio::spawn(async move {
            let mut scripts = VecDeque::from(scripts);
            while let Some(script) = scripts.pop_front() {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut request = Vec::new();
                let header_end;
                loop {
                    let mut buf = [0; 1024];
                    let n = socket.read(&mut buf).await.unwrap();
                    assert!(n > 0);
                    request.extend_from_slice(&buf[..n]);
                    if let Some(end) = request.windows(4).position(|w| w == b"\r\n\r\n") {
                        header_end = end + 4;
                        break;
                    }
                }
                let headers = std::str::from_utf8(&request[..header_end]).unwrap();
                assert!(headers.starts_with("POST /v1/responses HTTP/1.1"));
                assert!(headers.to_ascii_lowercase().contains(&format!(
                    "authorization: bearer {}",
                    SECRET.to_ascii_lowercase()
                )));
                let length = headers
                    .lines()
                    .find_map(|l| {
                        l.to_ascii_lowercase()
                            .strip_prefix("content-length: ")
                            .map(|v| v.parse::<usize>().unwrap())
                    })
                    .unwrap();
                while request.len() < header_end + length {
                    let mut buf = [0; 1024];
                    let n = socket.read(&mut buf).await.unwrap();
                    assert!(n > 0);
                    request.extend_from_slice(&buf[..n]);
                }
                captured.lock().unwrap().push(
                    serde_json::from_slice(&request[header_end..header_end + length]).unwrap(),
                );
                notify.notify_one();
                if let Some(gate) = &gate {
                    gate.notified().await;
                }
                if script.status == 0 {
                    continue;
                }
                let header = format!(
                    "HTTP/1.1 {} Fixture\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n",
                    script.status
                );
                if socket.write_all(header.as_bytes()).await.is_err() {
                    continue;
                }
                for chunk in script.chunks {
                    if socket
                        .write_all(format!("{:x}\r\n", chunk.len()).as_bytes())
                        .await
                        .is_err()
                        || socket.write_all(&chunk).await.is_err()
                        || socket.write_all(b"\r\n").await.is_err()
                    {
                        break;
                    }
                    tokio::task::yield_now().await;
                }
                if script.hold {
                    let mut b = [0];
                    let _ = socket.read(&mut b).await;
                } else {
                    let _ = socket.write_all(b"0\r\n\r\n").await;
                }
            }
        });
        Self {
            endpoint,
            requests,
            arrived,
            task,
        }
    }
    async fn runtime(&self) -> Arc<dyn RuntimeInstance> {
        let mut factory = OpenAiFactory::new(SECRET);
        factory.endpoint = Some(self.endpoint.clone());
        factory.create().await.unwrap()
    }
}
fn text_events(text: &str) -> Vec<Value> {
    vec![
        json!({"type":"response.created"}),
        json!({"type":"response.in_progress"}),
        json!({"type":"response.output_text.delta","delta":text}),
        completed(vec![
            json!({"type":"message","id":"private-message","role":"assistant","content":[{"type":"output_text","text":text}]}),
        ]),
    ]
}

struct LocalServer {
    origin: String,
    requests: Arc<Mutex<Vec<String>>>,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for LocalServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl LocalServer {
    async fn new(replies: Vec<(u16, String)>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let requests = Arc::new(Mutex::new(Vec::new()));
        let captured = requests.clone();
        let task = tokio::spawn(async move {
            for (status, body) in replies {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut bytes = Vec::new();
                let header_end = loop {
                    let mut chunk = [0; 2048];
                    let n = socket.read(&mut chunk).await.unwrap();
                    if n == 0 {
                        return;
                    }
                    bytes.extend_from_slice(&chunk[..n]);
                    if let Some(i) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                        break i + 4;
                    }
                };
                let header = String::from_utf8(bytes[..header_end].to_vec()).unwrap();
                let len = header
                    .lines()
                    .find_map(|l| {
                        l.to_ascii_lowercase()
                            .strip_prefix("content-length: ")
                            .map(|n| n.parse::<usize>().unwrap())
                    })
                    .unwrap_or(0);
                while bytes.len() < header_end + len {
                    let mut chunk = [0; 2048];
                    let n = socket.read(&mut chunk).await.unwrap();
                    if n == 0 {
                        return;
                    }
                    bytes.extend_from_slice(&chunk[..n]);
                }
                captured
                    .lock()
                    .unwrap()
                    .push(String::from_utf8(bytes).unwrap());
                if body == "HOLD" {
                    socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n").await.unwrap();
                    let mut byte = [0];
                    let _ = socket.read(&mut byte).await;
                    continue;
                }
                let kind = if body.starts_with("data:") {
                    "text/event-stream"
                } else {
                    "application/json"
                };
                let header = format!(
                    "HTTP/1.1 {status} Fixture\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                if socket.write_all(header.as_bytes()).await.is_err() {
                    continue;
                }
                // Separate writes exercise the streaming transport, not a blocking JSON path.
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
    async fn llama(models: &[&str], turns: Vec<String>) -> Self {
        let mut replies = vec![
            (200, json!({"status":"ok"}).to_string()),
            (
                200,
                json!({"data": models.iter().map(|id| json!({"id":id})).collect::<Vec<_>>()})
                    .to_string(),
            ),
        ];
        replies.extend(turns.into_iter().map(|s| (200, s)));
        Self::new(replies).await
    }
}
fn frames(events: Vec<Value>) -> String {
    events
        .into_iter()
        .map(|v| format!("data: {v}\n\n"))
        .collect()
}
fn llama_function_events(values: &[i32]) -> Vec<Value> {
    let reasoning = json!({"id":"private-reasoning","type":"reasoning","summary":[],"content":[],"encrypted_content":""});
    let mut events = vec![json!({"type":"response.output_item.added","item":reasoning})];
    for mut event in function_events(values) {
        if event["type"] == "response.function_call_arguments.done" {
            continue;
        }
        event.as_object_mut().unwrap().remove("output_index");
        if event["type"] == "response.completed" {
            event["response"]["output"]
                .as_array_mut()
                .unwrap()
                .insert(0, reasoning.clone());
        }
        events.push(event);
    }
    events
}
fn llama_error(error: &RuntimeFailure) -> &LlamaCppError {
    error.source().unwrap().downcast_ref().unwrap()
}
#[tokio::test]
async fn llama_health_loading_malformed_and_unreachable() {
    for (status, body, expected) in [
        (503, "{}", LlamaCppError::Loading),
        (200, "oops", LlamaCppError::Malformed),
        (200, "{\"status\":\"loading\"}", LlamaCppError::Malformed),
    ] {
        let server = LocalServer::new(vec![(status, body.into())]).await;
        let error = LlamaCppFactory::new(&server.origin, None)
            .create()
            .await
            .err()
            .unwrap();
        assert_eq!(llama_error(&error), &expected);
    }
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    drop(listener);
    let error = LlamaCppFactory::new(origin, None)
        .create()
        .await
        .err()
        .unwrap();
    assert_eq!(llama_error(&error), &LlamaCppError::Unreachable);
}
#[test]
fn llama_endpoint_and_credential_boundaries() {
    for endpoint in [
        "http://example.com:8080",
        "http://127.0.0.1:8080/path",
        "http://key@localhost:8080",
        "http://localhost:8080?key=x",
        "ftp://localhost",
        "invalid",
    ] {
        assert!(LlamaCppFactory::new(endpoint, None).validate().is_err());
    }
    for endpoint in [
        "http://127.0.0.1:8080",
        "http://[::1]:8080",
        "http://localhost:8080",
    ] {
        LlamaCppFactory::new(endpoint, None).validate().unwrap();
    }
    assert!(
        !format!(
            "{:?}",
            LlamaCppFactory::new("http://localhost:8080", Some(SECRET.into()))
        )
        .contains(SECRET)
    );
}
#[tokio::test]
async fn assistant_replay_discriminator_is_local_provider_only() {
    fn request() -> TurnInput {
        let TurnInput::TextReplay(mut request) = replay() else {
            unreachable!()
        };
        request.input.messages.extend([
            Message {
                role: MessageRole::Assistant,
                content: "previous answer".into(),
            },
            Message {
                role: MessageRole::User,
                content: "next question".into(),
            },
        ]);
        TurnInput::TextReplay(request)
    }
    let official = Fixture::new(vec![Script::text("answer")]).await;
    let runtime = official.runtime().await;
    let (host, _) = scope();
    let chat = conversation(&runtime, host.port()).await;
    let events = collect(chat.send(request()).await.unwrap()).await;
    assert!(
        events
            .iter()
            .any(|e| matches!(e.event(), AgentEvent::Completed { .. }))
    );
    runtime.shutdown().await.unwrap();
    assert_eq!(
        official.requests.lock().unwrap()[0]["input"][1],
        json!({"role":"assistant","content":"previous answer"})
    );

    let local = LocalServer::llama(&["fixture-model"], vec![frames(text_events("answer"))]).await;
    let runtime = LlamaCppFactory::new(&local.origin, None)
        .create()
        .await
        .unwrap();
    let (host, _) = scope();
    let chat = conversation(&runtime, host.port()).await;
    let events = collect(chat.send(request()).await.unwrap()).await;
    assert!(
        events
            .iter()
            .any(|e| matches!(e.event(), AgentEvent::Completed { .. }))
    );
    runtime.shutdown().await.unwrap();
    let requests = local.requests.lock().unwrap();
    let body: Value = serde_json::from_str(requests[2].split_once("\r\n\r\n").unwrap().1).unwrap();
    assert_eq!(
        body["input"][1],
        json!({"type":"message","role":"assistant","content":"previous answer"})
    );
    assert_eq!(
        body["input"][0],
        json!({"role":"user","content":"fixture user"})
    );
}

#[tokio::test]
async fn llama_health_models_stream_and_reconnect() {
    for _ in 0..2 {
        let server =
            LocalServer::llama(&["local-model"], vec![frames(text_events("RAH_LLAMA_OK"))]).await;
        let runtime = LlamaCppFactory::new(&server.origin, None)
            .create()
            .await
            .unwrap();
        assert!(runtime.capabilities().streaming);
        let (scope, _) = scope();
        let conversation = runtime
            .open(ConversationSeed {
                id: ConversationId::new("host-owned"),
                model: ModelSelection::RuntimeDefault,
                tools: scope.port(),
            })
            .await
            .unwrap();
        assert_eq!(conversation.id().as_str(), "host-owned");
        let events = collect(conversation.send(replay()).await.unwrap()).await;
        assert!(
            events
                .iter()
                .any(|e| matches!(e.event(), AgentEvent::Completed { .. }))
        );
        assert!(events.iter().any(
            |e| matches!(e.event(), AgentEvent::ModelDelta { delta, .. } if delta == "RAH_LLAMA_OK")
        ));
        conversation.close().await.unwrap();
        runtime.shutdown().await.unwrap();
        assert!(!runtime.is_alive());
        let requests = server.requests.lock().unwrap();
        assert!(requests[0].starts_with("GET /health "));
        assert!(requests[1].starts_with("GET /v1/models "));
        assert!(requests[2].starts_with("POST /v1/responses "));
        assert!(
            requests
                .iter()
                .all(|r| !r.to_ascii_lowercase().contains("authorization:"))
        );
        assert!(requests[2].contains("local-model"));
    }
}
#[tokio::test]
async fn llama_model_selection_and_malformed_stream() {
    let server = LocalServer::llama(&["one", "two"], vec!["data: malformed\n\n".into()]).await;
    let runtime = LlamaCppFactory::new(&server.origin, None)
        .create()
        .await
        .unwrap();
    let (scope, _) = scope();
    for model in [
        ModelSelection::RuntimeDefault,
        ModelSelection::Explicit("absent".into()),
    ] {
        assert!(
            runtime
                .open(ConversationSeed {
                    id: ConversationId::new("host"),
                    model,
                    tools: scope.port()
                })
                .await
                .is_err()
        );
    }
    let c = runtime
        .open(ConversationSeed {
            id: ConversationId::new("host"),
            model: ModelSelection::Explicit("one".into()),
            tools: scope.port(),
        })
        .await
        .unwrap();
    let events = collect(c.send(replay()).await.unwrap()).await;
    assert_eq!(source(failure(&events)), &E::EventJson);
    runtime.shutdown().await.unwrap();
}
#[tokio::test]
async fn llama_provider_local_key_and_host_tool_round_trip() {
    let local_calls = llama_function_events(&[7]);
    let server = LocalServer::llama(
        &["local-model"],
        vec![frames(local_calls), frames(text_events("done"))],
    )
    .await;
    let runtime = LlamaCppFactory::new(&server.origin, Some("LOCAL-ONLY".into()))
        .create()
        .await
        .unwrap();
    let (scope, effects) = scope();
    let c = runtime
        .open(ConversationSeed {
            id: ConversationId::new("host"),
            model: ModelSelection::RuntimeDefault,
            tools: scope.port(),
        })
        .await
        .unwrap();
    let events = collect(c.send(replay()).await.unwrap()).await;
    assert!(
        events
            .iter()
            .any(|e| matches!(e.event(), AgentEvent::Completed { .. }))
    );
    assert_eq!(effects.load(Ordering::SeqCst), 1);
    runtime.shutdown().await.unwrap();
    let requests = server.requests.lock().unwrap();
    assert!(!requests[0].contains("LOCAL-ONLY"));
    assert!(
        requests[1..]
            .iter()
            .all(|r| r.contains("LOCAL-ONLY") && !r.contains(SECRET))
    );
    assert!(requests[3].contains("function_call_output"));
}
#[tokio::test]
async fn llama_cancel_stream_then_next_turn_recovers() {
    let server = LocalServer::llama(
        &["local"],
        vec!["HOLD".into(), frames(text_events("recovered"))],
    )
    .await;
    let runtime = LlamaCppFactory::new(&server.origin, None)
        .create()
        .await
        .unwrap();
    let (scope, _) = scope();
    let c = runtime
        .open(ConversationSeed {
            id: ConversationId::new("host"),
            model: ModelSelection::RuntimeDefault,
            tools: scope.port(),
        })
        .await
        .unwrap();
    let turn = c.send(replay()).await.unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        while server.requests.lock().unwrap().len() < 3 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let control = turn.control.clone();
    let events = tokio::spawn(collect(turn));
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(5), control.cancel())
            .await
            .unwrap()
            .unwrap(),
        CancelOutcome::Stopped
    );
    let events = events.await.unwrap();
    assert_eq!(source(failure(&events)), &E::Cancelled);
    assert!(
        !events
            .iter()
            .any(|e| matches!(e.event(), AgentEvent::Completed { .. }))
    );
    assert!(runtime.is_alive());
    let events = collect(c.send(replay()).await.unwrap()).await;
    assert!(
        events
            .iter()
            .any(|e| matches!(e.event(), AgentEvent::Completed { .. }))
    );
    c.close().await.unwrap();
    runtime.shutdown().await.unwrap();
}
#[test]
fn llama_missing_indices_are_correlated_by_item_and_final_arguments_stay_checked() {
    for wrong in ["id", "call_id", "name", "arguments"] {
        let mut normalizer = crate::llamacpp::EventNormalizer::default();
        let mut parser = protocol::Response::default();
        let mut rejected = false;
        for mut event in function_events(&[7]) {
            if event["type"] == "response.function_call_arguments.done" {
                continue;
            }
            event.as_object_mut().unwrap().remove("output_index");
            if event["type"] == "response.output_item.done" {
                event["item"][wrong] = json!("mismatched");
            }
            let events = match normalizer.normalize(event) {
                Ok(events) => events,
                Err(_) => {
                    rejected = true;
                    break;
                }
            };
            for event in events {
                if parser.event(event).is_err() {
                    rejected = true;
                    break;
                }
            }
            if rejected {
                break;
            }
        }
        assert!(rejected, "mismatched {wrong} admitted");
        assert!(parser.output.is_none());
    }
    let mut normalizer = crate::llamacpp::EventNormalizer::default();
    assert!(normalizer.normalize(json!({"type":"response.function_call_arguments.delta","item_id":"unknown","delta":"{}"})).is_err());
    let added = json!({"type":"response.output_item.added","item":{"type":"function_call","id":"id","call_id":"call","name":"echo","arguments":""}});
    normalizer.normalize(added.clone()).unwrap();
    assert!(normalizer.normalize(added).is_err());
    assert!(normalizer.normalize(json!({"type":"response.function_call_arguments.delta","item_id":"id","output_index":99,"delta":"{}"})).is_err());
}

#[test]
fn llama_reasoning_before_function_preserves_completed_output_order() {
    let bytes = frames(llama_function_events(&[7])).into_bytes();
    let mut sse = crate::sse::Sse::default();
    let events = sse.push(&bytes).unwrap();
    sse.finish().unwrap();
    let mut normalizer = crate::llamacpp::EventNormalizer::default();
    let mut response = protocol::Response::default();
    for event in events {
        let kind = event["type"].as_str().unwrap().to_owned();
        for normalized in normalizer
            .normalize(event)
            .unwrap_or_else(|error| panic!("normalization failed at {kind}: {error}"))
        {
            let normalized_kind = normalized["type"].as_str().unwrap().to_owned();
            response
                .event(normalized)
                .unwrap_or_else(|error| panic!("parser failed at {normalized_kind}: {error}"));
        }
    }
    assert!(response.output.is_some());
}
fn completed(output: Vec<Value>) -> Value {
    json!({"type":"response.completed","response":{"id":"private-response-id","status":"completed","output":output}})
}
fn function_events(values: &[i32]) -> Vec<Value> {
    let mut events = Vec::new();
    let mut output = Vec::new();
    for (i, value) in values.iter().enumerate() {
        let args = format!("{{\"value\":{value}}}");
        let item = json!({"type":"function_call","id":format!("private-fc-{i}"),"call_id":format!("private-call-{i}"),"name":"test_effect","arguments":args});
        let mut added = item.clone();
        added["arguments"] = json!("");
        events.push(json!({"type":"response.output_item.added","output_index":i,"item":added}));
        for delta in [&args[..5], &args[5..]] {
            events.push(json!({"type":"response.function_call_arguments.delta","output_index":i,"item_id":item["id"],"delta":delta}));
        }
        events.push(json!({"type":"response.function_call_arguments.done","output_index":i,"item_id":item["id"],"arguments":args}));
        events.push(json!({"type":"response.output_item.done","output_index":i,"item":item}));
        output.push(item);
    }
    events.push(completed(output));
    events
}
struct Effect {
    count: Arc<AtomicUsize>,
}
#[async_trait]
impl Tool for Effect {
    fn definition(&self) -> ToolDefinition {
        definition(
            json!({"type":"object","properties":{"value":{"type":"integer"}},"required":["value"],"additionalProperties":false}),
        )
    }
    async fn execute(&self, input: ToolInput, _: ToolContext) -> Result<ToolOutput, ToolError> {
        self.count.fetch_add(1, Ordering::SeqCst);
        Ok(ToolOutput {
            content: vec![ToolContent::Json(input.0)],
            is_error: false,
        })
    }
}
fn definition(schema: Value) -> ToolDefinition {
    ToolDefinition {
        name: ToolName::new("test_effect"),
        description: "fixture".into(),
        input_schema: schema,
        permission: PermissionLevel::Execute,
    }
}
fn scope() -> (HostToolScope, Arc<AtomicUsize>) {
    let count = Arc::new(AtomicUsize::new(0));
    let mut registry = ToolRegistry::new();
    registry
        .register(Arc::new(Effect {
            count: count.clone(),
        }))
        .unwrap();
    (
        HostToolScope::new(Arc::new(registry), vec![PermissionLevel::Execute]),
        count,
    )
}
fn replay() -> TurnInput {
    TurnInput::TextReplay(AgentRequest {
        request_id: RequestId::new(),
        input: AgentInput {
            messages: vec![Message {
                role: MessageRole::User,
                content: "fixture user".into(),
            }],
        },
        options: AgentOptions::default(),
    })
}
async fn conversation(
    runtime: &Arc<dyn RuntimeInstance>,
    tools: Arc<dyn HostToolPort>,
) -> Arc<dyn RuntimeConversation> {
    runtime
        .open(ConversationSeed {
            id: ConversationId::new("rah-conversation"),
            model: ModelSelection::Explicit("fixture-model".into()),
            tools,
        })
        .await
        .unwrap()
}
async fn collect(turn: TurnHandle) -> Vec<RuntimeEvent> {
    tokio::time::timeout(Duration::from_secs(10), turn.events.collect())
        .await
        .unwrap()
}
fn source(f: &RuntimeFailure) -> &E {
    f.source().unwrap().downcast_ref::<E>().unwrap()
}
fn safe(f: &RuntimeFailure) {
    for projection in [
        format!("{f}"),
        format!("{f:?}"),
        serde_json::to_string(f.diagnostic()).unwrap(),
        format!("{}", source(f)),
        format!("{:?}", source(f)),
    ] {
        assert!(!projection.contains(SECRET));
        assert!(!projection.contains("private-"));
    }
}
fn failure(events: &[RuntimeEvent]) -> &RuntimeFailure {
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e.event(), AgentEvent::Failed { .. }))
            .count(),
        1
    );
    assert!(
        !events
            .iter()
            .any(|e| matches!(e.event(), AgentEvent::Completed { .. }))
    );
    events.iter().find_map(RuntimeEvent::failure).unwrap()
}

#[tokio::test]
async fn factory_capabilities_explicit_model_and_configuration_redaction() {
    for key in ["", "\r\nsecret"] {
        let factory = OpenAiFactory::new(key);
        let error = factory.create().await.err().unwrap();
        assert_eq!(source(&error), &E::Configuration);
        safe(&error);
    }
    assert!(!format!("{:?}", OpenAiFactory::new(SECRET)).contains(SECRET));
    let f = Fixture::new(vec![]).await;
    let runtime = f.runtime().await;
    assert_eq!(
        runtime.capabilities(),
        Capabilities {
            discovery: false,
            native_continuation: false,
            text_replay: true,
            tool_calls: true,
            cancellation: true,
            streaming: true
        }
    );
    assert_eq!(
        runtime.discover_models().await.unwrap(),
        ModelDiscovery::Unsupported
    );
    let (scope, _) = scope();
    let error = runtime
        .open(ConversationSeed {
            id: ConversationId::new("x"),
            model: ModelSelection::RuntimeDefault,
            tools: scope.port(),
        })
        .await
        .err()
        .unwrap();
    assert_eq!(source(&error), &E::Configuration);
    assert!(f.requests.lock().unwrap().is_empty());
    runtime.shutdown().await.unwrap();
}
#[tokio::test]
async fn ordinary_text_arbitrary_chunks_completion_once_and_replay() {
    let f = Fixture::new(vec![Script::text("中文🙂"), Script::text("next")]).await;
    let runtime = f.runtime().await;
    let (scope, _) = scope();
    let c = conversation(&runtime, scope.port()).await;
    assert_eq!(c.id().as_str(), "rah-conversation");
    for expected in ["中文🙂", "next"] {
        let events = collect(c.send(replay()).await.unwrap()).await;
        let text = events
            .iter()
            .filter_map(|e| {
                if let AgentEvent::ModelDelta { delta, .. } = e.event() {
                    Some(delta.as_str())
                } else {
                    None
                }
            })
            .collect::<String>();
        assert_eq!(text, expected);
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e.event(), AgentEvent::Completed { .. }))
                .count(),
            1
        );
        assert!(!format!("{events:?}").contains("private-response-id"));
    }
    {
        let requests = f.requests.lock().unwrap();
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0]["model"], "fixture-model");
        assert_eq!(requests[0]["stream"], true);
        assert_eq!(requests[0]["store"], false);
        assert_eq!(requests[1]["input"].as_array().unwrap().len(), 1);
        drop(requests);
    }
    runtime.shutdown().await.unwrap();
}
#[tokio::test]
async fn function_round_trip_and_two_calls_do_not_cross_results() {
    for values in [vec![7], vec![11, 22]] {
        let f = Fixture::new(vec![
            Script::events(function_events(&values)),
            Script::text("used results"),
        ])
        .await;
        let runtime = f.runtime().await;
        let (scope, count) = scope();
        let c = conversation(&runtime, scope.port()).await;
        let events = collect(c.send(replay()).await.unwrap()).await;
        assert!(matches!(
            events.last().unwrap().event(),
            AgentEvent::Completed { .. }
        ));
        let calls = events
            .iter()
            .filter_map(|e| {
                if let AgentEvent::ToolRequested { tool_call, .. } = e.event() {
                    Some(tool_call)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), values.len());
        assert_eq!(count.load(Ordering::SeqCst), values.len());
        if calls.len() == 2 {
            assert_ne!(calls[0].id, calls[1].id);
        }
        for call in &calls {
            assert_eq!(call.name.as_str(), "test_effect");
            assert!(!call.id.to_string().contains("private-call"));
        }
        {
            let requests = f.requests.lock().unwrap();
            assert_eq!(requests.len(), 2);
            let outputs = requests[1]["input"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|v| v["type"] == "function_call_output")
                .collect::<Vec<_>>();
            for (i, output) in outputs.iter().enumerate() {
                assert_eq!(output["call_id"], format!("private-call-{i}"));
                let result: ToolOutput =
                    serde_json::from_str(output["output"].as_str().unwrap()).unwrap();
                assert_eq!(
                    result.content,
                    vec![ToolContent::Json(json!({"value":values[i]}))]
                );
            }
            assert_eq!(requests[0]["tools"][0]["strict"], true);
            drop(requests);
        }
        runtime.shutdown().await.unwrap();
    }
}
#[tokio::test]
async fn revoked_retained_conversation_and_denied_host_have_zero_effects() {
    let f = Fixture::new(vec![]).await;
    let runtime = f.runtime().await;
    let (scope, count) = scope();
    let c = conversation(&runtime, scope.port()).await;
    scope.revoke();
    assert!(c.send(replay()).await.is_err());
    assert_eq!(count.load(Ordering::SeqCst), 0);
    assert!(f.requests.lock().unwrap().is_empty());
    runtime.shutdown().await.unwrap();
    let f = Fixture::new(vec![Script::events(function_events(&[1]))]).await;
    let runtime = f.runtime().await;
    let count = Arc::new(AtomicUsize::new(0));
    let mut registry = ToolRegistry::new();
    registry
        .register(Arc::new(Effect {
            count: count.clone(),
        }))
        .unwrap();
    let scope = HostToolScope::new(Arc::new(registry), vec![]);
    let c = conversation(&runtime, scope.port()).await;
    let events = collect(c.send(replay()).await.unwrap()).await;
    failure(&events);
    assert_eq!(count.load(Ordering::SeqCst), 0);
    assert_eq!(f.requests.lock().unwrap().len(), 1);
    runtime.shutdown().await.unwrap();
}
#[tokio::test]
async fn statuses_malformed_sse_json_api_and_interrupted_stream_are_typed_safe() {
    let cases = vec![
        (
            Script {
                status: 401,
                chunks: vec![SECRET.as_bytes().to_vec()],
                hold: false,
            },
            E::HttpStatus(401),
        ),
        (
            Script {
                status: 403,
                chunks: vec![],
                hold: false,
            },
            E::HttpStatus(403),
        ),
        (
            Script {
                status: 429,
                chunks: vec![],
                hold: false,
            },
            E::HttpStatus(429),
        ),
        (
            Script {
                status: 500,
                chunks: vec![],
                hold: false,
            },
            E::HttpStatus(500),
        ),
        (
            Script {
                status: 400,
                chunks: vec![],
                hold: false,
            },
            E::HttpStatus(400),
        ),
        (
            Script {
                status: 200,
                chunks: vec![b"data: \xff\n\n".to_vec()],
                hold: false,
            },
            E::Sse,
        ),
        (
            Script {
                status: 200,
                chunks: vec![format!("data: {SECRET}\n\n").into_bytes()],
                hold: false,
            },
            E::EventJson,
        ),
        (
            Script::events(vec![json!({"type":"error","message":SECRET})]),
            E::Api,
        ),
        (
            Script::events(vec![
                json!({"type":"response.output_text.delta","delta":"partial"}),
            ]),
            E::Protocol,
        ),
        (
            Script {
                status: 200,
                chunks: vec![b"data: {".to_vec()],
                hold: false,
            },
            E::Sse,
        ),
    ];
    for (script, expected) in cases {
        let f = Fixture::new(vec![script]).await;
        let runtime = f.runtime().await;
        let (scope, _) = scope();
        let c = conversation(&runtime, scope.port()).await;
        let events = collect(c.send(replay()).await.unwrap()).await;
        let error = failure(&events);
        assert_eq!(source(error), &expected);
        safe(error);
        runtime.shutdown().await.unwrap();
    }
}
#[tokio::test]
async fn continuation_bound_is_typed_and_does_not_execute_ninth_round() {
    let f = Fixture::new(
        (0..=MAX_TOOL_ROUNDS)
            .map(|_| Script::events(function_events(&[1])))
            .collect(),
    )
    .await;
    let runtime = f.runtime().await;
    let (scope, count) = scope();
    let c = conversation(&runtime, scope.port()).await;
    let events = collect(c.send(replay()).await.unwrap()).await;
    let error = failure(&events);
    assert_eq!(source(error), &E::ContinuationLimit);
    safe(error);
    assert_eq!(count.load(Ordering::SeqCst), MAX_TOOL_ROUNDS);
    assert_eq!(f.requests.lock().unwrap().len(), MAX_TOOL_ROUNDS + 1);
    runtime.shutdown().await.unwrap();
}
#[tokio::test]
async fn cancel_open_stream_is_terminal_and_shutdown_rejects_new_network() {
    let f = Fixture::new(vec![Script {
        status: 200,
        chunks: vec![],
        hold: true,
    }])
    .await;
    let runtime = f.runtime().await;
    let (scope, _) = scope();
    let c = conversation(&runtime, scope.port()).await;
    let turn = c.send(replay()).await.unwrap();
    f.arrived.notified().await;
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(5), turn.control.cancel())
            .await
            .unwrap()
            .unwrap(),
        CancelOutcome::Stopped
    );
    let control = turn.control.clone();
    let events = collect(turn).await;
    let error = failure(&events);
    assert_eq!(source(error), &E::Cancelled);
    safe(error);
    assert_eq!(
        control.cancel().await.unwrap(),
        CancelOutcome::AlreadyTerminal
    );
    runtime.shutdown().await.unwrap();
    assert!(!runtime.is_alive());
    assert!(c.send(replay()).await.is_err());
    let error = runtime
        .open(ConversationSeed {
            id: ConversationId::new("new"),
            model: ModelSelection::Explicit("fixture".into()),
            tools: scope.port(),
        })
        .await
        .err()
        .unwrap();
    assert_eq!(source(&error), &E::Shutdown);
    assert_eq!(f.requests.lock().unwrap().len(), 1);
}
#[tokio::test]
async fn shutdown_cancels_an_active_stream_and_close_rejects_send() {
    let f = Fixture::new(vec![Script {
        status: 200,
        chunks: vec![],
        hold: true,
    }])
    .await;
    let runtime = f.runtime().await;
    let (scope, _) = scope();
    let c = conversation(&runtime, scope.port()).await;
    let turn = c.send(replay()).await.unwrap();
    f.arrived.notified().await;
    runtime.shutdown().await.unwrap();
    let events = collect(turn).await;
    assert_eq!(source(failure(&events)), &E::Cancelled);
    c.close().await.unwrap();
    assert!(c.send(replay()).await.is_err());
    runtime.shutdown().await.unwrap();
}
#[test]
fn schemas_preserve_meaning_and_unsupported_names_fail() {
    let optional = json!({"type":"object","properties":{"value":{"type":"integer"}}});
    let translated = protocol::tools(&[definition(optional.clone())]).unwrap();
    assert_eq!(translated[0]["parameters"], optional);
    assert_eq!(translated[0]["strict"], false);
    let mut d = definition(json!({}));
    d.name = ToolName::new("repo.status");
    assert_eq!(protocol::tools(&[d]).unwrap()[0]["name"], "repo.status");
    let mut d = definition(json!({}));
    d.name = ToolName::new("");
    assert_eq!(protocol::tools(&[d]).unwrap_err(), E::ToolSchema);
    assert_eq!(
        protocol::tools(&[definition(json!(true))]).unwrap_err(),
        E::ToolSchema
    );
}
#[test]
fn sse_all_chunk_boundaries_multiline_cr_lf_and_limits() {
    let body = ": comment\r\nevent: response.output_text.delta\r\ndata: {\r\ndata: \"type\":\"response.output_text.delta\",\"delta\":\"中文🙂\"}\r\n\r\n";
    for split in 0..=body.len() {
        let mut p = sse::Sse::default();
        let mut e = p.push(&body.as_bytes()[..split]).unwrap();
        e.extend(p.push(&body.as_bytes()[split..]).unwrap());
        p.finish().unwrap();
        assert_eq!(e.len(), 1);
        assert_eq!(e[0]["delta"], "中文🙂");
    }
    for ending in ["\n", "\r", "\r\n"] {
        let frame = format!("data: {{\"type\":\"response.created\"}}{ending}{ending}");
        let mut p = sse::Sse::default();
        assert_eq!(p.push(frame.repeat(2).as_bytes()).unwrap().len(), 2);
        p.finish().unwrap();
    }
    assert_eq!(
        sse::Sse::default()
            .push(&vec![b'x'; sse::MAX_FRAME + 1])
            .unwrap_err(),
        E::Sse
    );
}

#[tokio::test]
async fn revocation_after_turn_admission_before_function_dispatch_has_zero_effect() {
    let gate = Arc::new(Notify::new());
    let f = Fixture::scripted(
        vec![Script::events(function_events(&[1]))],
        Some(gate.clone()),
    )
    .await;
    let runtime = f.runtime().await;
    let (scope, count) = scope();
    let c = conversation(&runtime, scope.port()).await;
    let turn = c.send(replay()).await.unwrap();
    f.arrived.notified().await;
    scope.revoke();
    gate.notify_one();
    let events = collect(turn).await;
    failure(&events);
    assert_eq!(count.load(Ordering::SeqCst), 0);
    assert_eq!(f.requests.lock().unwrap().len(), 1);
    assert!(c.send(replay()).await.is_err());
    runtime.shutdown().await.unwrap();
}
#[tokio::test]
async fn transport_disconnect_is_typed_without_retry() {
    let f = Fixture::new(vec![Script {
        status: 0,
        chunks: vec![],
        hold: false,
    }])
    .await;
    let runtime = f.runtime().await;
    let (scope, _) = scope();
    let c = conversation(&runtime, scope.port()).await;
    let events = collect(c.send(replay()).await.unwrap()).await;
    let error = failure(&events);
    assert_eq!(source(error), &E::Transport);
    safe(error);
    assert_eq!(f.requests.lock().unwrap().len(), 1);
    runtime.shutdown().await.unwrap();
}
#[tokio::test]
async fn text_deltas_are_ordered_once_and_drop_releases_control() {
    let mut events = text_events("onetwo三");
    events.splice(
        2..3,
        [
            json!({"type":"response.output_text.delta","delta":"one"}),
            json!({"type":"response.output_text.delta","delta":"two"}),
            json!({"type":"response.output_text.delta","delta":"三"}),
        ],
    );
    let f = Fixture::new(vec![
        Script::events(events),
        Script {
            status: 200,
            chunks: vec![],
            hold: true,
        },
    ])
    .await;
    let runtime = f.runtime().await;
    let (scope, _) = scope();
    let c = conversation(&runtime, scope.port()).await;
    let turn = c.send(replay()).await.unwrap();
    let events = collect(turn).await;
    assert_eq!(
        events
            .iter()
            .filter_map(
                |e| if let AgentEvent::ModelDelta { delta, .. } = e.event() {
                    Some(delta.as_str())
                } else {
                    None
                }
            )
            .collect::<Vec<_>>(),
        vec!["one", "two", "三"]
    );
    let turn = c.send(replay()).await.unwrap();
    let control = turn.control.clone();
    drop(turn);
    tokio::time::timeout(Duration::from_secs(5), control.cancel())
        .await
        .unwrap()
        .unwrap();
    runtime.shutdown().await.unwrap();
}
#[test]
fn malformed_function_identity_partial_arguments_and_duplicate_completion_fail_closed() {
    let mut events = function_events(&[1, 2]);
    events[5]["item"]["call_id"] = json!("private-call-0");
    let mut response = protocol::Response::default();
    assert!(events.into_iter().any(|e| response.event(e).is_err()));
    let mut events = function_events(&[1]);
    events[3]["arguments"] = json!("{\"value\":99}");
    let mut response = protocol::Response::default();
    assert!(events.into_iter().any(|e| response.event(e).is_err()));
    let mut response = protocol::Response::default();
    assert!(response.event(completed(vec![json!({"type":"function_call","id":"private","call_id":"private","name":"test_effect","arguments":"{}"})])).is_err());
    let mut response = protocol::Response::default();
    for e in text_events("x") {
        response.event(e).unwrap();
    }
    assert!(response.event(completed(vec![])).is_err());
}
