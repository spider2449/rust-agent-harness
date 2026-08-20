use std::{io::Write, process::ExitCode, sync::Arc};

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use futures::StreamExt;
use rah_model::{MockBackend, ModelEvent};
use rah_protocol::{
    AgentEvent, AgentInput, AgentOptions, AgentRequest, Message, MessageRole, RequestId, ToolCall,
    ToolCallId, ToolInput, ToolName,
};
use rah_runtime::{AgentRuntime, MinimalTestRuntime};
use rah_tools::{EchoTool, ToolRegistry};
use serde_json::json;

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
    match execute(Cli::parse()).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error:#}");
            ExitCode::FAILURE
        }
    }
}

async fn execute(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Run { prompt } => run_prompt(prompt).await,
        Command::Tools => list_tools(),
        Command::Doctor => doctor(),
    }
}

async fn run_prompt(prompt: String) -> Result<()> {
    let backend = Arc::new(MockBackend::new(vec![
        vec![
            Ok(ModelEvent::ToolCall {
                call: ToolCall {
                    id: ToolCallId::new(),
                    name: ToolName::new("echo"),
                    input: ToolInput(json!({"text": prompt.clone()})),
                },
            }),
            Ok(ModelEvent::Completed),
        ],
        vec![
            Ok(ModelEvent::TextDelta {
                text: prompt.clone(),
            }),
            Ok(ModelEvent::Completed),
        ],
    ]));
    let runtime = MinimalTestRuntime::new(backend, Arc::new(tool_registry()?));
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
    for definition in tool_registry()?.definitions() {
        println!("{}", definition.name);
    }
    Ok(())
}

fn doctor() -> Result<()> {
    let registry = tool_registry()?;
    if registry.definitions().is_empty() {
        bail!("deterministic runtime has no registered tools");
    }
    println!("RAH doctor: ok");
    Ok(())
}

fn tool_registry() -> Result<ToolRegistry> {
    let mut registry = ToolRegistry::new();
    registry
        .register(Arc::new(EchoTool::new()))
        .context("failed to register echo tool")?;
    Ok(registry)
}
