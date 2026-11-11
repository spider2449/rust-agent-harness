//! `telemetry-log` in a distribution: the loop publishes its events, the
//! model reports usage and the plugin writes turn, model and tool records.

mod common;

use std::time::Duration;

use common::{TempDir, headless, kernel, new_session, turn};
use serde_json::{Value, json};

static ONE_LIVE_MEASUREMENT: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Explicitly opt in: sends synthetic prompts to the local model, never CI.
#[tokio::test]
#[ignore = "live localhost:8080 inference measurement; run explicitly"]
async fn live_loopback_turn_latency_with_tools_and_history() {
    let _one = ONE_LIVE_MEASUREMENT.lock().await;
    assert!(
        measure_live_turns(None)
            .await
            .into_iter()
            .all(|correct| correct)
    );
}

#[tokio::test]
#[ignore = "live localhost:8080 thinking comparison; run explicitly"]
async fn live_loopback_thinking_mode_comparison() {
    let _one = ONE_LIVE_MEASUREMENT.lock().await;
    for mode in [true, false] {
        let checks = measure_live_turns(Some(mode)).await;
        // This is a measurement, not a guarantee of either mode's quality.
        eprintln!("live quality (thinking={mode}): answer and exact tool-plan checks = {checks:?}");
    }
}

async fn measure_live_turns(enable_thinking: Option<bool>) -> Vec<bool> {
    use rah_api::CancellationToken;
    use rah_protocol::AgentEvent;

    let dir = TempDir::new("live-latency");
    let mut distribution = headless(&dir);
    distribution
        .set_permissions(rah_cli::Permissions::ReadOnly)
        .unwrap();
    // Keep the base tools/context and telemetry, without background model work.
    distribution.plugin.retain(|spec| {
        !matches!(
            spec.kind.as_str(),
            "session-titles" | "scheduler-basic" | "skills-fs" | "plugin-builder"
        )
    });
    distribution
        .set_model(
            "openai-compatible",
            &serde_json::from_value(json!({
                "base_url": "http://127.0.0.1:8080/v1",
                "max_tokens": 512,
                "temperature": 0,
            }))
            .unwrap(),
        )
        .unwrap();
    if let Some(mode) = enable_thinking {
        assert!(distribution.set_config("model-chat-completions", "enable_thinking", json!(mode)));
    }
    let data = (0..128)
        .map(|index| format!("Record {index:06}: alpha beta gamma delta epsilon zeta.\n"))
        .collect::<String>()
        + "Verification code: RAH-LATENCY-7319\n";
    std::fs::write(dir.workspace().join("benchmark.txt"), data).unwrap();
    let kernel = kernel(&distribution).await;
    let handle = kernel.handle();
    let first = new_session(&handle).await;
    let fresh = new_session(&handle).await;
    let read = "Use fs.read to read benchmark.txt exactly once. Then reply with only its verification code. Do not use other tools.";
    let mut cases = vec![
        (&first, read),
        (
            &first,
            "What was the verification code? Reply with only the code, without calling tools.",
        ),
        (&fresh, read),
    ];
    if enable_thinking.is_some() {
        cases.push((
            &first,
            "What is 17 * 23? Reply with only the integer, without calling tools.",
        ));
    }
    let expected_turns = cases.len();
    let mut quality = Vec::new();
    for (index, (session, prompt)) in cases.into_iter().enumerate() {
        let cancel = CancellationToken::new();
        let result = tokio::time::timeout(
            Duration::from_secs(180),
            common::turn_with(&handle, session, prompt, cancel.clone()),
        )
        .await;
        cancel.cancel();
        let events = result.expect("live measurement exceeded 180 seconds");
        assert!(
            events
                .iter()
                .any(|event| matches!(event, AgentEvent::Completed { .. })),
            "live turn {index} did not complete; no automatic replay"
        );
        let answer = common::reply(&events);
        let correct_answer = if index == 3 {
            answer.trim() == "391"
        } else {
            answer.contains("RAH-LATENCY-7319")
        };
        let calls: Vec<_> = events
            .iter()
            .filter_map(|event| match event {
                AgentEvent::ToolRequested { tool_call, .. } => Some(tool_call.name.as_str()),
                _ => None,
            })
            .collect();
        let expected_calls = if matches!(index, 0 | 2) {
            vec!["fs.read"]
        } else {
            vec![]
        };
        let exact_tools = calls == expected_calls;
        quality.push(correct_answer && exact_tools);
        eprintln!(
            "live turn {} (thinking={enable_thinking:?}): answer_correct={correct_answer}, exact_tools={exact_tools}, tools={calls:?}",
            index + 1,
        );
    }
    let telemetry = dir.0.join("telemetry");
    let mut found = Vec::new();
    for _ in 0..200 {
        found = records(&telemetry);
        if found
            .iter()
            .filter(|record| record["kind"] == "turn")
            .count()
            == expected_turns
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(
        found
            .iter()
            .filter(|record| record["kind"] == "turn")
            .count(),
        expected_turns
    );
    for record in found {
        eprintln!("live telemetry (thinking={enable_thinking:?}): {record}");
    }
    kernel.shutdown().await.unwrap();
    quality
}

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
    // `headless` turns the `telemetry` feature on and keeps its files in `dir`.
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
