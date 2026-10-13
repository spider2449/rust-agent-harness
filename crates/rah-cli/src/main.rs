use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use clap::Parser;
use rah_api::{
    Kernel, PluginStatus,
    host::{ExitRequested, FrontendListening, Interrupt},
    tools::ToolInvoker,
};
use rah_cli::{Distribution, boot};
use serde_json::Value;

/// RAH: a plugin-kernel agent harness.
#[derive(Debug, Parser)]
#[command(name = "rah", version)]
struct Args {
    /// Built-in preset to run (ignored with --config): default (llama.cpp),
    /// dev (mock model), minimal (mock model, no policy), openai, anthropic.
    #[arg(long, default_value = "default")]
    preset: String,
    /// Model id (model-openai, model-anthropic, model-llamacpp).
    #[arg(long)]
    model: Option<String>,
    /// Distribution file listing the plugins to mount.
    #[arg(long)]
    config: Option<PathBuf>,
    /// Serve the Web UI (frontend-web) instead of the terminal REPL.
    #[arg(long)]
    web: bool,
    /// Web UI address, loopback only (frontend-web); port 0 picks one.
    #[arg(long)]
    bind: Option<String>,
    /// Run one prompt and exit (frontend-cli).
    #[arg(long)]
    prompt: Option<String>,
    /// Resume this session (frontend-cli).
    #[arg(long)]
    session: Option<String>,
    /// Session log directory (session-jsonl).
    #[arg(long)]
    session_dir: Option<PathBuf>,
    /// Workspace root for file tools and processes (tools-builtin,
    /// sandbox-process).
    #[arg(long)]
    root: Option<PathBuf>,
}

fn distribution(args: &Args) -> Result<Distribution, String> {
    let mut distribution = match &args.config {
        Some(path) => {
            let text = std::fs::read_to_string(path)
                .map_err(|error| format!("cannot read `{}`: {error}", path.display()))?;
            Distribution::parse(&text).map_err(|error| error.to_string())?
        }
        None => Distribution::preset(&args.preset).map_err(|error| error.to_string())?,
    };
    if args.web {
        if args.prompt.is_some() || args.session.is_some() {
            return Err("--web cannot be combined with --prompt or --session".into());
        }
        if !distribution.replace("frontend-cli", "frontend-web") {
            return Err("--web needs a `frontend-cli` plugin to replace".into());
        }
    }
    let overrides = [
        ("frontend-web", "bind", args.bind.clone().map(Value::String)),
        (
            "frontend-cli",
            "prompt",
            args.prompt.clone().map(Value::String),
        ),
        (
            "frontend-cli",
            "session",
            args.session.clone().map(Value::String),
        ),
        (
            "session-jsonl",
            "dir",
            args.session_dir
                .as_ref()
                .map(|p| Value::String(p.display().to_string())),
        ),
    ];
    for (kind, key, value) in overrides {
        if let Some(value) = value
            && !distribution.set_config(kind, key, value)
        {
            return Err(format!(
                "--{key} needs a `{kind}` plugin in the distribution"
            ));
        }
    }
    if let Some(root) = &args.root {
        let root = Value::String(root.display().to_string());
        let mut matched = false;
        for kind in rah_cli::WORKSPACE_PLUGINS {
            matched |= distribution.set_config(kind, "root", root.clone());
        }
        if !matched {
            return Err("--root needs a `tools-builtin` or `sandbox-process` plugin".into());
        }
    }
    if let Some(model) = &args.model {
        let mut matched = false;
        for kind in ["model-openai", "model-anthropic", "model-llamacpp"] {
            matched |= distribution.set_config(kind, "model", Value::String(model.clone()));
        }
        if !matched {
            return Err(
                "--model needs a `model-openai`, `model-anthropic` or `model-llamacpp` plugin"
                    .into(),
            );
        }
    }
    Ok(distribution)
}

async fn run(args: Args) -> i32 {
    let distribution = match distribution(&args) {
        Ok(distribution) => distribution,
        Err(message) => {
            eprintln!("rah: {message}");
            return 2;
        }
    };
    let kernel = Kernel::new();
    let handle = kernel.handle();
    let (exit_sender, exit_receiver) = tokio::sync::oneshot::channel::<i32>();
    let exit_sender = Arc::new(Mutex::new(Some(exit_sender)));
    let _exit = handle.subscribe(move |exit: &ExitRequested| {
        let sender = exit_sender
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        if let Some(sender) = sender {
            let _ = sender.send(exit.code);
        }
    });

    let _listening = handle.subscribe(|listening: &FrontendListening| {
        eprintln!(
            "rah: {} is listening at {}",
            listening.plugin, listening.url
        );
    });

    match boot(&kernel, &distribution).await {
        Ok(statuses) => {
            for (id, status) in statuses {
                if status != PluginStatus::Active {
                    eprintln!("rah: plugin `{id}` is not active: {status:?}");
                }
            }
        }
        Err(error) => {
            eprintln!("rah: {error}");
            let _ = kernel.shutdown().await;
            return 2;
        }
    }
    if !ToolInvoker::new(handle.clone()).is_guarded() {
        eprintln!(
            "rah: WARNING: UNGUARDED - no tool policy plugin is mounted; \
             model tool calls run without authorization."
        );
    }

    // Ctrl-C first lets a frontend cancel its running turn; it exits only
    // when nothing took the interrupt.
    let mut exit_receiver = exit_receiver;
    let code = loop {
        tokio::select! {
            code = &mut exit_receiver => break code.unwrap_or(1),
            _ = tokio::signal::ctrl_c() => {
                if !matches!(handle.bail::<Interrupt>(()).await, Ok(Some(()))) {
                    break 130;
                }
            }
        }
    };
    let _ = kernel.shutdown().await;
    code
}

#[tokio::main]
async fn main() {
    let code = run(Args::parse()).await;
    // Exit without dropping the runtime: a pending terminal read must not
    // keep the process alive after every plugin has been shut down.
    std::process::exit(code);
}
