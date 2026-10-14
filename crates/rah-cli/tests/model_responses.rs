//! P3 acceptance: a Responses model plugin drives the real loop, tools and
//! session log against a loopback llama-server fixture.

mod common;

use std::{
    process::{Command, Stdio},
    sync::{Arc, Mutex},
};

use common::{TempDir, entries, headless, kernel, kinds, new_session, reply, turn};
use serde_json::{Value, json};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

/// Minimal sequential HTTP fixture: replies `(status, content type, body)`.
async fn fixture(replies: Vec<(u16, &'static str, String)>) -> (String, Arc<Mutex<Vec<Value>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let bodies = Arc::new(Mutex::new(Vec::new()));
    let captured = bodies.clone();
    tokio::spawn(async move {
        for (status, kind, body) in replies {
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
            captured
                .lock()
                .unwrap()
                .push(serde_json::from_slice(&bytes[end..]).unwrap_or(Value::Null));
            let header = format!(
                "HTTP/1.1 {status} OK\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = socket.write_all(header.as_bytes()).await;
            let _ = socket.write_all(body.as_bytes()).await;
        }
    });
    (origin, bodies)
}

fn sse(events: &[Value]) -> String {
    events.iter().map(|v| format!("data: {v}\n\n")).collect()
}

fn completed(output: Vec<Value>) -> Value {
    json!({"type":"response.completed","response":{"status":"completed","output":output}})
}

/// llama-server style: no output_index, no separate arguments-done event.
fn echo_call() -> String {
    let item = json!({"type":"function_call","id":"fc_1","call_id":"call_1","name":"echo","arguments":"{\"text\":\"hello\"}"});
    let mut added = item.clone();
    added["arguments"] = json!("");
    sse(&[
        json!({"type":"response.output_item.added","item":added}),
        json!({"type":"response.function_call_arguments.delta","item_id":"fc_1","delta":"{\"text\":\"hello\"}"}),
        json!({"type":"response.output_item.done","item":item}),
        completed(vec![item.clone()]),
    ])
}

fn text(reply: &str) -> String {
    let item = json!({"type":"message","role":"assistant","content":[{"type":"output_text","text":reply}]});
    sse(&[
        json!({"type":"response.output_text.delta","delta":reply}),
        completed(vec![item]),
    ])
}

#[tokio::test]
async fn llamacpp_plugin_drives_the_loop_tools_and_session() {
    let (origin, bodies) = fixture(vec![
        (200, "application/json", json!({"status":"ok"}).to_string()),
        (
            200,
            "application/json",
            json!({"data":[{"id":"local-model"}]}).to_string(),
        ),
        (200, "text/event-stream", echo_call()),
        (200, "text/event-stream", text("echo said hello")),
    ])
    .await;
    let dir = TempDir::new("llamacpp-e2e");
    let mut distribution = headless(&dir);
    for spec in &mut distribution.plugin {
        if spec.kind == "model-mock" {
            spec.kind = "model-llamacpp".into();
            spec.config = json!({ "endpoint": origin });
        }
    }
    let kernel = kernel(&distribution).await;
    let handle = kernel.handle();
    let session = new_session(&handle).await;

    let events = turn(&handle, &session, "say hello").await;
    assert_eq!(reply(&events), "echo said hello");
    assert_eq!(
        kinds(&events),
        [
            "started",
            "model",
            "tool-requested",
            "tool-started",
            "tool-finished",
            "model",
            "completed"
        ]
    );
    assert_eq!(
        entries(&handle, &session).await,
        [
            "turn-started",
            "tool-call",
            "tool-result",
            "assistant",
            "turn-completed"
        ]
    );

    let bodies = bodies.lock().unwrap().clone();
    let first = &bodies[2];
    assert_eq!(first["model"], "local-model");
    let tools: Vec<&str> = first["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert!(
        tools.contains(&"echo") && tools.contains(&"fs_read"),
        "{tools:?}"
    );
    let second = bodies[3]["input"].as_array().unwrap();
    assert_eq!(second[1]["type"], "function_call");
    assert_eq!(
        second[2],
        json!({"type":"function_call_output","call_id":"call_1","output":"hello"})
    );
}

#[test]
fn openai_preset_without_a_key_runs_but_reports_the_inactive_model() {
    let dir = TempDir::new("openai-no-key");
    let output = Command::new(env!("CARGO_BIN_EXE_rah"))
        .args([
            "--preset",
            "openai",
            "--model",
            "gpt-test",
            "--prompt",
            "hi",
            "--session-dir",
        ])
        .arg(dir.sessions())
        .arg("--root")
        .arg(dir.workspace())
        .env_remove("OPENAI_API_KEY")
        .env("RAH_HOME", dir.0.join("home"))
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{stdout}\n{stderr}");
    assert!(
        stderr.contains("plugin `model-openai` is not active"),
        "{stderr}"
    );
    assert!(stderr.contains("OPENAI_API_KEY is not set"), "{stderr}");
    assert!(stdout.contains("no model provider is mounted"), "{stdout}");

    let output = Command::new(env!("CARGO_BIN_EXE_rah"))
        .args(["--preset", "dev", "--model", "x", "--prompt", "hi"])
        .env("RAH_HOME", dir.0.join("home"))
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("--model needs"));
}
