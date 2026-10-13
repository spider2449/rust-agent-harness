//! P2 acceptance: tool round trip, session resume and fork, and hot-swapping
//! the model plugin while the loop keeps running.

mod common;

use std::sync::{Arc, Mutex};

use common::{TempDir, entries, headless, kernel, kinds, new_session, reply, turn, turn_with};
use rah_api::{
    CancellationToken, MountOptions, Plugin, PluginError, PluginId, PluginManifest, PluginStatus,
    Scope, admin::ADMIN, async_trait, hooks::ToolApproval, kernel::PluginStatusChanged,
    session::SESSION,
};
use rah_cli::catalog;
use rah_plugin_frontend_cli::{ApprovalPrompt, ReplOptions, RunningTurn, run_repl};
use rah_protocol::{AgentEvent, SessionEntry, ToolEffect};
use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader};

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
    let input: &[u8] =
        b"hello\n/history\n/plugins\n/rename  My trip \n/fork 5\n/sessions\n/bogus\n/call echo {\"text\":\"called\"}\n\
          /\n/st\n/stop\n/stop policy-authority\n/stop loop-react\n/plugins\nnot run\n/start loop-react\n/quit\n";
    let mut output = Vec::new();
    let code = run_repl(
        &kernel.handle(),
        input,
        &mut output,
        ReplOptions::default(),
        None,
        RunningTurn::default(),
    )
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
        ": My trip",
        "(forked at 5)",
        "(fork of ",
        // `/sessions` names the renamed session; the fork shows its preview.
        "  My trip",
        "  hello",
        "type /help for commands",
        "unknown command /bogus; try /help",
        // `/call` is a turn for the model, not a REPL command.
        "[model-mock] echo returned: called",
        // `/` alone lists the commands.
        "  /stop <plugin>        stop a plugin, keeping its configuration",
        "unknown command /st; did you mean /stop <plugin>, /start <plugin>?",
        "usage: /stop <plugin>  (see /plugins)",
        "error: plugin `policy-authority` guards tool calls and cannot be stopped",
        "loop-react: stopped",
        "error: no agent loop is mounted",
        "loop-react: active",
    ] {
        assert!(text.contains(expected), "missing {expected:?} in:\n{text}");
    }
    assert!(!text.contains("unknown command /call"), "{text}");
}

/// Answers `tool.approval` by forwarding each request, like `frontend-cli`.
struct Approver {
    manifest: PluginManifest,
    prompts: tokio::sync::mpsc::Sender<ApprovalPrompt>,
}

#[async_trait]
impl Plugin for Approver {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    async fn apply(&self, scope: &mut Scope) -> Result<(), PluginError> {
        let prompts = self.prompts.clone();
        scope.hook_bail::<ToolApproval, _, _>(0, move |request| {
            let prompts = prompts.clone();
            async move {
                let (reply, answer) = tokio::sync::oneshot::channel();
                prompts.send((request, reply)).await.ok()?;
                Some(answer.await.unwrap_or(false))
            }
        });
        Ok(())
    }
}

/// Reads `out` until `text` appears; returns everything read.
async fn read_until(out: &mut tokio::io::DuplexStream, seen: &mut String, text: &str) {
    let mut buffer = [0u8; 1024];
    while !seen.contains(text) {
        let read = tokio::time::timeout(std::time::Duration::from_secs(10), out.read(&mut buffer))
            .await
            .unwrap_or_else(|_| panic!("timed out waiting for {text:?} in:\n{seen}"))
            .unwrap();
        assert!(read > 0, "output ended before {text:?}:\n{seen}");
        seen.push_str(&String::from_utf8_lossy(&buffer[..read]));
    }
}

#[tokio::test]
async fn an_interrupt_cancels_the_running_turn_and_the_repl_continues() {
    let dir = TempDir::new("repl-interrupt");
    let kernel = kernel(&headless(&dir)).await;
    let (prompts, approvals) = tokio::sync::mpsc::channel(1);
    kernel
        .mount(
            Arc::new(Approver {
                manifest: PluginManifest::new("approver", "0"),
                prompts,
            }),
            MountOptions::default(),
        )
        .await
        .unwrap();
    let (mut input, input_end) = tokio::io::duplex(4096);
    let (output_end, mut output) = tokio::io::duplex(65536);
    let running = RunningTurn::default();
    assert!(!running.interrupt(), "no turn is running yet");
    let repl = tokio::spawn({
        let handle = kernel.handle();
        let running = running.clone();
        async move {
            run_repl(
                &handle,
                BufReader::new(input_end),
                output_end,
                ReplOptions::default(),
                Some(approvals),
                running,
            )
            .await
        }
    });
    let mut seen = String::new();
    read_until(&mut output, &mut seen, "> ").await;
    // `repo.create-file` is a `write` tool: the policy asks, and the turn
    // waits for an answer that never comes.
    input
        .write_all(b"/call repo.create-file {\"path\":\"a.txt\",\"content\":\"a\"}\n")
        .await
        .unwrap();
    read_until(&mut output, &mut seen, "Allow? [y/N] ").await;
    assert!(running.interrupt(), "the turn is running");
    read_until(&mut output, &mut seen, "[cancelled]").await;
    assert!(seen.contains("[denied]"), "{seen}");
    assert!(!dir.workspace().join("a.txt").exists());
    assert!(!running.interrupt(), "the turn has ended");

    // The REPL keeps running.
    input.write_all(b"hello\n/quit\n").await.unwrap();
    read_until(&mut output, &mut seen, "echo returned: hello").await;
    assert_eq!(repl.await.unwrap().unwrap(), 0);
}

#[tokio::test]
async fn a_stopped_plugin_keeps_its_configuration_and_its_dependents_wait() {
    let dir = TempDir::new("admin-stop");
    let mut distribution = headless(&dir);
    distribution.set_config("model-mock", "label", json!("L"));
    let kernel = kernel(&distribution).await;
    let handle = kernel.handle();
    let admin = handle.get(ADMIN).unwrap();
    let sandbox = PluginId::new("sandbox-process");
    let repo = PluginId::new("tools-repo");
    assert_eq!(handle.status(&repo), Some(PluginStatus::Active));

    admin.stop(&sandbox).await.unwrap();
    assert_eq!(handle.status(&sandbox), None);
    assert_eq!(admin.stopped(), std::slice::from_ref(&sandbox));
    assert!(admin.instance(&sandbox).unwrap().stopped);
    // A dependent waits for the stopped service.
    assert!(
        matches!(handle.status(&repo), Some(PluginStatus::Pending { .. })),
        "{:?}",
        handle.status(&repo)
    );
    // The stopped id stays taken, and only `start` and `unmount` apply.
    let error = admin
        .mount(rah_api::admin::MountRequest {
            kind: "sandbox-process".into(),
            ..Default::default()
        })
        .await
        .unwrap_err();
    assert!(
        matches!(error, rah_api::admin::AdminError::AlreadyMounted(_)),
        "{error}"
    );
    for error in [
        admin.stop(&sandbox).await.unwrap_err(),
        admin.restart(&sandbox).await.map(|_| ()).unwrap_err(),
    ] {
        assert!(
            matches!(error, rah_api::admin::AdminError::Stopped(_)),
            "{error}"
        );
    }

    assert_eq!(admin.start(&sandbox).await.unwrap(), PluginStatus::Active);
    assert_eq!(handle.status(&repo), Some(PluginStatus::Active));
    assert!(admin.stopped().is_empty());
    let error = admin.start(&sandbox).await.unwrap_err();
    assert!(
        matches!(error, rah_api::admin::AdminError::NotStopped(_)),
        "{error}"
    );

    // The configuration survives a stop and start.
    let model = PluginId::new("model-mock");
    admin.stop(&model).await.unwrap();
    admin.start(&model).await.unwrap();
    let session = new_session(&handle).await;
    assert_eq!(
        reply(&turn(&handle, &session, "/notool x").await),
        "[L] you said: x (0 earlier items)"
    );

    // Policy plugins and the distribution itself cannot be stopped.
    assert!(
        admin
            .stop(&PluginId::new("policy-authority"))
            .await
            .is_err()
    );
    assert!(admin.stop(&PluginId::new("distribution")).await.is_err());
    assert_eq!(
        handle.status(&PluginId::new("policy-authority")),
        Some(PluginStatus::Active)
    );

    // Unmounting a stopped plugin forgets it.
    admin.stop(&model).await.unwrap();
    admin.unmount(&model).await.unwrap();
    assert!(admin.stopped().is_empty() && admin.instance(&model).is_none());
}
