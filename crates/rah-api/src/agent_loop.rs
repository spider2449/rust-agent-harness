//! The `loop` service: drives one turn of model and tool rounds.

use std::{
    collections::HashSet,
    sync::{Arc, Mutex, PoisonError},
};

use futures::stream::BoxStream;
use rah_kernel::{KernelHandle, ServiceKey};
use rah_protocol::{AgentEvent, SessionId, SessionRecord};
use tokio::sync::{OwnedRwLockReadGuard, RwLock};
use tokio_util::sync::CancellationToken;

pub const LOOP: ServiceKey<dyn AgentLoop> = ServiceKey::new("loop");

/// The distribution holds the write guard while moving workspace plugins. The
/// value is true while their roots are consistent. Loops keep a read guard
/// throughout each turn, including model hooks, approvals and tool calls.
pub const WORKSPACE_TURNS: ServiceKey<WorkspaceTurns> = ServiceKey::new("workspace-turns");

pub struct WorkspaceTurns(Arc<RwLock<bool>>);

impl WorkspaceTurns {
    /// The host owns the write side and provides this read side to loops.
    pub fn new(consistent: Arc<RwLock<bool>>) -> Self {
        Self(consistent)
    }
}

/// Keep this history alive until the turn ends to pin the workspace.
pub struct TurnHistory {
    pub records: Vec<SessionRecord>,
    _lease: Option<OwnedRwLockReadGuard<bool>>,
}

#[derive(Debug, thiserror::Error)]
pub enum OpenTurnError {
    #[error(transparent)]
    Session(#[from] crate::session::SessionError),
    #[error("the workspace is changing; retry the turn")]
    Switching,
    #[error("the workspace switch was incomplete; select the workspace again")]
    IncompleteSwitch,
    #[error("select this session's workspace before starting a turn: {0}")]
    WrongWorkspace(String),
}

pub async fn open_turn_history(
    kernel: &KernelHandle,
    sessions: &dyn crate::session::SessionLog,
    id: &SessionId,
) -> Result<TurnHistory, OpenTurnError> {
    let lease = kernel
        .get(WORKSPACE_TURNS)
        .map(|workspace| workspace.0.clone().try_read_owned())
        .transpose()
        .map_err(|_| OpenTurnError::Switching)?;
    if lease.as_deref() == Some(&false) {
        return Err(OpenTurnError::IncompleteSwitch);
    }
    let (records, workspace) = sessions.turn_history(id).await?;
    if let Some(expected) = workspace {
        let active = kernel
            .get(crate::sandbox::SANDBOX)
            .and_then(|sandbox| sandbox.root());
        if active.as_deref() != Some(std::path::Path::new(&expected)) {
            return Err(OpenTurnError::WrongWorkspace(expected));
        }
    }
    Ok(TurnHistory {
        records,
        _lease: lease,
    })
}

/// Events of one turn. The stream ends after exactly one terminal event:
/// `Completed`, `Failed` or `Cancelled`.
pub type TurnStream = BoxStream<'static, AgentEvent>;

#[derive(Clone, Debug)]
pub struct TurnRequest {
    pub session: SessionId,
    pub input: String,
    pub cancel: CancellationToken,
}

pub trait AgentLoop: Send + Sync {
    /// Starts a turn. Loops must route every tool call through
    /// [`crate::tools::ToolInvoker`] and record the turn in the session log.
    /// Hold the history from [`open_turn_history`] while running the turn.
    /// A session runs one turn at a time (see [`RunningTurns`]): a second
    /// one ends at once with `Failed` and records nothing.
    fn run_turn(&self, request: TurnRequest) -> TurnStream;
}

/// What a turn fails with when its session already runs one.
pub const TURN_RUNNING: &str =
    "a turn is already running in this session; wait for it or cancel it";

/// The sessions a loop is running a turn in. Two turns in one session
/// would interleave their records in its log: the context rebuilt from it
/// then pairs tool calls with the wrong results, for every later turn.
#[derive(Clone, Default)]
pub struct RunningTurns(Arc<Mutex<HashSet<SessionId>>>);

impl RunningTurns {
    /// Marks `session` as running a turn until the returned guard is
    /// dropped. `None` when it already runs one.
    pub fn begin(&self, session: &SessionId) -> Option<RunningTurn> {
        let mut running = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        running.insert(session.clone()).then(|| RunningTurn {
            turns: self.clone(),
            session: session.clone(),
        })
    }
}

/// Held by a running turn; dropping it frees the session, however the turn
/// ends (completion, failure, cancellation or a disposed scope).
pub struct RunningTurn {
    turns: RunningTurns,
    session: SessionId,
}

impl Drop for RunningTurn {
    fn drop(&mut self) {
        self.turns
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&self.session);
    }
}

/// Published on the kernel event bus for every event a loop puts into a
/// [`TurnStream`], so observers (telemetry, indexing) can follow all turns,
/// including background and subagent turns nobody streams (ADR 0040). It is
/// a copy for observation: the turn does not wait for or depend on
/// subscribers.
#[derive(Clone, Debug, PartialEq)]
pub struct TurnEvent {
    pub event: AgentEvent,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_session_runs_one_turn_until_its_guard_is_dropped() {
        let turns = RunningTurns::default();
        let (one, other) = (SessionId::new(), SessionId::new());
        let first = turns.begin(&one);
        assert!(first.is_some());
        assert!(turns.begin(&one).is_none());
        assert!(turns.clone().begin(&other).is_some());
        drop(first);
        assert!(turns.begin(&one).is_some());
    }
}
