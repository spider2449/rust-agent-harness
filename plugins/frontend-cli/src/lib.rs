//! `frontend-cli`: a terminal REPL.
//!
//! The loop and session services are resolved per use, so other plugins can
//! be swapped while the REPL keeps running. When the REPL ends it emits
//! [`ExitRequested`](rah_api::host::ExitRequested).
//!
//! Configuration: `{ "session": "<id>", "prompt": "<text>" }`. With `session`
//! the REPL resumes that session; with `prompt` it runs one turn and exits.
//!
//! The plugin answers `tool.approval`: while a turn runs, a policy `Ask` is
//! shown on the terminal and approved only by an explicit `y` or `yes`. End
//! of input, any other answer, or a REPL that is no longer running denies it.
//! The answer `a` (or `always`) approves and also allows that tool for the
//! rest of the session without asking again (ADR 0056); a tool that declares
//! `execute` is not offered it.
//!
//! It also answers `host.interrupt` (Ctrl-C): a running turn is cancelled
//! and the REPL continues. Without a running turn the host exits.
//!
//! `/stop`, `/start` and `/restart` manage plugins through the distribution's
//! `admin` service, when one is mounted. `/changes` and `/undo` list and take
//! back the session's file changes through the `file-undo` service (ADR
//! 0060); the next turn's input starts with a note telling the model.

use std::{
    collections::BTreeSet,
    str::FromStr,
    sync::{Arc, Mutex, PoisonError},
};

use futures::StreamExt;
use rah_api::{
    CancellationToken, KernelHandle, Plugin, PluginError, PluginId, PluginManifest, PluginStatus,
    Scope,
    admin::ADMIN,
    agent_loop::{LOOP, TurnRequest},
    async_trait,
    hooks::{ApprovalRequest, ToolApproval},
    host::{ExitRequested, Interrupt},
    session::{SESSION, SessionLog},
    tools::describe_image,
    undo,
};
use rah_protocol::{AgentEvent, PermissionLevel, SessionEntry, SessionId, ToolContent, ToolOutput};
use serde::Deserialize;
use tokio::{
    io::{AsyncBufRead, AsyncBufReadExt, AsyncWrite, AsyncWriteExt, BufReader, Lines},
    sync::{mpsc, oneshot},
};

/// A pending `Ask` confirmation and where to send the answer.
pub type ApprovalPrompt = (ApprovalRequest, oneshot::Sender<bool>);

/// The cancellation token of the turn the REPL is running, if any, and the
/// tools the user allowed for the rest of a session (answer `a`).
#[derive(Clone, Default)]
pub struct RunningTurn {
    token: Arc<Mutex<Option<CancellationToken>>>,
    /// `(session, tool)`. It lives as long as the process: a session that is
    /// started again later asks again.
    allowed: Arc<Mutex<BTreeSet<(String, String)>>>,
}

impl RunningTurn {
    /// Cancels the running turn. Returns whether there was one.
    pub fn interrupt(&self) -> bool {
        match self.lock().as_ref() {
            Some(token) => {
                token.cancel();
                true
            }
            None => false,
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Option<CancellationToken>> {
        self.token.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn is_allowed(&self, session: &SessionId, tool: &str) -> bool {
        self.allowed
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .contains(&(session.to_string(), tool.to_owned()))
    }

    fn allow(&self, session: &SessionId, tool: &str) {
        self.allowed
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert((session.to_string(), tool.to_owned()));
    }

    /// Registers a new turn; the returned guard clears it when dropped.
    fn begin(&self) -> (CancellationToken, TurnGuard<'_>) {
        let token = CancellationToken::new();
        *self.lock() = Some(token.clone());
        (token, TurnGuard(self))
    }
}

struct TurnGuard<'a>(&'a RunningTurn);

impl Drop for TurnGuard<'_> {
    fn drop(&mut self) {
        *self.0.lock() = None;
    }
}

pub struct CliFrontendPlugin {
    manifest: PluginManifest,
}

impl CliFrontendPlugin {
    pub fn new(id: impl Into<PluginId>) -> Self {
        Self {
            manifest: PluginManifest::new(id, env!("CARGO_PKG_VERSION"))
                .optional(["loop", "session"]),
        }
    }
}

/// REPL options, also the plugin configuration.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReplOptions {
    /// Resume this session instead of creating one.
    pub session: Option<String>,
    /// Run this single prompt and exit.
    pub prompt: Option<String>,
}

#[async_trait]
impl Plugin for CliFrontendPlugin {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    fn check_config(&self, config: &serde_json::Value) -> Result<(), PluginError> {
        rah_api::config::parse::<ReplOptions>(config)?;
        Ok(())
    }

    async fn apply(&self, scope: &mut Scope) -> Result<(), PluginError> {
        let options: ReplOptions = rah_api::config::parse(scope.config())?;
        let kernel = scope.kernel();
        let (prompts, approvals) = mpsc::channel::<ApprovalPrompt>(1);
        scope.hook_bail::<ToolApproval, _, _>(0, move |request| {
            let prompts = prompts.clone();
            async move {
                let (reply, answer) = oneshot::channel();
                prompts.send((request, reply)).await.ok()?;
                Some(answer.await.unwrap_or(false))
            }
        });
        let running = RunningTurn::default();
        let interrupts = running.clone();
        scope.hook_bail::<Interrupt, _, _>(0, move |()| {
            let handled = interrupts.interrupt();
            async move { handled.then_some(()) }
        });
        scope.spawn(async move {
            let stdin = BufReader::new(tokio::io::stdin());
            let stdout = tokio::io::stdout();
            let repl = run_repl(&kernel, stdin, stdout, options, Some(approvals), running);
            let code = match repl.await {
                Ok(code) => code,
                Err(error) => {
                    eprintln!("rah: terminal I/O failed: {error}");
                    1
                }
            };
            kernel.emit(&ExitRequested { code });
        });
        Ok(())
    }
}

/// Slash-prefixed input that is a turn for the model, not a REPL command
/// (`model-mock` reads these).
const TURN_COMMANDS: [&str; 2] = ["/call", "/notool"];

/// REPL commands: usage and description, as `/help` lists them.
const COMMANDS: [(&str, &str); 17] = [
    ("/new", "start a new session"),
    ("/sessions", "list sessions"),
    ("/resume <id>", "switch to a session"),
    (
        "/fork <seq>",
        "fork the current session after record <seq> and switch to it",
    ),
    (
        "/rename [title]",
        "rename the current session (no title clears it)",
    ),
    ("/history", "show the current session log"),
    (
        "/changes",
        "list the file changes of this session that /undo can take back",
    ),
    (
        "/undo",
        "undo the newest file change of this session, if no file changed since",
    ),
    ("/plugins", "show plugin status"),
    ("/stop <plugin>", "stop a plugin, keeping its configuration"),
    ("/start <plugin>", "start a stopped plugin"),
    ("/restart <plugin>", "restart a plugin"),
    (
        "/call <tool> <json>",
        "run a turn that requests a tool (mock model)",
    ),
    ("/notool <text>", "run a turn without tools (mock model)"),
    ("/help", "show this help"),
    ("/quit", "exit"),
    ("/exit", "exit"),
];

fn help() -> String {
    let mut text =
        String::from("commands:\n  <text>                run a turn; Ctrl-C cancels it\n");
    for (usage, description) in COMMANDS {
        text.push_str(&format!("  {usage:<21} {description}\n"));
    }
    text
}

/// Commands starting with `prefix`, for hints on unknown input.
fn suggestions(prefix: &str) -> Vec<&'static str> {
    COMMANDS
        .iter()
        .map(|(usage, _)| *usage)
        .filter(|usage| usage.starts_with(prefix))
        .collect()
}

/// Runs the REPL until EOF or `/quit`. Returns the process exit code.
/// `approvals` delivers `Ask` confirmations, answered from `input` while a
/// turn runs. `running` holds the running turn, so it can be interrupted.
pub async fn run_repl<R, W>(
    kernel: &KernelHandle,
    input: R,
    mut out: W,
    options: ReplOptions,
    mut approvals: Option<mpsc::Receiver<ApprovalPrompt>>,
    running: RunningTurn,
) -> std::io::Result<i32>
where
    R: AsyncBufRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let Some(sessions) = kernel.get(SESSION) else {
        out.write_all(b"error: no session service is mounted\n")
            .await?;
        return Ok(1);
    };
    let mut current = match &options.session {
        Some(id) => match open(sessions.as_ref(), id).await {
            Ok(id) => id,
            Err(message) => {
                line(&mut out, &format!("error: {message}")).await?;
                return Ok(1);
            }
        },
        None => match sessions.create().await {
            Ok(info) => info.id,
            Err(error) => {
                line(&mut out, &format!("error: {error}")).await?;
                return Ok(1);
            }
        },
    };
    line(&mut out, &format!("session {current}")).await?;
    if options.prompt.is_none() {
        line(&mut out, "type /help for commands").await?;
    }

    let mut lines = input.lines();
    if let Some(prompt) = &options.prompt {
        let mut terminal = Terminal {
            out: &mut out,
            lines: &mut lines,
            approvals: &mut approvals,
            running: &running,
        };
        let completed = turn(kernel, &current, prompt, &mut terminal).await?;
        return Ok(if completed { 0 } else { 1 });
    }

    loop {
        out.write_all(b"> ").await?;
        out.flush().await?;
        let Some(text) = lines.next_line().await? else {
            break;
        };
        let text = text.trim();
        let (command, argument) = text.split_once(' ').unwrap_or((text, ""));
        match command {
            "" => {}
            "/quit" | "/exit" => break,
            "/" | "/help" => out.write_all(help().as_bytes()).await?,
            "/new" => match sessions.create().await {
                Ok(info) => {
                    current = info.id;
                    line(&mut out, &format!("session {current}")).await?;
                }
                Err(error) => line(&mut out, &format!("error: {error}")).await?,
            },
            "/sessions" => match sessions.list().await {
                Ok(list) => {
                    for info in list {
                        let marker = if info.id == current { "*" } else { " " };
                        let parent = info
                            .parent
                            .map(|p| format!(" (fork of {} at {})", p.session, p.at))
                            .unwrap_or_default();
                        let title = info
                            .title
                            .or(info.preview)
                            .map(|title| format!("  {title}"))
                            .unwrap_or_default();
                        line(&mut out, &format!("{marker} {}{parent}{title}", info.id)).await?;
                    }
                }
                Err(error) => line(&mut out, &format!("error: {error}")).await?,
            },
            "/resume" => match open(sessions.as_ref(), argument.trim()).await {
                Ok(id) => {
                    current = id;
                    line(&mut out, &format!("session {current}")).await?;
                }
                Err(message) => line(&mut out, &format!("error: {message}")).await?,
            },
            "/fork" => match argument.trim().parse::<u64>() {
                Ok(at) => match sessions.fork(&current, at).await {
                    Ok(info) => {
                        current = info.id;
                        line(&mut out, &format!("session {current} (forked at {at})")).await?;
                    }
                    Err(error) => line(&mut out, &format!("error: {error}")).await?,
                },
                Err(_) => line(&mut out, "usage: /fork <seq>").await?,
            },
            "/rename" => {
                let title = argument.trim();
                let title = (!title.is_empty()).then(|| title.to_owned());
                match sessions.rename(&current, title).await {
                    Ok(info) => {
                        let shown = info.title.as_deref().unwrap_or("(no title)");
                        line(&mut out, &format!("session {current}: {shown}")).await?;
                    }
                    Err(error) => line(&mut out, &format!("error: {error}")).await?,
                }
            }
            "/history" => match sessions.read(&current).await {
                Ok(records) => {
                    for record in records {
                        line(
                            &mut out,
                            &format!("{:>4} {}", record.seq, describe(&record.entry)),
                        )
                        .await?;
                    }
                }
                Err(error) => line(&mut out, &format!("error: {error}")).await?,
            },
            "/changes" => {
                let changes = undo::changes(kernel, &current);
                if changes.is_empty() {
                    line(&mut out, "no file changes to undo in this session").await?;
                }
                for change in changes {
                    line(
                        &mut out,
                        &format!("{} {}", change.tool, change.files.join(", ")),
                    )
                    .await?;
                }
            }
            "/undo" => {
                let message = match undo::undo_newest(kernel, &current).await {
                    None => "error: undo is not available (no `file-undo` service)".to_owned(),
                    Some(Ok(report)) => report.summary(),
                    Some(Err(error)) => format!("error: {error}"),
                };
                line(&mut out, &message).await?;
            }
            "/plugins" => {
                let stopped = kernel
                    .get(ADMIN)
                    .map(|admin| admin.stopped())
                    .unwrap_or_default();
                let mut plugins: Vec<(PluginId, String)> = kernel
                    .plugins()
                    .into_iter()
                    .map(|(id, status)| (id, status_text(&status)))
                    .chain(stopped.into_iter().map(|id| (id, "stopped".to_owned())))
                    .collect();
                plugins.sort();
                for (id, status) in plugins {
                    line(&mut out, &format!("{id}: {status}")).await?;
                }
            }
            "/stop" | "/start" | "/restart" => {
                let message = manage(kernel, command, argument.trim()).await;
                line(&mut out, &message).await?;
            }
            _ if command.starts_with('/') && !TURN_COMMANDS.contains(&command) => {
                let close = suggestions(command);
                let hint = if close.is_empty() {
                    "try /help".to_owned()
                } else {
                    format!("did you mean {}?", close.join(", "))
                };
                line(&mut out, &format!("unknown command {command}; {hint}")).await?;
            }
            _ => {
                let text = text.to_owned();
                let mut terminal = Terminal {
                    out: &mut out,
                    lines: &mut lines,
                    approvals: &mut approvals,
                    running: &running,
                };
                turn(kernel, &current, &text, &mut terminal).await?;
            }
        }
    }
    Ok(0)
}

/// Runs `/stop`, `/start` or `/restart` through the `admin` service.
async fn manage(kernel: &KernelHandle, command: &str, id: &str) -> String {
    if id.is_empty() || id.contains(' ') {
        return format!("usage: {command} <plugin>  (see /plugins)");
    }
    let Some(admin) = kernel.get(ADMIN) else {
        return "error: plugin management is not available (no `admin` service)".to_owned();
    };
    let id = PluginId::new(id);
    let result = match command {
        "/stop" => admin.stop(&id).await.map(|()| "stopped".to_owned()),
        "/start" => admin.start(&id).await.map(|status| status_text(&status)),
        _ => admin.restart(&id).await.map(|status| status_text(&status)),
    };
    match result {
        Ok(status) => format!("{id}: {status}"),
        Err(error) => format!("error: {error}"),
    }
}

async fn open(sessions: &dyn SessionLog, id: &str) -> Result<SessionId, String> {
    let id = SessionId::from_str(id).map_err(|_| format!("`{id}` is not a session id"))?;
    sessions
        .read(&id)
        .await
        .map_err(|error| error.to_string())?;
    Ok(id)
}

async fn line<W: AsyncWrite + Unpin>(out: &mut W, text: &str) -> std::io::Result<()> {
    out.write_all(text.as_bytes()).await?;
    out.write_all(b"\n").await?;
    out.flush().await
}

/// What a running turn needs from the terminal.
struct Terminal<'a, R, W> {
    out: &'a mut W,
    lines: &'a mut Lines<R>,
    approvals: &'a mut Option<mpsc::Receiver<ApprovalPrompt>>,
    running: &'a RunningTurn,
}

/// The next confirmation request; pending forever once there is none.
async fn next_prompt(approvals: &mut Option<mpsc::Receiver<ApprovalPrompt>>) -> ApprovalPrompt {
    if let Some(receiver) = approvals {
        if let Some(prompt) = receiver.recv().await {
            return prompt;
        }
        *approvals = None;
    }
    std::future::pending().await
}

/// Asks the user to confirm one tool call. Only `y` or `yes` approves, and
/// `a` or `always` approves and allows the tool for the rest of the session,
/// except for a tool that declares `execute`, which is asked every time.
/// Cancelling the turn denies it.
async fn confirm<R, W>(
    terminal: &mut Terminal<'_, R, W>,
    request: &ApprovalRequest,
    cancel: &CancellationToken,
) -> std::io::Result<bool>
where
    R: AsyncBufRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let call = &request.request.call;
    // A tool that declares `execute`, or that the policy treats as `execute`
    // (its reason says so: the request carries no level), asks every time.
    let may_remember = request.request.definition.permission != PermissionLevel::Execute
        && !request.reason.contains("`execute`");
    let question = format!(
        "[approve] {} {} ({}). Allow? [y/N{}] ",
        call.name,
        call.input.0,
        request.reason,
        if may_remember { "/a=always" } else { "" }
    );
    terminal.out.write_all(question.as_bytes()).await?;
    terminal.out.flush().await?;
    // `next_line` is cancellation safe: a partly typed answer is kept.
    let answer = tokio::select! {
        answer = terminal.lines.next_line() => answer?.unwrap_or_default(),
        () = cancel.cancelled() => String::new(),
    };
    let answer = answer.trim().to_ascii_lowercase();
    let always = may_remember && matches!(answer.as_str(), "a" | "always");
    let approved = always || matches!(answer.as_str(), "y" | "yes");
    if always {
        terminal
            .running
            .allow(&request.request.session, call.name.as_str());
        line(
            terminal.out,
            &format!("[`{}` is allowed for the rest of this session]", call.name),
        )
        .await?;
    } else if !approved {
        line(terminal.out, "[denied]").await?;
    }
    Ok(approved)
}

/// Runs one turn and renders its events. Returns whether it completed.
async fn turn<R, W>(
    kernel: &KernelHandle,
    session: &SessionId,
    input: &str,
    terminal: &mut Terminal<'_, R, W>,
) -> std::io::Result<bool>
where
    R: AsyncBufRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let Some(agent_loop) = kernel.get(LOOP) else {
        line(terminal.out, "error: no agent loop is mounted").await?;
        return Ok(false);
    };
    let (cancel, _running) = terminal.running.begin();
    let mut events = agent_loop.run_turn(TurnRequest {
        session: session.clone(),
        // Tells the model about files the user undid since its last turn.
        input: undo::with_notes(kernel, session, input.to_owned()),
        cancel: cancel.clone(),
    });
    loop {
        // Events first: the loop sends `ToolRequested` before it asks for
        // the approval, so a call is shown before its question.
        let event = tokio::select! {
            biased;
            event = events.next() => event,
            (request, reply) = next_prompt(terminal.approvals) => {
                let tool = request.request.call.name.as_str();
                let approved = terminal.running.is_allowed(&request.request.session, tool)
                    || confirm(terminal, &request, &cancel).await?;
                let _ = reply.send(approved);
                continue;
            }
        };
        let Some(event) = event else {
            break;
        };
        let out = &mut *terminal.out;
        match event {
            AgentEvent::ModelDelta { delta, .. } => {
                out.write_all(delta.as_bytes()).await?;
                out.flush().await?;
            }
            AgentEvent::ModelReasoningStarted { .. } => line(out, "[thinking]").await?,
            AgentEvent::ModelRetrying {
                attempt,
                retries,
                delay_ms,
                reason,
                ..
            } => {
                line(
                    out,
                    &format!(
                        "[model busy: {reason}; retry {attempt} of {retries} in {} s]",
                        delay_ms.div_ceil(1000)
                    ),
                )
                .await?;
            }
            AgentEvent::ToolRequested { tool_call, .. } => {
                line(
                    out,
                    &format!("[tool] {} {}", tool_call.name, tool_call.input.0),
                )
                .await?;
            }
            AgentEvent::ToolFinished { output, .. } => {
                let label = if output.is_error { "error" } else { "result" };
                line(out, &format!("[tool {label}] {}", render(&output))).await?;
            }
            AgentEvent::Completed { .. } => {
                line(out, "").await?;
                return Ok(true);
            }
            AgentEvent::Failed { message, .. } => {
                line(out, &format!("\n[error] {message}")).await?;
                return Ok(false);
            }
            AgentEvent::Cancelled { .. } => {
                line(out, "\n[cancelled]").await?;
                return Ok(false);
            }
            _ => {}
        }
    }
    line(
        terminal.out,
        "\n[error] the agent loop stopped before the turn ended",
    )
    .await?;
    Ok(false)
}

fn render(output: &ToolOutput) -> String {
    output
        .content
        .iter()
        .map(|content| match content {
            ToolContent::Text(text) => text.clone(),
            ToolContent::Json(value) => value.to_string(),
            ToolContent::Image(image) => describe_image(image),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn describe(entry: &SessionEntry) -> String {
    match entry {
        SessionEntry::TurnStarted { input, .. } => format!("user: {input}"),
        SessionEntry::AssistantMessage { text, .. } => format!("assistant: {text}"),
        SessionEntry::ToolCall {
            name, arguments, ..
        } => format!("tool call: {name} {arguments}"),
        SessionEntry::ToolResult { output, effect, .. } => {
            format!("tool result ({effect:?}): {}", render(output))
        }
        SessionEntry::TurnCompleted { .. } => "turn completed".to_owned(),
        SessionEntry::TurnFailed { message, .. } => format!("turn failed: {message}"),
    }
}

fn status_text(status: &PluginStatus) -> String {
    match status {
        PluginStatus::Pending { missing } => format!(
            "pending (missing {})",
            missing
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        PluginStatus::DependencyCycle { cycle } => format!(
            "dependency cycle ({})",
            cycle
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        PluginStatus::Failed { message } => format!("failed: {message}"),
        other => format!("{other:?}").to_lowercase(),
    }
}

impl CliFrontendPlugin {
    /// An example configuration, mostly the defaults, for reference (the Web
    /// UI shows it in an empty configuration editor). It is never applied.
    pub const CONFIG_EXAMPLE: &str = r#"{
  "session": "<session id>",
  "prompt": "<one prompt, then exit>"
}"#;
}

#[cfg(test)]
#[test]
fn the_config_example_is_a_valid_configuration() {
    let parsed: Result<ReplOptions, _> = serde_json::from_str(CliFrontendPlugin::CONFIG_EXAMPLE);
    assert!(parsed.is_ok(), "{:?}", parsed.err());
}

#[cfg(test)]
mod tests {
    use futures::stream;
    use rah_api::{
        Kernel, MountOptions,
        agent_loop::{AgentLoop, TurnStream},
        hooks::ToolRequestContext,
    };
    use rah_protocol::{
        AgentOutput, Message, MessageRole, PermissionLevel, ToolCall, ToolCallId, ToolDefinition,
        ToolInput, ToolName,
    };

    use super::*;

    fn call(tool: &str) -> ToolCall {
        ToolCall {
            id: ToolCallId::new(),
            name: ToolName::new(tool),
            input: ToolInput(serde_json::json!({"path": "a.txt"})),
        }
    }

    /// A turn that requests one tool call per entry of `answers` and ends once
    /// every question about them is answered, as a loop does when the tools
    /// wait for their approvals.
    struct ToolCalls {
        session: SessionId,
        tools: Vec<String>,
        answers: Mutex<Option<Vec<oneshot::Receiver<bool>>>>,
        /// What the terminal answered, in the order of the calls.
        seen: Arc<Mutex<Vec<bool>>>,
    }

    impl AgentLoop for ToolCalls {
        fn run_turn(&self, _request: TurnRequest) -> TurnStream {
            let session = self.session.clone();
            let answers = self.answers.lock().unwrap().take().unwrap();
            let requested: Vec<AgentEvent> = self
                .tools
                .iter()
                .map(|tool| AgentEvent::ToolRequested {
                    session_id: session.clone(),
                    tool_call: call(tool),
                })
                .collect();
            let seen = self.seen.clone();
            let completed = async move {
                for answer in answers {
                    let answer = answer.await.unwrap_or(false);
                    seen.lock().unwrap().push(answer);
                }
                AgentEvent::Completed {
                    session_id: session,
                    output: AgentOutput {
                        message: Message {
                            role: MessageRole::Assistant,
                            content: String::new(),
                        },
                    },
                }
            };
            stream::iter(requested)
                .chain(stream::once(completed))
                .boxed()
        }
    }

    struct LoopPlugin(PluginManifest, Arc<ToolCalls>);

    #[async_trait]
    impl Plugin for LoopPlugin {
        fn manifest(&self) -> &PluginManifest {
            &self.0
        }

        async fn apply(&self, scope: &mut Scope) -> Result<(), PluginError> {
            scope.provide(LOOP, self.1.clone())?;
            Ok(())
        }
    }

    fn approval(session: &SessionId, tool: &str, permission: PermissionLevel) -> ApprovalRequest {
        ApprovalRequest {
            request: ToolRequestContext {
                session: session.clone(),
                call: call(tool),
                definition: ToolDefinition {
                    name: ToolName::new(tool),
                    description: String::new(),
                    input_schema: serde_json::json!({}),
                    permission,
                },
            },
            reason: "needs `write`".into(),
        }
    }

    /// Runs one turn that requests every tool in `asked` with its approval
    /// question already waiting, and `typed` on standard input. Returns what
    /// the terminal showed and the answer each question got.
    async fn run_turn(
        running: &RunningTurn,
        session: &SessionId,
        asked: &[(&str, PermissionLevel)],
        typed: &str,
    ) -> (String, Vec<bool>) {
        let mut replies = Vec::new();
        let mut answers = Vec::new();
        for _ in asked {
            let (reply, answer) = oneshot::channel();
            replies.push(reply);
            answers.push(answer);
        }
        let seen = Arc::new(Mutex::new(Vec::new()));
        let agent_loop = Arc::new(ToolCalls {
            session: session.clone(),
            tools: asked.iter().map(|(tool, _)| (*tool).to_owned()).collect(),
            answers: Mutex::new(Some(answers)),
            seen: seen.clone(),
        });
        let kernel = Kernel::new();
        let plugin = LoopPlugin(
            PluginManifest::new("loop", "1").provides(["loop"]),
            agent_loop,
        );
        kernel
            .mount(Arc::new(plugin), MountOptions::default())
            .await
            .unwrap();
        let handle = kernel.handle();

        let (prompts, receiver) = mpsc::channel::<ApprovalPrompt>(asked.len().max(1));
        let mut approvals = Some(receiver);
        for ((tool, permission), reply) in asked.iter().zip(replies) {
            prompts
                .send((approval(session, tool, *permission), reply))
                .await
                .unwrap();
        }

        let mut out = Vec::new();
        let mut lines = BufReader::new(typed.as_bytes()).lines();
        let mut terminal = Terminal {
            out: &mut out,
            lines: &mut lines,
            approvals: &mut approvals,
            running,
        };
        assert!(turn(&handle, session, "go", &mut terminal).await.unwrap());
        let got = seen.lock().unwrap().clone();
        (String::from_utf8(out).unwrap(), got)
    }

    const WRITE: PermissionLevel = PermissionLevel::Write;
    const EXECUTE: PermissionLevel = PermissionLevel::Execute;

    #[tokio::test]
    async fn always_allows_a_tool_for_the_session_and_asks_about_nothing_else() {
        let running = RunningTurn::default();
        let session = SessionId::new();
        // One answer `a` is typed; the second call to the same tool is not
        // asked about, and the other tool still is.
        let (shown, answers) = run_turn(
            &running,
            &session,
            &[("repo.create-file", WRITE), ("repo.create-file", WRITE)],
            "a\n",
        )
        .await;
        assert_eq!(answers, [true, true], "{shown}");
        assert_eq!(shown.matches("[approve]").count(), 1, "{shown}");
        assert!(shown.contains("/a=always]"), "{shown}");
        assert!(
            shown.contains("allowed for the rest of this session"),
            "{shown}"
        );

        // A later turn of the same session is not asked either, another tool
        // and another session are.
        let (shown, answers) = run_turn(
            &running,
            &session,
            &[("repo.create-file", WRITE), ("repo.edit-files", WRITE)],
            "n\n",
        )
        .await;
        assert_eq!(answers, [true, false], "{shown}");
        assert_eq!(shown.matches("[approve]").count(), 1, "{shown}");
        let (shown, answers) = run_turn(
            &running,
            &SessionId::new(),
            &[("repo.create-file", WRITE)],
            "n\n",
        )
        .await;
        assert_eq!(answers, [false], "{shown}");
        assert_eq!(shown.matches("[approve]").count(), 1, "{shown}");
    }

    #[tokio::test]
    async fn an_execute_tool_is_never_offered_always() {
        let running = RunningTurn::default();
        let session = SessionId::new();
        let (shown, answers) = run_turn(
            &running,
            &session,
            &[("shell.exec", EXECUTE), ("shell.exec", EXECUTE)],
            "a\ny\n",
        )
        .await;
        // `a` is no answer there (denied), and nothing was remembered, so the
        // second call asks again and `y` approves it once.
        assert_eq!(answers, [false, true], "{shown}");
        assert_eq!(shown.matches("[approve]").count(), 2, "{shown}");
        assert!(!shown.contains("/a=always"), "{shown}");
        assert!(!running.is_allowed(&session, "shell.exec"));
    }

    #[tokio::test]
    async fn only_a_or_always_is_remembered_and_yes_is_once() {
        let running = RunningTurn::default();
        let session = SessionId::new();
        let (_, answers) = run_turn(
            &running,
            &session,
            &[("repo.create-file", WRITE), ("repo.create-file", WRITE)],
            "yes\nAlways\n",
        )
        .await;
        assert_eq!(answers, [true, true]);
        assert!(running.is_allowed(&session, "repo.create-file"));
        let running = RunningTurn::default();
        let (shown, answers) = run_turn(
            &running,
            &session,
            &[("repo.create-file", WRITE), ("repo.create-file", WRITE)],
            "y\ny\n",
        )
        .await;
        assert_eq!(answers, [true, true]);
        assert_eq!(shown.matches("[approve]").count(), 2, "{shown}");
        assert!(!running.is_allowed(&session, "repo.create-file"));
    }

    #[tokio::test]
    async fn a_tool_call_is_shown_before_its_approval_question() {
        // The question is already waiting when the turn starts, so both the
        // event and the prompt are ready at the first poll. A random choice
        // between them would show the question first in about half the runs.
        for _ in 0..40 {
            let (shown, _) = run_turn(
                &RunningTurn::default(),
                &SessionId::new(),
                &[("repo.create-file", PermissionLevel::Write)],
                "y\n",
            )
            .await;
            let tool = shown.find("[tool] ").expect(&shown);
            let question = shown.find("[approve] ").expect(&shown);
            assert!(tool < question, "{shown}");
        }
    }
}
