//! Experimental host-owned, connection-bound Tool composition. Used by Desktop production composition.
//! Retained ports hold only a weak host reference. Revocation and dispatch
//! reservation share one lock; effects admitted before revocation are awaited,
//! never replayed or described as rolled back.

use crate::{RuntimeEvent, RuntimeFailure, experimental::*};
use async_trait::async_trait;
use rah_protocol::{
    AgentErrorCode, AgentEvent, PermissionLevel, RuntimeDiagnostic, RuntimeFailureKind,
    RuntimeOperation, SessionId, ToolCall, ToolCallId, ToolOutput,
};
use rah_tools::{
    AuthorizedDispatchError, ToolContext, ToolRegistry, authorize_tool_dispatch,
    authorized_tool_dispatch,
};
use std::sync::{Arc, Mutex, Weak};
use tokio::sync::{Notify, mpsc};

#[derive(Debug, thiserror::Error)]
#[error("host Tool scope is unavailable or busy")]
struct ScopeUnavailable;
pub(crate) fn unavailable() -> RuntimeFailure {
    RuntimeFailure::new(
        RuntimeDiagnostic {
            operation: RuntimeOperation::Turn,
            kind: RuntimeFailureKind::Unavailable,
            rpc_code: None,
        },
        ScopeUnavailable,
    )
}
struct Active {
    session: SessionId,
    events: mpsc::UnboundedSender<RuntimeEvent>,
    accepting: bool,
}
struct State {
    revoked: bool,
    active: Option<Active>,
    inflight: usize,
}
struct Owner {
    registry: Arc<ToolRegistry>,
    permissions: Vec<PermissionLevel>,
    state: Mutex<State>,
    drained: Notify,
    tasks: Mutex<Vec<tokio::task::JoinHandle<()>>>,
}
/// Trusted host owner. Never pass this object to an adapter.
pub struct HostToolScope(Arc<Owner>);
impl HostToolScope {
    pub fn new(registry: Arc<ToolRegistry>, permissions: Vec<PermissionLevel>) -> Self {
        Self(Arc::new(Owner {
            registry,
            permissions,
            state: Mutex::new(State {
                revoked: false,
                active: None,
                inflight: 0,
            }),
            drained: Notify::new(),
            tasks: Mutex::new(Vec::new()),
        }))
    }
    pub fn port(&self) -> Arc<dyn HostToolPort> {
        Arc::new(Port {
            owner: Arc::downgrade(&self.0),
            snapshot: self.0.registry.definitions(),
        })
    }
    /// Withdraw before any transport/provider teardown wait.
    pub fn revoke(&self) {
        let mut state = self
            .0
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.revoked = true;
        state.active = None;
    }
    /// Wait for all already admitted executions and their terminal publication.
    pub async fn drained(&self) {
        loop {
            let notified = self.0.drained.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            if self
                .0
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .inflight
                == 0
            {
                break;
            }
            notified.await;
        }
        let tasks = std::mem::take(
            &mut *self
                .0
                .tasks
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        );
        for task in tasks {
            let _ = task.await;
        }
    }
}
impl Drop for HostToolScope {
    fn drop(&mut self) {
        self.revoke();
    }
}
struct Port {
    owner: Weak<Owner>,
    snapshot: Vec<rah_protocol::ToolDefinition>,
}
struct Lease {
    owner: Weak<Owner>,
    session: SessionId,
}
impl HostTurnLifetime for Lease {
    fn stop_requests(&self) {
        if let Some(owner) = self.owner.upgrade() {
            let mut state = owner
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(active) = state.active.as_mut().filter(|a| a.session == self.session) {
                active.accepting = false;
            }
        }
    }
}
impl Drop for Lease {
    fn drop(&mut self) {
        if let Some(owner) = self.owner.upgrade() {
            let mut state = owner
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if state
                .active
                .as_ref()
                .is_some_and(|a| a.session == self.session)
            {
                state.active = None;
            }
        }
    }
}
struct Reservation(Arc<Owner>);
impl Drop for Reservation {
    fn drop(&mut self) {
        self.0
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .inflight -= 1;
        self.0.drained.notify_waiters();
    }
}
#[async_trait]
impl HostToolPort for Port {
    fn definitions(&self) -> ToolSnapshot {
        ToolSnapshot(self.snapshot.clone())
    }
    async fn request(&self, _request: ToolRequest) -> Result<ToolReply, RuntimeFailure> {
        // Explicitly live-only: cannot accidentally duplicate buffered events.
        Err(unavailable())
    }
    async fn admit_turn(&self) -> Result<HostTurnLease, RuntimeFailure> {
        let owner = self.owner.upgrade().ok_or_else(unavailable)?;
        let mut state = owner
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.revoked || state.active.is_some() || state.inflight != 0 {
            return Err(unavailable());
        }
        let session_id = SessionId::new();
        let (events, mut receiver) = mpsc::unbounded_channel();
        state.active = Some(Active {
            session: session_id.clone(),
            events,
            accepting: true,
        });
        Ok(HostTurnLease {
            session_id: session_id.clone(),
            lifetime: Box::new(Lease {
                owner: self.owner.clone(),
                session: session_id,
            }),
            events: Box::pin(async_stream::stream! {
                while let Some(event) = receiver.recv().await { yield event; }
            }),
        })
    }
    async fn request_live(&self, request: ToolRequest) -> Result<ToolOutput, RuntimeFailure> {
        #[cfg(feature = "live-test-support")]
        let _timing = DispatchTiming::new();
        let owner = self.owner.upgrade().ok_or_else(unavailable)?;
        let expected = self
            .snapshot
            .iter()
            .find(|d| d.name == request.name)
            .cloned()
            .ok_or_else(unavailable)?;
        let call = ToolCall {
            id: ToolCallId::new(),
            name: request.name,
            input: request.input,
        };
        let (events, reservation) = {
            let mut state = owner
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if state.revoked || state.inflight >= 128 {
                return Err(unavailable());
            }
            let active = state
                .active
                .as_ref()
                .filter(|a| a.session == request.session_id && a.accepting)
                .ok_or_else(unavailable)?;
            let events = active.events.clone();
            let _ = events.send(
                AgentEvent::ToolRequested {
                    session_id: request.session_id.clone(),
                    tool_call: call.clone(),
                }
                .into(),
            );
            if let Err(source) =
                authorize_tool_dispatch(&owner.registry, &expected, &owner.permissions, &call)
            {
                let failure = dispatch_failure(AuthorizedDispatchError::Rejected(source));
                let _ = events.send(RuntimeEvent::failed(
                    request.session_id,
                    AgentErrorCode::PermissionDenied,
                    failure.clone(),
                ));
                return Err(failure);
            }
            #[cfg(feature = "live-test-support")]
            dispatch_mark("authorization_complete");
            state.inflight += 1;
            let _ = events.send(
                AgentEvent::ToolStarted {
                    session_id: request.session_id.clone(),
                    tool_call_id: call.id.clone(),
                }
                .into(),
            );
            (events, Reservation(owner.clone()))
        };
        // This future is adapter-owned; the reserved execution is host-owned and
        // survives a dropped response future. Reservation owns terminal accounting.
        let (reply, receiver) = tokio::sync::oneshot::channel();
        {
            let task_owner = owner.clone();
            let mut tasks = task_owner
                .tasks
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            tasks.retain(|task| !task.is_finished());
            let task = tokio::spawn(async move {
                let result = authorized_tool_dispatch(
                    &owner.registry,
                    &expected,
                    &owner.permissions,
                    call.clone(),
                    ToolContext::default(),
                )
                .await;
                #[cfg(feature = "live-test-support")]
                dispatch_mark("tool_execution_complete");
                let result = match result {
                    Ok(output) => {
                        let _ = events.send(
                            AgentEvent::ToolFinished {
                                session_id: request.session_id,
                                tool_call_id: call.id,
                                output: output.clone(),
                            }
                            .into(),
                        );
                        Ok(output)
                    }
                    Err(source) => {
                        let code = if matches!(source, AuthorizedDispatchError::Rejected(_)) {
                            AgentErrorCode::PermissionDenied
                        } else {
                            AgentErrorCode::Tool
                        };
                        let failure = dispatch_failure(source);
                        let _ = events.send(RuntimeEvent::failed(
                            request.session_id,
                            code,
                            failure.clone(),
                        ));
                        Err(failure)
                    }
                };
                #[cfg(feature = "live-test-support")]
                dispatch_mark("result_conversion_complete");
                let _ = reply.send(result);
                drop(reservation);
            });
            tasks.push(task);
        }
        receiver.await.map_err(|_| unavailable())?
    }
}
fn dispatch_failure(source: AuthorizedDispatchError) -> RuntimeFailure {
    RuntimeFailure::new(
        RuntimeDiagnostic {
            operation: RuntimeOperation::Turn,
            kind: RuntimeFailureKind::Operation,
            rpc_code: None,
        },
        source,
    )
}

/// Closed classification only; never formats a private error or provider input.
pub fn dispatch_error_category(source: &AuthorizedDispatchError) -> &'static str {
    use rah_tools::{AuthorizedDispatchRejection as R, ToolError as T};
    match source {
        AuthorizedDispatchError::Rejected(R::NameMismatch { .. }) => "name_mismatch",
        AuthorizedDispatchError::Rejected(R::UnknownTool { .. }) => "unknown_tool",
        AuthorizedDispatchError::Rejected(R::DefinitionMismatch { .. }) => "definition_mismatch",
        AuthorizedDispatchError::Rejected(R::PermissionDenied { .. }) => "permission_denied",
        AuthorizedDispatchError::Tool(T::InvalidInput { .. }) => "tool_invalid_input",
        AuthorizedDispatchError::Tool(T::Execution { .. }) => "tool_execution",
        AuthorizedDispatchError::Tool(T::UnknownTool { .. }) => "tool_unknown",
        AuthorizedDispatchError::Tool(T::DuplicateTool { .. }) => "tool_duplicate",
    }
}

#[cfg(feature = "live-test-support")]
struct DispatchTiming(std::time::Instant);
#[cfg(feature = "live-test-support")]
impl DispatchTiming {
    fn new() -> Self {
        let started = std::time::Instant::now();
        dispatch_mark("dispatch_start");
        Self(started)
    }
}
#[cfg(feature = "live-test-support")]
impl Drop for DispatchTiming {
    fn drop(&mut self) {
        if std::env::var_os("RAH_R4H_TIMING").is_some() {
            dispatch_mark("dispatch_return");
            eprintln!("R4H dispatch_total={:?}", self.0.elapsed());
        }
    }
}
#[cfg(feature = "live-test-support")]
fn dispatch_mark(event: &str) {
    if std::env::var_os("RAH_R4H_TIMING").is_some() {
        eprintln!(
            "R4H host event={event} wall_ns={}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::StreamExt;
    use rah_protocol::{ToolContent, ToolDefinition, ToolInput, ToolName};
    use rah_tools::{Tool, ToolError};
    use std::{
        error::Error,
        sync::atomic::{AtomicUsize, Ordering},
    };
    #[tokio::test]
    async fn repo_list_root_nested_invalid_and_denied_through_live_host() {
        use std::process::Command;
        let root = std::env::temp_dir().join(format!("rah-525b-{}", ToolCallId::new()));
        std::fs::create_dir_all(root.join("nested")).unwrap();
        std::fs::write(
            root.join("nested/file.txt"),
            "private-file-content-sentinel",
        )
        .unwrap();
        #[cfg(windows)]
        let locator = Command::new("where.exe").arg("git.exe").output().unwrap();
        #[cfg(not(windows))]
        let locator = Command::new("which").arg("git").output().unwrap();
        assert!(locator.status.success());
        let git = std::path::PathBuf::from(
            String::from_utf8(locator.stdout)
                .unwrap()
                .lines()
                .next()
                .unwrap(),
        );
        for args in [
            vec!["init", "--quiet"],
            vec!["add", "--", "nested/file.txt"],
        ] {
            assert!(
                Command::new(&git)
                    .args(args)
                    .current_dir(&root)
                    .output()
                    .unwrap()
                    .status
                    .success()
            );
        }
        for (input, allowed, category) in [
            (serde_json::json!({}), true, None),
            (serde_json::json!({"path":"nested"}), true, None),
            (
                serde_json::json!({"path":""}),
                true,
                Some("tool_invalid_input"),
            ),
            (
                serde_json::json!({"path":"../private"}),
                true,
                Some("tool_invalid_input"),
            ),
            (serde_json::json!({}), false, Some("permission_denied")),
        ] {
            let mut registry = ToolRegistry::new();
            registry
                .register(Arc::new(
                    rah_tools::RepositoryListTool::new(&git, &root).unwrap(),
                ))
                .unwrap();
            let scope = HostToolScope::new(
                Arc::new(registry),
                if allowed {
                    vec![PermissionLevel::Execute]
                } else {
                    vec![]
                },
            );
            let port = scope.port();
            let mut lease = port.admit_turn().await.unwrap();
            let result = port
                .request_live(ToolRequest {
                    session_id: lease.session_id.clone(),
                    name: ToolName::new("repo.list"),
                    input: ToolInput(input),
                })
                .await;
            let requested = lease.events.next().await.unwrap();
            let AgentEvent::ToolRequested {
                session_id,
                tool_call,
            } = requested.event()
            else {
                panic!("Requested missing")
            };
            assert_eq!(session_id, &lease.session_id);
            let id = tool_call.id.clone();
            if allowed {
                assert!(
                    matches!(lease.events.next().await.unwrap().event(), AgentEvent::ToolStarted { tool_call_id,.. } if *tool_call_id == id)
                );
            }
            let terminal = lease.events.next().await.unwrap();
            if let Some(category) = category {
                let error = result.unwrap_err();
                assert_eq!(
                    dispatch_error_category(
                        error
                            .source()
                            .unwrap()
                            .downcast_ref::<AuthorizedDispatchError>()
                            .unwrap()
                    ),
                    category
                );
                assert!(
                    matches!(terminal.event(), AgentEvent::Failed { session_id,.. } if *session_id == lease.session_id)
                );
            } else {
                let output = result.unwrap();
                assert!(!output.is_error);
                assert!(
                    !serde_json::to_string(&output)
                        .unwrap()
                        .contains("private-file-content-sentinel")
                );
                assert!(
                    matches!(terminal.event(), AgentEvent::ToolFinished { session_id,tool_call_id,.. } if *session_id == lease.session_id && *tool_call_id == id)
                );
            }
            scope.drained().await;
            drop(lease.lifetime);
            assert!(lease.events.next().await.is_none());
        }
        std::fs::remove_dir_all(root).unwrap();
    }
    struct CountingTool {
        effects: Arc<AtomicUsize>,
        release: Arc<Notify>,
        fail: bool,
    }
    #[async_trait]
    impl Tool for CountingTool {
        fn definition(&self) -> ToolDefinition {
            ToolDefinition {
                name: ToolName::new("test.effect"),
                description: "test".into(),
                input_schema: serde_json::json!({}),
                permission: PermissionLevel::Execute,
            }
        }
        async fn execute(&self, input: ToolInput, _: ToolContext) -> Result<ToolOutput, ToolError> {
            self.effects.fetch_add(1, Ordering::SeqCst);
            self.release.notified().await;
            if self.fail {
                return Err(ToolError::Execution {
                    message: "private sentinel".into(),
                });
            }
            Ok(ToolOutput {
                content: vec![ToolContent::Json(input.0)],
                is_error: false,
            })
        }
    }
    fn setup(
        allowed: bool,
        fail: bool,
    ) -> (
        HostToolScope,
        Arc<dyn HostToolPort>,
        Arc<AtomicUsize>,
        Arc<Notify>,
    ) {
        let effects = Arc::new(AtomicUsize::new(0));
        let release = Arc::new(Notify::new());
        let mut registry = ToolRegistry::new();
        registry
            .register(Arc::new(CountingTool {
                effects: effects.clone(),
                release: release.clone(),
                fail,
            }))
            .unwrap();
        let scope = HostToolScope::new(
            Arc::new(registry),
            if allowed {
                vec![PermissionLevel::Execute]
            } else {
                vec![]
            },
        );
        let port = scope.port();
        (scope, port, effects, release)
    }
    fn request(session: &SessionId, value: i32) -> ToolRequest {
        ToolRequest {
            session_id: session.clone(),
            name: ToolName::new("test.effect"),
            input: ToolInput(serde_json::json!({"uncertain":true,"value":value})),
        }
    }
    #[tokio::test]
    async fn live_events_precede_result_and_revocation_rejects_retained_port() {
        let (scope, port, effects, release) = setup(true, false);
        let mut lease = port.admit_turn().await.unwrap();
        assert!(port.admit_turn().await.is_err());
        let p = port.clone();
        let r = request(&lease.session_id, 7);
        let task = tokio::spawn(async move { p.request_live(r).await });
        assert!(matches!(
            lease.events.next().await.unwrap().event(),
            AgentEvent::ToolRequested { .. }
        ));
        assert!(matches!(
            lease.events.next().await.unwrap().event(),
            AgentEvent::ToolStarted { .. }
        ));
        while effects.load(Ordering::SeqCst) == 0 {
            tokio::task::yield_now().await;
        }
        scope.revoke();
        assert!(
            port.request_live(request(&lease.session_id, 8))
                .await
                .is_err()
        );
        assert!(port.admit_turn().await.is_err());
        assert_eq!(effects.load(Ordering::SeqCst), 1);
        release.notify_one();
        let output = task.await.unwrap().unwrap();
        assert_eq!(
            output.content,
            vec![ToolContent::Json(
                serde_json::json!({"uncertain":true,"value":7})
            )]
        );
        assert!(matches!(
            lease.events.next().await.unwrap().event(),
            AgentEvent::ToolFinished { .. }
        ));
        scope.drained().await;
        drop(scope);
        assert!(
            port.request_live(request(&lease.session_id, 9))
                .await
                .is_err()
        );
    }
    #[tokio::test]
    async fn host_allows_distinct_calls_in_one_turn_and_keeps_effect_after_response_drop() {
        let (scope, port, effects, release) = setup(true, false);
        let mut lease = port.admit_turn().await.unwrap();
        let mut tasks = Vec::new();
        for value in [1, 2] {
            let p = port.clone();
            let r = request(&lease.session_id, value);
            tasks.push(tokio::spawn(async move { p.request_live(r).await }));
        }
        for _ in 0..4 {
            lease.events.next().await.unwrap();
        }
        while effects.load(Ordering::SeqCst) != 2 {
            tokio::task::yield_now().await;
        }
        tasks.remove(0).abort();
        drop(lease.lifetime);
        assert!(port.admit_turn().await.is_err());
        release.notify_waiters();
        tasks.remove(0).await.unwrap().unwrap();
        scope.drained().await;
        assert_eq!(effects.load(Ordering::SeqCst), 2);
        assert!(port.admit_turn().await.is_ok());
    }
    #[tokio::test]
    async fn permission_and_tool_sources_are_typed_and_live_even_on_error() {
        for denied in [true, false] {
            let (scope, port, effects, release) = setup(!denied, true);
            let mut lease = port.admit_turn().await.unwrap();
            let p = port.clone();
            let r = request(&lease.session_id, 1);
            let task = tokio::spawn(async move { p.request_live(r).await });
            assert!(matches!(
                lease.events.next().await.unwrap().event(),
                AgentEvent::ToolRequested { .. }
            ));
            if !denied {
                lease.events.next().await.unwrap();
                release.notify_one();
            }
            let event = lease.events.next().await.unwrap();
            assert!(
                matches!(event.event(), AgentEvent::Failed { code, .. } if *code == if denied { AgentErrorCode::PermissionDenied } else { AgentErrorCode::Tool })
            );
            let error = task.await.unwrap().unwrap_err();
            let source = error
                .source()
                .unwrap()
                .downcast_ref::<AuthorizedDispatchError>()
                .unwrap();
            assert_eq!(
                matches!(source, AuthorizedDispatchError::Rejected(_)),
                denied
            );
            assert_eq!(effects.load(Ordering::SeqCst), usize::from(!denied));
            assert!(!format!("{error:?}").contains("private sentinel"));
            scope.revoke();
            scope.drained().await;
        }
    }
}
