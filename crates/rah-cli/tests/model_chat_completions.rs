//! `model-chat-completions` drives the real `rah` binary, loop, policy, tools
//! and session log against a loopback Chat Completions fixture.

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

const KEY_ENV: &str = "RAH_TEST_CHAT_COMPLETIONS_KEY";

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

fn sse(deltas: &[Value], finish_reason: &str) -> String {
    let mut body = String::new();
    for delta in deltas {
        let chunk = json!({"choices": [{"index": 0, "delta": delta, "finish_reason": null}]});
        body.push_str(&format!("data: {chunk}\n\n"));
    }
    let finish = json!({"choices": [{"index": 0, "delta": {}, "finish_reason": finish_reason}]});
    let usage = json!({"choices": [], "usage": {"prompt_tokens": 20, "completion_tokens": 5}});
    body.push_str(&format!(
        "data: {finish}\n\ndata: {usage}\n\ndata: [DONE]\n\n"
    ));
    body
}

#[tokio::test]
async fn chat_completions_plugin_drives_the_binary_loop_tools_and_session() {
    let echo = sse(
        &[
            json!({"role": "assistant", "content": "Calling echo."}),
            json!({"tool_calls": [{"index": 0, "id": "call_1", "type": "function", "function": {"name": "echo", "arguments": "{\"text\":"}}]}),
            json!({"tool_calls": [{"index": 0, "function": {"arguments": "\"hello\"}"}}]}),
        ],
        "tool_calls",
    );
    let done = sse(&[json!({"content": "echo said hello"})], "stop");
    let (origin, requests) = fixture(vec![echo, done]).await;

    let dir = TempDir::new("chat-completions-e2e");
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
use = "model-chat-completions"
config = {{ base_url = "{origin}/v1", api_key_env = "{KEY_ENV}" }}

[[plugin]]
use = "loop-react"

[[plugin]]
use = "frontend-cli"
config = {{ prompt = "say hello" }}
"#
        ),
    )
    .unwrap();
    let home = dir.0.join("home");
    let output = tokio::task::spawn_blocking(move || {
        Command::new(env!("CARGO_BIN_EXE_rah"))
            .arg("--config")
            .arg(&config)
            .args(["--model", "fixture-model"])
            .env(KEY_ENV, "fixture-key")
            .env("RAH_HOME", home)
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
    assert!(requests[0].0.starts_with("post /v1/chat/completions "));
    assert!(requests[0].0.contains("authorization: bearer fixture-key"));
    let first = &requests[0].1;
    // `--model` sets the model of a model-chat-completions plugin.
    assert_eq!(first["model"], "fixture-model");
    let tools: Vec<&str> = first["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["function"]["name"].as_str().unwrap())
        .collect();
    assert!(tools.contains(&"echo"), "{tools:?}");

    // The second request replays the tool round: the call joins the
    // assistant message, and the result is a `tool` message.
    let messages = requests[1].1["messages"].as_array().unwrap();
    let roles: Vec<&str> = messages
        .iter()
        .map(|m| m["role"].as_str().unwrap())
        .collect();
    assert_eq!(roles, ["user", "assistant", "tool"], "{messages:?}");
    assert_eq!(messages[1]["content"], "Calling echo.");
    assert_eq!(messages[1]["tool_calls"][0]["id"], "call_1");
    assert_eq!(
        messages[1]["tool_calls"][0]["function"],
        json!({"name": "echo", "arguments": "{\"text\":\"hello\"}"})
    );
    assert_eq!(messages[2]["tool_call_id"], "call_1");
    assert_eq!(messages[2]["content"], "hello");
}

#[test]
fn deepseek_preset_without_a_key_runs_but_reports_the_inactive_model() {
    let dir = TempDir::new("deepseek-no-key");
    let output = Command::new(env!("CARGO_BIN_EXE_rah"))
        .args(["--preset", "deepseek", "--prompt", "hi", "--session-dir"])
        .arg(dir.sessions())
        .arg("--root")
        .arg(dir.workspace())
        .env_remove("DEEPSEEK_API_KEY")
        .env("RAH_HOME", dir.0.join("home"))
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{stdout}\n{stderr}");
    assert!(
        stderr.contains("plugin `model-chat-completions` is not active"),
        "{stderr}"
    );
    assert!(stderr.contains("DEEPSEEK_API_KEY is not set"), "{stderr}");
    assert!(stdout.contains("no model provider is mounted"), "{stdout}");
}
