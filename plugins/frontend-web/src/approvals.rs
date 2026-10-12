//! `tool.approval` answered by connected browsers.

use std::{
    collections::BTreeMap,
    sync::{
        Mutex, PoisonError,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use rah_api::hooks::ApprovalRequest;
use serde_json::{Value, json};
use tokio::sync::{broadcast, oneshot};

/// Messages pushed to every connected browser.
pub type Notice = Value;

pub struct Approvals {
    clients: AtomicUsize,
    pending: Mutex<BTreeMap<String, (Notice, oneshot::Sender<bool>)>>,
    notices: broadcast::Sender<Notice>,
    timeout: Duration,
}

/// Counts one connected browser while alive.
pub struct Client<'a>(&'a Approvals);

impl Drop for Client<'_> {
    fn drop(&mut self) {
        self.0.clients.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Removes a request that is no longer waited for.
struct Pending<'a>(&'a Approvals, String);

impl Drop for Pending<'_> {
    fn drop(&mut self) {
        self.0.lock().remove(&self.1);
        let _ = self
            .0
            .notices
            .send(json!({ "type": "approval_resolved", "id": self.1 }));
    }
}

impl Approvals {
    pub fn new(timeout: Duration) -> Self {
        Self {
            clients: AtomicUsize::new(0),
            pending: Mutex::new(BTreeMap::new()),
            notices: broadcast::channel(64).0,
            timeout,
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, BTreeMap<String, (Notice, oneshot::Sender<bool>)>> {
        self.pending.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Registers a browser; returns its guard, its notice stream and the
    /// requests already waiting.
    pub fn connect(&self) -> (Client<'_>, broadcast::Receiver<Notice>, Vec<Notice>) {
        self.clients.fetch_add(1, Ordering::SeqCst);
        let receiver = self.notices.subscribe();
        let waiting = self
            .lock()
            .values()
            .map(|(notice, _)| notice.clone())
            .collect();
        (Client(self), receiver, waiting)
    }

    /// `None` without a connected browser (no approver); otherwise the
    /// answer, or `false` on timeout.
    pub async fn ask(&self, request: ApprovalRequest) -> Option<bool> {
        if self.clients.load(Ordering::SeqCst) == 0 {
            return None;
        }
        let id = uuid::Uuid::new_v4().simple().to_string();
        let call = &request.request.call;
        let notice = json!({
            "type": "approval_required",
            "id": id,
            "session": request.request.session.to_string(),
            "tool": call.name,
            "input": call.input.0,
            "permission": request.request.definition.permission,
            "reason": request.reason,
        });
        let (answer, answered) = oneshot::channel();
        self.lock().insert(id.clone(), (notice.clone(), answer));
        let _guard = Pending(self, id);
        let _ = self.notices.send(notice);
        Some(matches!(
            tokio::time::timeout(self.timeout, answered).await,
            Ok(Ok(true))
        ))
    }

    /// Answers a waiting request; `false` if it is unknown or already over.
    pub fn answer(&self, id: &str, approve: bool) -> bool {
        match self.lock().remove(id) {
            Some((_, answer)) => answer.send(approve).is_ok(),
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rah_api::hooks::ToolRequestContext;
    use rah_protocol::{
        PermissionLevel, SessionId, ToolCall, ToolCallId, ToolDefinition, ToolInput, ToolName,
    };
    use std::sync::Arc;

    fn request() -> ApprovalRequest {
        ApprovalRequest {
            request: ToolRequestContext {
                session: SessionId::new(),
                call: ToolCall {
                    id: ToolCallId::new(),
                    name: ToolName::new("shell.exec"),
                    input: ToolInput(json!({ "program": "git" })),
                },
                definition: ToolDefinition {
                    name: ToolName::new("shell.exec"),
                    description: String::new(),
                    input_schema: json!({}),
                    permission: PermissionLevel::Execute,
                },
            },
            reason: "requires `execute` permission".into(),
        }
    }

    #[tokio::test]
    async fn without_a_browser_there_is_no_approver() {
        let approvals = Approvals::new(Duration::from_secs(5));
        assert_eq!(approvals.ask(request()).await, None);
    }

    #[tokio::test]
    async fn browsers_see_requests_and_answer_them() {
        let approvals = Arc::new(Approvals::new(Duration::from_secs(5)));
        let (client, mut notices, waiting) = approvals.connect();
        assert!(waiting.is_empty());
        let asking = tokio::spawn({
            let approvals = approvals.clone();
            async move { approvals.ask(request()).await }
        });
        let notice = notices.recv().await.unwrap();
        assert_eq!(notice["type"], "approval_required");
        assert_eq!(notice["tool"], "shell.exec");
        assert_eq!(notice["input"]["program"], "git");
        let id = notice["id"].as_str().unwrap().to_owned();
        // A late browser sees the waiting request too.
        let (_late, _, waiting) = approvals.connect();
        assert_eq!(waiting.len(), 1);
        assert!(approvals.answer(&id, true));
        assert_eq!(asking.await.unwrap(), Some(true));
        assert_eq!(notices.recv().await.unwrap()["type"], "approval_resolved");
        assert!(!approvals.answer(&id, true));
        drop(client);
    }

    #[tokio::test]
    async fn unanswered_requests_time_out_as_denied() {
        let approvals = Approvals::new(Duration::from_millis(50));
        let (_client, _notices, _) = approvals.connect();
        assert_eq!(approvals.ask(request()).await, Some(false));
        assert!(approvals.lock().is_empty());
    }
}
