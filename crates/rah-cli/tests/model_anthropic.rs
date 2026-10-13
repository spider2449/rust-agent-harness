//! `model-anthropic` drives the real `rah` binary, loop, policy, tools and
//! session log against a loopback Messages API fixture.

mod common;

use std::{
    process::{Command, Stdio},
    sync::{Arc, Mutex},
};

use common::TempDir;
use serde_json::{Value, json};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

const KEY_ENV: &str = "RAH_TEST_ANTHROPIC_KEY";

/// Minimal sequential SSE fixture; captures each request head and body.
async fn fixture(replies: Vec<String>) -> (String, Arc<Mutex<Vec<(String, Value)>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let requests = Arc::new(Mutex::new(Vec::new()));
    let captured = requests.clone();
    tokio::spawn(async move {
        for body in replies {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut bytes = Vec::new();
            let end = loop {
                let mut chunk = [0; 4096];
                let n = socket.read(&mut chunk).await.unwrap();
                bytes.extend_from_slice(&chunk[..n]);
                if let Some(i) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                    break i + 4;
                }
            };
            let head = String::from_utf8_lossy(&bytes[..end]).to_ascii_lowercase();
            let length = head
                .lines()
                .find_map(|l| {
                    l.strip_prefix("content-length: ")
                        .map(|n| n.parse::<usize>().unwrap())
                })
                .unwrap_or(0);
            while bytes.len() < end + length {
                let mut chunk = [0; 4096];
                let n = socket.read(&mut chunk).await.unwrap();
                bytes.extend_from_slice(&chunk[..n]);
            }
            let request = serde_json::from_slice(&bytes[end..]).unwrap_or(Value::Null);
            captured.lock().unwrap().push((head, request));
            let header = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = socket.write_all(header.as_bytes()).await;
            let _ = socket.write_all(body.as_bytes()).await;
        }
    });
    (origin, requests)
}

fn sse(events: &[Value]) -> String {
    events
        .iter()
        .map(|v| format!("event: {}\ndata: {v}\n\n", v["type"].as_str().unwrap()))
        .collect()
}

fn message(blocks: &[Value], deltas: &[Value], stop_reason: &str) -> String {
    let mut events =
        vec![json!({"type":"message_start","message":{"type":"message","role":"assistant"}})];
    for (index, block) in blocks.iter().enumerate() {
        events.push(json!({"type":"content_block_start","index":index,"content_block":block}));
        let mut delta = deltas[index].clone();
        if !delta.is_null() {
            delta = json!({"type":"content_block_delta","index":index,"delta":delta});
            events.push(delta);
        }
        events.push(json!({"type":"content_block_stop","index":index}));
    }
    events.push(json!({"type":"message_delta","delta":{"stop_reason":stop_reason}}));
    events.push(json!({"type":"message_stop"}));
    sse(&events)
}

#[tokio::test]
async fn anthropic_plugin_drives_the_binary_loop_tools_and_session() {
    let echo = message(
        &[
            json!({"type":"text","text":""}),
            json!({"type":"tool_use","id":"toolu_1","name":"echo","input":{}}),
        ],
        &[
            json!({"type":"text_delta","text":"Calling echo."}),
            json!({"type":"input_json_delta","partial_json":"{\"text\":\"hello\"}"}),
        ],
        "tool_use",
    );
    let done = message(
        &[json!({"type":"text","text":""})],
        &[json!({"type":"text_delta","text":"echo said hello"})],
        "end_turn",
    );
    let (origin, requests) = fixture(vec![echo, done]).await;

    let dir = TempDir::new("anthropic-e2e");
    let sessions = dir.sessions().display().to_string().replace('\\', "/");
    let workspace = dir.workspace().display().to_string().replace('\\', "/");
    let config = dir.0.join("rah.toml");
    std::fs::write(
        &config,
        format!(
            r#"
[[plugin]]
use = "policy-authority"
config = {{ allow = ["none"], tools = {{ echo = "none" }} }}

[[plugin]]
use = "session-jsonl"
config = {{ dir = "{sessions}" }}

[[plugin]]
use = "tools-builtin"
config = {{ root = "{workspace}" }}

[[plugin]]
use = "model-anthropic"
config = {{ model = "claude-test", endpoint = "{origin}/v1/messages", api_key_env = "{KEY_ENV}" }}

[[plugin]]
use = "loop-react"

[[plugin]]
use = "frontend-cli"
config = {{ prompt = "say hello" }}
"#
        ),
    )
    .unwrap();
    let output = tokio::task::spawn_blocking(move || {
        Command::new(env!("CARGO_BIN_EXE_rah"))
            .arg("--config")
            .arg(&config)
            .env(KEY_ENV, "fixture-key")
            .stdin(Stdio::null())
            .output()
            .unwrap()
    })
    .await
    .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{stdout}\n{stderr}");
    assert!(stdout.contains("[tool] echo"), "{stdout}");
    assert!(stdout.contains("echo said hello"), "{stdout}");

    let requests = requests.lock().unwrap().clone();
    assert_eq!(requests.len(), 2);
    assert!(requests[0].0.contains("x-api-key: fixture-key"));
    let first = &requests[0].1;
    assert_eq!(first["model"], "claude-test");
    let tools: Vec<&str> = first["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert!(tools.contains(&"echo"), "{tools:?}");

    // The second request replays the tool round as alternating messages.
    let messages = requests[1].1["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 3, "{messages:?}");
    assert_eq!(messages[0]["role"], "user");
    assert_eq!(messages[1]["role"], "assistant");
    let call = messages[1]["content"]
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["type"] == "tool_use")
        .unwrap();
    assert_eq!(call["id"], "toolu_1");
    assert_eq!(call["input"], json!({"text":"hello"}));
    assert_eq!(messages[2]["role"], "user");
    assert_eq!(messages[2]["content"][0]["type"], "tool_result");
    assert_eq!(messages[2]["content"][0]["tool_use_id"], "toolu_1");
    assert_eq!(messages[2]["content"][0]["content"], "hello");
}

#[test]
fn anthropic_preset_without_a_key_runs_but_reports_the_inactive_model() {
    let dir = TempDir::new("anthropic-no-key");
    let output = Command::new(env!("CARGO_BIN_EXE_rah"))
        .args(["--preset", "anthropic", "--prompt", "hi", "--session-dir"])
        .arg(dir.sessions())
        .arg("--root")
        .arg(dir.workspace())
        .env_remove("ANTHROPIC_API_KEY")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{stdout}\n{stderr}");
    assert!(
        stderr.contains("plugin `model-anthropic` is not active"),
        "{stderr}"
    );
    assert!(stderr.contains("ANTHROPIC_API_KEY is not set"), "{stderr}");
    assert!(stdout.contains("no model provider is mounted"), "{stdout}");
}
