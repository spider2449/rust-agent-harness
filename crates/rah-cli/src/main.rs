use std::{
    env,
    io::Write,
    path::{Path, PathBuf},
    process::ExitCode,
    sync::Arc,
};

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use futures::StreamExt;
use rah_model::{MockBackend, ModelEvent};
use rah_protocol::{
    AgentEvent, AgentInput, AgentOptions, AgentRequest, Message, MessageRole, PermissionLevel,
    RequestId, ToolCall, ToolCallId, ToolInput, ToolName,
};
use rah_runtime::{AgentRuntime, MinimalTestRuntime};
use rah_tools::{EchoTool, FsReadTool, ToolRegistry, TrustedStaticProfile};
use rah_tools_mcp::compose_trusted_profile;
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
    /// Validates one explicitly selected trusted static capability profile.
    Profile {
        #[command(subcommand)]
        command: ProfileCommand,
    },
}

#[derive(Debug, Subcommand)]
enum ProfileCommand {
    /// Validates source/schema/static resources without launching providers.
    Validate {
        /// Absolute profile path selected by the trusted operator.
        profile_path: PathBuf,
    },
    /// Launches configured local MCP providers and prints redacted effective inventory.
    ValidateEffective {
        /// Absolute profile path selected by the trusted operator.
        profile_path: PathBuf,
    },
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
        Command::Profile { command } => match command {
            ProfileCommand::Validate { profile_path } => validate_profile(profile_path),
            ProfileCommand::ValidateEffective { profile_path } => {
                validate_effective_profile(profile_path).await
            }
        },
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

fn validate_profile(profile_path: PathBuf) -> Result<()> {
    let profile =
        TrustedStaticProfile::load(profile_path).context("trusted profile validation failed")?;
    let effective = profile.effective_profile();

    println!("profile_version={}", effective.profile_version);
    println!("profile_id={}", effective.profile_id);
    println!("source_class={}", effective.source_class);
    render_profile(effective);
    Ok(())
}

async fn validate_effective_profile(profile_path: PathBuf) -> Result<()> {
    let profile = TrustedStaticProfile::load(profile_path)
        .context("trusted profile static validation failed")?;
    let effective = compose_trusted_profile(profile)
        .await
        .context("trusted profile effective validation failed")?;
    render_profile(effective.effective_profile());
    Ok(())
}

fn render_profile(effective: &rah_tools::EffectiveProfile) {
    for capability in &effective.capabilities {
        println!(
            "capability name={} enabled={} registered={} permission={:?} resources={} validation={}",
            capability.capability_id,
            capability.enabled,
            capability.registered,
            capability.permission,
            capability.resources.join(","),
            capability.validation,
        );
    }
    for provider in &effective.providers {
        println!(
            "provider kind={} id={} status={} tool_count={}",
            provider.kind, provider.provider_id, provider.status, provider.tool_count,
        );
    }
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
