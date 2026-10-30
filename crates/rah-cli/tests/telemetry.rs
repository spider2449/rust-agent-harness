//! `telemetry-log` in a distribution: the loop publishes its events, the
//! model reports usage and the plugin writes turn, model and tool records.

mod common;

use std::time::Duration;

use common::{TempDir, headless, kernel, new_session, turn};
use serde_json::{Value, json};

fn records(dir: &std::path::Path) -> Vec<Value> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut records = Vec::new();
    for entry in entries {
        let text = std::fs::read_to_string(entry.unwrap().path()).unwrap_or_default();
        records.extend(
            text.lines()
                .filter_map(|line| serde_json::from_str(line).ok()),
        );
    }
    records
}

#[tokio::test]
async fn turns_are_recorded_with_usage_and_tool_decisions() {
    let dir = TempDir::new("telemetry");
    let telemetry = dir.0.join("telemetry");
    // Every preset mounts `telemetry-log`; `headless` keeps it in `dir`.
    let distribution = headless(&dir);
    let kernel = kernel(&distribution).await;
    let handle = kernel.handle();
    let session = new_session(&handle).await;
    // model-mock asks for `echo`, then answers with its result.
    turn(&handle, &session, "hello").await;

    let mut found = Vec::new();
    for _ in 0..200 {
        found = records(&telemetry);
        if found.iter().any(|record| record["kind"] == "turn") {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let kinds: Vec<&str> = found.iter().map(|r| r["kind"].as_str().unwrap()).collect();
    assert_eq!(
        kinds,
        ["model_request", "tool", "model_request", "turn"],
        "{found:#?}"
    );
    assert_eq!(found[1]["tool"], "echo");
    assert_eq!(found[1]["decision"], "allowed");
    assert!(found[0]["input_tokens"].as_u64().unwrap() > 0);
    let turn = &found[3];
    assert_eq!(turn["outcome"], "completed");
    assert_eq!(turn["session"], json!(session));
    assert_eq!(turn["model_requests"], 2);
    assert_eq!(turn["tool_calls"], 1);
    assert_eq!(
        turn["input_tokens"].as_u64(),
        Some(
            found[0]["input_tokens"].as_u64().unwrap() + found[2]["input_tokens"].as_u64().unwrap()
        )
    );
}
