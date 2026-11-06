use std::{
    io::IsTerminal,
    path::{Path, PathBuf},
    process::Command as Process,
    sync::{Arc, Mutex},
    time::Duration,
};

use clap::{Parser, Subcommand};
use rah_api::{
    Kernel, PluginStatus,
    host::{ExitRequested, FrontendListening, HostNotice, Interrupt},
    tools::ToolInvoker,
};
use rah_cli::{
    Distribution, Permissions, boot,
    cli::{GlobalArgs, NO_MODEL, absolute},
    is_model_kind, setup, user_file_text,
};
use serde_json::Value;

/// RAH: a plugin-kernel agent harness.
///
/// Without a command, `rah` opens the terminal REPL. What runs comes from
/// `rah.toml` in the data directory (`rah config show` prints it); the
/// options below override it for one run.
#[derive(Debug, Parser)]
#[command(
    name = "rah",
    version,
    after_help = "Examples:\n  \
        rah config init --model anthropic/claude-sonnet-5-5\n  \
        rah\n  \
        rah run \"summarize README.md\" --permissions read-only\n  \
        rah web --with web-search\n  \
        rah-desktop          the Web UI in a native window (same global options)"
)]
struct Cli {
    #[command(flatten)]
    global: GlobalArgs,
    /// Resume this session in the REPL.
    #[arg(long, value_name = "ID")]
    session: Option<String>,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Run one prompt and exit.
    Run {
        prompt: String,
        /// Continue this session instead of starting one.
        #[arg(long, value_name = "ID")]
        session: Option<String>,
        /// Write a JSON summary of the run to this file, even when the turn
        /// fails: how it ended, wall time, model requests, summed token usage
        /// and the turn's tool calls.
        #[arg(long, value_name = "FILE")]
        summary: Option<PathBuf>,
    },
    /// Serve the Web UI in the browser.
    Web {
        /// Address to listen on, loopback only; port 0 picks one.
        #[arg(long)]
        bind: Option<String>,
        /// Mode to enter, such as `daily` or `coding`, instead of the last one.
        #[arg(long)]
        mode: Option<String>,
    },
    /// List or export sessions.
    Sessions {
        #[command(subcommand)]
        command: SessionsCommand,
    },
    /// Show, check, write or edit the configuration file.
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
    /// List the opt-in features (`--with` or `features = [...]`).
    Features,
}

#[derive(Debug, Subcommand)]
enum SessionsCommand {
    /// List the sessions, the most recent first.
    List,
    /// Write a session to standard output.
    Export {
        id: String,
        /// `md`, `json` or `trace` (one tagged line per record, for tools
        /// that read agent runs).
        #[arg(long, default_value = "md")]
        format: String,
    },
}

#[derive(Debug, Subcommand)]
enum ConfigCommand {
    /// Print what would run and where each value comes from.
    Show {
        /// Also list every plugin with its configuration.
        #[arg(long)]
        plugins: bool,
    },
    /// Check every plugin's configuration as a start would; exits with 2 on
    /// a problem. Starts no plugin or program.
    Check,
    /// Write a configuration file from --model, --permissions, --root and
    /// --with. An existing file is never overwritten.
    Init,
    /// Open the configuration file in $VISUAL, $EDITOR or the system editor.
    Edit,
    /// Move settings an earlier version of the Web UI kept in the data
    /// directory into the configuration file, and delete them there.
    Migrate,
}

fn fail(message: impl std::fmt::Display) -> i32 {
    eprintln!("rah: {message}");
    2
}

/// Sets `key` of every plugin of `kind`, or explains which plugin `option`
/// needs.
fn set(
    distribution: &mut Distribution,
    option: &str,
    kind: &str,
    key: &str,
    value: Value,
) -> Result<(), String> {
    if distribution.set_config(kind, key, value) {
        Ok(())
    } else {
        Err(format!(
            "{option} needs a `{kind}` plugin in the configuration"
        ))
    }
}

/// Points the frontend at what `command` asks for. Returns whether the run
/// only reads sessions, so no model is needed and nothing is guarded.
fn frontend(
    distribution: &mut Distribution,
    session: Option<&str>,
    command: Option<&Command>,
) -> Result<bool, String> {
    let text = |value: &str| Value::String(value.to_owned());
    match command {
        None => {
            if let Some(session) = session {
                set(
                    distribution,
                    "--session",
                    "frontend-cli",
                    "session",
                    text(session),
                )?;
            }
            Ok(false)
        }
        Some(Command::Run {
            prompt,
            session,
            summary,
        }) => {
            set(distribution, "run", "frontend-cli", "prompt", text(prompt))?;
            if let Some(session) = session {
                set(
                    distribution,
                    "--session",
                    "frontend-cli",
                    "session",
                    text(session),
                )?;
            }
            if let Some(summary) = summary {
                let path = absolute(summary)?.display().to_string();
                set(
                    distribution,
                    "--summary",
                    "frontend-cli",
                    "summary",
                    text(&path),
                )?;
            }
            Ok(false)
        }
        Some(Command::Web { bind, mode }) => {
            if !distribution.use_web_frontend() {
                return Err("web needs a `frontend-cli` or `frontend-web` plugin".into());
            }
            if let Some(bind) = bind {
                set(distribution, "--bind", "frontend-web", "bind", text(bind))?;
            }
            if let Some(mode) = mode {
                set(distribution, "--mode", "frontend-web", "mode", text(mode))?;
            }
            Ok(false)
        }
        Some(Command::Sessions { command }) => {
            match command {
                SessionsCommand::List => {
                    set(
                        distribution,
                        "sessions list",
                        "frontend-cli",
                        "list",
                        Value::Bool(true),
                    )?;
                }
                SessionsCommand::Export { id, format } => {
                    set(
                        distribution,
                        "sessions export",
                        "frontend-cli",
                        "session",
                        text(id),
                    )?;
                    set(
                        distribution,
                        "sessions export",
                        "frontend-cli",
                        "export",
                        text(format),
                    )?;
                }
            }
            // Reading sessions needs no model, so no API key either.
            distribution
                .plugin
                .retain(|spec| !is_model_kind(&spec.kind));
            Ok(true)
        }
        Some(Command::Config { .. } | Command::Features) => Ok(false),
    }
}

/// Whether a person can answer questions: stdin and stdout are a terminal.
fn on_terminal() -> bool {
    std::io::stdin().is_terminal() && std::io::stdout().is_terminal()
}

/// `rah config init`: writes a file with user keys only. With `--model` it
/// asks nothing; without, it asks on a terminal and refuses elsewhere.
fn init(global: &GlobalArgs) -> Result<PathBuf, String> {
    let permissions = global
        .permissions
        .as_deref()
        .map(str::parse::<Permissions>)
        .transpose()
        .map_err(|error| format!("--permissions: {error}"))?;
    let root = global
        .root
        .as_deref()
        .map(absolute)
        .transpose()?
        .map(|root| root.display().to_string());
    let path = global.config_path()?;
    if path.exists() {
        return Err(already_exists(&path));
    }
    let text = match &global.model {
        Some(model) => user_file_text(
            model,
            &[],
            permissions.unwrap_or(Permissions::Ask),
            root.as_deref(),
            &global.with,
        )?,
        None if on_terminal() => {
            let is_set = |name: &str| std::env::var_os(name).is_some_and(|value| !value.is_empty());
            let environment = setup::Environment {
                is_set: &is_set,
                llama_cpp_answers: setup::answers(
                    setup::LLAMA_CPP_ADDRESS,
                    Duration::from_millis(300),
                ),
                permissions,
                root,
                features: &global.with,
            };
            let mut stdout = std::io::stdout();
            setup::interview(&mut std::io::stdin().lock(), &mut stdout, &environment)
                .map_err(|error| error.to_string())?
        }
        None => {
            return Err("config init needs --model when it cannot ask, such as \
                 `rah config init --model anthropic/claude-sonnet-5-5`"
                .into());
        }
    };
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
            std::io::ErrorKind::AlreadyExists => already_exists(&path),
            _ => format!("cannot create `{}`: {error}", path.display()),
        })?;
    std::io::Write::write_all(&mut file, text.as_bytes())
        .map_err(|error| format!("cannot write `{}`: {error}", path.display()))?;
    Ok(path)
}

fn already_exists(path: &Path) -> String {
    format!(
        "`{}` already exists; edit it with `rah config edit`, or remove it first",
        path.display()
    )
}

/// Runs `init` off the async workers: it may wait for a person to answer.
async fn init_blocking(global: &GlobalArgs) -> Result<PathBuf, String> {
    let global = global.clone();
    tokio::task::spawn_blocking(move || init(&global))
        .await
        .map_err(|error| format!("config init failed: {error}"))?
}

/// `rah config edit`: runs the editor on the configuration file and returns
/// its exit code.
fn edit(global: &GlobalArgs) -> Result<i32, String> {
    let path = global.config_path()?;
    if !path.is_file() {
        return Err(format!(
            "`{}` does not exist; write one with `rah config init --model <provider>/<id>`",
            path.display()
        ));
    }
    let editor = ["VISUAL", "EDITOR"]
        .into_iter()
        .filter_map(|name| std::env::var(name).ok())
        .find(|value| !value.trim().is_empty())
        .unwrap_or_else(|| if cfg!(windows) { "notepad" } else { "vi" }.to_owned());
    // A fixed program and an argument vector, never a shell.
    let mut words = editor.split_whitespace();
    let program = words.next().ok_or("the editor command is empty")?;
    let status = Process::new(program)
        .args(words)
        .arg(&path)
        .status()
        .map_err(|error| format!("cannot start the editor `{program}`: {error}"))?;
    Ok(status.code().unwrap_or(1))
}

/// `rah config migrate`: applies each stored setting to the configuration
/// file and deletes the stored file once all of its changes are kept.
/// Returns how many stored files were moved.
fn migrate(global: &GlobalArgs) -> Result<usize, String> {
    let path = global.config_path()?;
    if !path.is_file() {
        return Err(format!(
            "`{}` does not exist; write one with `rah config init` first",
            path.display()
        ));
    }
    let home = global.home()?;
    let dir = home.join("storage").join("frontend-web");
    let found = rah_cli::persist::legacy_changes(&dir)?;
    if found.is_empty() {
        return Ok(0);
    }
    let mut moved = 0;
    let mut problems = Vec::new();
    for (stored, changes) in found {
        let mut kept = true;
        for change in &changes {
            if let Err(problem) = rah_cli::persist::persist(&path, change, Some(&home)) {
                problems.push(format!("{}: {problem}", stored.display()));
                kept = false;
            }
        }
        if kept {
            std::fs::remove_file(&stored)
                .map_err(|error| format!("cannot delete `{}`: {error}", stored.display()))?;
            moved += 1;
        }
    }
    if problems.is_empty() {
        Ok(moved)
    } else {
        Err(format!(
            "moved {moved} stored settings; these were kept where they are:\n  {}",
            problems.join("\n  ")
        ))
    }
}

/// What clap cannot say: `--session` before a command belongs to the REPL.
fn validate(cli: &Cli) -> Result<(), String> {
    if cli.session.is_some() && cli.command.is_some() {
        return Err(
            "`--session` before a command resumes the REPL; use `rah run --session <id> ...` \
             or `rah sessions export <id>`"
                .into(),
        );
    }
    Ok(())
}

async fn run(cli: Cli) -> i32 {
    if let Err(message) = validate(&cli) {
        return fail(message);
    }
    let global = &cli.global;
    match &cli.command {
        Some(Command::Features) => {
            print!("{}", rah_cli::list_features());
            return 0;
        }
        Some(Command::Config { command }) => {
            return match command {
                ConfigCommand::Init => match init_blocking(global).await {
                    Ok(path) => {
                        eprintln!("rah: wrote `{}`", path.display());
                        0
                    }
                    Err(message) => fail(message),
                },
                ConfigCommand::Edit => edit(global).unwrap_or_else(fail),
                ConfigCommand::Migrate => match migrate(global) {
                    Ok(0) => {
                        eprintln!("rah: no settings of an earlier version to move");
                        0
                    }
                    Ok(moved) => {
                        eprintln!(
                            "rah: moved {moved} stored settings into `{}`",
                            global
                                .config_path()
                                .map_or_else(|e| e, |p| p.display().to_string())
                        );
                        0
                    }
                    Err(message) => fail(message),
                },
                ConfigCommand::Show { plugins } => match global.prepare(false) {
                    Ok(prepared) => {
                        print!("{}", rah_cli::show(&prepared, *plugins));
                        0
                    }
                    Err(message) => fail(message),
                },
                ConfigCommand::Check => {
                    let prepared = match global.prepare(true) {
                        Ok(prepared) => prepared,
                        Err(message) => return fail(message),
                    };
                    let problems = rah_cli::check_distribution(&prepared.distribution);
                    if problems.is_empty() {
                        println!(
                            "configuration ok: {} plugins (keys, values and ids; API keys \
                             and programs are found only at start)",
                            prepared.distribution.plugin.len()
                        );
                        0
                    } else {
                        fail(rah_cli::BootError::InvalidConfig(problems))
                    }
                }
            };
        }
        _ => {}
    }
    let reads_sessions = matches!(cli.command, Some(Command::Sessions { .. }));
    // A first start: nothing names a model. Ask on a terminal, else explain.
    match global.unconfigured() {
        Ok(true) if !reads_sessions && on_terminal() => {
            eprintln!("rah: no configuration yet; a few questions write one.");
            match init_blocking(global).await {
                Ok(path) => eprintln!("rah: wrote `{}`", path.display()),
                Err(message) => return fail(message),
            }
        }
        Ok(true) if !reads_sessions => return fail(NO_MODEL),
        Ok(_) => {}
        Err(message) => return fail(message),
    }
    let prepared = match global.prepare(!reads_sessions) {
        Ok(prepared) => prepared,
        Err(message) => return fail(message),
    };
    if let Some(file) = &prepared.file
        && !reads_sessions
    {
        eprintln!("rah: using `{}`", file.display());
    }
    let mut distribution = prepared.distribution;
    let quiet = match frontend(
        &mut distribution,
        cli.session.as_deref(),
        cli.command.as_ref(),
    ) {
        Ok(quiet) => quiet,
        Err(message) => return fail(message),
    };
    serve(&distribution, quiet).await
}

/// Boots `distribution` and runs until a frontend asks to exit. `quiet`
/// skips the warnings about plugins and guards, for runs that only read.
async fn serve(distribution: &Distribution, quiet: bool) -> i32 {
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

    // Something a plugin skips quietly but the user can act on (ADR 0057).
    let _notices = handle.subscribe(move |notice: &HostNotice| {
        if !quiet {
            eprintln!("rah: {}: {}", notice.plugin, notice.message);
        }
    });

    let _listening = handle.subscribe(|listening: &FrontendListening| {
        eprintln!(
            "rah: {} is listening at {}",
            listening.plugin, listening.url
        );
    });

    match boot(&kernel, distribution).await {
        Ok(statuses) => {
            for (id, status) in statuses {
                if status != PluginStatus::Active && !quiet {
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
    if !quiet {
        if distribution.permissions == Some(Permissions::Full) {
            eprintln!(
                "rah: WARNING: permissions `full` - every tool with a permission assignment \
                 runs without asking."
            );
        }
        if !ToolInvoker::new(handle.clone()).is_guarded() {
            eprintln!(
                "rah: WARNING: UNGUARDED - no tool policy plugin is mounted; \
                 model tool calls run without authorization."
            );
        }
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
    let code = run(Cli::parse()).await;
    // Exit without dropping the runtime: a pending terminal read must not
    // keep the process alive after every plugin has been shut down.
    std::process::exit(code);
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    use super::*;

    #[test]
    fn the_command_line_is_consistent() {
        Cli::command().debug_assert();
    }

    #[test]
    fn global_options_go_before_or_after_the_command() {
        let before = Cli::try_parse_from(["rah", "--model", "mock", "run", "hi"]).unwrap();
        let after = Cli::try_parse_from(["rah", "run", "hi", "--model", "mock"]).unwrap();
        assert_eq!(before.global.model.as_deref(), Some("mock"));
        assert_eq!(after.global.model.as_deref(), Some("mock"));
        // `--session` of the REPL is not taken with a command.
        let cli = Cli::try_parse_from(["rah", "--session", "x", "web"]).unwrap();
        assert!(validate(&cli).unwrap_err().contains("`--session`"));
        let repl = Cli::try_parse_from(["rah", "--session", "x"]).unwrap();
        assert!(validate(&repl).is_ok());
    }

    #[test]
    fn removed_options_are_refused() {
        for removed in [
            "--preset",
            "--prompt",
            "--web",
            "--export",
            "--show-config",
            "--check-config",
            "--init-config",
            "--list-features",
            "--approve",
            "--session-dir",
        ] {
            let parsed = Cli::try_parse_from(["rah", removed, "x"]);
            assert!(parsed.is_err(), "{removed}");
        }
    }
}
