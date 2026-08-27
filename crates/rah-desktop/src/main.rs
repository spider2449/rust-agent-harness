#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(target_os = "windows")]
use futures::StreamExt;
#[cfg(target_os = "windows")]
use rah_protocol::{
    AgentEvent, AgentInput, AgentOptions, AgentRequest, Message, MessageRole, PermissionLevel,
    RequestId, ToolContent, ToolInput,
};
#[cfg(target_os = "windows")]
use rah_runtime::AgentRuntime;
#[cfg(target_os = "windows")]
use rah_runtime_codex::{CodexAdapterError, CodexRuntime, SUPPORTED_CODEX_VERSION};
#[cfg(target_os = "windows")]
use rah_tools::{
    EchoTool, FsReadTool, RepositoryDiffStagedTool, RepositoryDiffTool, RepositoryFileCreationTool,
    RepositoryFileInfoTool, RepositoryMultiFileEditTool, RepositoryStatusTool,
    RepositoryWorktreePatchTool, Tool, ToolContext, ToolError, ToolRegistry,
};
#[cfg(target_os = "windows")]
use serde::Serialize;
#[cfg(target_os = "windows")]
use std::{
    collections::HashMap,
    ffi::OsString,
    path::PathBuf,
    process::ExitCode,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
#[cfg(target_os = "windows")]
use tauri::{AppHandle, Emitter, Manager, State, WindowEvent};
#[cfg(target_os = "windows")]
use tauri_plugin_dialog::DialogExt;

#[cfg(target_os = "windows")]
const MAX_PROMPT_BYTES: usize = 32 * 1024;
#[cfg(target_os = "windows")]
const DESKTOP_FS_READ_MAX_BYTES: usize = 1024 * 1024;
#[cfg(all(test, target_os = "windows"))]
const DESKTOP_TOOL_NAME: &str = "echo";
#[cfg(target_os = "windows")]
const MAX_TURN_TOOL_CALLS: usize = 64;

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
    repository_tools_status: &'static str,
}

#[cfg(target_os = "windows")]
enum ConnectionState {
    NotConnected,
    Connecting,
    Connected {
        runtime: Arc<CodexRuntime>,
        repository_generation: u64,
    },
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
        ConnectionState::Connected { .. } => ConnectRequest::AlreadyConnected,
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
    repository: Mutex<Option<Arc<DesktopRepository>>>,
    repository_generation: Mutex<u64>,
    close_started: AtomicBool,
}

#[cfg(target_os = "windows")]
impl Default for DesktopAppState {
    fn default() -> Self {
        Self {
            connection: Mutex::new(ConnectionState::NotConnected),
            chat: Mutex::new(ChatState::Idle),
            repository: Mutex::new(None),
            repository_generation: Mutex::new(0),
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
        let repository_selected = self
            .repository
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_some();
        let repository_generation = *self
            .repository_generation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        current_app_status(&connection, repository_selected, repository_generation)
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
                ConnectionState::Connected { runtime, .. } => Some(runtime),
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
struct DesktopRepository {
    display_path: String,
    git_executable: PathBuf,
    root: PathBuf,
    status: Arc<RepositoryStatusTool>,
    worktree_diff: Arc<RepositoryDiffTool>,
    staged_diff: Arc<RepositoryDiffStagedTool>,
}

#[cfg(target_os = "windows")]
impl DesktopRepository {
    fn new(
        git_executable: &std::path::Path,
        repository_root: &std::path::Path,
    ) -> Result<Self, ToolError> {
        let status = RepositoryStatusTool::new(git_executable, repository_root)?;
        let worktree_diff = RepositoryDiffTool::new(git_executable, repository_root)?;
        let staged_diff = RepositoryDiffStagedTool::new(git_executable, repository_root)?;
        Ok(Self {
            display_path: repository_root.display().to_string(),
            git_executable: git_executable.to_path_buf(),
            root: repository_root.to_path_buf(),
            status: Arc::new(status),
            worktree_diff: Arc::new(worktree_diff),
            staged_diff: Arc::new(staged_diff),
        })
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
    GitUnavailable,
    RepositoryNotSelected,
    RepositoryInvalid,
    RepositoryObservationFailed,
    RepositoryDialogFailed,
    RepositoryBusy,
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
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum ActivityEvent {
    #[serde(rename = "tool_requested")]
    Requested { tool: String },
    #[serde(rename = "tool_started")]
    Started { tool: String },
    #[serde(rename = "tool_finished")]
    Finished {
        tool: String,
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
        CodexAdapterError::InvalidModelProviderConfig { .. } => {
            FrontendError::CodexConnectionFailed
        }
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
fn current_app_status(
    connection: &ConnectionState,
    repository_selected: bool,
    repository_generation: u64,
) -> AppStatus {
    let (runtime_status, codex_status, codex_version, codex_error) = match connection {
        ConnectionState::NotConnected => ("not connected", "not connected", None, None),
        ConnectionState::Error(error) => ("not connected", "error", None, Some(*error)),
        ConnectionState::Connecting => ("not connected", "connecting", None, None),
        ConnectionState::Connected { .. } => (
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
        repository_status: if repository_selected {
            "selected"
        } else {
            "not selected"
        },
        repository_tools_status: repository_tool_authority(
            repository_selected,
            match connection {
                ConnectionState::Connected {
                    repository_generation,
                    ..
                } => Some(*repository_generation),
                _ => None,
            },
            repository_generation,
        ),
    }
}

#[cfg(target_os = "windows")]
fn repository_tool_authority(
    repository_selected: bool,
    connection_generation: Option<u64>,
    repository_generation: u64,
) -> &'static str {
    match (repository_selected, connection_generation) {
        (true, Some(connection_generation)) if connection_generation == repository_generation => {
            "active"
        }
        (true, Some(_)) => "reconnect required",
        _ => "inactive",
    }
}

#[cfg(target_os = "windows")]
#[tauri::command]
fn app_status(state: State<'_, DesktopAppState>) -> AppStatus {
    state.status()
}

#[cfg(target_os = "windows")]
fn selected_git_executable() -> Result<std::path::PathBuf, FrontendError> {
    let Some(path) = std::env::var_os("RAH_GIT_EXECUTABLE") else {
        return Err(FrontendError::GitUnavailable);
    };
    let path = std::path::PathBuf::from(path);
    if !path.is_absolute() {
        return Err(FrontendError::GitUnavailable);
    }
    Ok(path)
}

#[cfg(target_os = "windows")]
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct RepositorySnapshot {
    path: String,
    status_entries: Vec<RepositoryStatusEntry>,
    worktree_diff: Vec<RepositoryDiffFile>,
    staged_diff: Vec<RepositoryDiffFile>,
}

#[cfg(target_os = "windows")]
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct RepositoryStatusEntry {
    path: String,
    previous_path: Option<String>,
    tracked: bool,
    index_state: String,
    worktree_state: String,
    conflict_state: String,
}

#[cfg(target_os = "windows")]
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct RepositoryDiffFile {
    old_path: Option<String>,
    new_path: Option<String>,
    change_kind: String,
    binary: bool,
    added_lines: Option<u64>,
    deleted_lines: Option<u64>,
    patch: Option<String>,
}

#[cfg(target_os = "windows")]
fn desktop_tagged_text(value: &serde_json::Value) -> Option<String> {
    let object = value.as_object()?;
    match object.get("encoding")?.as_str()? {
        "utf8" => object.get("value")?.as_str().map(str::to_owned),
        _ => Some("[non-utf8 value]".to_owned()),
    }
}

#[cfg(target_os = "windows")]
fn observer_json(output: rah_protocol::ToolOutput) -> Result<serde_json::Value, FrontendError> {
    match output.content.as_slice() {
        [ToolContent::Json(value)] if !output.is_error => Ok(value.clone()),
        _ => Err(FrontendError::RepositoryObservationFailed),
    }
}

#[cfg(target_os = "windows")]
fn desktop_snapshot(
    path: String,
    status: rah_protocol::ToolOutput,
    worktree_diff: rah_protocol::ToolOutput,
    staged_diff: rah_protocol::ToolOutput,
) -> Result<RepositorySnapshot, FrontendError> {
    let status = observer_json(status)?;
    let worktree_diff = observer_json(worktree_diff)?;
    let staged_diff = observer_json(staged_diff)?;
    let status_entries = status
        .get("entries")
        .and_then(serde_json::Value::as_array)
        .ok_or(FrontendError::RepositoryObservationFailed)?
        .iter()
        .map(|entry| {
            Ok(RepositoryStatusEntry {
                path: entry
                    .get("path")
                    .and_then(desktop_tagged_text)
                    .ok_or(FrontendError::RepositoryObservationFailed)?,
                previous_path: entry.get("previous_path").and_then(|value| {
                    if value.is_null() {
                        None
                    } else {
                        desktop_tagged_text(value)
                    }
                }),
                tracked: entry
                    .get("tracked")
                    .and_then(serde_json::Value::as_bool)
                    .ok_or(FrontendError::RepositoryObservationFailed)?,
                index_state: entry
                    .get("index_state")
                    .and_then(serde_json::Value::as_str)
                    .ok_or(FrontendError::RepositoryObservationFailed)?
                    .to_owned(),
                worktree_state: entry
                    .get("worktree_state")
                    .and_then(serde_json::Value::as_str)
                    .ok_or(FrontendError::RepositoryObservationFailed)?
                    .to_owned(),
                conflict_state: entry
                    .get("conflict_state")
                    .and_then(serde_json::Value::as_str)
                    .ok_or(FrontendError::RepositoryObservationFailed)?
                    .to_owned(),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let convert_diff =
        |value: serde_json::Value| -> Result<Vec<RepositoryDiffFile>, FrontendError> {
            value
                .get("files")
                .and_then(serde_json::Value::as_array)
                .ok_or(FrontendError::RepositoryObservationFailed)?
                .iter()
                .map(|file| {
                    Ok(RepositoryDiffFile {
                        old_path: file.get("old_path").and_then(|value| {
                            if value.is_null() {
                                None
                            } else {
                                desktop_tagged_text(value)
                            }
                        }),
                        new_path: file.get("new_path").and_then(|value| {
                            if value.is_null() {
                                None
                            } else {
                                desktop_tagged_text(value)
                            }
                        }),
                        change_kind: file
                            .get("change_kind")
                            .and_then(serde_json::Value::as_str)
                            .ok_or(FrontendError::RepositoryObservationFailed)?
                            .to_owned(),
                        binary: file
                            .get("binary")
                            .and_then(serde_json::Value::as_bool)
                            .ok_or(FrontendError::RepositoryObservationFailed)?,
                        added_lines: file.get("added_lines").and_then(serde_json::Value::as_u64),
                        deleted_lines: file
                            .get("deleted_lines")
                            .and_then(serde_json::Value::as_u64),
                        patch: file.get("patch").and_then(|value| {
                            if value.is_null() {
                                None
                            } else {
                                desktop_tagged_text(value)
                            }
                        }),
                    })
                })
                .collect()
        };
    Ok(RepositorySnapshot {
        path,
        status_entries,
        worktree_diff: convert_diff(worktree_diff)?,
        staged_diff: convert_diff(staged_diff)?,
    })
}

#[cfg(target_os = "windows")]
#[tauri::command]
fn choose_repository(
    app: AppHandle,
    state: State<'_, DesktopAppState>,
) -> Result<(), FrontendError> {
    repository_selection_allowed(
        *state
            .chat
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
    )?;
    let selected = app.dialog().file().blocking_pick_folder();
    let Some(selected) = selected else {
        return Ok(());
    };
    let path = selected
        .into_path()
        .map_err(|_| FrontendError::RepositoryDialogFailed)?;
    let git = selected_git_executable()?;
    let repository = DesktopRepository::new(&git, &path).map_err(|error| {
        tracing::warn!(error = %error, "selected repository is invalid");
        FrontendError::RepositoryInvalid
    })?;
    repository_selection_allowed(
        *state
            .chat
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
    )?;
    *state
        .repository
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(Arc::new(repository));
    *state
        .repository_generation
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) += 1;
    Ok(())
}

#[cfg(target_os = "windows")]
#[tauri::command]
async fn repository_snapshot(
    state: State<'_, DesktopAppState>,
) -> Result<RepositorySnapshot, FrontendError> {
    let repository = state
        .repository
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    let Some(repository) = repository else {
        return Err(FrontendError::RepositoryNotSelected);
    };
    let input = ToolInput(serde_json::json!({}));
    let status = repository
        .status
        .execute(input.clone(), ToolContext::default())
        .await;
    let worktree_diff = repository
        .worktree_diff
        .execute(input.clone(), ToolContext::default())
        .await;
    let staged_diff = repository
        .staged_diff
        .execute(input, ToolContext::default())
        .await;
    let (status, worktree_diff, staged_diff) = match (status, worktree_diff, staged_diff) {
        (Ok(status), Ok(worktree_diff), Ok(staged_diff)) => (status, worktree_diff, staged_diff),
        _ => return Err(FrontendError::RepositoryObservationFailed),
    };
    desktop_snapshot(
        repository.display_path.clone(),
        status,
        worktree_diff,
        staged_diff,
    )
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
fn desktop_tool_registry(
    repository: Option<&DesktopRepository>,
) -> Result<Arc<ToolRegistry>, ToolError> {
    let mut registry = ToolRegistry::new();
    registry.register(Arc::new(EchoTool::new()))?;
    if let Some(repository) = repository {
        let fs_read =
            FsReadTool::new(&repository.root, DESKTOP_FS_READ_MAX_BYTES).map_err(|error| {
                ToolError::Execution {
                    message: error.to_string(),
                }
            })?;
        let file_info = RepositoryFileInfoTool::new(&repository.git_executable, &repository.root)?;
        let status = RepositoryStatusTool::new(&repository.git_executable, &repository.root)?;
        let diff = RepositoryDiffTool::new(&repository.git_executable, &repository.root)?;
        let diff_staged =
            RepositoryDiffStagedTool::new(&repository.git_executable, &repository.root)?;
        let patch = RepositoryWorktreePatchTool::new(&repository.git_executable, &repository.root)?;
        let create_file =
            RepositoryFileCreationTool::new(&repository.git_executable, &repository.root)?;
        let edit_files =
            RepositoryMultiFileEditTool::new(&repository.git_executable, &repository.root)?;
        registry.register(Arc::new(fs_read))?;
        registry.register(Arc::new(file_info))?;
        registry.register(Arc::new(status))?;
        registry.register(Arc::new(diff))?;
        registry.register(Arc::new(diff_staged))?;
        registry.register(Arc::new(patch))?;
        registry.register(Arc::new(create_file))?;
        registry.register(Arc::new(edit_files))?;
    }
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

    let (repository, repository_generation) = {
        let repository = state
            .repository
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        let generation = *state
            .repository_generation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        (repository, generation)
    };
    let registry = match desktop_tool_registry(repository.as_deref()) {
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
        if repository.is_some() {
            vec![
                PermissionLevel::None,
                PermissionLevel::Read,
                PermissionLevel::Execute,
            ]
        } else {
            vec![PermissionLevel::None]
        },
    )
    .await
    {
        Ok(runtime) => {
            let mut connection = state
                .connection
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            *connection = ConnectionState::Connected {
                runtime: Arc::new(runtime),
                repository_generation,
            };
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
            ConnectionState::Connected { runtime, .. } => Some(runtime),
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
fn repository_selection_allowed(chat: ChatState) -> Result<(), FrontendError> {
    if chat == ChatState::Running {
        Err(FrontendError::RepositoryBusy)
    } else {
        Ok(())
    }
}

#[cfg(target_os = "windows")]
fn emit_chat_event(app: &AppHandle, event: ChatEvent) {
    if let Err(error) = app.emit("chat_event", event) {
        tracing::warn!(error = %error, "failed to emit desktop chat event");
    }
}

#[cfg(target_os = "windows")]
fn activity_event(
    event: &AgentEvent,
    tool_calls: &mut HashMap<rah_protocol::ToolCallId, String>,
) -> Option<(ActivityEvent, bool)> {
    match event {
        AgentEvent::ToolRequested { tool_call, .. } => {
            if tool_calls.len() >= MAX_TURN_TOOL_CALLS {
                return None;
            }
            let tool = tool_call.name.as_str().to_owned();
            tool_calls.insert(tool_call.id.clone(), tool.clone());
            Some((ActivityEvent::Requested { tool }, false))
        }
        AgentEvent::ToolStarted { tool_call_id, .. } => tool_calls
            .get(tool_call_id)
            .cloned()
            .map(|tool| (ActivityEvent::Started { tool }, false)),
        AgentEvent::ToolFinished {
            tool_call_id,
            output,
            ..
        } => tool_calls.remove(tool_call_id).map(|tool| {
            let refresh = matches!(
                tool.as_str(),
                "repo.patch" | "repo.create-file" | "repo.edit-files"
            );
            (
                ActivityEvent::Finished {
                    tool,
                    result: if output.is_error {
                        ActivityResult::Failed
                    } else {
                        ActivityResult::Success
                    },
                },
                refresh,
            )
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
fn emit_repository_refresh(app: &AppHandle) {
    if let Err(error) = app.emit("repository_snapshot_refresh", ()) {
        tracing::warn!(error = %error, "failed to request desktop repository refresh");
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
    let mut tool_calls = HashMap::new();
    let mut events = handle.into_events();
    while let Some(event) = events.next().await {
        if let Some((activity, refresh_repository)) = activity_event(&event, &mut tool_calls) {
            emit_activity_event(&app, activity);
            if refresh_repository {
                emit_repository_refresh(&app);
            }
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
            ConnectionState::Connected { runtime, .. } => Arc::clone(runtime),
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
        .plugin(tauri_plugin_dialog::init())
        .manage(DesktopAppState::default())
        .invoke_handler(tauri::generate_handler![
            app_status,
            choose_repository,
            connect_codex,
            disconnect_codex,
            repository_snapshot,
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
        DESKTOP_TOOL_NAME, DesktopRepository, FrontendError, MAX_PROMPT_BYTES, activity_event,
        begin_chat, current_app_status, desktop_tool_registry, frontend_error,
        repository_selection_allowed, repository_tool_authority, request_connect, validate_prompt,
    };
    use rah_protocol::{
        AgentEvent, PermissionLevel, SessionId, ToolCall, ToolCallId, ToolInput, ToolName,
        ToolOutput,
    };
    use rah_runtime_codex::CodexAdapterError;
    use rah_tools::ToolContext;
    use std::{
        collections::HashMap,
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    static NEXT_TEST_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    struct TestRepository(PathBuf);

    impl TestRepository {
        fn new() -> Self {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time should follow Unix epoch")
                .as_nanos();
            let sequence = NEXT_TEST_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let root = std::env::temp_dir()
                .join(format!("rah-desktop-tool-registry-{timestamp}-{sequence}"));
            fs::create_dir(&root).expect("test repository root should be created");
            fs::create_dir(root.join(".git")).expect("test repository metadata should be created");
            fs::write(root.join("inside.txt"), "inside").expect("test file should be written");
            Self(root)
        }

        fn desktop_repository(&self) -> DesktopRepository {
            let executable =
                std::env::current_exe().expect("current test executable should be available");
            DesktopRepository::new(&executable, &self.0).expect("test repository should construct")
        }
    }

    impl Drop for TestRepository {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn status_contains_only_the_desktop_application_state() {
        let status = current_app_status(&ConnectionState::NotConnected, false, 0);

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
        assert_eq!(status.repository_tools_status, "inactive");

        let serialized = serde_json::to_string(&status).expect("status serializes");
        assert!(!serialized.contains('\\'));
        assert!(!serialized.contains('/'));
    }

    #[test]
    fn status_reflects_connection_transitions_without_exposing_runtime_details() {
        let connecting = current_app_status(&ConnectionState::Connecting, false, 0);
        assert_eq!(connecting.runtime_status, "not connected");
        assert_eq!(connecting.codex_status, "connecting");
        assert_eq!(connecting.codex_version, None);
        assert_eq!(connecting.codex_error, None);

        let error = current_app_status(
            &ConnectionState::Error(FrontendError::CodexConnectionFailed),
            false,
            0,
        );
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
    fn repository_status_is_dynamic_without_exposing_repository_details() {
        let status = current_app_status(&ConnectionState::NotConnected, true, 1);
        assert_eq!(status.repository_status, "selected");
        let serialized = serde_json::to_string(&status).expect("status serializes");
        assert!(!serialized.contains("git.exe"));
    }

    #[test]
    fn repository_errors_are_sanitized_for_the_frontend() {
        for error in [
            FrontendError::GitUnavailable,
            FrontendError::RepositoryNotSelected,
            FrontendError::RepositoryInvalid,
            FrontendError::RepositoryObservationFailed,
            FrontendError::RepositoryDialogFailed,
        ] {
            let serialized = serde_json::to_string(&error).expect("repository error serializes");
            assert!(!serialized.contains("stderr"));
            assert!(!serialized.contains("path"));
            assert!(!serialized.contains("environment"));
        }
    }

    #[test]
    fn repository_presentation_preserves_utf8_and_sanitizes_non_utf8_tags() {
        let utf8 = serde_json::json!({"encoding":"utf8", "value":"src/main.rs"});
        let non_utf8 = serde_json::json!({"encoding":"base64", "value":"YmFk/w=="});
        assert_eq!(
            super::desktop_tagged_text(&utf8),
            Some("src/main.rs".to_owned())
        );
        assert_eq!(
            super::desktop_tagged_text(&non_utf8),
            Some("[non-utf8 value]".to_owned())
        );
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
        let registry = desktop_tool_registry(None).expect("desktop registry should build");
        let definitions = registry.definitions();

        assert_eq!(definitions.len(), 1);
        assert_eq!(definitions[0].name.as_str(), DESKTOP_TOOL_NAME);
        assert_eq!(
            definitions[0].permission,
            rah_protocol::PermissionLevel::None
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn selected_repository_registry_has_only_the_intended_bounded_tools() {
        let fixture = TestRepository::new();
        let repository = fixture.desktop_repository();
        let registry = desktop_tool_registry(Some(&repository)).expect("registry should build");
        let definitions = registry.definitions();
        let names = definitions
            .iter()
            .map(|definition| definition.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            names,
            vec![
                "echo",
                "fs.read",
                "repo.create-file",
                "repo.diff",
                "repo.diff-staged",
                "repo.edit-files",
                "repo.file-info",
                "repo.patch",
                "repo.status",
            ]
        );
        let permissions = definitions
            .iter()
            .map(|definition| definition.permission)
            .collect::<Vec<_>>();
        assert!(permissions.contains(&PermissionLevel::None));
        assert!(permissions.contains(&PermissionLevel::Read));
        assert!(permissions.contains(&PermissionLevel::Execute));
        assert_eq!(
            permissions
                .iter()
                .filter(|&&permission| permission == PermissionLevel::None)
                .count(),
            1
        );
        assert_eq!(
            permissions
                .iter()
                .filter(|&&permission| permission == PermissionLevel::Read)
                .count(),
            1
        );
        assert_eq!(
            permissions
                .iter()
                .filter(|&&permission| permission == PermissionLevel::Execute)
                .count(),
            7
        );
        let output = registry
            .execute(
                ToolCall {
                    id: ToolCallId::new(),
                    name: ToolName::new("fs.read"),
                    input: ToolInput(serde_json::json!({"path": "inside.txt"})),
                },
                ToolContext::default(),
            )
            .await
            .expect("fs.read should use the selected repository root");
        assert!(!output.is_error);
        let outside = registry
            .execute(
                ToolCall {
                    id: ToolCallId::new(),
                    name: ToolName::new("fs.read"),
                    input: ToolInput(serde_json::json!({"path": "../outside.txt"})),
                },
                ToolContext::default(),
            )
            .await;
        assert!(
            outside.is_err(),
            "fs.read must not escape the selected root"
        );
    }

    #[test]
    fn repository_authority_requires_reconnect_after_selection_generation_changes() {
        assert_eq!(repository_tool_authority(false, None, 0), "inactive");
        assert_eq!(repository_tool_authority(true, Some(4), 4), "active");
        assert_eq!(
            repository_tool_authority(true, Some(4), 5),
            "reconnect required"
        );
    }

    #[test]
    fn repository_tool_construction_failure_returns_no_partial_registry() {
        let fixture = TestRepository::new();
        let mut repository = fixture.desktop_repository();
        repository.root = fixture.0.join("missing-root");
        assert!(desktop_tool_registry(Some(&repository)).is_err());
    }

    #[test]
    fn repository_selection_is_blocked_while_chat_is_running() {
        assert_eq!(repository_selection_allowed(ChatState::Idle), Ok(()));
        assert_eq!(
            repository_selection_allowed(ChatState::Running),
            Err(FrontendError::RepositoryBusy)
        );
    }

    #[test]
    fn serialized_activity_events_expose_only_tool_lifecycle() {
        let event = ActivityEvent::Finished {
            tool: DESKTOP_TOOL_NAME.to_owned(),
            result: ActivityResult::Success,
        };
        let serialized = serde_json::to_string(&event).expect("activity event serializes");

        assert_eq!(
            serialized,
            r#"{"kind":"tool_finished","tool":"echo","result":"success"}"#
        );
        for forbidden in [
            "tool_call_id",
            "thread_id",
            "turn_id",
            "session_id",
            "request_id",
            "input",
            "output",
        ] {
            assert!(!serialized.contains(forbidden));
        }
    }

    #[test]
    fn activity_uses_actual_tool_name_and_hides_internal_call_identity() {
        let id = ToolCallId::new();
        let requested = AgentEvent::ToolRequested {
            session_id: SessionId::new(),
            tool_call: ToolCall {
                id: id.clone(),
                name: ToolName::new("repo.edit-files"),
                input: ToolInput(serde_json::json!({})),
            },
        };
        let started_id = id.clone();
        let finished = AgentEvent::ToolFinished {
            session_id: SessionId::new(),
            tool_call_id: id,
            output: ToolOutput {
                content: vec![],
                is_error: true,
            },
        };
        let mut calls = HashMap::new();
        let requested = activity_event(&requested, &mut calls).expect("requested activity");
        assert_eq!(
            requested.0,
            ActivityEvent::Requested {
                tool: "repo.edit-files".to_owned()
            }
        );
        assert_eq!(
            activity_event(
                &AgentEvent::ToolStarted {
                    session_id: SessionId::new(),
                    tool_call_id: started_id,
                },
                &mut calls,
            ),
            Some((
                ActivityEvent::Started {
                    tool: "repo.edit-files".to_owned(),
                },
                false,
            ))
        );
        assert_eq!(
            activity_event(&finished, &mut calls),
            Some((
                ActivityEvent::Finished {
                    tool: "repo.edit-files".to_owned(),
                    result: ActivityResult::Failed,
                },
                true,
            ))
        );
        let serialized = serde_json::to_string(&requested.0).expect("activity serializes");
        assert_eq!(
            serialized,
            r#"{"kind":"tool_requested","tool":"repo.edit-files"}"#
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
