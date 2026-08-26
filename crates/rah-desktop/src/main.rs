#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(target_os = "windows")]
use futures::StreamExt;
#[cfg(target_os = "windows")]
use rah_protocol::{
    AgentEvent, AgentInput, AgentOptions, AgentRequest, Message, MessageRole, PermissionLevel,
    RequestId,
};
#[cfg(target_os = "windows")]
use rah_runtime::AgentRuntime;
#[cfg(target_os = "windows")]
use rah_runtime_codex::{CodexAdapterError, CodexRuntime, SUPPORTED_CODEX_VERSION};
#[cfg(target_os = "windows")]
use rah_tools::{EchoTool, ToolError, ToolRegistry};
#[cfg(target_os = "windows")]
use serde::Serialize;
#[cfg(target_os = "windows")]
use std::{
    ffi::OsString,
    process::ExitCode,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
#[cfg(target_os = "windows")]
use tauri::{AppHandle, Emitter, Manager, State, WindowEvent};

#[cfg(target_os = "windows")]
const MAX_PROMPT_BYTES: usize = 32 * 1024;
#[cfg(target_os = "windows")]
const DESKTOP_TOOL_NAME: &str = "echo";

#[cfg(target_os = "windows")]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AppStatus {
    app_name: &'static str,
    app_version: &'static str,
    platform: &'static str,
    desktop_shell: &'static str,
    runtime_status: &'static str,
    codex_status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    codex_version: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    codex_error: Option<FrontendError>,
    profile_status: &'static str,
    repository_status: &'static str,
}

#[cfg(target_os = "windows")]
enum ConnectionState {
    NotConnected,
    Connecting,
    Connected(Arc<CodexRuntime>),
    Disconnecting,
    Error(FrontendError),
}

#[cfg(target_os = "windows")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ChatState {
    Idle,
    Running,
}

#[cfg(target_os = "windows")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ConnectRequest {
    Start,
    AlreadyConnected,
    InProgress,
}

#[cfg(target_os = "windows")]
fn request_connect(connection: &mut ConnectionState) -> ConnectRequest {
    match connection {
        ConnectionState::Connected(_) => ConnectRequest::AlreadyConnected,
        ConnectionState::Connecting | ConnectionState::Disconnecting => ConnectRequest::InProgress,
        ConnectionState::NotConnected | ConnectionState::Error(_) => {
            *connection = ConnectionState::Connecting;
            ConnectRequest::Start
        }
    }
}

#[cfg(target_os = "windows")]
struct DesktopAppState {
    connection: Mutex<ConnectionState>,
    chat: Mutex<ChatState>,
    close_started: AtomicBool,
}

#[cfg(target_os = "windows")]
impl Default for DesktopAppState {
    fn default() -> Self {
        Self {
            connection: Mutex::new(ConnectionState::NotConnected),
            chat: Mutex::new(ChatState::Idle),
            close_started: AtomicBool::new(false),
        }
    }
}

#[cfg(target_os = "windows")]
impl DesktopAppState {
    fn finish_chat(&self) {
        *self
            .chat
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = ChatState::Idle;
    }
}

#[cfg(target_os = "windows")]
impl DesktopAppState {
    fn status(&self) -> AppStatus {
        let connection = self
            .connection
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        current_app_status(&connection)
    }

    fn close_started(&self) -> bool {
        self.close_started
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }

    async fn shutdown_for_exit(&self) {
        let runtime = {
            let mut connection = self
                .connection
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            match std::mem::replace(&mut *connection, ConnectionState::Disconnecting) {
                ConnectionState::Connected(runtime) => Some(runtime),
                state => {
                    *connection = state;
                    None
                }
            }
        };
        if let Some(runtime) = runtime
            && let Err(error) = runtime.shutdown().await
        {
            tracing::warn!(error = %error, "failed to shut down Codex during desktop exit");
        }
    }
}

#[cfg(target_os = "windows")]
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum FrontendError {
    CodexNotFound,
    UnsupportedCodexVersion,
    CodexSchemaIncompatible,
    CodexStartFailed,
    CodexConnectionFailed,
    ToolRegistryFailed,
    ChatEmptyPrompt,
    ChatPromptTooLarge,
    CodexNotConnected,
    ChatAlreadyRunning,
    ChatStartFailed,
    ChatRuntimeFailed,
    ChatCancelled,
}

#[cfg(target_os = "windows")]
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum ChatEvent {
    Started,
    Delta { text: String },
    Completed,
    Failed { code: FrontendError },
    Cancelled { code: FrontendError },
}

#[cfg(target_os = "windows")]
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum ActivityEvent {
    #[serde(rename = "tool_requested")]
    Requested { tool: &'static str },
    #[serde(rename = "tool_started")]
    Started { tool: &'static str },
    #[serde(rename = "tool_finished")]
    Finished {
        tool: &'static str,
        result: ActivityResult,
    },
}

#[cfg(target_os = "windows")]
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum ActivityResult {
    Success,
    Failed,
}

#[cfg(target_os = "windows")]
fn frontend_error(error: &CodexAdapterError) -> FrontendError {
    match error {
        CodexAdapterError::ExecutableDiscovery { .. } => FrontendError::CodexNotFound,
        CodexAdapterError::VersionMismatch { .. } => FrontendError::UnsupportedCodexVersion,
        CodexAdapterError::SchemaInspection { .. } | CodexAdapterError::SchemaMismatch { .. } => {
            FrontendError::CodexSchemaIncompatible
        }
        CodexAdapterError::ProcessStartup { .. } => FrontendError::CodexStartFailed,
        CodexAdapterError::ProcessExited { .. }
        | CodexAdapterError::MalformedFraming { .. }
        | CodexAdapterError::JsonRpc { .. }
        | CodexAdapterError::ProtocolViolation { .. }
        | CodexAdapterError::Transport { .. } => FrontendError::CodexConnectionFailed,
    }
}

#[cfg(target_os = "windows")]
fn current_app_status(connection: &ConnectionState) -> AppStatus {
    let (runtime_status, codex_status, codex_version, codex_error) = match connection {
        ConnectionState::NotConnected => ("not connected", "not connected", None, None),
        ConnectionState::Error(error) => ("not connected", "error", None, Some(*error)),
        ConnectionState::Connecting => ("not connected", "connecting", None, None),
        ConnectionState::Connected(_) => (
            "connected",
            "connected",
            Some(SUPPORTED_CODEX_VERSION),
            None,
        ),
        ConnectionState::Disconnecting => ("connected", "disconnecting", None, None),
    };
    AppStatus {
        app_name: "RAH",
        app_version: env!("CARGO_PKG_VERSION"),
        platform: "windows",
        desktop_shell: "ready",
        runtime_status,
        codex_status,
        codex_version,
        codex_error,
        profile_status: "not loaded",
        repository_status: "not selected",
    }
}

#[cfg(target_os = "windows")]
#[tauri::command]
fn app_status(state: State<'_, DesktopAppState>) -> AppStatus {
    state.status()
}

#[cfg(target_os = "windows")]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ConnectionResult {
    status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    version: Option<&'static str>,
}

#[cfg(target_os = "windows")]
impl ConnectionResult {
    fn connected() -> Self {
        Self {
            status: "connected",
            version: Some(SUPPORTED_CODEX_VERSION),
        }
    }

    fn connecting() -> Self {
        Self {
            status: "connecting",
            version: None,
        }
    }

    fn not_connected() -> Self {
        Self {
            status: "not connected",
            version: None,
        }
    }
}

#[cfg(target_os = "windows")]
fn selected_codex_executable() -> OsString {
    std::env::var_os("RAH_CODEX_EXECUTABLE").unwrap_or_else(|| OsString::from("codex"))
}

#[cfg(target_os = "windows")]
fn desktop_tool_registry() -> Result<Arc<ToolRegistry>, ToolError> {
    let mut registry = ToolRegistry::new();
    registry.register(Arc::new(EchoTool::new()))?;
    Ok(Arc::new(registry))
}

#[cfg(target_os = "windows")]
#[tauri::command]
async fn connect_codex(
    state: State<'_, DesktopAppState>,
) -> Result<ConnectionResult, FrontendError> {
    {
        let mut connection = state
            .connection
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match request_connect(&mut connection) {
            ConnectRequest::AlreadyConnected => return Ok(ConnectionResult::connected()),
            ConnectRequest::InProgress => return Ok(ConnectionResult::connecting()),
            ConnectRequest::Start => {}
        }
    }

    let registry = match desktop_tool_registry() {
        Ok(registry) => registry,
        Err(error) => {
            tracing::error!(error = %error, "failed to construct desktop tool registry");
            let mut connection = state
                .connection
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            *connection = ConnectionState::Error(FrontendError::ToolRegistryFailed);
            return Err(FrontendError::ToolRegistryFailed);
        }
    };

    match CodexRuntime::connect_tool_bridge(
        selected_codex_executable(),
        registry,
        vec![PermissionLevel::None],
    )
    .await
    {
        Ok(runtime) => {
            let mut connection = state
                .connection
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            *connection = ConnectionState::Connected(Arc::new(runtime));
            Ok(ConnectionResult::connected())
        }
        Err(error) => {
            tracing::warn!(error = %error, "Codex desktop connection failed");
            let frontend_error = frontend_error(&error);
            let mut connection = state
                .connection
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            *connection = ConnectionState::Error(frontend_error);
            Err(frontend_error)
        }
    }
}

#[cfg(target_os = "windows")]
#[tauri::command]
async fn disconnect_codex(
    state: State<'_, DesktopAppState>,
) -> Result<ConnectionResult, FrontendError> {
    if *state
        .chat
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        == ChatState::Running
    {
        return Err(FrontendError::ChatAlreadyRunning);
    }
    let runtime = {
        let mut connection = state
            .connection
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match std::mem::replace(&mut *connection, ConnectionState::Disconnecting) {
            ConnectionState::Connected(runtime) => Some(runtime),
            state => {
                *connection = state;
                None
            }
        }
    };

    let Some(runtime) = runtime else {
        return Ok(ConnectionResult::not_connected());
    };

    if let Err(error) = runtime.shutdown().await {
        tracing::warn!(error = %error, "Codex desktop disconnection failed");
        let frontend_error = frontend_error(&error);
        let mut connection = state
            .connection
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *connection = ConnectionState::Error(frontend_error);
        return Err(frontend_error);
    }

    let mut connection = state
        .connection
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    *connection = ConnectionState::NotConnected;
    Ok(ConnectionResult::not_connected())
}

#[cfg(target_os = "windows")]
fn validate_prompt(prompt: &str) -> Result<(), FrontendError> {
    if prompt.trim().is_empty() {
        return Err(FrontendError::ChatEmptyPrompt);
    }
    if prompt.len() > MAX_PROMPT_BYTES {
        return Err(FrontendError::ChatPromptTooLarge);
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn begin_chat(chat: &mut ChatState) -> Result<(), FrontendError> {
    if *chat == ChatState::Running {
        return Err(FrontendError::ChatAlreadyRunning);
    }
    *chat = ChatState::Running;
    Ok(())
}

#[cfg(target_os = "windows")]
fn emit_chat_event(app: &AppHandle, event: ChatEvent) {
    if let Err(error) = app.emit("chat_event", event) {
        tracing::warn!(error = %error, "failed to emit desktop chat event");
    }
}

#[cfg(target_os = "windows")]
fn activity_event(event: &AgentEvent) -> Option<ActivityEvent> {
    match event {
        AgentEvent::ToolRequested { tool_call, .. }
            if tool_call.name.as_str() == DESKTOP_TOOL_NAME =>
        {
            Some(ActivityEvent::Requested {
                tool: DESKTOP_TOOL_NAME,
            })
        }
        AgentEvent::ToolStarted { .. } => Some(ActivityEvent::Started {
            tool: DESKTOP_TOOL_NAME,
        }),
        AgentEvent::ToolFinished { output, .. } => Some(ActivityEvent::Finished {
            tool: DESKTOP_TOOL_NAME,
            result: if output.is_error {
                ActivityResult::Failed
            } else {
                ActivityResult::Success
            },
        }),
        _ => None,
    }
}

#[cfg(target_os = "windows")]
fn emit_activity_event(app: &AppHandle, event: ActivityEvent) {
    if let Err(error) = app.emit("activity_event", event) {
        tracing::warn!(error = %error, "failed to emit desktop activity event");
    }
}

#[cfg(target_os = "windows")]
async fn run_chat(app: AppHandle, runtime: Arc<CodexRuntime>, prompt: String) {
    let request = AgentRequest {
        request_id: RequestId::new(),
        input: AgentInput {
            messages: vec![Message {
                role: MessageRole::User,
                content: prompt,
            }],
        },
        options: AgentOptions::default(),
    };

    let handle = match runtime.start(request).await {
        Ok(handle) => handle,
        Err(error) => {
            tracing::warn!(error = %error, "desktop chat turn failed to start");
            emit_chat_event(
                &app,
                ChatEvent::Failed {
                    code: FrontendError::ChatStartFailed,
                },
            );
            app.state::<DesktopAppState>().finish_chat();
            return;
        }
    };

    emit_chat_event(&app, ChatEvent::Started);
    let mut terminal = false;
    let mut events = handle.into_events();
    while let Some(event) = events.next().await {
        if let Some(activity) = activity_event(&event) {
            emit_activity_event(&app, activity);
        }
        match event {
            AgentEvent::ModelDelta { delta, .. } => {
                emit_chat_event(&app, ChatEvent::Delta { text: delta })
            }
            AgentEvent::Completed { .. } => {
                emit_chat_event(&app, ChatEvent::Completed);
                terminal = true;
                break;
            }
            AgentEvent::Failed { .. } => {
                emit_chat_event(
                    &app,
                    ChatEvent::Failed {
                        code: FrontendError::ChatRuntimeFailed,
                    },
                );
                terminal = true;
                break;
            }
            AgentEvent::Cancelled { .. } => {
                emit_chat_event(
                    &app,
                    ChatEvent::Cancelled {
                        code: FrontendError::ChatCancelled,
                    },
                );
                terminal = true;
                break;
            }
            AgentEvent::Started { .. }
            | AgentEvent::ModelRequestStarted { .. }
            | AgentEvent::ToolRequested { .. }
            | AgentEvent::ToolStarted { .. }
            | AgentEvent::ToolFinished { .. }
            | AgentEvent::ApprovalRequired { .. } => {}
        }
    }
    if !terminal {
        emit_chat_event(
            &app,
            ChatEvent::Failed {
                code: FrontendError::ChatRuntimeFailed,
            },
        );
    }
    app.state::<DesktopAppState>().finish_chat();
}

#[cfg(target_os = "windows")]
#[tauri::command]
async fn send_chat(
    prompt: String,
    app: AppHandle,
    state: State<'_, DesktopAppState>,
) -> Result<(), FrontendError> {
    validate_prompt(&prompt)?;
    {
        let mut chat = state
            .chat
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        begin_chat(&mut chat)?;
    }
    let runtime = {
        let connection = state
            .connection
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match &*connection {
            ConnectionState::Connected(runtime) => Arc::clone(runtime),
            _ => {
                state.finish_chat();
                return Err(FrontendError::CodexNotConnected);
            }
        }
    };
    tauri::async_runtime::spawn(run_chat(app, runtime, prompt));
    Ok(())
}

#[cfg(target_os = "windows")]
fn main() -> ExitCode {
    match tauri::Builder::default()
        .manage(DesktopAppState::default())
        .invoke_handler(tauri::generate_handler![
            app_status,
            connect_codex,
            disconnect_codex,
            send_chat
        ])
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let state = window.app_handle().state::<DesktopAppState>();
                if state.close_started() {
                    return;
                }
                api.prevent_close();
                let app = window.app_handle().clone();
                let closing_window = window.clone();
                tauri::async_runtime::spawn(async move {
                    app.state::<DesktopAppState>().shutdown_for_exit().await;
                    let _ = closing_window.close();
                });
            }
        })
        .run(tauri::generate_context!())
    {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("failed to run RAH desktop shell: {error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn main() {
    eprintln!("rah-desktop is available on Windows only");
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::{
        ActivityEvent, ActivityResult, ChatEvent, ChatState, ConnectRequest, ConnectionState,
        DESKTOP_TOOL_NAME, FrontendError, MAX_PROMPT_BYTES, activity_event, begin_chat,
        current_app_status, desktop_tool_registry, frontend_error, request_connect,
        validate_prompt,
    };
    use rah_protocol::{AgentEvent, SessionId, ToolCallId, ToolOutput};
    use rah_runtime_codex::CodexAdapterError;
    use std::path::PathBuf;

    #[test]
    fn status_contains_only_the_desktop_application_state() {
        let status = current_app_status(&ConnectionState::NotConnected);

        assert_eq!(status.app_name, "RAH");
        assert_eq!(status.app_version, env!("CARGO_PKG_VERSION"));
        assert_eq!(status.platform, "windows");
        assert_eq!(status.desktop_shell, "ready");
        assert_eq!(status.runtime_status, "not connected");
        assert_eq!(status.codex_status, "not connected");
        assert_eq!(status.codex_version, None);
        assert_eq!(status.codex_error, None);
        assert_eq!(status.profile_status, "not loaded");
        assert_eq!(status.repository_status, "not selected");

        let serialized = serde_json::to_string(&status).expect("status serializes");
        assert!(!serialized.contains('\\'));
        assert!(!serialized.contains('/'));
    }

    #[test]
    fn status_reflects_connection_transitions_without_exposing_runtime_details() {
        let connecting = current_app_status(&ConnectionState::Connecting);
        assert_eq!(connecting.runtime_status, "not connected");
        assert_eq!(connecting.codex_status, "connecting");
        assert_eq!(connecting.codex_version, None);
        assert_eq!(connecting.codex_error, None);

        let error = current_app_status(&ConnectionState::Error(
            FrontendError::CodexConnectionFailed,
        ));
        assert_eq!(error.runtime_status, "not connected");
        assert_eq!(error.codex_status, "error");
        assert_eq!(error.codex_version, None);
        assert_eq!(
            error.codex_error,
            Some(FrontendError::CodexConnectionFailed)
        );
    }

    #[test]
    fn adapter_errors_are_sanitized_for_the_frontend() {
        let error = CodexAdapterError::ExecutableDiscovery {
            path: PathBuf::from(r"C:\\private\\codex.exe"),
            source: std::io::Error::new(std::io::ErrorKind::NotFound, "missing executable"),
        };
        let frontend = frontend_error(&error);
        assert_eq!(frontend, FrontendError::CodexNotFound);
        let serialized = serde_json::to_string(&frontend).expect("frontend error serializes");
        assert_eq!(serialized, "\"codex_not_found\"");
        assert!(!serialized.contains("private"));
        assert!(!serialized.contains("codex.exe"));
    }

    #[test]
    fn duplicate_connect_while_connecting_does_not_start_another_runtime() {
        let mut connection = ConnectionState::NotConnected;
        assert_eq!(request_connect(&mut connection), ConnectRequest::Start);
        assert!(matches!(connection, ConnectionState::Connecting));
        assert_eq!(request_connect(&mut connection), ConnectRequest::InProgress);
        assert!(matches!(connection, ConnectionState::Connecting));
    }

    #[test]
    fn chat_prompt_validation_is_bounded_and_preserves_input() {
        assert_eq!(
            validate_prompt(" \n\t "),
            Err(FrontendError::ChatEmptyPrompt)
        );
        assert_eq!(
            validate_prompt(&"a".repeat(MAX_PROMPT_BYTES + 1)),
            Err(FrontendError::ChatPromptTooLarge)
        );
        assert_eq!(validate_prompt("  keep these spaces  "), Ok(()));
    }

    #[test]
    fn serialized_chat_events_expose_only_the_closed_frontend_contract() {
        let event = ChatEvent::Delta {
            text: "hello".to_owned(),
        };
        let serialized = serde_json::to_string(&event).expect("chat event serializes");
        assert_eq!(serialized, r#"{"kind":"delta","text":"hello"}"#);
        assert!(!serialized.contains("session"));
        assert!(!serialized.contains("thread"));
        assert!(!serialized.contains("path"));
    }

    #[test]
    fn desktop_registry_contains_only_permission_free_echo() {
        let registry = desktop_tool_registry().expect("desktop registry should build");
        let definitions = registry.definitions();

        assert_eq!(definitions.len(), 1);
        assert_eq!(definitions[0].name.as_str(), DESKTOP_TOOL_NAME);
        assert_eq!(
            definitions[0].permission,
            rah_protocol::PermissionLevel::None
        );
    }

    #[test]
    fn serialized_activity_events_expose_only_tool_lifecycle() {
        let event = ActivityEvent::Finished {
            tool: DESKTOP_TOOL_NAME,
            result: ActivityResult::Success,
        };
        let serialized = serde_json::to_string(&event).expect("activity event serializes");

        assert_eq!(
            serialized,
            r#"{"kind":"tool_finished","tool":"echo","result":"success"}"#
        );
        for forbidden in [
            "call", "thread", "turn", "session", "request", "input", "output",
        ] {
            assert!(!serialized.contains(forbidden));
        }
    }

    #[test]
    fn failed_tool_output_is_sanitized_to_failed_activity() {
        let event = AgentEvent::ToolFinished {
            session_id: SessionId::new(),
            tool_call_id: ToolCallId::new(),
            output: ToolOutput {
                content: vec![],
                is_error: true,
            },
        };

        assert_eq!(
            activity_event(&event),
            Some(ActivityEvent::Finished {
                tool: DESKTOP_TOOL_NAME,
                result: ActivityResult::Failed,
            })
        );
    }

    #[test]
    fn chat_state_rejects_duplicate_active_turns_and_returns_to_idle() {
        let mut state = ChatState::Idle;
        assert_eq!(begin_chat(&mut state), Ok(()));
        assert_eq!(state, ChatState::Running);
        assert_eq!(
            begin_chat(&mut state),
            Err(FrontendError::ChatAlreadyRunning)
        );
        state = ChatState::Idle;
        assert_eq!(state, ChatState::Idle);
    }
}
