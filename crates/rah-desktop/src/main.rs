#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(target_os = "windows")]
use rah_runtime_codex::{CodexAdapterError, CodexRuntime, SUPPORTED_CODEX_VERSION};
#[cfg(target_os = "windows")]
use serde::Serialize;
#[cfg(target_os = "windows")]
use std::{
    ffi::OsString,
    process::ExitCode,
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
#[cfg(target_os = "windows")]
use tauri::{Manager, State, WindowEvent};

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
    Connected(CodexRuntime),
    Disconnecting,
    Error(FrontendError),
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
    close_started: AtomicBool,
}

#[cfg(target_os = "windows")]
impl Default for DesktopAppState {
    fn default() -> Self {
        Self {
            connection: Mutex::new(ConnectionState::NotConnected),
            close_started: AtomicBool::new(false),
        }
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

    match CodexRuntime::connect(selected_codex_executable()).await {
        Ok(runtime) => {
            let mut connection = state
                .connection
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            *connection = ConnectionState::Connected(runtime);
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
fn main() -> ExitCode {
    match tauri::Builder::default()
        .manage(DesktopAppState::default())
        .invoke_handler(tauri::generate_handler![
            app_status,
            connect_codex,
            disconnect_codex
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
        ConnectRequest, ConnectionState, FrontendError, current_app_status, frontend_error,
        request_connect,
    };
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
}
