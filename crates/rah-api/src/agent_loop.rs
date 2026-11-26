//! The `loop` service: drives one turn of model and tool rounds.

use std::{
    collections::HashSet,
    sync::{Arc, Mutex, PoisonError},
};

use futures::stream::BoxStream;
use rah_kernel::{KernelHandle, ServiceKey};
use rah_protocol::{AgentEvent, ContextItem, RequestId, SessionEntry, SessionId, SessionRecord};
use tokio::sync::{OwnedRwLockReadGuard, OwnedRwLockWriteGuard, RwLock, oneshot};
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TurnOrigin {
    User,
    Background,
}

#[derive(Clone, Debug)]
pub struct TurnRequest {
    pub origin: TurnOrigin,
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

    /// Starts a turn that may accept further user input. Loops that only
    /// support an initial prompt leave the steering handle absent.
    fn run_steerable_turn(&self, request: TurnRequest) -> (TurnStream, Option<TurnSteering>) {
        (self.run_turn(request), None)
    }
}

struct SteeringInput {
    input: String,
    recorded: oneshot::Sender<()>,
}

type SteeringQueue = Arc<Mutex<Option<Vec<SteeringInput>>>>;

/// A handle to the input queue of exactly one running turn.
#[derive(Clone)]
pub struct TurnSteering(SteeringQueue);

/// The loop's half of the steering handle. Dropping it rejects pending input.
pub struct SteeringInbox(SteeringQueue);

#[derive(Debug, thiserror::Error)]
pub enum SteeringError {
    #[error("the turn has stopped accepting input")]
    Closed,
    #[error("guidance must contain between 1 and 32768 bytes of text")]
    Invalid,
    #[error("the turn has too much guidance waiting to be applied")]
    Full,
}

impl TurnSteering {
    pub fn channel() -> (Self, SteeringInbox) {
        let queue = Arc::new(Mutex::new(Some(Vec::new())));
        (Self(queue.clone()), SteeringInbox(queue))
    }

    /// The receipt succeeds once the guidance has been recorded in the turn.
    pub fn send(&self, input: &str) -> Result<oneshot::Receiver<()>, SteeringError> {
        let input = input.trim();
        if input.is_empty() || input.len() > 32_768 {
            return Err(SteeringError::Invalid);
        }
        let mut queue = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        let queue = queue.as_mut().ok_or(SteeringError::Closed)?;
        if queue.len() >= 8 {
            return Err(SteeringError::Full);
        }
        let (recorded, receipt) = oneshot::channel();
        queue.push(SteeringInput {
            input: input.to_owned(),
            recorded,
        });
        Ok(receipt)
    }
}

impl SteeringInbox {
    pub fn close(&self) {
        *self.0.lock().unwrap_or_else(PoisonError::into_inner) = None;
    }

    pub fn has_pending(&self) -> bool {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
            .is_some_and(|queue| !queue.is_empty())
    }

    /// Records queued messages at a model boundary, after any in-flight tool
    /// result. When completing a turn, atomically close the empty queue so a
    /// concurrent submission is either consumed or rejected.
    pub async fn record_pending(
        &self,
        sessions: &dyn crate::session::SessionLog,
        session: &SessionId,
        turn: &RequestId,
        items: &mut Vec<ContextItem>,
        completing: bool,
    ) -> Result<bool, crate::session::SessionError> {
        let messages = {
            let mut queue = self.0.lock().unwrap_or_else(PoisonError::into_inner);
            match queue.as_mut() {
                Some(messages) if !messages.is_empty() => std::mem::take(messages),
                _ => {
                    if completing {
                        *queue = None;
                    }
                    return Ok(false);
                }
            }
        };
        for SteeringInput { input, recorded } in messages {
            sessions
                .append(
                    session,
                    SessionEntry::UserMessage {
                        turn: turn.clone(),
                        input: input.clone(),
                    },
                )
                .await?;
            items.push(ContextItem::User { text: input });
            let _ = recorded.send(());
        }
        Ok(true)
    }
}

impl Drop for SteeringInbox {
    fn drop(&mut self) {
        self.close();
    }
}

/// What a turn fails with when its session already runs one.
pub const TURN_RUNNING: &str =
    "a turn is already running in this session; wait for it or cancel it";

/// Distribution-wide admission state shared by the standard loop instances.
pub const RUNNING_TURNS: ServiceKey<RunningTurns> = ServiceKey::new("running-turns");

/// The sessions a loop is running a turn in. Two turns in one session
/// would interleave their records in its log: the context rebuilt from it
/// then pairs tool calls with the wrong results, for every later turn.
#[derive(Clone, Default)]
pub struct RunningTurns {
    sessions: Arc<Mutex<HashSet<SessionId>>>,
    usage: Arc<RwLock<()>>,
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("The loop is in use. Save again after the current work finishes.")]
pub struct LoopBusy;

impl RunningTurns {
    /// Marks `session` as running a turn until the returned guard is
    /// dropped. `None` when it already runs one.
    pub fn begin(&self, session: &SessionId) -> Option<RunningTurn> {
        self.admit(session).ok()
    }

    pub fn try_change(&self) -> Result<OwnedRwLockWriteGuard<()>, LoopBusy> {
        self.usage.clone().try_write_owned().map_err(|_| LoopBusy)
    }

    pub fn admit(&self, session: &SessionId) -> Result<RunningTurn, &'static str> {
        let lease = self
            .usage
            .clone()
            .try_read_owned()
            .map_err(|_| "the loop is changing; retry the turn")?;
        let mut running = self.sessions.lock().unwrap_or_else(PoisonError::into_inner);
        if !running.insert(session.clone()) {
            return Err(TURN_RUNNING);
        }
        Ok(RunningTurn {
            turns: self.clone(),
            session: session.clone(),
            _lease: lease,
        })
    }
}

/// Held by a running turn; dropping it frees the session, however the turn
/// ends (completion, failure, cancellation or a disposed scope).
pub struct RunningTurn {
    _lease: OwnedRwLockReadGuard<()>,
    turns: RunningTurns,
    session: SessionId,
}

impl Drop for RunningTurn {
    fn drop(&mut self) {
        self.turns
            .sessions
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
        assert!(turns.try_change().is_err());
        assert!(turns.begin(&one).is_none());
        assert!(turns.clone().begin(&other).is_some());
        drop(first);
        let change = turns.try_change().unwrap();
        assert!(turns.admit(&one).is_err());
        drop(change);
        assert!(turns.begin(&one).is_some());
    }
}
