//! `scheduler-basic`: background turns and subagents.
//!
//! - Provides the `scheduler` service: jobs run a turn through the `loop`
//!   service (resolved per run) after an optional delay, once or repeatedly,
//!   at most `max_concurrent` at a time. Each run uses a new session unless
//!   the job names one. Jobs are tasks of this plugin's scope; unmounting it
//!   cancels them.
//! - Provides `agent.delegate {task}`: runs a subagent turn in a new session
//!   and returns its answer and session id. The loop is a service, so this
//!   nests a turn inside a tool call; depth is limited by `max_depth`. The
//!   subagent's own tool calls pass through the policy one by one and are
//!   recorded, with their effects, in its session; the tool itself declares
//!   `read`.
//! - Named subagents: each `<agents_dir>/<name>.md` (front matter `name`,
//!   `description`, then instructions, as a skill) is offered to the model in
//!   the description of `agent.delegate`, which then takes an optional
//!   `agent`. The subagent's session starts with those instructions followed
//!   by the task. The directory is read when the plugin starts; a file that
//!   cannot be used is told to the user once with a `HostNotice` (ADR 0057).
//!   A named subagent has the same tools and the same policy as any other:
//!   the file grants nothing and cannot narrow the tools.
//!
//! Configuration (all optional): `{ "max_depth": 2, "max_concurrent": 2,
//! "delegate_timeout_secs": 600, "agents_dir": ".rah/agents", "jobs": [{
//! "input": "...", "delay_secs": 0, "every_secs": 3600, "session": "<id>" }]
//! }`.

mod agents;

use std::{
    collections::HashMap,
    path::PathBuf,
    str::FromStr,
    sync::{Arc, Mutex, PoisonError},
    time::Duration,
};

use futures::StreamExt;
use rah_api::{
    CancellationToken, KernelHandle, Plugin, PluginError, PluginId, PluginManifest, Scope,
    agent_loop::{LOOP, TurnRequest},
    async_trait,
    host::NoticeOnce,
    kernel::Spawner,
    scheduler::{JobInfo, JobSpec, JobState, SCHEDULER, Scheduler, SchedulerError},
    session::SESSION,
    tools::{TOOLS_SERVICE, Tool, ToolContext, ToolError, provide_tools},
};
use rah_protocol::{
    AgentEvent, PermissionLevel, SessionId, ToolContent, ToolDefinition, ToolName, ToolOutput,
};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::Semaphore;

pub struct SchedulerPlugin {
    manifest: PluginManifest,
}

impl SchedulerPlugin {
    pub fn new(id: impl Into<PluginId>) -> Self {
        Self {
            manifest: PluginManifest::new(id, env!("CARGO_PKG_VERSION"))
                .provides(["scheduler", TOOLS_SERVICE])
                .optional(["loop", "session"]),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ConfiguredJob {
    input: String,
    #[serde(default)]
    delay_secs: u64,
    every_secs: Option<u64>,
    session: Option<String>,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    max_depth: Option<usize>,
    max_concurrent: Option<usize>,
    delegate_timeout_secs: Option<u64>,
    agents_dir: Option<PathBuf>,
    #[serde(default)]
    jobs: Vec<ConfiguredJob>,
}

#[async_trait]
impl Plugin for SchedulerPlugin {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    fn check_config(&self, config: &serde_json::Value) -> Result<(), PluginError> {
        rah_api::config::parse::<Config>(config)?;
        Ok(())
    }

    async fn apply(&self, scope: &mut Scope) -> Result<(), PluginError> {
        let config: Config = rah_api::config::parse(scope.config())?;
        let scheduler = Arc::new(BasicScheduler {
            kernel: scope.kernel(),
            spawner: scope.spawner(),
            jobs: Arc::new(Mutex::new(Vec::new())),
            permits: Arc::new(Semaphore::new(config.max_concurrent.unwrap_or(2).max(1))),
        });
        for job in config.jobs {
            let session = job
                .session
                .map(|id| {
                    SessionId::from_str(&id).map_err(|_| format!("`{id}` is not a session id"))
                })
                .transpose()?;
            scheduler
                .submit(JobSpec {
                    input: job.input,
                    session,
                    delay: Duration::from_secs(job.delay_secs),
                    every: job.every_secs.map(Duration::from_secs),
                })
                .map_err(|error| error.to_string())?;
        }
        scope.provide(SCHEDULER, scheduler as Arc<dyn Scheduler>)?;
        let agents = match &config.agents_dir {
            Some(dir) => {
                let (agents, problems) = agents::read_dir(dir).await;
                let notices = NoticeOnce::new(scope.kernel(), scope.id().clone());
                for problem in problems {
                    notices.emit(&problem, &problem);
                }
                agents
            }
            None => Vec::new(),
        };
        let delegate = Delegate {
            kernel: scope.kernel(),
            agents,
            depths: Mutex::new(HashMap::new()),
            max_depth: config.max_depth.unwrap_or(2),
            timeout: Duration::from_secs(config.delegate_timeout_secs.unwrap_or(600).max(1)),
        };
        provide_tools(scope, vec![Arc::new(delegate)])?;
        Ok(())
    }
}

type TurnResult = Result<(SessionId, String), (Option<SessionId>, String)>;

/// Runs one turn and waits for its end: `(session, answer)` or a message.
async fn run_turn(
    kernel: &KernelHandle,
    session: Option<SessionId>,
    input: String,
    cancel: CancellationToken,
) -> TurnResult {
    let sessions = kernel
        .get(SESSION)
        .ok_or((None, "no session service is mounted".to_owned()))?;
    let session = match session {
        Some(session) => session,
        None => {
            sessions
                .create()
                .await
                .map_err(|error| (None, error.to_string()))?
                .id
        }
    };
    let Some(agent_loop) = kernel.get(LOOP) else {
        return Err((Some(session), "no agent loop is mounted".into()));
    };
    let mut events = agent_loop.run_turn(TurnRequest {
        session: session.clone(),
        input,
        cancel,
    });
    while let Some(event) = events.next().await {
        match event {
            AgentEvent::Completed { output, .. } => return Ok((session, output.message.content)),
            AgentEvent::Failed { message, .. } => return Err((Some(session), message)),
            AgentEvent::Cancelled { .. } => return Err((Some(session), "cancelled".into())),
            _ => {}
        }
    }
    Err((
        Some(session),
        "the agent loop stopped before the turn ended".into(),
    ))
}

struct Job {
    info: JobInfo,
    cancel: CancellationToken,
}

type Jobs = Arc<Mutex<Vec<Job>>>;

/// Applies `change` to a job, unless it was cancelled meanwhile.
fn update(jobs: &Jobs, id: &str, change: impl FnOnce(&mut JobInfo)) {
    let mut jobs = jobs.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(job) = jobs.iter_mut().find(|job| job.info.id == id) {
        if job.cancel.is_cancelled() {
            job.info.state = JobState::Cancelled;
        } else {
            change(&mut job.info);
        }
    }
}

struct BasicScheduler {
    kernel: KernelHandle,
    spawner: Spawner,
    jobs: Jobs,
    permits: Arc<Semaphore>,
}

/// One job's runs, until it is done or cancelled.
async fn drive(
    kernel: KernelHandle,
    permits: Arc<Semaphore>,
    jobs: Jobs,
    id: String,
    spec: JobSpec,
    cancel: CancellationToken,
) {
    let mut wait = spec.delay;
    loop {
        tokio::select! {
            () = cancel.cancelled() => return,
            () = tokio::time::sleep(wait) => {}
        }
        let _permit = tokio::select! {
            () = cancel.cancelled() => return,
            permit = permits.clone().acquire_owned() => match permit {
                Ok(permit) => permit,
                Err(_) => return,
            },
        };
        update(&jobs, &id, |info| info.state = JobState::Running);
        let result = run_turn(
            &kernel,
            spec.session.clone(),
            spec.input.clone(),
            cancel.child_token(),
        )
        .await;
        update(&jobs, &id, |info| {
            info.runs += 1;
            match result {
                Ok((session, answer)) => {
                    info.last_session = Some(session);
                    info.state = JobState::Done { answer };
                }
                Err((session, message)) => {
                    if session.is_some() {
                        info.last_session = session;
                    }
                    info.state = JobState::Failed { message };
                }
            }
        });
        match spec.every {
            Some(every) => wait = every,
            None => return,
        }
    }
}

impl Scheduler for BasicScheduler {
    fn submit(&self, spec: JobSpec) -> Result<JobInfo, SchedulerError> {
        if spec.input.trim().is_empty() {
            return Err(SchedulerError::Invalid("the input is empty".into()));
        }
        if spec
            .every
            .is_some_and(|every| every < Duration::from_secs(1))
        {
            return Err(SchedulerError::Invalid(
                "`every` must be at least one second".into(),
            ));
        }
        let info = JobInfo {
            id: uuid::Uuid::new_v4().simple().to_string(),
            spec: spec.clone(),
            state: JobState::Waiting,
            runs: 0,
            last_session: None,
        };
        let cancel = self.spawner.cancellation();
        self.jobs
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(Job {
                info: info.clone(),
                cancel: cancel.clone(),
            });
        let task = drive(
            self.kernel.clone(),
            self.permits.clone(),
            self.jobs.clone(),
            info.id.clone(),
            spec,
            cancel,
        );
        if !self.spawner.spawn(task) {
            return Err(SchedulerError::Stopped);
        }
        Ok(info)
    }

    fn list(&self) -> Vec<JobInfo> {
        self.jobs
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .map(|job| job.info.clone())
            .collect()
    }

    fn cancel(&self, id: &str) -> bool {
        let mut jobs = self.jobs.lock().unwrap_or_else(PoisonError::into_inner);
        let Some(job) = jobs.iter_mut().find(|job| job.info.id == id) else {
            return false;
        };
        let over = matches!(job.info.state, JobState::Cancelled)
            || (job.info.spec.every.is_none()
                && matches!(
                    job.info.state,
                    JobState::Done { .. } | JobState::Failed { .. }
                ));
        if over {
            return false;
        }
        job.cancel.cancel();
        job.info.state = JobState::Cancelled;
        true
    }
}

/// `agent.delegate {task}`: a subagent turn in a new session.
struct Delegate {
    kernel: KernelHandle,
    /// The named subagents, sorted by name.
    agents: Vec<agents::Agent>,
    /// Delegation depth of the sessions this tool created.
    depths: Mutex<HashMap<SessionId, usize>>,
    max_depth: usize,
    timeout: Duration,
}

#[async_trait]
impl Tool for Delegate {
    fn definition(&self) -> ToolDefinition {
        let mut description = String::from(
            "Gives a self-contained task to a subagent, which works in its own new session \
             with the same tools, and returns its final answer. Describe the task completely; \
             the subagent does not see this conversation.",
        );
        let mut properties = json!({ "task": { "type": "string", "minLength": 1 } });
        if !self.agents.is_empty() {
            description
                .push_str(" Set `agent` to start the subagent from one of these instructions:");
            for agent in &self.agents {
                description.push_str(&format!("\n- {}: {}", agent.name, agent.description));
            }
            let names: Vec<&str> = self
                .agents
                .iter()
                .map(|agent| agent.name.as_str())
                .collect();
            properties["agent"] = json!({ "type": "string", "enum": names });
        }
        ToolDefinition {
            name: ToolName::new("agent.delegate"),
            description,
            input_schema: json!({
                "type": "object",
                "properties": properties,
                "required": ["task"],
                "additionalProperties": false
            }),
            permission: PermissionLevel::Read,
        }
    }

    async fn call(&self, input: Value, cx: ToolContext) -> Result<ToolOutput, ToolError> {
        let task = input
            .get("task")
            .and_then(Value::as_str)
            .filter(|task| !task.trim().is_empty())
            .ok_or_else(|| ToolError::InvalidInput("expected a non-empty string `task`".into()))?
            .to_owned();
        let task = match input.get("agent") {
            None | Some(Value::Null) => task,
            Some(Value::String(name)) => {
                let agent = self
                    .agents
                    .iter()
                    .find(|agent| agent.name == *name)
                    .ok_or_else(|| {
                        let known: Vec<&str> = self
                            .agents
                            .iter()
                            .map(|agent| agent.name.as_str())
                            .collect();
                        ToolError::InvalidInput(if known.is_empty() {
                            format!("there is no agent `{name}`; there are no named agents")
                        } else {
                            format!(
                                "there is no agent `{name}`; use one of: {}",
                                known.join(", ")
                            )
                        })
                    })?;
                agents::input(agent, &task)
            }
            Some(_) => {
                return Err(ToolError::InvalidInput(
                    "expected `agent` to be a string".into(),
                ));
            }
        };
        let depth = self
            .depths
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&cx.session)
            .copied()
            .unwrap_or(0);
        if depth >= self.max_depth {
            return Err(ToolError::NotStarted(format!(
                "subagents may delegate at most {} levels deep",
                self.max_depth
            )));
        }
        let sessions = self
            .kernel
            .get(SESSION)
            .ok_or_else(|| ToolError::NotStarted("no session service is mounted".into()))?;
        let child = sessions
            .create()
            .await
            .map_err(|error| ToolError::NotStarted(error.to_string()))?
            .id;
        self.depths
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(child.clone(), depth + 1);
        let run = run_turn(
            &self.kernel,
            Some(child.clone()),
            task,
            cx.cancel.child_token(),
        );
        let result = tokio::time::timeout(self.timeout, run).await.map_err(|_| {
            ToolError::Execution(format!("the subagent in session {child} timed out"))
        })?;
        match result {
            Ok((session, answer)) => Ok(ToolOutput {
                content: vec![
                    ToolContent::Text(answer),
                    ToolContent::Json(json!({ "session": session.to_string() })),
                ],
                is_error: false,
            }),
            Err((_, message)) => Err(ToolError::Execution(format!(
                "the subagent in session {child} failed: {message}"
            ))),
        }
    }
}

impl SchedulerPlugin {
    /// An example configuration, mostly the defaults, for reference (the Web
    /// UI shows it in an empty configuration editor). It is never applied.
    pub const CONFIG_EXAMPLE: &str = r#"{
  "max_depth": 2,
  "max_concurrent": 2,
  "delegate_timeout_secs": 600,
  "agents_dir": "agents",
  "jobs": [
    {
      "input": "<prompt>",
      "delay_secs": 0,
      "every_secs": 3600
    }
  ]
}"#;
}

#[cfg(test)]
#[test]
fn the_config_example_is_a_valid_configuration() {
    let parsed: Result<Config, _> = serde_json::from_str(SchedulerPlugin::CONFIG_EXAMPLE);
    assert!(parsed.is_ok(), "{:?}", parsed.err());
}
