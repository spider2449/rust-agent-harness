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
//!
//! It also answers `host.interrupt` (Ctrl-C): a running turn is cancelled
//! and the REPL continues. Without a running turn the host exits.
//!
//! `/stop`, `/start` and `/restart` manage plugins through the distribution's
//! `admin` service, when one is mounted.

use std::{
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
};
use rah_protocol::{AgentEvent, SessionEntry, SessionId, ToolContent, ToolOutput};
use serde::Deserialize;
use tokio::{
    io::{AsyncBufRead, AsyncBufReadExt, AsyncWrite, AsyncWriteExt, BufReader, Lines},
    sync::{mpsc, oneshot},
};

/// A pending `Ask` confirmation and where to send the answer.
pub type ApprovalPrompt = (ApprovalRequest, oneshot::Sender<bool>);

/// The cancellation token of the turn the REPL is running, if any.
#[derive(Clone, Default)]
pub struct RunningTurn(Arc<Mutex<Option<CancellationToken>>>);

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
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
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

    async fn apply(&self, scope: &mut Scope) -> Result<(), PluginError> {
        let options: ReplOptions = if scope.config().is_null() {
            ReplOptions::default()
        } else {
            scope.config_as()?
        };
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
const COMMANDS: [(&str, &str); 15] = [
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

/// Asks the user to confirm one tool call. Only `y` or `yes` approves.
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
    let question = format!(
        "[approve] {} {} ({}). Allow? [y/N] ",
        call.name, call.input.0, request.reason
    );
    terminal.out.write_all(question.as_bytes()).await?;
    terminal.out.flush().await?;
    // `next_line` is cancellation safe: a partly typed answer is kept.
    let answer = tokio::select! {
        answer = terminal.lines.next_line() => answer?.unwrap_or_default(),
        () = cancel.cancelled() => String::new(),
    };
    let approved = matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes");
    if !approved {
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
        input: input.to_owned(),
        cancel: cancel.clone(),
    });
    loop {
        let event = tokio::select! {
            event = events.next() => event,
            (request, reply) = next_prompt(terminal.approvals) => {
                let approved = confirm(terminal, &request, &cancel).await?;
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
