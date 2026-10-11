//! `frontend-cli`: a terminal REPL.
//!
//! The loop and session services are resolved per use, so other plugins can
//! be swapped while the REPL keeps running. When the REPL ends it emits
//! [`ExitRequested`](rah_api::host::ExitRequested).
//!
//! Configuration: `{ "session": "<id>", "prompt": "<text>" }`. With `session`
//! the REPL resumes that session; with `prompt` it runs one turn and exits.

use std::str::FromStr;

use futures::StreamExt;
use rah_api::{
    CancellationToken, KernelHandle, Plugin, PluginError, PluginId, PluginManifest, PluginStatus,
    Scope,
    agent_loop::{LOOP, TurnRequest},
    async_trait,
    host::ExitRequested,
    session::{SESSION, SessionLog},
};
use rah_protocol::{AgentEvent, SessionEntry, SessionId, ToolContent, ToolOutput};
use serde::Deserialize;
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncWrite, AsyncWriteExt, BufReader};

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
        scope.spawn(async move {
            let stdin = BufReader::new(tokio::io::stdin());
            let code = match run_repl(&kernel, stdin, tokio::io::stdout(), options).await {
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

const HELP: &str = "\
commands:
  <text>            run a turn
  /new              start a new session
  /sessions         list sessions
  /resume <id>      switch to a session
  /fork <seq>       fork the current session after record <seq> and switch to it
  /history          show the current session log
  /plugins          show plugin status
  /help             show this help
  /quit             exit
";

/// Runs the REPL until EOF or `/quit`. Returns the process exit code.
pub async fn run_repl<R, W>(
    kernel: &KernelHandle,
    input: R,
    mut out: W,
    options: ReplOptions,
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

    if let Some(prompt) = &options.prompt {
        let completed = turn(kernel, &current, prompt, &mut out).await?;
        return Ok(if completed { 0 } else { 1 });
    }

    let mut lines = input.lines();
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
            "/help" => out.write_all(HELP.as_bytes()).await?,
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
                        line(&mut out, &format!("{marker} {}{parent}", info.id)).await?;
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
                for (id, status) in kernel.plugins() {
                    line(&mut out, &format!("{id}: {}", status_text(&status))).await?;
                }
            }
            _ if command.starts_with('/') && !command.starts_with("/notool") => {
                line(&mut out, &format!("unknown command {command}; try /help")).await?;
            }
            _ => {
                turn(kernel, &current, text, &mut out).await?;
            }
        }
    }
    Ok(0)
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

/// Runs one turn and renders its events. Returns whether it completed.
async fn turn<W: AsyncWrite + Unpin>(
    kernel: &KernelHandle,
    session: &SessionId,
    input: &str,
    out: &mut W,
) -> std::io::Result<bool> {
    let Some(agent_loop) = kernel.get(LOOP) else {
        line(out, "error: no agent loop is mounted").await?;
        return Ok(false);
    };
    let mut events = agent_loop.run_turn(TurnRequest {
        session: session.clone(),
        input: input.to_owned(),
        cancel: CancellationToken::new(),
    });
    while let Some(event) = events.next().await {
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
        out,
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
