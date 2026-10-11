//! The `loop` service: drives one turn of model and tool rounds.

use futures::stream::BoxStream;
use rah_kernel::ServiceKey;
use rah_protocol::{AgentEvent, SessionId};
use tokio_util::sync::CancellationToken;

pub const LOOP: ServiceKey<dyn AgentLoop> = ServiceKey::new("loop");

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
    fn run_turn(&self, request: TurnRequest) -> TurnStream;
}
