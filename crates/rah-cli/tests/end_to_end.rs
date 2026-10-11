//! P2 acceptance: tool round trip, session resume and fork, and hot-swapping
//! the model plugin while the loop keeps running.

mod common;

use std::sync::{Arc, Mutex};

use common::{TempDir, entries, headless, kernel, kinds, new_session, reply, turn, turn_with};
use rah_api::{
    CancellationToken, MountOptions, PluginId, PluginStatus, kernel::PluginStatusChanged,
    session::SESSION,
};
use rah_cli::catalog;
use rah_plugin_frontend_cli::{ReplOptions, run_repl};
use rah_protocol::{AgentEvent, SessionEntry, ToolEffect};
use serde_json::json;

#[tokio::test]
async fn tool_round_trip_is_streamed_and_recorded() {
    let dir = TempDir::new("round-trip");
    let kernel = kernel(&headless(&dir)).await;
    let handle = kernel.handle();
    let session = new_session(&handle).await;

    let events = turn(&handle, &session, "hello").await;
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
    assert_eq!(reply(&events), "[model-mock] echo returned: hello");
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
    let records = handle.get(SESSION).unwrap().read(&session).await.unwrap();
    assert!(matches!(
        records[2].entry,
        SessionEntry::ToolResult {
            effect: ToolEffect::None,
            ..
        }
    ));
}

#[tokio::test]
async fn fs_read_is_bounded_to_the_workspace() {
    let dir = TempDir::new("fs-read");
    std::fs::write(dir.workspace().join("notes.txt"), "plugin kernel").unwrap();
    let kernel = kernel(&headless(&dir)).await;
    let handle = kernel.handle();
    let session = new_session(&handle).await;
    let events = turn(&handle, &session, "read notes.txt").await;
    assert_eq!(
        reply(&events),
        "[model-mock] fs.read returned: plugin kernel"
    );
    let events = turn(&handle, &session, "read ../outside.txt").await;
    assert!(
        reply(&events).contains("must not contain `..`"),
        "{}",
        reply(&events)
    );
}

#[tokio::test]
async fn sessions_resume_after_restart_and_forks_diverge() {
    let dir = TempDir::new("resume-fork");
    let session = {
        let kernel = kernel(&headless(&dir)).await;
        let handle = kernel.handle();
        let session = new_session(&handle).await;
        turn(&handle, &session, "hello").await;
        kernel.shutdown().await.unwrap();
        session
    };

    // A new kernel over the same directory sees the full history.
    let kernel = kernel(&headless(&dir)).await;
    let handle = kernel.handle();
    let events = turn(&handle, &session, "/notool again").await;
    assert_eq!(
        reply(&events),
        "[model-mock] you said: again (4 earlier items)"
    );

    // Fork after the first turn (records 1-5) and diverge.
    let sessions = handle.get(SESSION).unwrap();
    let fork = sessions.fork(&session, 5).await.unwrap();
    let events = turn(&handle, &fork.id, "/notool forked").await;
    assert_eq!(
        reply(&events),
        "[model-mock] you said: forked (4 earlier items)"
    );
    assert_eq!(sessions.read(&session).await.unwrap().len(), 8);
    let forked = sessions.read(&fork.id).await.unwrap();
    assert_eq!(forked.len(), 8);
    assert_eq!(forked[..5], sessions.read(&session).await.unwrap()[..5]);
    assert!(matches!(
        &forked[5].entry,
        SessionEntry::TurnStarted { input, .. } if input == "/notool forked"
    ));
}

#[tokio::test]
async fn model_plugins_hot_swap_while_the_loop_keeps_running() {
    let dir = TempDir::new("hot-swap");
    let mut distribution = headless(&dir);
    distribution.plugin.retain(|spec| spec.kind != "model-mock");
    let kernel = kernel(&distribution).await;
    let handle = kernel.handle();
    let mock = catalog()["model-mock"];
    let mount_model = |id: &'static str, label: &'static str| {
        kernel.mount(
            mock(PluginId::new(id)),
            MountOptions::with_config(json!({ "label": label })),
        )
    };

    let loop_changes = Arc::new(Mutex::new(Vec::new()));
    let _watch = {
        let loop_changes = loop_changes.clone();
        handle.subscribe(move |change: &PluginStatusChanged| {
            if change.plugin.as_str() == "loop-react" {
                loop_changes.lock().unwrap().push(change.status.clone());
            }
        })
    };

    let session = new_session(&handle).await;
    let events = turn(&handle, &session, "/notool one").await;
    assert!(
        matches!(events.last(), Some(AgentEvent::Failed { message, .. }) if message.contains("no model")),
        "{events:?}"
    );

    mount_model("model-a", "A").await.unwrap();
    let events = turn(&handle, &session, "/notool two").await;
    assert_eq!(reply(&events), "[A] you said: two (1 earlier items)");

    kernel.unmount(&PluginId::new("model-a")).await.unwrap();
    mount_model("model-b", "B").await.unwrap();
    let events = turn(&handle, &session, "/notool three").await;
    assert_eq!(reply(&events), "[B] you said: three (3 earlier items)");

    assert_eq!(
        kernel.status(&PluginId::new("loop-react")),
        Some(PluginStatus::Active)
    );
    assert!(
        loop_changes.lock().unwrap().is_empty(),
        "the loop restarted"
    );
    assert_eq!(
        entries(&handle, &session).await,
        [
            "turn-started",
            "turn-failed",
            "turn-started",
            "assistant",
            "turn-completed",
            "turn-started",
            "assistant",
            "turn-completed"
        ]
    );
}

#[tokio::test]
async fn cancelled_turns_are_recorded_and_terminate() {
    let dir = TempDir::new("cancel");
    let kernel = kernel(&headless(&dir)).await;
    let handle = kernel.handle();
    let session = new_session(&handle).await;
    let cancel = CancellationToken::new();
    cancel.cancel();
    let events = turn_with(&handle, &session, "hello", cancel).await;
    assert_eq!(kinds(&events), ["started", "cancelled"]);
    assert_eq!(
        entries(&handle, &session).await,
        ["turn-started", "turn-failed"]
    );
}

#[tokio::test]
async fn the_loop_plugin_can_be_replaced_without_losing_sessions() {
    let dir = TempDir::new("loop-gone");
    let kernel = kernel(&headless(&dir)).await;
    let handle = kernel.handle();
    let session = new_session(&handle).await;
    kernel.unmount(&PluginId::new("loop-react")).await.unwrap();
    assert!(handle.get(rah_api::agent_loop::LOOP).is_none());
    // Sessions survive; a remounted loop continues the same session.
    kernel
        .mount(
            catalog()["loop-react"](PluginId::new("loop-react")),
            MountOptions::default(),
        )
        .await
        .unwrap();
    let events = turn(&handle, &session, "/notool back").await;
    assert_eq!(
        reply(&events),
        "[model-mock] you said: back (0 earlier items)"
    );
}

#[tokio::test]
async fn repl_runs_turns_and_commands() {
    let dir = TempDir::new("repl");
    let kernel = kernel(&headless(&dir)).await;
    let input: &[u8] = b"hello\n/history\n/plugins\n/fork 5\n/sessions\n/bogus\n/quit\n";
    let mut output = Vec::new();
    let code = run_repl(&kernel.handle(), input, &mut output, ReplOptions::default())
        .await
        .unwrap();
    let text = String::from_utf8(output).unwrap();
    assert_eq!(code, 0);
    for expected in [
        "[tool] echo {\"text\":\"hello\"}",
        "[tool result] hello",
        "[model-mock] echo returned: hello",
        "   1 user: hello",
        "   3 tool result (None): hello",
        "loop-react: active",
        "(forked at 5)",
        "(fork of ",
        "unknown command /bogus",
    ] {
        assert!(text.contains(expected), "missing {expected:?} in:\n{text}");
    }
}
