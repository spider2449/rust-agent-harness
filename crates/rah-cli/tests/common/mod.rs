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

    /// The data home the binary tests give `rah` (`RAH_HOME`).
    pub fn home(&self) -> PathBuf {
        self.0.join("home")
    }

    /// The session directory: the data home's, where `rah` writes them.
    pub fn sessions(&self) -> PathBuf {
        self.home().join("sessions")
    }

    pub fn storage(&self) -> PathBuf {
        self.0.join("storage")
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

/// The configuration file [`from_file`] starts with: user keys only, as
/// `rah config init` writes one.
pub const USER_FILE: &str = "# written by the tests\nmodel = \"mock\"\n";

/// The configuration file of `dir`.
pub fn config_file(dir: &TempDir) -> PathBuf {
    dir.0.join("rah.toml")
}

/// What `dir`'s configuration file composes (written from [`USER_FILE`]
/// when missing), as `rah` would read it, without the terminal frontend and
/// with the data paths of [`headless`]. Settings the Web UI saves go to the
/// file, so a later call starts with them.
pub fn from_file(dir: &TempDir) -> Distribution {
    let path = config_file(dir);
    if !path.exists() {
        std::fs::write(&path, USER_FILE).unwrap();
    }
    let (mut distribution, _) = rah_cli::select_distribution(Some(&path), &dir.home()).unwrap();
    isolate(&mut distribution, dir);
    distribution
}

/// The dev preset without the terminal frontend, rooted in `dir`.
pub fn headless(dir: &TempDir) -> Distribution {
    let mut distribution = Distribution::preset("dev").unwrap();
    isolate(&mut distribution, dir);
    distribution
}

/// Drops the terminal frontend and keeps every data path in `dir`.
fn isolate(distribution: &mut Distribution, dir: &TempDir) {
    distribution
        .plugin
        .retain(|spec| spec.kind != "frontend-cli");
    distribution.set_config("session-jsonl", "dir", json!(path(&dir.sessions())));
    distribution.set_config("tools-builtin", "root", json!(path(&dir.workspace())));
    distribution.set_config("sandbox-process", "root", json!(path(&dir.workspace())));
    distribution.set_config("tools-repo-write", "root", json!(path(&dir.workspace())));
    distribution.set_config("skills-fs", "dir", json!(path(&dir.0.join("skills"))));
    distribution.set_config(
        "scheduler-basic",
        "agents_dir",
        json!(path(&dir.0.join("agents"))),
    );
    distribution.set_config("storage-fs", "dir", json!(path(&dir.storage())));
    distribution.set_config("plugin-builder", "dir", json!(path(&dir.0.join("plugins"))));
    distribution.set_config(
        "telemetry-log",
        "dir",
        json!(path(&dir.0.join("telemetry"))),
    );
    distribution.set_config(
        "context-default",
        "memory_dir",
        json!(path(&dir.0.join("memory"))),
    );
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
            AgentEvent::ModelUsage { .. } => "usage",
            AgentEvent::ModelRetrying { .. } => "retry",
            AgentEvent::ToolRequested { .. } => "tool-requested",
            AgentEvent::ToolStarted { .. } => "tool-started",
            AgentEvent::ToolFinished { .. } => "tool-finished",
            AgentEvent::ApprovalRequired { .. } => "approval",
            AgentEvent::Completed { .. } => "completed",
            AgentEvent::Failed { .. } => "failed",
            AgentEvent::Cancelled { .. } => "cancelled",
        })
        .filter(|kind| !matches!(*kind, "delta" | "usage"))
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

/// Provides a fixed `ui-layout` (ADR 0062).
struct FixedLayout {
    manifest: rah_api::PluginManifest,
    layout: rah_api::ui::Layout,
}

#[rah_api::async_trait]
impl rah_api::ui::UiLayout for FixedLayout {
    async fn layout(&self) -> rah_api::ui::Layout {
        self.layout.clone()
    }
}

#[rah_api::async_trait]
impl rah_api::Plugin for FixedLayout {
    fn manifest(&self) -> &rah_api::PluginManifest {
        &self.manifest
    }

    async fn apply(&self, scope: &mut rah_api::Scope) -> Result<(), rah_api::PluginError> {
        let provider: std::sync::Arc<dyn rah_api::ui::UiLayout> =
            std::sync::Arc::new(FixedLayout {
                manifest: self.manifest.clone(),
                layout: self.layout.clone(),
            });
        scope.provide(rah_api::ui::UI_LAYOUT, provider)?;
        Ok(())
    }
}

/// Mounts a `ui-layout` provider `id` that answers `layout` (its JSON form).
pub async fn mount_layout(
    kernel: &rah_api::Kernel,
    id: &rah_api::PluginId,
    layout: serde_json::Value,
) {
    let plugin = FixedLayout {
        manifest: rah_api::PluginManifest::new(id.clone(), "0")
            .provides([rah_api::ui::UI_LAYOUT_SERVICE]),
        layout: serde_json::from_value(layout).unwrap(),
    };
    kernel
        .mount(
            std::sync::Arc::new(plugin),
            rah_api::MountOptions::default(),
        )
        .await
        .unwrap();
}
