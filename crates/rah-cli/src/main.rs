use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use clap::Parser;
use rah_api::{Kernel, PluginStatus, host::ExitRequested, tools::ToolInvoker};
use rah_cli::{Distribution, boot};
use serde_json::Value;

/// RAH: a plugin-kernel agent harness.
#[derive(Debug, Parser)]
#[command(name = "rah", version)]
struct Args {
    /// Built-in preset to run (ignored with --config): default (llama.cpp),
    /// dev (mock model), minimal (mock model, no policy), openai.
    #[arg(long, default_value = "default")]
    preset: String,
    /// Model id (model-openai, model-llamacpp).
    #[arg(long)]
    model: Option<String>,
    /// Distribution file listing the plugins to mount.
    #[arg(long)]
    config: Option<PathBuf>,
    /// Run one prompt and exit (frontend-cli).
    #[arg(long)]
    prompt: Option<String>,
    /// Resume this session (frontend-cli).
    #[arg(long)]
    session: Option<String>,
    /// Session log directory (session-jsonl).
    #[arg(long)]
    session_dir: Option<PathBuf>,
    /// Workspace root for file tools (tools-builtin).
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
    let overrides = [
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
        (
            "tools-builtin",
            "root",
            args.root
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
    if let Some(model) = &args.model {
        let openai = distribution.set_config("model-openai", "model", Value::String(model.clone()));
        let llama =
            distribution.set_config("model-llamacpp", "model", Value::String(model.clone()));
        if !openai && !llama {
            return Err("--model needs a `model-openai` or `model-llamacpp` plugin".into());
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

    let code = tokio::select! {
        code = exit_receiver => code.unwrap_or(1),
        _ = tokio::signal::ctrl_c() => 130,
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
