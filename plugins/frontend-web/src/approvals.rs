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
    /// The exact `shell.exec` command the answer may allow from now on
    /// (ADR 0066), taken from the call, never from the browser.
    command: Option<Vec<String>>,
}

/// The tool whose calls can be remembered as a policy `commands` rule.
const SHELL_TOOL: &str = "shell.exec";

/// The policy `commands` rule (ADR 0050) that allows exactly this call:
/// `[program, args...]`. `None` for another tool, for input that is not a
/// program string and an array of strings, and when an element is `*`, which
/// a rule would read as a wildcard.
pub fn command_rule(tool: &str, input: &Value) -> Option<Vec<String>> {
    if tool != SHELL_TOOL {
        return None;
    }
    let program = input.get("program")?.as_str()?;
    let mut rule = vec![program.to_owned()];
    match input.get("args") {
        None | Some(Value::Null) => {}
        Some(Value::Array(args)) => {
            for arg in args {
                rule.push(arg.as_str()?.to_owned());
            }
        }
        Some(_) => return None,
    }
    let usable = !program.trim().is_empty() && rule.iter().all(|part| part != "*");
    usable.then_some(rule)
}

/// What answering a request did.
#[derive(Debug, PartialEq, Eq)]
pub struct Answered {
    /// The command to add to the policy's `commands`, when the user chose to
    /// allow it from now on.
    pub allow_command: Option<Vec<String>>,
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
        // A call judged at `execute`, or at no known level, is asked about
        // every time (ADR 0074).
        let may_remember = request.may_remember();
        let command = command_rule(call.name.as_str(), &call.input.0);
        let notice = json!({
            "type": "approval_required",
            "id": id,
            "session": key.0,
            "tool": call.name,
            "input": call.input.0,
            "permission": permission,
            "level": request.level,
            "reason": request.reason,
            "may_remember": may_remember,
            "command": command,
        });
        let (answer, answered) = oneshot::channel();
        self.lock().insert(
            id.clone(),
            Waiting {
                notice: notice.clone(),
                answer,
                key,
                may_remember,
                command,
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

    /// Answers a waiting request; `None` if it is unknown or already over.
    /// `always` with an approval also allows the tool for the rest of the
    /// session, unless the tool declares `execute`: the browser's word is not
    /// taken for that. `command` with an approval returns the exact
    /// `shell.exec` command of the call, for the caller to add to the policy
    /// (ADR 0066); a request without one returns none.
    pub fn answer(&self, id: &str, approve: bool, always: bool, command: bool) -> Option<Answered> {
        let waiting = self.lock().remove(id)?;
        if approve && always && waiting.may_remember {
            self.remembered().insert(waiting.key);
        }
        let allow_command = waiting.command.filter(|_| approve && command);
        waiting
            .answer
            .send(approve)
            .is_ok()
            .then_some(Answered { allow_command })
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
            level: Some(PermissionLevel::Execute),
        }
    }

    /// A request for `tool` in `session`, declared and judged at
    /// `permission`.
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
            level: Some(permission),
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
        assert!(approvals.answer(&id, approve, always, false).is_some());
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

    #[test]
    fn only_exact_shell_commands_become_rules() {
        let rule = |tool: &str, input: Value| command_rule(tool, &input);
        assert_eq!(
            rule(
                "shell.exec",
                json!({ "program": "netstat", "args": ["-ano"] })
            ),
            Some(vec!["netstat".to_owned(), "-ano".to_owned()])
        );
        assert_eq!(
            rule("shell.exec", json!({ "program": "git" })),
            Some(vec!["git".to_owned()])
        );
        assert_eq!(rule("repo.patch", json!({ "program": "git" })), None);
        assert_eq!(rule("shell.exec", json!({ "program": 3 })), None);
        assert_eq!(rule("shell.exec", json!({ "program": " " })), None);
        assert_eq!(
            rule("shell.exec", json!({ "program": "git", "args": "status" })),
            None
        );
        assert_eq!(
            rule("shell.exec", json!({ "program": "git", "args": [1] })),
            None
        );
        // `*` would widen the rule into a wildcard.
        assert_eq!(
            rule("shell.exec", json!({ "program": "git", "args": ["*"] })),
            None
        );
        assert_eq!(rule("shell.exec", json!({ "program": "*" })), None);
    }

    #[tokio::test]
    async fn an_approved_command_is_returned_only_when_asked_for() {
        let approvals = Arc::new(Approvals::new(Duration::from_secs(5)));
        for (approve, command, expected) in [
            (true, true, Some(vec!["git".to_owned()])),
            (true, false, None),
            (false, true, None),
        ] {
            let (_client, mut notices, _) = approvals.connect();
            let asking = tokio::spawn({
                let approvals = approvals.clone();
                async move { approvals.ask(request()).await }
            });
            let notice = notices.recv().await.unwrap();
            assert_eq!(notice["command"], json!(["git"]));
            assert_eq!(notice["may_remember"], false);
            let id = notice["id"].as_str().unwrap();
            let answered = approvals.answer(id, approve, true, command).unwrap();
            assert_eq!(answered.allow_command, expected);
            assert_eq!(asking.await.unwrap(), Some(approve));
        }
        // Another tool offers no command.
        let (notice, _) = answered(
            &approvals,
            request_for(
                &SessionId::new(),
                "repo.create-file",
                PermissionLevel::Write,
                "write",
            ),
            true,
            false,
        )
        .await;
        assert_eq!(notice["command"], Value::Null);
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
        // Declared lower, but the policy judges it at `execute`, whatever
        // its reason says: a `decisions = ask` entry does not name the level.
        let assigned = ApprovalRequest {
            level: Some(PermissionLevel::Execute),
            ..request_for(
                &session,
                "custom.run",
                PermissionLevel::Write,
                "the policy asks before tool `custom.run` runs",
            )
        };
        let (notice, _) = answered(&approvals, assigned.clone(), true, true).await;
        assert_eq!(notice["may_remember"], false);
        assert_eq!(notice["level"], "execute");
        assert_eq!(approvals.ask(assigned).await, None);
        // A policy that gives no level: not known to be harmless.
        let unknown = ApprovalRequest {
            level: None,
            ..request_for(&session, "custom.note", PermissionLevel::Read, "asks")
        };
        let (notice, _) = answered(&approvals, unknown.clone(), true, true).await;
        assert_eq!(notice["may_remember"], false);
        assert_eq!(approvals.ask(unknown).await, None);
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
        assert!(approvals.answer(&id, true, false, false).is_some());
        assert_eq!(asking.await.unwrap(), Some(true));
        assert_eq!(notices.recv().await.unwrap()["type"], "approval_resolved");
        assert!(approvals.answer(&id, true, false, false).is_none());
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
