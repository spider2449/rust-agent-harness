//! Opt-in live cancellation smoke test for the restricted Codex runtime adapter.
//!
//! This example requires the exactly supported Codex CLI version and live model
//! access. It is intentionally excluded from the normal deterministic test suite.

use std::{env, path::PathBuf, process::ExitCode, sync::Arc, time::Duration};

use futures::StreamExt;
use rah_protocol::{
    AgentEvent, AgentInput, AgentOptions, AgentRequest, Message, MessageRole, RequestId,
};
use rah_runtime::AgentRuntime;
use rah_runtime_codex::{CodexRuntime, SUPPORTED_CODEX_VERSION};

const LONG_TEXT_ONLY_PROMPT: &str = "Write 500 numbered paragraphs explaining the history of mathematics from ancient counting systems through modern abstract algebra. Each paragraph must contain at least four complete sentences. Produce text only from your existing knowledge. Do not use shell commands, file operations, MCP, web search, images, apps, or any other tools.";
const TERMINAL_TIMEOUT: Duration = Duration::from_secs(60);

fn main() -> ExitCode {
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("LIVE_CANCEL_FAIL failed to create Tokio runtime: {error}");
            return ExitCode::FAILURE;
        }
    };

    match runtime.block_on(run()) {
        Ok(()) => {
            println!("LIVE_CANCEL_PASS");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("LIVE_CANCEL_FAIL {error}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), String> {
    let executable = env::var_os("RAH_CODEX_EXECUTABLE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("codex"));

    println!("CODEX_EXECUTABLE {}", executable.display());
    println!("REQUIRED_CODEX_VERSION {SUPPORTED_CODEX_VERSION}");
    println!(
        "PROMPT {}",
        serde_json::to_string(LONG_TEXT_ONLY_PROMPT).map_err(|error| error.to_string())?
    );

    let runtime = Arc::new(
        CodexRuntime::connect(&executable)
            .await
            .map_err(|error| format!("connection failed: {error}"))?,
    );
    let smoke_result = run_cancelled_turn(Arc::clone(&runtime)).await;
    let shutdown_result = runtime.shutdown().await;

    match &shutdown_result {
        Ok(()) => println!("PROCESS_CLEANUP app-server shutdown completed"),
        Err(error) => eprintln!("PROCESS_CLEANUP_FAIL {error}"),
    }

    if let Err(error) = shutdown_result {
        return Err(format!("app-server shutdown failed: {error}"));
    }
    smoke_result
}

async fn run_cancelled_turn(runtime: Arc<CodexRuntime>) -> Result<(), String> {
    let handle = runtime
        .start(AgentRequest {
            request_id: RequestId::new(),
            input: AgentInput {
                messages: vec![Message {
                    role: MessageRole::User,
                    content: LONG_TEXT_ONLY_PROMPT.to_owned(),
                }],
            },
            options: AgentOptions::default(),
        })
        .await
        .map_err(|error| format!("failed to start agent request: {error}"))?;
    let session_id = handle.session_id().clone();
    let mut events = handle.into_events();
    let mut event_names = Vec::new();
    let mut model_delta_count = 0_usize;

    loop {
        let event = events
            .next()
            .await
            .ok_or_else(|| "event stream ended before ModelRequestStarted".to_owned())?;
        print_event(&event)?;
        event_names.push(event_name(&event));
        match event {
            AgentEvent::ModelDelta { .. } => {
                model_delta_count += 1;
                break;
            }
            AgentEvent::Started { .. } | AgentEvent::ModelRequestStarted { .. } => {}
            terminal => {
                return Err(format!(
                    "turn stopped before the first model delta; received {}",
                    event_name(&terminal)
                ));
            }
        }
    }

    println!("CANCELLATION_TRIGGER first ModelDelta observed from active turn");
    let cancelling = tokio::spawn(async move { runtime.cancel(session_id).await });
    tokio::time::timeout(TERMINAL_TIMEOUT, async {
        while let Some(event) = events.next().await {
            if matches!(event, AgentEvent::ModelDelta { .. }) {
                model_delta_count += 1;
            } else {
                print_event(&event)?;
            }
            event_names.push(event_name(&event));
        }
        Ok::<(), String>(())
    })
    .await
    .map_err(|_| "timed out waiting for terminal RAH event".to_owned())??;
    tokio::time::timeout(TERMINAL_TIMEOUT, cancelling)
        .await
        .map_err(|_| "timed out waiting for AgentRuntime::cancel".to_owned())?
        .map_err(|error| format!("cancellation task failed: {error}"))?
        .map_err(|error| format!("AgentRuntime::cancel failed: {error}"))?;

    println!("TURN_INTERRUPT_RESULT accepted; interrupted terminal notification confirmed");
    println!("CODEX_TERMINAL_STATUS interrupted");
    println!("MODEL_DELTA_COUNT {model_delta_count}");
    println!(
        "EVENT_SEQUENCE Started -> ModelRequestStarted -> ModelDelta({model_delta_count}) -> {}",
        event_names.last().copied().unwrap_or("missing")
    );

    let cancelled_index = event_names
        .iter()
        .position(|name| *name == "Cancelled")
        .ok_or_else(|| "missing Cancelled event".to_owned())?;
    if event_names[..cancelled_index].contains(&"Completed") {
        return Err("Completed was emitted before Cancelled".to_owned());
    }
    if event_names[cancelled_index + 1..].contains(&"Completed") {
        return Err("Completed was emitted after Cancelled".to_owned());
    }
    if event_names.iter().any(|name| {
        matches!(
            *name,
            "ToolRequested" | "ToolStarted" | "ToolFinished" | "ApprovalRequired"
        )
    }) {
        return Err("restricted live smoke received a tool or approval event".to_owned());
    }
    if event_names.last() != Some(&"Cancelled") {
        return Err(format!(
            "Cancelled was not the terminal RAH event: {event_names:?}"
        ));
    }

    println!("TERMINAL_RAH_EVENT Cancelled");
    println!("COMPLETED_AFTER_CANCELLED false");
    Ok(())
}

fn print_event(event: &AgentEvent) -> Result<(), String> {
    println!(
        "EVENT {}",
        serde_json::to_string(event).map_err(|error| error.to_string())?
    );
    Ok(())
}

fn event_name(event: &AgentEvent) -> &'static str {
    match event {
        AgentEvent::Started { .. } => "Started",
        AgentEvent::ModelRequestStarted { .. } => "ModelRequestStarted",
        AgentEvent::ModelDelta { .. } => "ModelDelta",
        AgentEvent::ToolRequested { .. } => "ToolRequested",
        AgentEvent::ToolStarted { .. } => "ToolStarted",
        AgentEvent::ToolFinished { .. } => "ToolFinished",
        AgentEvent::ApprovalRequired { .. } => "ApprovalRequired",
        AgentEvent::Completed { .. } => "Completed",
        AgentEvent::Failed { .. } => "Failed",
        AgentEvent::Cancelled { .. } => "Cancelled",
    }
}
