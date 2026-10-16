use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use clap::Parser;
use rah_api::{
    Kernel, PluginStatus,
    host::{ExitRequested, FrontendListening, Interrupt},
    tools::ToolInvoker,
};
use rah_cli::{Distribution, USER_CONFIG, boot, default_data_home, select_distribution};
use serde_json::Value;

/// RAH: a plugin-kernel agent harness.
#[derive(Debug, Parser)]
#[command(name = "rah", version)]
struct Args {
    /// Built-in preset to run (ignored with --config): default (llama.cpp),
    /// dev (mock model), minimal (mock model, no policy), openai, anthropic,
    /// deepseek (DEEPSEEK_API_KEY).
    /// Without --preset and --config, `rah.toml` in the data directory runs
    /// when it exists, otherwise `default`.
    #[arg(long)]
    preset: Option<String>,
    /// Model id (model-openai, model-anthropic, model-llamacpp,
    /// model-chat-completions).
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
    /// Mode to enter at start (frontend-web), such as `daily` or `coding`,
    /// instead of the last one used.
    #[arg(long)]
    mode: Option<String>,
    /// Run one prompt and exit (frontend-cli).
    #[arg(long)]
    prompt: Option<String>,
    /// Resume this session (frontend-cli).
    #[arg(long)]
    session: Option<String>,
    /// Data directory for sessions, storage, memory, skills and telemetry;
    /// relative data paths in the configuration resolve against it.
    /// Default: RAH_HOME, else `.rah` in the user's home directory.
    #[arg(long)]
    data_dir: Option<PathBuf>,
    /// Session log directory (session-jsonl), instead of `sessions` in the
    /// data directory.
    #[arg(long)]
    session_dir: Option<PathBuf>,
    /// Workspace root for file tools and processes (tools-builtin,
    /// tools-repo-write, sandbox-process, sandbox-os).
    #[arg(long)]
    root: Option<PathBuf>,
    /// Write the preset (--preset, else `default`) as a distribution file and
    /// exit: to --config when given, else `rah.toml` in the data directory.
    /// An existing file is never overwritten.
    #[arg(long, conflicts_with_all = ["model", "web", "bind", "mode", "prompt", "session", "session_dir", "root"])]
    init_config: bool,
}

/// `path` made absolute against the current directory, so a relative path
/// given on the command line means what it says where it was typed.
fn absolute(path: &Path) -> Result<PathBuf, String> {
    std::path::absolute(path).map_err(|error| format!("`{}`: {error}", path.display()))
}

/// `--data-dir`, else the default data home; `.rah` in the current directory
/// only when no home directory is known.
fn data_home(args: &Args) -> Result<PathBuf, String> {
    let home = args
        .data_dir
        .clone()
        .or_else(default_data_home)
        .unwrap_or_else(|| PathBuf::from(".rah"));
    absolute(&home)
}

/// Appended to a file written by `--init-config`: the opt-in plugins no
/// preset mounts, commented out.
const INIT_CONFIG_FOOTER: &str = r#"
# Opt-in: program execution (`shell.exec`), here limited to `git`.
# Uncomment it and add `"shell.exec" = "execute"` to the policy's `tools`.
# The `sandbox-process` above is process supervision, not an OS sandbox.
#
# [[plugin]]
# use = "tools-shell"
# config = { allow_programs = ["git"] }
"#;

/// `--init-config`: writes the preset's distribution file; returns its path.
fn init_config(args: &Args) -> Result<PathBuf, String> {
    let path = match &args.config {
        Some(path) => absolute(path)?,
        None => data_home(args)?.join(USER_CONFIG),
    };
    let preset = args.preset.as_deref().unwrap_or("default");
    let text = rah_cli::preset_text(preset).map_err(|error| error.to_string())?;
    let text = format!(
        "# RAH distribution, written by `rah --init-config` from the `{preset}` preset.\n\
         # Without --preset and --config, `rah` runs `rah.toml` in the data directory.\n\
         # Settings saved in the Web UI are kept in `storage` and replace a plugin's\n\
         # configuration from this file at the next start.\n{text}{INIT_CONFIG_FOOTER}"
    );
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("cannot create `{}`: {error}", parent.display()))?;
    }
    // `create_new` refuses an existing file without a check-then-write race.
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|error| match error.kind() {
            std::io::ErrorKind::AlreadyExists => format!(
                "`{}` already exists; remove or rename it first",
                path.display()
            ),
            _ => format!("cannot create `{}`: {error}", path.display()),
        })?;
    std::io::Write::write_all(&mut file, text.as_bytes())
        .map_err(|error| format!("cannot write `{}`: {error}", path.display()))?;
    Ok(path)
}

fn distribution(args: &Args) -> Result<Distribution, String> {
    let home = data_home(args)?;
    let (mut distribution, user_config) =
        select_distribution(args.config.as_deref(), args.preset.as_deref(), &home)?;
    if let Some(path) = user_config {
        eprintln!("rah: using `{}`", path.display());
    }
    let session_dir = args.session_dir.as_deref().map(absolute).transpose()?;
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
        ("frontend-web", "mode", args.mode.clone().map(Value::String)),
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
            session_dir.map(|p| Value::String(p.display().to_string())),
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
    if let Some(root) = &args.root
        && !distribution.set_root(root)
    {
        return Err("--root needs a `tools-builtin` or sandbox plugin".into());
    }
    if let Some(model) = &args.model {
        let mut matched = false;
        for kind in [
            "model-openai",
            "model-anthropic",
            "model-llamacpp",
            "model-chat-completions",
        ] {
            matched |= distribution.set_config(kind, "model", Value::String(model.clone()));
        }
        if !matched {
            return Err(
                "--model needs a `model-openai`, `model-anthropic`, `model-llamacpp` \
                 or `model-chat-completions` plugin"
                    .into(),
            );
        }
    }
    Ok(distribution)
}

async fn run(args: Args) -> i32 {
    if args.init_config {
        return match init_config(&args) {
            Ok(path) => {
                eprintln!("rah: wrote `{}`", path.display());
                0
            }
            Err(message) => {
                eprintln!("rah: {message}");
                2
            }
        };
    }
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
