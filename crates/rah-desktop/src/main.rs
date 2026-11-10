//! `rah-desktop`: RAH in a native window.
//!
//! The window shows the Web UI of `frontend-web`, served on a free loopback
//! port by the same distribution `rah web` runs, in this process. Every
//! behavior (sessions, turns, approvals and settings) is the Web
//! UI's, so a change there reaches the browser and the desktop alike; this
//! binary only owns the window and the process lifetime (ADR 0046).
//!
//! Closing the window shuts the kernel down. A plugin asking to exit closes
//! the window.

// A release build is a GUI program without a console window on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{
    sync::{Mutex, PoisonError, mpsc},
    time::Duration,
};

use clap::Parser;
use rah_api::{
    Kernel,
    host::{ExitRequested, FrontendListening},
    tools::ToolInvoker,
};
use rah_cli::{Distribution, boot, cli::GlobalArgs, setup};

mod first_start;
use serde_json::Value;
use tao::{
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop, EventLoopBuilder},
    window::{Icon, Window, WindowBuilder},
};
use wry::{WebView, WebViewBuilder};

/// RAH desktop: the Web UI in a native window.
#[derive(Debug, Parser)]
#[command(name = "rah-desktop", version)]
struct Args {
    #[command(flatten)]
    global: GlobalArgs,
}

/// The window and taskbar icon: the logo as 64x64 RGBA, made by
/// `scripts/make-icons.ps1`. A bad asset only leaves the default icon.
fn window_icon() -> Option<Icon> {
    const RGBA: &[u8] = include_bytes!("../assets/icon.rgba");
    Icon::from_rgba(RGBA.to_vec(), 64, 64).ok()
}

/// How long the Web UI may take to start listening.
const LISTEN_TIMEOUT: Duration = Duration::from_secs(30);

/// The distribution `rah web` would run for these arguments, with the Web UI
/// on a free loopback port.
fn distribution(args: &Args) -> Result<Distribution, String> {
    let mut distribution = args.global.prepare(true)?.distribution;
    if !distribution.use_web_frontend() {
        return Err("the distribution needs a `frontend-cli` or `frontend-web` plugin".into());
    }
    distribution.set_config("frontend-web", "bind", Value::from("127.0.0.1:0"));
    Ok(distribution)
}

enum UserEvent {
    Exit(i32),
    /// The setup form's answers, as JSON.
    Setup(String),
}

/// Boots the distribution; returns the Web UI's URL.
async fn start(kernel: &Kernel, distribution: &Distribution) -> Result<String, String> {
    let (sender, receiver) = mpsc::channel();
    let sender = Mutex::new(sender);
    let _listening = kernel
        .handle()
        .subscribe(move |listening: &FrontendListening| {
            let _ = sender
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .send(listening.url.clone());
        });
    let statuses = boot(kernel, distribution)
        .await
        .map_err(|error| error.to_string())?;
    for (id, status) in statuses {
        if status != rah_api::PluginStatus::Active {
            eprintln!("rah-desktop: plugin `{id}` is not active: {status:?}");
        }
    }
    if !ToolInvoker::new(kernel.handle()).is_guarded() {
        eprintln!(
            "rah-desktop: WARNING: UNGUARDED - no tool policy plugin is mounted; \
             model tool calls run without authorization."
        );
    }
    // The URL is announced while `frontend-web` starts, inside the boot.
    receiver
        .recv_timeout(LISTEN_TIMEOUT)
        .map_err(|_| "the Web UI did not start listening".to_owned())
}

fn webview(window: &Window, builder: WebViewBuilder<'_>) -> wry::Result<WebView> {
    #[cfg(not(target_os = "linux"))]
    {
        builder.build(window)
    }
    #[cfg(target_os = "linux")]
    {
        use tao::platform::unix::WindowExtUnix;
        use wry::WebViewBuilderExtUnix;
        let vbox = window.default_vbox().ok_or(wry::Error::MessageSender)?;
        builder.build_gtk(vbox)
    }
}

/// Moves keyboard focus into the WebView when the window owes it one and can
/// give it: a minimized window cannot, and stays in debt until it is restored.
fn give_focus(view: &WebView, window: &Window, owed: &mut bool) {
    if *owed && !window.is_minimized() && view.focus().is_ok() {
        *owed = false;
    }
}

/// The window title, with the version of this build (`RAH 0.45.0`).
const TITLE: &str = concat!("RAH ", env!("CARGO_PKG_VERSION"));

/// A page that shows why RAH did not start; a GUI program has no console.
fn failure_page(message: &str) -> String {
    let escaped = first_start::escape(message);
    format!(
        "<!doctype html><meta charset=\"utf-8\"><title>{TITLE}</title>\
         <body style=\"font:15px system-ui,sans-serif;margin:32px\">\
         <h1 style=\"font-size:20px\">{TITLE} did not start</h1><pre style=\"white-space:pre-wrap\">{escaped}</pre>"
    )
}

/// What the window shows first.
enum Start {
    Running(String),
    Failed(String),
    /// No configuration yet: the setup form.
    Setup(String),
}

/// The setup form for a first start, with what is known about the machine.
fn setup_page(global: &GlobalArgs) -> String {
    let is_set = |name: &str| std::env::var_os(name).is_some_and(|value| !value.is_empty());
    let root = global
        .root
        .as_deref()
        .and_then(|root| rah_cli::cli::absolute(root).ok())
        .or_else(|| {
            std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(Into::into)
        })
        .map(|root: std::path::PathBuf| root.display().to_string())
        .unwrap_or_default();
    let environment = setup::Environment {
        is_set: &is_set,
        llama_cpp_answers: setup::answers(setup::LLAMA_CPP_ADDRESS, Duration::from_millis(300)),
        permissions: global
            .permissions
            .as_deref()
            .and_then(|permissions| permissions.parse().ok()),
        root: None,
        features: &global.with,
    };
    first_start::page(TITLE, &environment, &root)
}

/// Writes the configuration file from the setup form's answers. An error
/// leaves nothing written, and the form says why.
fn configure(global: &GlobalArgs, body: &str) -> Result<(), String> {
    let text = first_start::answers(body)?.text(&global.with)?;
    setup::write_new(&global.config_path()?, &text)
}

/// Boots the distribution `args` select.
fn launch(runtime: &tokio::runtime::Runtime, kernel: &Kernel, args: &Args) -> Start {
    match distribution(args).and_then(|distribution| runtime.block_on(start(kernel, &distribution)))
    {
        Ok(url) => Start::Running(url),
        Err(message) => {
            eprintln!("rah-desktop: {message}");
            Start::Failed(message)
        }
    }
}

fn run(args: Args) -> Result<(), String> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("cannot start the async runtime: {error}"))?;
    let kernel = Kernel::new();
    let first = match args.global.unconfigured() {
        Ok(true) => Start::Setup(setup_page(&args.global)),
        Ok(false) => launch(&runtime, &kernel, &args),
        Err(message) => Start::Failed(message),
    };

    let event_loop: EventLoop<UserEvent> = EventLoopBuilder::with_user_event().build();
    let proxy = Mutex::new(event_loop.create_proxy());
    let _exit = kernel.handle().subscribe(move |exit: &ExitRequested| {
        let _ = proxy
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .send_event(UserEvent::Exit(exit.code));
    });
    let setup_proxy = event_loop.create_proxy();
    let window = WindowBuilder::new()
        .with_title(TITLE)
        .with_window_icon(window_icon())
        .with_inner_size(LogicalSize::new(1280.0, 820.0))
        .with_min_inner_size(LogicalSize::new(480.0, 360.0))
        .build(&event_loop)
        .map_err(|error| format!("cannot open a window: {error}"))?;
    let mut awaiting_setup = matches!(first, Start::Setup(_));
    let builder = match &first {
        Start::Running(url) => WebViewBuilder::new().with_url(url),
        Start::Failed(message) => WebViewBuilder::new().with_html(failure_page(message)),
        // Only the setup form posts messages; the event loop ignores any
        // that arrive once it is answered.
        Start::Setup(page) => {
            WebViewBuilder::new()
                .with_html(page)
                .with_ipc_handler(move |request| {
                    let _ = setup_proxy.send_event(UserEvent::Setup(request.body().clone()));
                })
        }
    };
    let view =
        webview(&window, builder).map_err(|error| format!("cannot create the WebView: {error}"))?;

    // The WebView lives as long as the event loop, and goes before the window
    // it is in.
    let mut shown = Some((view, window));
    // Set while the window has keyboard focus that the WebView has not taken.
    let mut focus_owed = false;
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        let code = match event {
            // Keyboard focus that stays on the window instead of the WebView
            // sends IME composition to a floating system window, away from
            // the input box. wry hands focus over as the window gains it,
            // but WebView2 refuses while the window is minimized, so the
            // hand-over is repeated here and again once the window is
            // restored.
            Event::WindowEvent {
                event: WindowEvent::Focused(focused),
                ..
            } => {
                focus_owed = focused;
                if let Some((view, window)) = &shown {
                    give_focus(view, window, &mut focus_owed);
                }
                return;
            }
            Event::WindowEvent {
                event: WindowEvent::Resized(_),
                ..
            } => {
                if let Some((view, window)) = &shown {
                    give_focus(view, window, &mut focus_owed);
                }
                return;
            }
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => 0,
            Event::UserEvent(UserEvent::Exit(code)) => code,
            Event::UserEvent(UserEvent::Setup(body)) if awaiting_setup => {
                let Some((view, _)) = &shown else { return };
                if let Err(message) = configure(&args.global, &body) {
                    let _ = view.evaluate_script(&first_start::failed_script(&message));
                    return;
                }
                awaiting_setup = false;
                let _ = match launch(&runtime, &kernel, &args) {
                    Start::Running(url) => view.load_url(&url),
                    Start::Failed(message) | Start::Setup(message) => {
                        view.load_html(&failure_page(&message))
                    }
                };
                return;
            }
            _ => return,
        };
        // The window goes first, so closing feels immediate. Both are
        // released before the process exits: WebView2 reports "Failed to
        // unregister class Chrome_WidgetWin_0" when a window outlives it.
        if let Some((view, window)) = shown.take() {
            window.set_visible(false);
            drop(view);
            drop(window);
        }
        let _ = runtime.block_on(kernel.shutdown());
        std::process::exit(code);
    })
}

fn main() {
    if let Err(message) = run(Args::parse()) {
        eprintln!("rah-desktop: {message}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    fn args(extra: &[&str]) -> Args {
        let mut argv = vec!["rah-desktop", "--model", "mock", "--data-dir", "."];
        argv.extend_from_slice(extra);
        Args::parse_from(argv)
    }

    fn web_config(distribution: &Distribution) -> Value {
        let mut webs = distribution
            .plugin
            .iter()
            .filter(|spec| spec.kind == "frontend-web");
        let web = webs.next().expect("frontend-web is mounted");
        assert!(webs.next().is_none());
        web.config.clone()
    }

    #[test]
    fn the_window_runs_the_web_ui_on_a_free_loopback_port() {
        let distribution = distribution(&args(&[])).unwrap();
        assert!(
            !distribution
                .plugin
                .iter()
                .any(|spec| spec.kind == "frontend-cli")
        );
        let config = web_config(&distribution);
        assert_eq!(config["bind"], "127.0.0.1:0");
    }

    #[test]
    fn the_root_is_passed_on() {
        let distribution = distribution(&args(&["--root", "."])).unwrap();
        let root = distribution
            .plugin
            .iter()
            .find(|spec| spec.kind == "tools-builtin")
            .unwrap()
            .config["root"]
            .clone();
        assert_eq!(
            root,
            Value::from(
                rah_cli::cli::absolute(Path::new("."))
                    .unwrap()
                    .display()
                    .to_string()
            )
        );
    }

    #[test]
    fn a_page_shows_why_rah_did_not_start() {
        let page = failure_page("bad <config> & more");
        assert!(page.contains("bad &lt;config&gt; &amp; more"));
    }
}
