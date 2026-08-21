use std::{env, io::Write, path::Path, process::ExitCode, sync::Arc};

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use futures::StreamExt;
use rah_model::{MockBackend, ModelEvent};
use rah_protocol::{
    AgentEvent, AgentInput, AgentOptions, AgentRequest, Message, MessageRole, PermissionLevel,
    RequestId, ToolCall, ToolCallId, ToolInput, ToolName,
};
use rah_runtime::{AgentRuntime, MinimalTestRuntime};
use rah_tools::{EchoTool, FsReadTool, ToolRegistry};
use serde_json::json;
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
#[command(name = "rah", version, about = "Rust Agent Harness")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Runs a prompt through the deterministic local configuration.
    Run {
        /// Prompt supplied to the deterministic backend.
        prompt: String,
    },
    /// Lists tools available to the deterministic runtime.
    Tools,
    /// Checks that the local deterministic configuration can be built.
    Doctor,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    match run_application().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error:#}");
            ExitCode::FAILURE
        }
    }
}

async fn run_application() -> Result<()> {
    tracing_subscriber::fmt()
        .with_ansi(false)
        .with_writer(std::io::stderr)
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("rah=warn")),
        )
        .try_init()
        .map_err(|error| anyhow::anyhow!("failed to initialize tracing: {error}"))?;
    execute(Cli::parse()).await
}

async fn execute(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Run { prompt } => run_prompt(prompt).await,
        Command::Tools => list_tools(),
        Command::Doctor => doctor(),
    }
}

async fn run_prompt(prompt: String) -> Result<()> {
    let read_workspace = prompt == "read Cargo.toml and report the workspace package information";
    let tool_name = if read_workspace { "fs.read" } else { "echo" };
    let tool_input = if read_workspace {
        json!({"path": "Cargo.toml"})
    } else {
        json!({"text": prompt.clone()})
    };
    let final_text = if read_workspace {
        format!(
            "workspace version {}, edition 2024",
            env!("CARGO_PKG_VERSION")
        )
    } else {
        prompt.clone()
    };
    let backend = Arc::new(MockBackend::new(vec![
        vec![
            Ok(ModelEvent::ToolCall {
                call: ToolCall {
                    id: ToolCallId::new(),
                    name: ToolName::new(tool_name),
                    input: ToolInput(tool_input),
                },
            }),
            Ok(ModelEvent::Completed),
        ],
        vec![
            Ok(ModelEvent::TextDelta { text: final_text }),
            Ok(ModelEvent::Completed),
        ],
    ]));
    let runtime = MinimalTestRuntime::new(backend, Arc::new(tool_registry(env::current_dir()?)?))
        .with_permission(PermissionLevel::Read);
    let handle = runtime
        .start(AgentRequest {
            request_id: RequestId::new(),
            input: AgentInput {
                messages: vec![Message {
                    role: MessageRole::User,
                    content: prompt,
                }],
            },
            options: AgentOptions::default(),
        })
        .await
        .context("failed to start deterministic runtime")?;
    let mut events = handle.into_events();
    let mut wrote_text = false;

    while let Some(event) = events.next().await {
        match event {
            AgentEvent::ModelDelta { delta, .. } => {
                print!("{delta}");
                std::io::stdout()
                    .flush()
                    .context("failed to flush model output")?;
                wrote_text = true;
            }
            AgentEvent::Failed { code, message, .. } => {
                bail!("agent failed ({code:?}): {message}");
            }
            AgentEvent::Cancelled { .. } => bail!("agent session was cancelled"),
            AgentEvent::Started { .. }
            | AgentEvent::ModelRequestStarted { .. }
            | AgentEvent::ToolRequested { .. }
            | AgentEvent::ToolStarted { .. }
            | AgentEvent::ToolFinished { .. }
            | AgentEvent::ApprovalRequired { .. }
            | AgentEvent::Completed { .. } => {}
        }
    }
    if wrote_text {
        println!();
    }
    Ok(())
}

fn list_tools() -> Result<()> {
    for definition in tool_registry(env::current_dir()?)?.definitions() {
        println!("{}", definition.name);
    }
    Ok(())
}

fn doctor() -> Result<()> {
    let registry = tool_registry(env::current_dir()?)?;
    if registry.definitions().is_empty() {
        bail!("deterministic runtime has no registered tools");
    }
    println!("RAH doctor: ok");
    Ok(())
}

fn tool_registry(workspace_root: impl AsRef<Path>) -> Result<ToolRegistry> {
    let mut registry = ToolRegistry::new();
    registry
        .register(Arc::new(EchoTool::new()))
        .context("failed to register echo tool")?;
    registry
        .register(Arc::new(
            FsReadTool::new(workspace_root, 1024 * 1024)
                .context("failed to configure fs.read workspace")?,
        ))
        .context("failed to register fs.read tool")?;
    Ok(registry)
}
