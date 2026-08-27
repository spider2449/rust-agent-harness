//! Opt-in live smoke test for the restricted Codex runtime adapter.
//!
//! This example requires the exactly supported Codex CLI version and live model
//! access. It is intentionally excluded from the normal deterministic test suite.

use std::{env, path::PathBuf, process::ExitCode};

use futures::StreamExt;
use rah_protocol::{
    AgentEvent, AgentInput, AgentOptions, AgentRequest, Message, MessageRole, RequestId,
};
use rah_runtime::AgentRuntime;
use rah_runtime_codex::{
    CodexLlamaCppProvider, CodexModelConfig, CodexModelProvider, CodexModelSelection, CodexRuntime,
    SUPPORTED_CODEX_VERSION,
};

const DEFAULT_PROMPT: &str = "Reply with exactly: RAH_CODEX_SMOKE_OK";
const DEFAULT_EXPECTED_TEXT: &str = "RAH_CODEX_SMOKE_OK";
const LLAMACPP_PROMPT: &str = "Reply exactly:\nRAH_LLAMACPP_CHAT_OK";
const LLAMACPP_EXPECTED_TEXT: &str = "RAH_LLAMACPP_CHAT_OK";

fn main() -> ExitCode {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("LIVE_SMOKE_FAIL failed to create Tokio runtime: {error}");
            return ExitCode::FAILURE;
        }
    };

    match runtime.block_on(run()) {
        Ok(()) => {
            println!("LIVE_SMOKE_PASS");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("LIVE_SMOKE_FAIL {error}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), String> {
    let llama_cpp = env::var("RAH_LLAMACPP_LIVE").as_deref() == Ok("1");
    let expected_text = if llama_cpp {
        LLAMACPP_EXPECTED_TEXT
    } else {
        DEFAULT_EXPECTED_TEXT
    };
    let prompt = env::args().nth(1).unwrap_or_else(|| {
        if llama_cpp {
            LLAMACPP_PROMPT.to_owned()
        } else {
            DEFAULT_PROMPT.to_owned()
        }
    });
    let executable = env::var_os("RAH_CODEX_EXECUTABLE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("codex"));

    println!("CODEX_EXECUTABLE {}", executable.display());
    println!("REQUIRED_CODEX_VERSION {SUPPORTED_CODEX_VERSION}");
    println!(
        "PROMPT {}",
        serde_json::to_string(&prompt).map_err(|error| error.to_string())?
    );
    if llama_cpp {
        println!("MODEL_CONFIG model=rah-local-model provider=rah-llamacpp");
    }

    let runtime = if llama_cpp {
        CodexRuntime::connect_with_model_config(&executable, llama_cpp_model_config()?).await
    } else {
        CodexRuntime::connect(&executable).await
    }
    .map_err(|error| format!("connection failed: {error}"))?;
    let smoke_result = run_turn(&runtime, &prompt, expected_text).await;
    let shutdown_result = runtime.shutdown().await;

    match &shutdown_result {
        Ok(()) => println!("SHUTDOWN_OK"),
        Err(error) => eprintln!("SHUTDOWN_FAIL {error}"),
    }

    if let Err(error) = shutdown_result {
        return Err(format!("app-server shutdown failed: {error}"));
    }
    smoke_result
}

fn llama_cpp_model_config() -> Result<CodexModelConfig, String> {
    Ok(CodexModelConfig::Explicit(
        CodexModelSelection::new(
            "rah-local-model",
            CodexModelProvider::LlamaCpp(CodexLlamaCppProvider::default_local()),
        )
        .map_err(|error| format!("invalid llama.cpp model configuration: {error}"))?,
    ))
}

async fn run_turn(runtime: &CodexRuntime, prompt: &str, expected_text: &str) -> Result<(), String> {
    let handle = runtime
        .start(AgentRequest {
            request_id: RequestId::new(),
            input: AgentInput {
                messages: vec![Message {
                    role: MessageRole::User,
                    content: prompt.to_owned(),
                }],
            },
            options: AgentOptions::default(),
        })
        .await
        .map_err(|error| format!("failed to start agent request: {error}"))?;
    let mut events = handle.into_events();
    let mut streamed_text = String::new();
    let mut completed_text = None;
    let mut terminal_error = None;

    while let Some(event) = events.next().await {
        println!(
            "EVENT {}",
            serde_json::to_string(&event).map_err(|error| error.to_string())?
        );
        match event {
            AgentEvent::ModelDelta { delta, .. } => streamed_text.push_str(&delta),
            AgentEvent::Completed { output, .. } => {
                if completed_text.replace(output.message.content).is_some() {
                    terminal_error = Some("received more than one Completed event".to_owned());
                }
            }
            AgentEvent::Failed { code, message, .. } => {
                terminal_error = Some(format!("received Failed event ({code:?}): {message}"));
            }
            AgentEvent::Cancelled { .. } => {
                terminal_error = Some("received Cancelled event".to_owned());
            }
            AgentEvent::ToolRequested { .. }
            | AgentEvent::ToolStarted { .. }
            | AgentEvent::ToolFinished { .. }
            | AgentEvent::ApprovalRequired { .. } => {
                terminal_error =
                    Some("restricted live smoke received a tool or approval event".to_owned());
            }
            AgentEvent::Started { .. } | AgentEvent::ModelRequestStarted { .. } => {}
        }
    }

    if let Some(error) = terminal_error {
        return Err(error);
    }
    let completed_text = completed_text.ok_or_else(|| "missing Completed event".to_owned())?;
    if streamed_text != completed_text {
        return Err(format!(
            "streamed text did not match Completed output: streamed={streamed_text:?}, completed={completed_text:?}"
        ));
    }
    if completed_text != expected_text {
        return Err(format!(
            "unexpected response: expected={expected_text:?}, actual={completed_text:?}"
        ));
    }

    println!(
        "FINAL_TEXT {}",
        serde_json::to_string(&completed_text).map_err(|error| error.to_string())?
    );
    println!("TERMINAL_EVENT Completed");
    Ok(())
}
