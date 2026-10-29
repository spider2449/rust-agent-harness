//! `rah-desktop`: RAH in a native window.
//!
//! The window shows the Web UI of `frontend-web`, served on a free loopback
//! port by the same distribution `rah --web` runs, in this process. Every
//! behavior (sessions, turns, approvals, settings and modes) is the Web
//! UI's, so a change there reaches the browser and the desktop alike; this
//! binary only owns the window and the process lifetime (ADR 0046).
//!
//! Closing the window shuts the kernel down. A plugin asking to exit closes
//! the window.

// A release build is a GUI program without a console window on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{
    path::{Path, PathBuf},
    sync::{Mutex, PoisonError, mpsc},
    time::Duration,
};

use clap::Parser;
use rah_api::{
    Kernel,
    host::{ExitRequested, FrontendListening},
    tools::ToolInvoker,
};
use rah_cli::{Distribution, boot, default_data_home, select_distribution};
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
    /// Built-in preset to run (ignored with --config), as for `rah`.
    #[arg(long)]
    preset: Option<String>,
    /// Distribution file listing the plugins to mount.
    #[arg(long)]
    config: Option<PathBuf>,
    /// Data directory, as for `rah`. Default: RAH_HOME, else `.rah` in the
    /// user's home directory.
    #[arg(long)]
    data_dir: Option<PathBuf>,
    /// Workspace root for file tools and processes.
    #[arg(long)]
    root: Option<PathBuf>,
    /// Mode to enter at start, such as `daily` or `coding`, instead of the
    /// last one used.
    #[arg(long)]
    mode: Option<String>,
    /// Turn on an opt-in feature, as for `rah`; repeatable.
    #[arg(long = "with", value_name = "FEATURE")]
    with: Vec<String>,
}

/// The window and taskbar icon: the logo as 64x64 RGBA, made by
/// `scripts/make-icons.ps1`. A bad asset only leaves the default icon.
fn window_icon() -> Option<Icon> {
    const RGBA: &[u8] = include_bytes!("../assets/icon.rgba");
    Icon::from_rgba(RGBA.to_vec(), 64, 64).ok()
}

/// How long the Web UI may take to start listening.
const LISTEN_TIMEOUT: Duration = Duration::from_secs(30);

fn absolute(path: &Path) -> Result<PathBuf, String> {
    std::path::absolute(path).map_err(|error| format!("`{}`: {error}", path.display()))
}

/// The distribution `rah --web` would run for these arguments, with the Web
/// UI on a free loopback port.
fn distribution(args: &Args) -> Result<Distribution, String> {
    let home = args
        .data_dir
        .clone()
        .or_else(default_data_home)
        .unwrap_or_else(|| PathBuf::from(".rah"));
    let (mut distribution, _) = select_distribution(
        args.config.as_deref(),
        args.preset.as_deref(),
        &absolute(&home)?,
    )?;
    for request in &args.with {
        distribution.enable(request)?;
    }
    if !distribution.use_web_frontend() {
        return Err("the distribution needs a `frontend-cli` or `frontend-web` plugin".into());
    }
    distribution.set_config("frontend-web", "bind", Value::from("127.0.0.1:0"));
    if let Some(mode) = &args.mode {
        distribution.set_config("frontend-web", "mode", Value::from(mode.as_str()));
    }
    if let Some(root) = &args.root
        && !distribution.set_root(&absolute(root)?)
    {
        return Err("--root needs a `tools-builtin` or sandbox plugin".into());
    }
    Ok(distribution)
}

enum UserEvent {
    Exit(i32),
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

/// The window title, with the version of this build (`RAH 0.45.0`).
const TITLE: &str = concat!("RAH ", env!("CARGO_PKG_VERSION"));

/// A page that shows why RAH did not start; a GUI program has no console.
fn failure_page(message: &str) -> String {
    let escaped = message
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    format!(
        "<!doctype html><meta charset=\"utf-8\"><title>{TITLE}</title>\
         <body style=\"font:15px system-ui,sans-serif;margin:32px\">\
         <h1 style=\"font-size:20px\">{TITLE} did not start</h1><pre style=\"white-space:pre-wrap\">{escaped}</pre>"
    )
}

fn run(args: Args) -> Result<(), String> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("cannot start the async runtime: {error}"))?;
    let kernel = Kernel::new();
    let started = distribution(&args)
        .and_then(|distribution| runtime.block_on(start(&kernel, &distribution)));
    if let Err(message) = &started {
        eprintln!("rah-desktop: {message}");
    }

    let event_loop: EventLoop<UserEvent> = EventLoopBuilder::with_user_event().build();
    let proxy = Mutex::new(event_loop.create_proxy());
    let _exit = kernel.handle().subscribe(move |exit: &ExitRequested| {
        let _ = proxy
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .send_event(UserEvent::Exit(exit.code));
    });
    let window = WindowBuilder::new()
        .with_title(TITLE)
        .with_window_icon(window_icon())
        .with_inner_size(LogicalSize::new(1280.0, 820.0))
        .with_min_inner_size(LogicalSize::new(480.0, 360.0))
        .build(&event_loop)
        .map_err(|error| format!("cannot open a window: {error}"))?;
    let builder = match &started {
        Ok(url) => WebViewBuilder::new().with_url(url),
        Err(message) => WebViewBuilder::new().with_html(failure_page(message)),
    };
    let view =
        webview(&window, builder).map_err(|error| format!("cannot create the WebView: {error}"))?;

    // The WebView lives as long as the event loop, and goes before the window
    // it is in.
    let mut shown = Some((view, window));
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        let code = match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => 0,
            Event::UserEvent(UserEvent::Exit(code)) => code,
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
    use super::*;

    fn args(extra: &[&str]) -> Args {
        let mut argv = vec!["rah-desktop", "--preset", "dev", "--data-dir", "."];
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
        assert!(config.get("mode").is_none());
    }

    #[test]
    fn the_mode_and_root_are_passed_on() {
        let distribution = distribution(&args(&["--mode", "coding", "--root", "."])).unwrap();
        assert_eq!(web_config(&distribution)["mode"], "coding");
        let root = distribution
            .plugin
            .iter()
            .find(|spec| spec.kind == "tools-builtin")
            .unwrap()
            .config["root"]
            .clone();
        assert_eq!(
            root,
            Value::from(absolute(Path::new(".")).unwrap().display().to_string())
        );
    }

    #[test]
    fn a_page_shows_why_rah_did_not_start() {
        let page = failure_page("bad <config> & more");
        assert!(page.contains("bad &lt;config&gt; &amp; more"));
    }
}
