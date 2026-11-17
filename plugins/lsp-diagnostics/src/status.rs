//! Scope-local, read-only LSP status using the existing UI view contract.

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

use rah_api::{
    KernelHandle, async_trait,
    ui::{Node, StatusIcon, StatusState, UiChanged, UiError, UiViews, View, ViewContext, ViewSpec},
};

const VIEW: &str = "status";

#[derive(Clone)]
pub(crate) struct Changes {
    pub kernel: KernelHandle,
    pub instance: String,
}

impl Changes {
    pub fn emit(&self) {
        self.kernel.emit(&UiChanged {
            instance: self.instance.clone(),
            view: VIEW.into(),
        });
    }
}

/// Also marks a connection closed when its scope cancels the actor future.
pub(crate) struct ConnectionWatch {
    pub closed: Arc<AtomicBool>,
    pub changes: Changes,
}

impl Drop for ConnectionWatch {
    fn drop(&mut self) {
        self.closed.store(true, Ordering::Release);
        self.changes.emit();
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Phase {
    Idle,
    Starting,
    Checking,
    Reported,
    Issues,
    Timeout,
    Failed,
    Interrupted,
    Disconnected,
}

impl Phase {
    fn label(self) -> &'static str {
        match self {
            Self::Idle => "Idle",
            Self::Starting => "Starting",
            Self::Checking => "Checking",
            Self::Reported => "Ready",
            Self::Issues => "Diagnostics",
            Self::Timeout => "Timed out",
            Self::Failed => "Failed",
            Self::Interrupted => "Interrupted",
            Self::Disconnected => "Disconnected",
        }
    }
}

struct Snapshot {
    revision: u64,
    phase: Phase,
    detail: String,
    closed: Option<Arc<AtomicBool>>,
}

pub(crate) struct ServerStatus {
    name: String,
    snapshot: Mutex<Snapshot>,
}

fn bounded(text: &str, limit: usize) -> String {
    text.chars()
        .map(|ch| if ch.is_control() { ' ' } else { ch })
        .take(limit)
        .collect()
}

impl ServerStatus {
    pub fn new(name: &str) -> Arc<Self> {
        Arc::new(Self {
            name: bounded(name, 100),
            snapshot: Mutex::new(Snapshot {
                revision: 0,
                phase: Phase::Idle,
                detail: "Starts after a matching file is written.".into(),
                closed: None,
            }),
        })
    }

    pub fn begin(self: &Arc<Self>, changes: &Changes, phase: Phase, detail: &str) -> Activity {
        let revision = {
            let mut snapshot = self.snapshot.lock().unwrap_or_else(|e| e.into_inner());
            snapshot.revision = snapshot.revision.wrapping_add(1);
            snapshot.phase = phase;
            snapshot.detail = bounded(detail, 400);
            if phase == Phase::Starting {
                snapshot.closed = None;
            }
            snapshot.revision
        };
        changes.emit();
        Activity {
            status: self.clone(),
            changes: changes.clone(),
            revision,
            finished: false,
        }
    }

    pub fn watch(&self, closed: Arc<AtomicBool>) {
        self.snapshot
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .closed = Some(closed);
    }

    pub fn checking(
        self: &Arc<Self>,
        changes: &Changes,
        closed: &Arc<AtomicBool>,
        detail: &str,
    ) -> Option<Activity> {
        let revision = {
            let mut snapshot = self.snapshot.lock().unwrap_or_else(|e| e.into_inner());
            if closed.load(Ordering::Acquire)
                || !snapshot
                    .closed
                    .as_ref()
                    .is_some_and(|current| Arc::ptr_eq(current, closed))
            {
                return None;
            }
            snapshot.revision = snapshot.revision.wrapping_add(1);
            snapshot.phase = Phase::Checking;
            snapshot.detail = bounded(detail, 400);
            snapshot.revision
        };
        changes.emit();
        Some(Activity {
            status: self.clone(),
            changes: changes.clone(),
            revision,
            finished: false,
        })
    }

    fn update(&self, revision: u64, phase: Phase, detail: &str) -> bool {
        let mut snapshot = self.snapshot.lock().unwrap_or_else(|e| e.into_inner());
        if snapshot.revision != revision {
            return false;
        }
        snapshot.phase = phase;
        snapshot.detail = bounded(detail, 400);
        true
    }

    fn read(&self) -> (Phase, String) {
        let snapshot = self.snapshot.lock().unwrap_or_else(|e| e.into_inner());
        if snapshot.phase != Phase::Failed
            && snapshot
                .closed
                .as_ref()
                .is_some_and(|closed| closed.load(Ordering::Acquire))
        {
            return (
                Phase::Disconnected,
                "The language server connection ended.".into(),
            );
        }
        (snapshot.phase, snapshot.detail.clone())
    }
}

pub(crate) struct Activity {
    status: Arc<ServerStatus>,
    changes: Changes,
    revision: u64,
    finished: bool,
}

impl Activity {
    pub fn finish(mut self, phase: Phase, detail: &str) {
        self.finished = true;
        if self.status.update(self.revision, phase, detail) {
            self.changes.emit();
        }
    }
}

impl Drop for Activity {
    fn drop(&mut self) {
        if !self.finished
            && self.status.update(
                self.revision,
                Phase::Interrupted,
                "The operation ended before a result was received.",
            )
        {
            self.changes.emit();
        }
    }
}

pub(crate) struct StatusView {
    pub workspace: String,
    pub servers: Vec<Arc<ServerStatus>>,
}

#[async_trait]
impl UiViews for StatusView {
    fn views(&self) -> Vec<ViewSpec> {
        vec![ViewSpec {
            id: VIEW.into(),
            title: "LSP".into(),
            status_bar: true,
        }]
    }

    async fn render(&self, view: &str, _cx: &ViewContext) -> Result<View, UiError> {
        if view != VIEW {
            return Err(UiError::UnknownView(view.into()));
        }
        let states: Vec<_> = self
            .servers
            .iter()
            .map(|server| (server.name.clone(), server.read()))
            .collect();
        let has = |phase| states.iter().any(|(_, (current, _))| *current == phase);
        let (state, label) = if has(Phase::Starting) {
            (StatusState::Busy, "LSP starting")
        } else if has(Phase::Checking) {
            (StatusState::Busy, "LSP checking")
        } else if has(Phase::Failed) {
            (StatusState::Error, "LSP failed")
        } else if has(Phase::Disconnected) {
            (StatusState::Warning, "LSP disconnected")
        } else if has(Phase::Timeout) {
            (StatusState::Warning, "LSP timed out")
        } else if has(Phase::Interrupted) {
            (StatusState::Warning, "LSP interrupted")
        } else if has(Phase::Issues) {
            (StatusState::Warning, "LSP diagnostics")
        } else if has(Phase::Reported) {
            (StatusState::Ready, "LSP ready")
        } else {
            (StatusState::Idle, "LSP idle")
        };
        let summary = if states.is_empty() {
            "No language servers configured.".to_owned()
        } else {
            states
                .iter()
                .take(8)
                .map(|(name, (phase, _))| format!("{name}: {}", phase.label()))
                .collect::<Vec<_>>()
                .join("\n")
        };
        let rows = states
            .iter()
            .take(64)
            .map(|(name, (phase, detail))| vec![name.clone(), phase.label().into(), detail.clone()])
            .collect();
        Ok(View { nodes: vec![
            Node::Heading { text: "LSP".into() },
            Node::Status { icon: StatusIcon::Activity, label: label.into(), state, detail: format!("{label}\n{}", summary.chars().take(950).collect::<String>()) },
            Node::Text { text: format!("Workspace: {}", bounded(&self.workspace, 2048)) },
            Node::Text { text: "Status covers the latest file check per server, across sessions in this workspace. Ready means the server replied; it does not certify the project. Only RAH writes trigger checks.".into() },
            Node::Text { text: if states.len() > 64 { format!("Showing 64 of {} servers.", states.len()) } else { summary } },
            Node::Table { columns: vec!["Server".into(), "Status".into(), "Latest result".into()], rows, row_actions: vec![] },
        ] })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rah_api::Kernel;

    #[tokio::test]
    async fn obsolete_operations_cannot_overwrite_new_connection_status() {
        let kernel = Kernel::new();
        let changes = Changes {
            kernel: kernel.handle(),
            instance: "lsp".into(),
        };
        let server = ServerStatus::new("rust");
        let old = server.begin(&changes, Phase::Checking, "old check");
        let old_closed = Arc::new(AtomicBool::new(false));
        server.watch(old_closed.clone());
        let current = server.begin(&changes, Phase::Starting, "new connection");
        let current_closed = Arc::new(AtomicBool::new(false));
        server.watch(current_closed);
        drop(old);
        assert!(
            server
                .checking(&changes, &old_closed, "queued old check")
                .is_none()
        );
        old_closed.store(true, Ordering::Release);
        assert!(server.read().0 == Phase::Starting);
        current.finish(Phase::Reported, "initialized");
        assert!(server.read().0 == Phase::Reported);
    }

    #[tokio::test]
    async fn large_untrusted_statuses_remain_bounded_and_empty_config_is_idle() {
        let kernel = Kernel::new();
        let changes = Changes {
            kernel: kernel.handle(),
            instance: "lsp".into(),
        };
        let mut view = StatusView {
            workspace: "界".repeat(5000),
            servers: vec![],
        };
        assert!(
            view.render("unknown", &ViewContext::default())
                .await
                .is_err()
        );
        let empty = view.render(VIEW, &ViewContext::default()).await.unwrap();
        empty.check().unwrap();
        assert!(
            serde_json::to_string(&empty)
                .unwrap()
                .contains("No language servers configured")
        );
        for _ in 0..100 {
            let server = ServerStatus::new(&"外部\n".repeat(300));
            server
                .begin(&changes, Phase::Starting, "starting")
                .finish(Phase::Failed, &"<script>\n錯誤\0".repeat(1000));
            view.servers.push(server);
        }
        let rendered = view.render(VIEW, &ViewContext::default()).await.unwrap();
        rendered.check().unwrap();
        assert!(
            rendered
                .nodes
                .iter()
                .any(|node| matches!(node, Node::Table { rows, .. } if rows.len() == 64))
        );
    }
}
