#![allow(dead_code)]

use std::path::{Path, PathBuf};

use futures::StreamExt;
use rah_api::{
    CancellationToken, Kernel, KernelHandle,
    agent_loop::{LOOP, TurnRequest},
    session::SESSION,
};
use rah_cli::{Distribution, boot};
use rah_protocol::{AgentEvent, SessionEntry, SessionId};
use serde_json::json;

pub struct TempDir(pub PathBuf);

impl TempDir {
    pub fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("rah-cli-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(path.join("workspace")).unwrap();
        Self(path)
    }

    pub fn sessions(&self) -> PathBuf {
        self.0.join("sessions")
    }

    pub fn workspace(&self) -> PathBuf {
        self.0.join("workspace")
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The dev preset without the terminal frontend, rooted in `dir`.
pub fn headless(dir: &TempDir) -> Distribution {
    let mut distribution = Distribution::preset("dev").unwrap();
    distribution
        .plugin
        .retain(|spec| spec.kind != "frontend-cli");
    distribution.set_config("session-jsonl", "dir", json!(path(&dir.sessions())));
    distribution.set_config("tools-builtin", "root", json!(path(&dir.workspace())));
    distribution.set_config("sandbox-process", "root", json!(path(&dir.workspace())));
    distribution.set_config("tools-repo-write", "root", json!(path(&dir.workspace())));
    distribution.set_config(
        "context-default",
        "memory_file",
        json!(path(&dir.0.join("memory.md"))),
    );
    distribution
}

fn path(path: &Path) -> String {
    path.display().to_string()
}

pub async fn kernel(distribution: &Distribution) -> Kernel {
    let kernel = Kernel::new();
    boot(&kernel, distribution).await.unwrap();
    kernel
}

pub async fn new_session(kernel: &KernelHandle) -> SessionId {
    kernel.get(SESSION).unwrap().create().await.unwrap().id
}

pub async fn turn_with(
    kernel: &KernelHandle,
    session: &SessionId,
    input: &str,
    cancel: CancellationToken,
) -> Vec<AgentEvent> {
    let agent_loop = kernel.get(LOOP).expect("loop is mounted");
    agent_loop
        .run_turn(TurnRequest {
            session: session.clone(),
            input: input.to_owned(),
            cancel,
        })
        .collect()
        .await
}

pub async fn turn(kernel: &KernelHandle, session: &SessionId, input: &str) -> Vec<AgentEvent> {
    turn_with(kernel, session, input, CancellationToken::new()).await
}

/// The concatenated model text of a turn.
pub fn reply(events: &[AgentEvent]) -> String {
    events
        .iter()
        .filter_map(|event| match event {
            AgentEvent::ModelDelta { delta, .. } => Some(delta.as_str()),
            _ => None,
        })
        .collect()
}

pub fn kinds(events: &[AgentEvent]) -> Vec<&'static str> {
    events
        .iter()
        .map(|event| match event {
            AgentEvent::Started { .. } => "started",
            AgentEvent::ModelRequestStarted { .. } => "model",
            AgentEvent::ModelDelta { .. } => "delta",
            AgentEvent::ModelReasoningStarted { .. } => "reasoning",
            AgentEvent::ToolRequested { .. } => "tool-requested",
            AgentEvent::ToolStarted { .. } => "tool-started",
            AgentEvent::ToolFinished { .. } => "tool-finished",
            AgentEvent::ApprovalRequired { .. } => "approval",
            AgentEvent::Completed { .. } => "completed",
            AgentEvent::Failed { .. } => "failed",
            AgentEvent::Cancelled { .. } => "cancelled",
        })
        .filter(|kind| *kind != "delta")
        .collect()
}

pub async fn entries(kernel: &KernelHandle, session: &SessionId) -> Vec<&'static str> {
    kernel
        .get(SESSION)
        .unwrap()
        .read(session)
        .await
        .unwrap()
        .iter()
        .map(|record| match record.entry {
            SessionEntry::TurnStarted { .. } => "turn-started",
            SessionEntry::AssistantMessage { .. } => "assistant",
            SessionEntry::ToolCall { .. } => "tool-call",
            SessionEntry::ToolResult { .. } => "tool-result",
            SessionEntry::TurnCompleted { .. } => "turn-completed",
            SessionEntry::TurnFailed { .. } => "turn-failed",
        })
        .collect()
}
