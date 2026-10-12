//! The `scheduler` service: turns that run in the background, once or
//! repeatedly.

use std::time::Duration;

use rah_kernel::ServiceKey;
use rah_protocol::SessionId;

pub const SCHEDULER: ServiceKey<dyn Scheduler> = ServiceKey::new("scheduler");

/// What to run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JobSpec {
    /// The turn input.
    pub input: String,
    /// Continue this session; `None` creates a new session for every run.
    pub session: Option<SessionId>,
    /// Wait before the first run.
    pub delay: Duration,
    /// Run again this long after each run ends; `None` runs once.
    pub every: Option<Duration>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JobState {
    Waiting,
    Running,
    /// The last run completed with this answer (a repeating job waits again).
    Done {
        answer: String,
    },
    Failed {
        message: String,
    },
    Cancelled,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JobInfo {
    pub id: String,
    pub spec: JobSpec,
    pub state: JobState,
    /// Completed or failed runs so far.
    pub runs: u32,
    /// The session of the latest run.
    pub last_session: Option<SessionId>,
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum SchedulerError {
    #[error("invalid job: {0}")]
    Invalid(String),
    #[error("the scheduler is shutting down")]
    Stopped,
}

pub trait Scheduler: Send + Sync {
    fn submit(&self, spec: JobSpec) -> Result<JobInfo, SchedulerError>;

    /// Every job this scheduler knows, oldest first.
    fn list(&self) -> Vec<JobInfo>;

    /// Cancels a waiting or running job. Returns whether the job existed and
    /// was not already over.
    fn cancel(&self, id: &str) -> bool;
}
