//! `telemetry-log`: diagnostic records of every turn, as JSONL.
//!
//! The plugin follows the [`TurnEvent`]s loops publish on the event bus
//! (ADR 0040), so it sees foreground, background and subagent turns alike,
//! and writes one JSON line per finished item:
//!
//! - `turn`: outcome (`completed`, `failed`, `cancelled`), error code and
//!   message, duration, model requests, tool calls and summed token usage;
//! - `model_request`: duration, time to the first text, whether the model
//!   reasoned, and the token usage the provider reported;
//! - `tool`: tool name, decision (`allowed`, `approved`, `denied`,
//!   `rejected`), duration and whether the output was an error.
//!
//! Message text and tool output are never recorded; tool arguments only with
//! `include_arguments`. Files are `<dir>/<YYYY-MM-DD>.jsonl` (UTC), continued
//! in `<YYYY-MM-DD>.<n>.jsonl` past `max_file_bytes`; files older than
//! `keep_days` are deleted. Writing happens in a task owned by the plugin's
//! scope; records still queued when the plugin stops are dropped, and a
//! failed write drops that record.
//!
//! The plugin also provides `ui-views` with a `usage` view (ADR 0062): the
//! sums of the records this process wrote, shown by the Web UI as a panel.
//!
//! Configuration (all optional): `{ "dir": ".rah/telemetry",
//! "max_file_bytes": 10485760, "keep_days": 14, "include_arguments": false }`.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, PoisonError},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use rah_api::{
    Plugin, PluginError, PluginId, PluginManifest, Scope,
    agent_loop::TurnEvent,
    async_trait,
    ui::{UI_VIEWS, UI_VIEWS_SERVICE, UiViews},
};
use rah_protocol::{AgentErrorCode, AgentEvent, ModelRequestId, ModelUsage, SessionId, ToolCallId};
use serde::Serialize;
use serde_json::Value;
use tokio::{io::AsyncWriteExt, sync::mpsc};

const DEFAULT_DIR: &str = ".rah/telemetry";
const DEFAULT_MAX_FILE_BYTES: u64 = 10 * 1024 * 1024;
const DEFAULT_KEEP_DAYS: u64 = 14;
/// Longest error message and argument text kept, in characters.
const MAX_TEXT_CHARS: usize = 500;

pub struct TelemetryPlugin {
    manifest: PluginManifest,
}

impl TelemetryPlugin {
    pub fn new(id: impl Into<PluginId>) -> Self {
        Self {
            manifest: PluginManifest::new(id, env!("CARGO_PKG_VERSION"))
                .provides([UI_VIEWS_SERVICE]),
        }
    }
}

mod usage;

#[derive(Clone, Debug, Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TelemetryConfig {
    pub dir: Option<PathBuf>,
    pub max_file_bytes: Option<u64>,
    pub keep_days: Option<u64>,
    pub include_arguments: Option<bool>,
}

#[async_trait]
impl Plugin for TelemetryPlugin {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    fn check_config(&self, config: &serde_json::Value) -> Result<(), PluginError> {
        rah_api::config::parse::<TelemetryConfig>(config)?;
        Ok(())
    }

    async fn apply(&self, scope: &mut Scope) -> Result<(), PluginError> {
        let config: TelemetryConfig = rah_api::config::parse(scope.config())?;
        let (records, mut queue) = mpsc::unbounded_channel::<Record>();
        let mut writer = Writer {
            dir: config.dir.unwrap_or_else(|| PathBuf::from(DEFAULT_DIR)),
            max_file_bytes: config
                .max_file_bytes
                .unwrap_or(DEFAULT_MAX_FILE_BYTES)
                .max(1),
            keep_days: config.keep_days.unwrap_or(DEFAULT_KEEP_DAYS).max(1),
            open: None,
        };
        let totals = Arc::new(Mutex::new(usage::Totals::new()));
        let views: Arc<dyn UiViews> = Arc::new(usage::UsageViews(totals.clone()));
        scope.provide(UI_VIEWS, views)?;
        scope.spawn(async move {
            while let Some(record) = queue.recv().await {
                totals
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .add(&record);
                writer.write(&record).await;
            }
        });
        let tracker = Mutex::new(Tracker::new(
            config.include_arguments.unwrap_or(false),
            records,
        ));
        scope.on::<TurnEvent>(move |event| {
            tracker
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .observe(&event.event, Instant::now());
        });
        Ok(())
    }
}

/// Token counts summed over model requests; a count stays `None` until a
/// provider reports it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Tokens {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cached_input_tokens: Option<u64>,
}

impl Tokens {
    fn add(&mut self, usage: &ModelUsage) {
        let sum = |total: &mut Option<u64>, count: Option<u64>| {
            if let Some(count) = count {
                *total = Some(total.unwrap_or(0).saturating_add(count));
            }
        };
        sum(&mut self.input_tokens, usage.input_tokens);
        sum(&mut self.output_tokens, usage.output_tokens);
        sum(&mut self.cached_input_tokens, usage.cached_input_tokens);
    }

    /// Adds `other`'s counts.
    fn sum(&mut self, other: &Tokens) {
        self.add(&ModelUsage {
            input_tokens: other.input_tokens,
            output_tokens: other.output_tokens,
            cached_input_tokens: other.cached_input_tokens,
        });
    }
}

/// One JSONL line.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Record {
    Turn {
        ts: String,
        session: SessionId,
        request: String,
        outcome: &'static str,
        #[serde(skip_serializing_if = "Option::is_none")]
        error_code: Option<AgentErrorCode>,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
        duration_ms: u64,
        model_requests: u32,
        tool_calls: u32,
        #[serde(flatten)]
        tokens: Tokens,
    },
    ModelRequest {
        ts: String,
        session: SessionId,
        request: String,
        model_request: ModelRequestId,
        duration_ms: u64,
        #[serde(skip_serializing_if = "Option::is_none")]
        first_text_ms: Option<u64>,
        reasoning: bool,
        #[serde(skip_serializing_if = "is_zero")]
        retries: u32,
        #[serde(flatten)]
        tokens: Tokens,
    },
    Tool {
        ts: String,
        session: SessionId,
        request: String,
        call: ToolCallId,
        tool: String,
        decision: &'static str,
        duration_ms: u64,
        is_error: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        arguments: Option<String>,
    },
}

impl Record {
    fn ts(&self) -> &str {
        match self {
            Self::Turn { ts, .. } | Self::ModelRequest { ts, .. } | Self::Tool { ts, .. } => ts,
        }
    }
}

struct ModelState {
    id: ModelRequestId,
    started: Instant,
    first_text: Option<Duration>,
    reasoning: bool,
    retries: u32,
    tokens: Tokens,
}

struct ToolState {
    name: String,
    arguments: Option<String>,
    requested: Instant,
    started: Option<Instant>,
    asked: bool,
}

struct TurnState {
    request: String,
    started: Instant,
    model_requests: u32,
    tool_calls: u32,
    tokens: Tokens,
    model: Option<ModelState>,
    tools: HashMap<ToolCallId, ToolState>,
}

/// Follows the turns in flight, one per session, and turns their events
/// into records.
pub struct Tracker {
    include_arguments: bool,
    turns: HashMap<SessionId, TurnState>,
    records: mpsc::UnboundedSender<Record>,
}

fn millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

fn is_zero(count: &u32) -> bool {
    *count == 0
}

fn clip(text: &str) -> String {
    match text.char_indices().nth(MAX_TEXT_CHARS) {
        Some((end, _)) => format!("{}…", &text[..end]),
        None => text.to_owned(),
    }
}

impl Tracker {
    pub fn new(include_arguments: bool, records: mpsc::UnboundedSender<Record>) -> Self {
        Self {
            include_arguments,
            turns: HashMap::new(),
            records,
        }
    }

    fn send(&self, record: Record) {
        let _ = self.records.send(record);
    }

    /// Ends the open model request of `session`'s turn, if any.
    fn close_model(&mut self, session: &SessionId, now: Instant) {
        let Some(turn) = self.turns.get_mut(session) else {
            return;
        };
        let Some(model) = turn.model.take() else {
            return;
        };
        let record = Record::ModelRequest {
            ts: timestamp(SystemTime::now()),
            session: session.clone(),
            request: turn.request.clone(),
            model_request: model.id,
            duration_ms: millis(now.saturating_duration_since(model.started)),
            first_text_ms: model.first_text.map(millis),
            reasoning: model.reasoning,
            retries: model.retries,
            tokens: model.tokens,
        };
        self.send(record);
    }

    pub fn observe(&mut self, event: &AgentEvent, now: Instant) {
        match event {
            AgentEvent::Started {
                session_id,
                request_id,
            } => {
                self.turns.insert(
                    session_id.clone(),
                    TurnState {
                        request: request_id.to_string(),
                        started: now,
                        model_requests: 0,
                        tool_calls: 0,
                        tokens: Tokens::default(),
                        model: None,
                        tools: HashMap::new(),
                    },
                );
            }
            AgentEvent::ModelRequestStarted {
                session_id,
                model_request_id,
            } => {
                self.close_model(session_id, now);
                if let Some(turn) = self.turns.get_mut(session_id) {
                    turn.model_requests += 1;
                    turn.model = Some(ModelState {
                        id: model_request_id.clone(),
                        started: now,
                        first_text: None,
                        reasoning: false,
                        retries: 0,
                        tokens: Tokens::default(),
                    });
                }
            }
            AgentEvent::ModelDelta { session_id, .. } => {
                if let Some(model) = self
                    .turns
                    .get_mut(session_id)
                    .and_then(|turn| turn.model.as_mut())
                    && model.first_text.is_none()
                {
                    model.first_text = Some(now.saturating_duration_since(model.started));
                }
            }
            AgentEvent::ModelRetrying {
                session_id,
                model_request_id,
                ..
            } => {
                if let Some(model) = self
                    .turns
                    .get_mut(session_id)
                    .and_then(|turn| turn.model.as_mut())
                    .filter(|model| model.id == *model_request_id)
                {
                    model.retries += 1;
                }
            }
            AgentEvent::ModelReasoningStarted { session_id, .. } => {
                if let Some(model) = self
                    .turns
                    .get_mut(session_id)
                    .and_then(|turn| turn.model.as_mut())
                {
                    model.reasoning = true;
                }
            }
            AgentEvent::ModelUsage {
                session_id,
                model_request_id,
                usage,
            } => {
                if let Some(turn) = self.turns.get_mut(session_id) {
                    turn.tokens.add(usage);
                    if let Some(model) = turn
                        .model
                        .as_mut()
                        .filter(|model| model.id == *model_request_id)
                    {
                        model.tokens.add(usage);
                    }
                }
            }
            AgentEvent::ToolRequested {
                session_id,
                tool_call,
            } => {
                self.close_model(session_id, now);
                let arguments = self
                    .include_arguments
                    .then(|| clip(&tool_call.input.0.to_string()));
                if let Some(turn) = self.turns.get_mut(session_id) {
                    turn.tool_calls += 1;
                    turn.tools.insert(
                        tool_call.id.clone(),
                        ToolState {
                            name: tool_call.name.to_string(),
                            arguments,
                            requested: now,
                            started: None,
                            asked: false,
                        },
                    );
                }
            }
            AgentEvent::ApprovalRequired {
                session_id,
                request,
            } => {
                if let Some(tool) = self
                    .turns
                    .get_mut(session_id)
                    .and_then(|turn| turn.tools.get_mut(&request.tool_call.id))
                {
                    tool.asked = true;
                }
            }
            AgentEvent::ToolStarted {
                session_id,
                tool_call_id,
            } => {
                if let Some(tool) = self
                    .turns
                    .get_mut(session_id)
                    .and_then(|turn| turn.tools.get_mut(tool_call_id))
                {
                    tool.started = Some(now);
                }
            }
            AgentEvent::ToolFinished {
                session_id,
                tool_call_id,
                output,
            } => {
                let Some(turn) = self.turns.get_mut(session_id) else {
                    return;
                };
                let Some(tool) = turn.tools.remove(tool_call_id) else {
                    return;
                };
                let decision = match (tool.started.is_some(), tool.asked) {
                    (true, false) => "allowed",
                    (true, true) => "approved",
                    (false, false) => "denied",
                    (false, true) => "rejected",
                };
                let record = Record::Tool {
                    ts: timestamp(SystemTime::now()),
                    session: session_id.clone(),
                    request: turn.request.clone(),
                    call: tool_call_id.clone(),
                    tool: tool.name,
                    decision,
                    duration_ms: millis(
                        now.saturating_duration_since(tool.started.unwrap_or(tool.requested)),
                    ),
                    is_error: output.is_error,
                    arguments: tool.arguments,
                };
                self.send(record);
            }
            AgentEvent::Completed { session_id, .. } => {
                self.finish(session_id, now, "completed", None);
            }
            AgentEvent::Failed {
                session_id,
                code,
                message,
            } => {
                self.finish(session_id, now, "failed", Some((*code, clip(message))));
            }
            AgentEvent::Cancelled { session_id } => {
                self.finish(session_id, now, "cancelled", None);
            }
        }
    }

    fn finish(
        &mut self,
        session: &SessionId,
        now: Instant,
        outcome: &'static str,
        error: Option<(AgentErrorCode, String)>,
    ) {
        self.close_model(session, now);
        let Some(turn) = self.turns.remove(session) else {
            return;
        };
        let (error_code, error) =
            error.map_or((None, None), |(code, text)| (Some(code), Some(text)));
        self.send(Record::Turn {
            ts: timestamp(SystemTime::now()),
            session: session.clone(),
            request: turn.request,
            outcome,
            error_code,
            error,
            duration_ms: millis(now.saturating_duration_since(turn.started)),
            model_requests: turn.model_requests,
            tool_calls: turn.tool_calls,
            tokens: turn.tokens,
        });
    }
}

/// Days since 1970-01-01 to a civil date (proleptic Gregorian).
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

/// RFC 3339 UTC with milliseconds.
pub fn timestamp(time: SystemTime) -> String {
    let since = time.duration_since(UNIX_EPOCH).unwrap_or_default();
    let secs = since.as_secs() as i64;
    let (year, month, day) = civil_from_days(secs.div_euclid(86_400));
    let rest = secs.rem_euclid(86_400);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{:03}Z",
        rest / 3600,
        rest % 3600 / 60,
        rest % 60,
        since.subsec_millis()
    )
}

/// The UTC day of a record timestamp, `YYYY-MM-DD`.
fn day_of(ts: &str) -> &str {
    ts.get(..10).unwrap_or("unknown")
}

/// Days since 1970-01-01 of a `YYYY-MM-DD` file name prefix.
fn days_of(name: &str) -> Option<i64> {
    let date = name.get(..10)?;
    let mut parts = date.split('-');
    let year: i64 = parts.next()?.parse().ok()?;
    let month: i64 = parts.next()?.parse().ok()?;
    let day: i64 = parts.next()?.parse().ok()?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let yoe = year.rem_euclid(400);
    let mp = if month > 2 { month - 3 } else { month + 9 };
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(era * 146_097 + doe - 719_468)
}

struct OpenFile {
    day: String,
    index: u32,
    file: tokio::fs::File,
    size: u64,
}

struct Writer {
    dir: PathBuf,
    max_file_bytes: u64,
    keep_days: u64,
    open: Option<OpenFile>,
}

fn file_name(day: &str, index: u32) -> String {
    if index == 0 {
        format!("{day}.jsonl")
    } else {
        format!("{day}.{index}.jsonl")
    }
}

impl Writer {
    async fn write(&mut self, record: &Record) {
        let Ok(mut line) = serde_json::to_vec(record) else {
            return;
        };
        line.push(b'\n');
        let day = day_of(record.ts()).to_owned();
        let len = line.len() as u64;
        let fits = self
            .open
            .as_ref()
            .is_some_and(|open| open.day == day && open.size + len <= self.max_file_bytes);
        if !fits {
            let index = match &self.open {
                Some(open) if open.day == day => open.index + 1,
                _ => {
                    self.prune(&day).await;
                    0
                }
            };
            self.open = self.open_file(&day, index, len).await;
        }
        let Some(open) = self.open.as_mut() else {
            return;
        };
        if open.file.write_all(&line).await.is_ok() && open.file.flush().await.is_ok() {
            open.size += len;
        } else {
            self.open = None;
        }
    }

    /// Opens the first file of `day` from `index` on that has room for
    /// `len` more bytes (or is empty).
    async fn open_file(&self, day: &str, mut index: u32, len: u64) -> Option<OpenFile> {
        tokio::fs::create_dir_all(&self.dir).await.ok()?;
        loop {
            let path = self.dir.join(file_name(day, index));
            let size = tokio::fs::metadata(&path).await.map_or(0, |m| m.len());
            if size == 0 || size + len <= self.max_file_bytes {
                let file = tokio::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&path)
                    .await
                    .ok()?;
                return Some(OpenFile {
                    day: day.to_owned(),
                    index,
                    file,
                    size,
                });
            }
            index = index.checked_add(1)?;
        }
    }

    /// Deletes this plugin's files from days more than `keep_days` before
    /// `today`.
    async fn prune(&self, today: &str) {
        let Some(today) = days_of(today) else {
            return;
        };
        let Ok(mut entries) = tokio::fs::read_dir(&self.dir).await else {
            return;
        };
        while let Ok(Some(entry)) = entries.next_entry().await {
            let name = entry.file_name().to_string_lossy().into_owned();
            if is_record_file(&name)
                && days_of(&name).is_some_and(|day| today - day >= self.keep_days as i64)
            {
                let _ = tokio::fs::remove_file(entry.path()).await;
            }
        }
    }
}

/// `YYYY-MM-DD.jsonl` or `YYYY-MM-DD.<n>.jsonl`.
fn is_record_file(name: &str) -> bool {
    let Some(rest) = name.strip_suffix(".jsonl") else {
        return false;
    };
    let (date, index) = match rest.split_once('.') {
        Some((date, index)) => (date, Some(index)),
        None => (rest, None),
    };
    date.len() == 10
        && days_of(date).is_some()
        && index.is_none_or(|index| !index.is_empty() && index.bytes().all(|b| b.is_ascii_digit()))
}

/// Reads the records of a telemetry file, for tests and tools.
pub fn read_records(path: &Path) -> Vec<Value> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect()
}

impl TelemetryPlugin {
    /// An example configuration, mostly the defaults, for reference (the Web
    /// UI shows it in an empty configuration editor). It is never applied.
    pub const CONFIG_EXAMPLE: &str = r#"{
  "dir": "telemetry",
  "max_file_bytes": 10485760,
  "keep_days": 14,
  "include_arguments": false
}"#;
}

#[cfg(test)]
mod tests;

#[cfg(test)]
#[test]
fn the_config_example_is_a_valid_configuration() {
    let parsed: Result<TelemetryConfig, _> = serde_json::from_str(TelemetryPlugin::CONFIG_EXAMPLE);
    assert!(parsed.is_ok(), "{:?}", parsed.err());
}
