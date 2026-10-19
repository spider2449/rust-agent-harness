//! `tool.approval` answered by connected browsers.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        Mutex, PoisonError,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use rah_api::hooks::ApprovalRequest;
use rah_protocol::PermissionLevel;
use serde_json::{Value, json};
use tokio::sync::{broadcast, oneshot};

/// Messages pushed to every connected browser.
pub type Notice = Value;

/// A request that waits for an answer.
struct Waiting {
    notice: Notice,
    answer: oneshot::Sender<bool>,
    /// `(session, tool)`.
    key: (String, String),
    /// Whether the answer may allow the tool for the rest of the session.
    may_remember: bool,
}

pub struct Approvals {
    clients: AtomicUsize,
    pending: Mutex<BTreeMap<String, Waiting>>,
    /// What the user allowed for the rest of a session, `(session, tool)`. It
    /// lives as long as the process: a session opened again later asks again.
    remembered: Mutex<BTreeSet<(String, String)>>,
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
            remembered: Mutex::new(BTreeSet::new()),
            notices: broadcast::channel(64).0,
            timeout,
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, BTreeMap<String, Waiting>> {
        self.pending.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn remembered(&self) -> std::sync::MutexGuard<'_, BTreeSet<(String, String)>> {
        self.remembered
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    /// Registers a browser; returns its guard, its notice stream and the
    /// requests already waiting.
    pub fn connect(&self) -> (Client<'_>, broadcast::Receiver<Notice>, Vec<Notice>) {
        self.clients.fetch_add(1, Ordering::SeqCst);
        let receiver = self.notices.subscribe();
        let waiting = self
            .lock()
            .values()
            .map(|waiting| waiting.notice.clone())
            .collect();
        (Client(self), receiver, waiting)
    }

    /// `None` without a connected browser (no approver); otherwise the
    /// answer, or `false` on timeout. A tool the user allowed for the rest of
    /// the session (ADR 0056) is approved at once, with or without a browser.
    pub async fn ask(&self, request: ApprovalRequest) -> Option<bool> {
        let call = &request.request.call;
        let key = (request.request.session.to_string(), call.name.to_string());
        if self.remembered().contains(&key) {
            return Some(true);
        }
        if self.clients.load(Ordering::SeqCst) == 0 {
            return None;
        }
        let id = uuid::Uuid::new_v4().simple().to_string();
        let permission = request.request.definition.permission;
        // A tool that declares `execute`, or that the policy treats as
        // `execute` (its reason says so: the request carries no level), is
        // asked about every time.
        let may_remember =
            permission != PermissionLevel::Execute && !request.reason.contains("`execute`");
        let notice = json!({
            "type": "approval_required",
            "id": id,
            "session": key.0,
            "tool": call.name,
            "input": call.input.0,
            "permission": permission,
            "reason": request.reason,
            "may_remember": may_remember,
        });
        let (answer, answered) = oneshot::channel();
        self.lock().insert(
            id.clone(),
            Waiting {
                notice: notice.clone(),
                answer,
                key,
                may_remember,
            },
        );
        let _guard = Pending(self, id);
        let _ = self.notices.send(notice);
        Some(matches!(
            tokio::time::timeout(self.timeout, answered).await,
            Ok(Ok(true))
        ))
    }

    /// Pushes `notice` to every connected browser.
    pub fn notify(&self, notice: Notice) {
        let _ = self.notices.send(notice);
    }

    /// Answers a waiting request; `false` if it is unknown or already over.
    /// `always` with an approval also allows the tool for the rest of the
    /// session, unless the tool declares `execute`: the browser's word is not
    /// taken for that.
    pub fn answer(&self, id: &str, approve: bool, always: bool) -> bool {
        let Some(waiting) = self.lock().remove(id) else {
            return false;
        };
        if approve && always && waiting.may_remember {
            self.remembered().insert(waiting.key);
        }
        waiting.answer.send(approve).is_ok()
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

    /// A request for `tool` in `session`, declared at `permission`.
    fn request_for(
        session: &SessionId,
        tool: &str,
        permission: PermissionLevel,
        reason: &str,
    ) -> ApprovalRequest {
        ApprovalRequest {
            request: ToolRequestContext {
                session: session.clone(),
                call: ToolCall {
                    id: ToolCallId::new(),
                    name: ToolName::new(tool),
                    input: ToolInput(json!({ "path": "a.txt" })),
                },
                definition: ToolDefinition {
                    name: ToolName::new(tool),
                    description: String::new(),
                    input_schema: json!({}),
                    permission,
                },
            },
            reason: reason.into(),
        }
    }

    /// Asks `request` with a browser connected and answers it as `approve`
    /// and `always`. Returns the notice the browser saw and the answer.
    async fn answered(
        approvals: &Arc<Approvals>,
        request: ApprovalRequest,
        approve: bool,
        always: bool,
    ) -> (Value, Option<bool>) {
        let (_client, mut notices, _) = approvals.connect();
        let asking = tokio::spawn({
            let approvals = approvals.clone();
            async move { approvals.ask(request).await }
        });
        let notice = notices.recv().await.unwrap();
        let id = notice["id"].as_str().unwrap().to_owned();
        assert!(approvals.answer(&id, approve, always));
        (notice, asking.await.unwrap())
    }

    #[tokio::test]
    async fn always_allows_a_tool_for_that_session_only() {
        let approvals = Arc::new(Approvals::new(Duration::from_secs(5)));
        let session = SessionId::new();
        let write = PermissionLevel::Write;
        let reason = "tool `repo.create-file` requires `write` permission";
        let (notice, answer) = answered(
            &approvals,
            request_for(&session, "repo.create-file", write, reason),
            true,
            true,
        )
        .await;
        assert_eq!(notice["may_remember"], true);
        assert_eq!(answer, Some(true));

        // Now the same tool in the same session is approved at once, even
        // with no browser connected; another tool and another session are not.
        assert_eq!(
            approvals
                .ask(request_for(&session, "repo.create-file", write, reason))
                .await,
            Some(true)
        );
        assert_eq!(
            approvals
                .ask(request_for(&session, "repo.edit-files", write, reason))
                .await,
            None
        );
        assert_eq!(
            approvals
                .ask(request_for(
                    &SessionId::new(),
                    "repo.create-file",
                    write,
                    reason
                ))
                .await,
            None
        );
    }

    #[tokio::test]
    async fn a_plain_allow_or_a_denial_remembers_nothing() {
        let approvals = Arc::new(Approvals::new(Duration::from_secs(5)));
        let session = SessionId::new();
        let write = PermissionLevel::Write;
        let reason = "tool `repo.create-file` requires `write` permission";
        for (approve, always) in [(true, false), (false, true), (false, false)] {
            answered(
                &approvals,
                request_for(&session, "repo.create-file", write, reason),
                approve,
                always,
            )
            .await;
        }
        assert_eq!(
            approvals
                .ask(request_for(&session, "repo.create-file", write, reason))
                .await,
            None,
            "no browser is connected, so the tool was not remembered"
        );
    }

    #[tokio::test]
    async fn an_execute_tool_is_never_remembered_whatever_the_browser_says() {
        let approvals = Arc::new(Approvals::new(Duration::from_secs(5)));
        let session = SessionId::new();
        // Declared `execute`.
        let declared = request_for(
            &session,
            "shell.exec",
            PermissionLevel::Execute,
            "tool `shell.exec` requires `execute` permission",
        );
        let (notice, answer) = answered(&approvals, declared.clone(), true, true).await;
        assert_eq!(notice["may_remember"], false);
        assert_eq!(answer, Some(true), "this call is approved");
        assert_eq!(approvals.ask(declared).await, None);
        // Declared lower, but the policy treats it as `execute`.
        let assigned = request_for(
            &session,
            "custom.run",
            PermissionLevel::Write,
            "tool `custom.run` requires `execute` permission",
        );
        let (notice, _) = answered(&approvals, assigned.clone(), true, true).await;
        assert_eq!(notice["may_remember"], false);
        assert_eq!(approvals.ask(assigned).await, None);
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
        assert!(approvals.answer(&id, true, false));
        assert_eq!(asking.await.unwrap(), Some(true));
        assert_eq!(notices.recv().await.unwrap()["type"], "approval_resolved");
        assert!(!approvals.answer(&id, true, false));
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
