#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(target_os = "windows")]
mod codex_baseline;
#[cfg(target_os = "windows")]
mod conversation_persistence;

#[cfg(target_os = "windows")]
use codex_baseline::{BaselineError, CodexExecutableSource, resolve as resolve_codex_executable};
#[cfg(target_os = "windows")]
use conversation_persistence::{
    Persistence, Presentation as ConversationTranscriptPresentation, ResumeError, ResumePair,
    SeparatorReason, Warning as ConversationPersistenceWarning,
};
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
use rah_runtime_codex::{
    CodexAdapterError, CodexLlamaCppProvider, CodexModelConfig, CodexModelProvider,
    CodexModelSelection, CodexRuntime, SUPPORTED_CODEX_VERSION,
};
#[cfg(target_os = "windows")]
use rah_tools::{
    EchoTool, FsReadTool, RepositoryDiffStagedTool, RepositoryDiffTool, RepositoryFileCreationTool,
    RepositoryFileInfoTool, RepositoryMultiFileEditTool, RepositoryStatusTool,
    RepositoryWorktreePatchTool, Tool, ToolContext, ToolError, ToolRegistry,
};
#[cfg(target_os = "windows")]
use serde::{Deserialize, Serialize};
#[cfg(target_os = "windows")]
use std::{
    collections::HashMap,
    future::Future,
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
const MAX_CONVERSATION_REPLAY_MESSAGES: usize = 8;
#[cfg(target_os = "windows")]
const MAX_CONVERSATION_REPLAY_BYTES: usize = 32 * 1024;
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
    codex_source: Option<CodexExecutableSourcePresentation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    codex_error: Option<FrontendError>,
    profile_status: &'static str,
    repository_status: &'static str,
    repository_tools_status: &'static str,
    model_configuration_status: &'static str,
}

#[cfg(target_os = "windows")]
enum ConnectionState {
    NotConnected,
    Connecting,
    Connected {
        runtime: Arc<CodexRuntime>,
        source: CodexExecutableSource,
        repository_generation: u64,
        model_generation: u64,
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
    model: Mutex<DesktopModelState>,
    conversation: Mutex<DesktopConversationState>,
    persistence: Mutex<Persistence>,
    close_started: AtomicBool,
}

#[cfg(target_os = "windows")]
impl DesktopAppState {
    fn persist_separator(
        &self,
        reason: SeparatorReason,
    ) -> Result<(), ConversationPersistenceWarning> {
        self.persistence
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .append_separator(reason)
    }

    fn persist_completed_pair(
        &self,
        prompt: String,
        assistant: String,
    ) -> Result<(), ConversationPersistenceWarning> {
        self.persistence
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .append_pair(prompt, assistant)
    }

    fn new(storage_directory: PathBuf) -> Self {
        Self {
            connection: Mutex::new(ConnectionState::NotConnected),
            chat: Mutex::new(ChatState::Idle),
            repository: Mutex::new(None),
            repository_generation: Mutex::new(0),
            model: Mutex::new(DesktopModelState::default()),
            conversation: Mutex::new(DesktopConversationState::default()),
            persistence: Mutex::new(Persistence::start(storage_directory)),
            close_started: AtomicBool::new(false),
        }
    }
}

#[cfg(target_os = "windows")]
fn emit_persistence_warning(app: &AppHandle, warning: ConversationPersistenceWarning) {
    if let Err(error) = app.emit("conversation_persistence_warning", warning) {
        tracing::warn!(error = %error, "failed to emit conversation persistence warning");
    }
}

#[cfg(target_os = "windows")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ConversationContextIdentity {
    repository_generation: u64,
    model_generation: u64,
}

#[cfg(target_os = "windows")]
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum ConversationContextChange {
    #[serde(rename = "repository_changed")]
    Repository,
    #[serde(rename = "model_configuration_changed")]
    ModelConfiguration,
    #[serde(rename = "repository_and_model_changed")]
    RepositoryAndModel,
}

#[cfg(target_os = "windows")]
#[derive(Debug, Default)]
struct DesktopConversationState {
    identity: Option<ConversationContextIdentity>,
    history: Vec<Message>,
    epoch: u64,
}

#[cfg(target_os = "windows")]
impl DesktopConversationState {
    fn reconcile(
        &mut self,
        identity: ConversationContextIdentity,
    ) -> Option<ConversationContextChange> {
        let Some(previous) = self.identity else {
            self.identity = Some(identity);
            return None;
        };
        if previous == identity {
            return None;
        }
        self.history.clear();
        self.identity = Some(identity);
        self.epoch = self.epoch.wrapping_add(1);
        Some(
            match (
                previous.repository_generation != identity.repository_generation,
                previous.model_generation != identity.model_generation,
            ) {
                (true, true) => ConversationContextChange::RepositoryAndModel,
                (true, false) => ConversationContextChange::Repository,
                (false, true) => ConversationContextChange::ModelConfiguration,
                (false, false) => unreachable!("changed conversation identity must differ"),
            },
        )
    }

    fn start_new(&mut self) {
        self.history.clear();
        self.identity = None;
        self.epoch = self.epoch.wrapping_add(1);
    }

    fn request_messages(&self, prompt: &str) -> Result<Vec<Message>, FrontendError> {
        if self.history.len() > MAX_CONVERSATION_REPLAY_MESSAGES
            || self
                .history
                .iter()
                .map(|message| message.content.len())
                .sum::<usize>()
                > MAX_CONVERSATION_REPLAY_BYTES
        {
            return Err(FrontendError::ConversationContextLimit);
        }
        let mut messages = self.history.clone();
        messages.push(Message {
            role: MessageRole::User,
            content: prompt.to_owned(),
        });
        Ok(messages)
    }

    fn commit(&mut self, epoch: u64, prompt: String, output: Message) -> Result<(), ()> {
        if self.epoch != epoch || output.role != MessageRole::Assistant {
            return Err(());
        }
        self.history.push(Message {
            role: MessageRole::User,
            content: prompt,
        });
        self.history.push(output);
        Ok(())
    }

    fn resume(
        &mut self,
        identity: ConversationContextIdentity,
        pairs: Vec<ResumePair>,
    ) -> Result<(), FrontendError> {
        if !self.history.is_empty() {
            return Err(FrontendError::ConversationResumeUnavailable);
        }
        let messages = pairs
            .into_iter()
            .flat_map(|pair| {
                [
                    Message {
                        role: MessageRole::User,
                        content: pair.user,
                    },
                    Message {
                        role: MessageRole::Assistant,
                        content: pair.assistant,
                    },
                ]
            })
            .collect::<Vec<_>>();
        if messages.is_empty()
            || messages.len() > MAX_CONVERSATION_REPLAY_MESSAGES
            || messages
                .iter()
                .map(|message| message.content.len())
                .sum::<usize>()
                > MAX_CONVERSATION_REPLAY_BYTES
            || messages.iter().enumerate().any(|(index, message)| {
                message.role
                    != if index % 2 == 0 {
                        MessageRole::User
                    } else {
                        MessageRole::Assistant
                    }
            })
        {
            return Err(FrontendError::ConversationResumeTooLarge);
        }
        self.identity = Some(identity);
        self.history = messages;
        self.epoch = self.epoch.wrapping_add(1);
        Ok(())
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
        let model_generation = self
            .model
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .generation;
        current_app_status(
            &connection,
            repository_selected,
            repository_generation,
            model_generation,
        )
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
    CodexBaselineInvalid,
    CodexHostUnsupported,
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
    ConversationContextLimit,
    ConversationHistoryBusy,
    ConversationHistoryClearFailed,
    ConversationResumeUnavailable,
    ConversationResumeBusy,
    ConversationResumeReconnectRequired,
    ConversationResumeTooLarge,
    ConversationResumePersistenceFailed,
    ConversationResumePersistenceIncompatible,
    GitUnavailable,
    RepositoryNotSelected,
    RepositoryInvalid,
    RepositoryObservationFailed,
    RepositoryDialogFailed,
    RepositoryBusy,
    ModelConfigurationInvalid,
    ModelConfigurationBusy,
}

#[cfg(target_os = "windows")]
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum CodexExecutableSourcePresentation {
    Override,
    CertifiedBaseline,
    Path,
}

#[cfg(target_os = "windows")]
impl From<CodexExecutableSource> for CodexExecutableSourcePresentation {
    fn from(source: CodexExecutableSource) -> Self {
        match source {
            CodexExecutableSource::Override => Self::Override,
            CodexExecutableSource::CertifiedBaseline => Self::CertifiedBaseline,
            CodexExecutableSource::Path => Self::Path,
        }
    }
}

#[cfg(target_os = "windows")]
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum DesktopModelProvider {
    Inherit,
    #[serde(rename = "openai")]
    OpenAi,
    Ollama,
    LmStudio,
    LlamaCpp,
}

#[cfg(target_os = "windows")]
#[derive(Clone, Debug, PartialEq, Eq)]
struct DesktopModelSelection {
    provider: DesktopModelProvider,
    model: Option<String>,
}

#[cfg(target_os = "windows")]
impl Default for DesktopModelSelection {
    fn default() -> Self {
        Self {
            provider: DesktopModelProvider::Inherit,
            model: None,
        }
    }
}

#[cfg(target_os = "windows")]
impl DesktopModelSelection {
    fn codex_model_config(&self) -> Result<CodexModelConfig, FrontendError> {
        let provider = match self.provider {
            DesktopModelProvider::Inherit => {
                return if self.model.is_none() {
                    Ok(CodexModelConfig::Inherit)
                } else {
                    Err(FrontendError::ModelConfigurationInvalid)
                };
            }
            DesktopModelProvider::OpenAi => CodexModelProvider::OpenAi,
            DesktopModelProvider::Ollama => CodexModelProvider::Ollama,
            DesktopModelProvider::LmStudio => CodexModelProvider::LmStudio,
            DesktopModelProvider::LlamaCpp => {
                CodexModelProvider::LlamaCpp(CodexLlamaCppProvider::default_local())
            }
        };
        let model = self
            .model
            .as_deref()
            .ok_or(FrontendError::ModelConfigurationInvalid)?;
        CodexModelSelection::new(model, provider)
            .map(CodexModelConfig::Explicit)
            .map_err(|_| FrontendError::ModelConfigurationInvalid)
    }
}

#[cfg(target_os = "windows")]
#[derive(Debug, Default)]
struct DesktopModelState {
    selection: DesktopModelSelection,
    generation: u64,
}

#[cfg(target_os = "windows")]
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct ModelConfigurationPresentation {
    provider: DesktopModelProvider,
    model: Option<String>,
    status: &'static str,
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
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct SendChatResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    context_change: Option<ConversationContextChange>,
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
    model_generation: u64,
) -> AppStatus {
    let (runtime_status, codex_status, codex_version, codex_source, codex_error) = match connection
    {
        ConnectionState::NotConnected => ("not connected", "not connected", None, None, None),
        ConnectionState::Error(error) => ("not connected", "error", None, None, Some(*error)),
        ConnectionState::Connecting => ("not connected", "connecting", None, None, None),
        ConnectionState::Connected { source, .. } => (
            "connected",
            "connected",
            Some(SUPPORTED_CODEX_VERSION),
            Some((*source).into()),
            None,
        ),
        ConnectionState::Disconnecting => ("connected", "disconnecting", None, None, None),
    };
    AppStatus {
        app_name: "RAH",
        app_version: env!("CARGO_PKG_VERSION"),
        platform: "windows",
        desktop_shell: "ready",
        runtime_status,
        codex_status,
        codex_version,
        codex_source,
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
        model_configuration_status: model_configuration_status(
            match connection {
                ConnectionState::Connected {
                    model_generation, ..
                } => Some(*model_generation),
                _ => None,
            },
            model_generation,
        ),
    }
}

#[cfg(target_os = "windows")]
fn model_configuration_status(
    connection_generation: Option<u64>,
    model_generation: u64,
) -> &'static str {
    match connection_generation {
        Some(connection_generation) if connection_generation == model_generation => "active",
        Some(_) => "reconnect required",
        None => "inactive",
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
#[tauri::command]
fn model_configuration(state: State<'_, DesktopAppState>) -> ModelConfigurationPresentation {
    let (selection, generation) = {
        let model = state
            .model
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        (model.selection.clone(), model.generation)
    };
    let connection = state
        .connection
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    ModelConfigurationPresentation {
        provider: selection.provider,
        model: selection.model,
        status: model_configuration_status(
            match &*connection {
                ConnectionState::Connected {
                    model_generation, ..
                } => Some(*model_generation),
                _ => None,
            },
            generation,
        ),
    }
}

#[cfg(target_os = "windows")]
#[tauri::command]
fn set_model_configuration(
    state: State<'_, DesktopAppState>,
    provider: DesktopModelProvider,
    model: Option<String>,
) -> Result<(), FrontendError> {
    let chat = *state
        .chat
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let selection = DesktopModelSelection { provider, model };
    let mut current = state
        .model
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    apply_model_selection(&mut current, chat, selection)
}

#[cfg(target_os = "windows")]
fn apply_model_selection(
    current: &mut DesktopModelState,
    chat: ChatState,
    selection: DesktopModelSelection,
) -> Result<(), FrontendError> {
    if chat == ChatState::Running {
        return Err(FrontendError::ModelConfigurationBusy);
    }
    selection.codex_model_config()?;
    if current.selection != selection {
        current.selection = selection;
        current.generation += 1;
    }
    Ok(())
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

/// One host-owned set of inputs for a single Codex connection attempt.
///
/// The executable source is presentation-only metadata. It must not choose a
/// model, bridge, or runtime-construction branch.
#[cfg(target_os = "windows")]
#[derive(Debug)]
struct PreparedCodexConnection {
    executable: std::ffi::OsString,
    model_config: CodexModelConfig,
    source: CodexExecutableSource,
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

/// Resolves host executable selection and combines it with the already chosen,
/// host-owned model configuration. Presentation must use only `source` and
/// never perform a second resolution.
#[cfg(target_os = "windows")]
fn prepare_codex_connection<R>(
    resolver: R,
    model_config: CodexModelConfig,
) -> Result<PreparedCodexConnection, FrontendError>
where
    R: FnOnce() -> Result<codex_baseline::CodexExecutableSelection, BaselineError>,
{
    let selection = match resolver() {
        Ok(selection) => selection,
        Err(BaselineError::Invalid) => return Err(FrontendError::CodexBaselineInvalid),
        Err(BaselineError::UnsupportedHost) => return Err(FrontendError::CodexHostUnsupported),
    };
    Ok(PreparedCodexConnection {
        executable: selection.executable,
        model_config,
        source: selection.source,
    })
}

/// Passes one prepared host-owned connection value through the sole Desktop
/// runtime-construction seam.
#[cfg(target_os = "windows")]
async fn connect_prepared_codex<T, F, Fut>(
    prepared: PreparedCodexConnection,
    runtime_factory: F,
) -> Result<(T, CodexExecutableSource), FrontendError>
where
    F: FnOnce(PreparedCodexConnection) -> Fut,
    Fut: Future<Output = Result<T, FrontendError>>,
{
    let source = prepared.source;
    let runtime = runtime_factory(prepared).await?;
    Ok((runtime, source))
}

#[cfg(target_os = "windows")]
async fn resolve_prepare_and_connect_codex<T, R, F, Fut>(
    resolver: R,
    model_config: CodexModelConfig,
    runtime_factory: F,
) -> Result<(T, CodexExecutableSource), FrontendError>
where
    R: FnOnce() -> Result<codex_baseline::CodexExecutableSelection, BaselineError>,
    F: FnOnce(PreparedCodexConnection) -> Fut,
    Fut: Future<Output = Result<T, FrontendError>>,
{
    let prepared = prepare_codex_connection(resolver, model_config)?;
    connect_prepared_codex(prepared, runtime_factory).await
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
    let (model_config, model_generation) = {
        let model = state
            .model
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match model.selection.codex_model_config() {
            Ok(config) => (config, model.generation),
            Err(error) => {
                let mut connection = state
                    .connection
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                *connection = ConnectionState::Error(error);
                return Err(error);
            }
        }
    };
    match resolve_prepare_and_connect_codex(
        resolve_codex_executable,
        model_config,
        |prepared| async move {
            let registry = desktop_tool_registry(repository.as_deref()).map_err(|error| {
                tracing::error!(error = %error, "failed to construct desktop tool registry");
                FrontendError::ToolRegistryFailed
            })?;
            CodexRuntime::connect_tool_bridge_with_model_config(
                prepared.executable,
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
                prepared.model_config,
            )
            .await
            .map_err(|error| {
                tracing::warn!(error = %error, "Codex desktop connection failed");
                frontend_error(&error)
            })
        },
    )
    .await
    {
        Ok((runtime, source)) => {
            let mut connection = state
                .connection
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            *connection = ConnectionState::Connected {
                runtime: Arc::new(runtime),
                source,
                repository_generation,
                model_generation,
            };
            Ok(ConnectionResult::connected())
        }
        Err(frontend_error) => {
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
async fn run_chat(
    app: AppHandle,
    runtime: Arc<CodexRuntime>,
    request: AgentRequest,
    prompt: String,
    conversation_epoch: u64,
) {
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
            AgentEvent::Completed { output, .. } => {
                let assistant_text = output.message.content.clone();
                let committed = app
                    .state::<DesktopAppState>()
                    .conversation
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .commit(conversation_epoch, prompt.clone(), output.message);
                if committed.is_err() {
                    emit_chat_event(
                        &app,
                        ChatEvent::Failed {
                            code: FrontendError::ChatRuntimeFailed,
                        },
                    );
                    terminal = true;
                    break;
                }
                if let Err(warning) = app
                    .state::<DesktopAppState>()
                    .persist_completed_pair(prompt.clone(), assistant_text)
                {
                    emit_persistence_warning(&app, warning);
                }
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
) -> Result<SendChatResult, FrontendError> {
    validate_prompt(&prompt)?;
    {
        let mut chat = state
            .chat
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        begin_chat(&mut chat)?;
    }
    let (runtime, identity) = {
        let connection = state
            .connection
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match &*connection {
            ConnectionState::Connected {
                runtime,
                repository_generation,
                model_generation,
                ..
            } => (
                Arc::clone(runtime),
                ConversationContextIdentity {
                    repository_generation: *repository_generation,
                    model_generation: *model_generation,
                },
            ),
            _ => {
                state.finish_chat();
                return Err(FrontendError::CodexNotConnected);
            }
        }
    };
    let (request, conversation_epoch, context_change) = {
        let mut conversation = state
            .conversation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let context_change = conversation.reconcile(identity);
        let messages = match conversation.request_messages(&prompt) {
            Ok(messages) => messages,
            Err(error) => {
                state.finish_chat();
                return Err(error);
            }
        };
        (
            AgentRequest {
                request_id: RequestId::new(),
                input: AgentInput { messages },
                options: AgentOptions::default(),
            },
            conversation.epoch,
            context_change,
        )
    };
    if let Some(context_change) = context_change {
        let reason = match context_change {
            ConversationContextChange::Repository => SeparatorReason::RepositoryChanged,
            ConversationContextChange::ModelConfiguration => {
                SeparatorReason::ModelConfigurationChanged
            }
            ConversationContextChange::RepositoryAndModel => {
                SeparatorReason::RepositoryAndModelChanged
            }
        };
        if let Err(warning) = state.persist_separator(reason) {
            emit_persistence_warning(&app, warning);
        }
    }
    tauri::async_runtime::spawn(run_chat(app, runtime, request, prompt, conversation_epoch));
    Ok(SendChatResult { context_change })
}

#[cfg(target_os = "windows")]
#[tauri::command]
fn new_conversation(
    app: AppHandle,
    state: State<'_, DesktopAppState>,
) -> Result<(), FrontendError> {
    if *state
        .chat
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        == ChatState::Running
    {
        return Err(FrontendError::ChatAlreadyRunning);
    }
    state
        .conversation
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .start_new();
    if let Err(warning) = state.persist_separator(SeparatorReason::NewConversation) {
        emit_persistence_warning(&app, warning);
    }
    Ok(())
}

#[cfg(target_os = "windows")]
#[tauri::command]
fn clear_conversation_history(state: State<'_, DesktopAppState>) -> Result<(), FrontendError> {
    clear_conversation_allowed(
        *state
            .chat
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
    )?;
    state
        .persistence
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clear()
        .map_err(|_| FrontendError::ConversationHistoryClearFailed)?;
    state
        .conversation
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .start_new();
    Ok(())
}

#[cfg(target_os = "windows")]
fn clear_conversation_allowed(chat: ChatState) -> Result<(), FrontendError> {
    if chat == ChatState::Running {
        Err(FrontendError::ConversationHistoryBusy)
    } else {
        Ok(())
    }
}

#[cfg(target_os = "windows")]
fn resume_persistence_error(error: ResumeError) -> FrontendError {
    match error {
        ResumeError::Unavailable => FrontendError::ConversationResumeUnavailable,
        ResumeError::Incompatible => FrontendError::ConversationResumePersistenceIncompatible,
        ResumeError::SaveFailed => FrontendError::ConversationResumePersistenceFailed,
    }
}

#[cfg(target_os = "windows")]
#[tauri::command]
fn resume_previous_conversation(state: State<'_, DesktopAppState>) -> Result<(), FrontendError> {
    if *state
        .chat
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        != ChatState::Idle
    {
        return Err(FrontendError::ConversationResumeBusy);
    }
    let repository_generation = *state
        .repository_generation
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let model_generation = state
        .model
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .generation;
    let identity = {
        let connection = state
            .connection
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match &*connection {
            ConnectionState::Connected {
                repository_generation: connected_repository_generation,
                model_generation: connected_model_generation,
                ..
            } if *connected_repository_generation == repository_generation
                && *connected_model_generation == model_generation =>
            {
                ConversationContextIdentity {
                    repository_generation,
                    model_generation,
                }
            }
            ConnectionState::Connected { .. } => {
                return Err(FrontendError::ConversationResumeReconnectRequired);
            }
            _ => return Err(FrontendError::ConversationResumeUnavailable),
        }
    };
    let mut conversation = state
        .conversation
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if !conversation.history.is_empty() {
        return Err(FrontendError::ConversationResumeUnavailable);
    }
    let pairs = {
        let mut persistence = state
            .persistence
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let pairs = persistence
            .resume_messages()
            .map_err(resume_persistence_error)?;
        let message_count = pairs.len().saturating_mul(2);
        let byte_count = pairs
            .iter()
            .map(|pair| pair.user.len() + pair.assistant.len())
            .sum::<usize>();
        if message_count > MAX_CONVERSATION_REPLAY_MESSAGES
            || byte_count > MAX_CONVERSATION_REPLAY_BYTES
        {
            return Err(FrontendError::ConversationResumeTooLarge);
        }
        persistence
            .commit_resume_lineage()
            .map_err(resume_persistence_error)?;
        pairs
    };
    conversation.resume(identity, pairs)
}

#[cfg(target_os = "windows")]
#[tauri::command]
fn conversation_transcript(
    state: State<'_, DesktopAppState>,
) -> ConversationTranscriptPresentation {
    state
        .persistence
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .presentation()
}

#[cfg(target_os = "windows")]
fn main() -> ExitCode {
    match tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let storage_directory = app.path().app_local_data_dir()?;
            app.manage(DesktopAppState::new(storage_directory));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_status,
            model_configuration,
            set_model_configuration,
            choose_repository,
            connect_codex,
            disconnect_codex,
            repository_snapshot,
            send_chat,
            new_conversation,
            clear_conversation_history,
            resume_previous_conversation,
            conversation_transcript
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
    use super::codex_baseline::{BaselineError, CodexExecutableSelection, CodexExecutableSource};
    use super::{
        ActivityEvent, ActivityResult, ChatEvent, ChatState, CodexExecutableSourcePresentation,
        ConnectRequest, ConnectionState, ConversationContextChange, ConversationContextIdentity,
        DESKTOP_TOOL_NAME, DesktopConversationState, DesktopModelProvider, DesktopModelSelection,
        DesktopModelState, DesktopRepository, FrontendError, MAX_CONVERSATION_REPLAY_BYTES,
        MAX_CONVERSATION_REPLAY_MESSAGES, MAX_PROMPT_BYTES, ModelConfigurationPresentation,
        ResumePair, SendChatResult, activity_event, apply_model_selection, begin_chat,
        clear_conversation_allowed, connect_prepared_codex, current_app_status,
        desktop_tool_registry, frontend_error, model_configuration_status,
        prepare_codex_connection, repository_selection_allowed, repository_tool_authority,
        request_connect, resolve_prepare_and_connect_codex, validate_prompt,
    };
    use rah_protocol::{
        AgentEvent, Message, MessageRole, PermissionLevel, SessionId, ToolCall, ToolCallId,
        ToolInput, ToolName, ToolOutput,
    };
    use rah_runtime_codex::{
        CodexAdapterError, CodexLlamaCppProvider, CodexModelConfig, CodexModelProvider,
    };
    use rah_tools::ToolContext;
    use std::{
        collections::HashMap,
        ffi::OsString,
        fs,
        path::PathBuf,
        sync::{
            Arc, Mutex,
            atomic::{AtomicBool, AtomicU64, Ordering},
        },
        time::{SystemTime, UNIX_EPOCH},
    };

    fn message(role: MessageRole, content: &str) -> Message {
        Message {
            role,
            content: content.to_owned(),
        }
    }

    static NEXT_TEST_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn desktop_connect_preparation_keeps_equivalent_auto_and_override_inputs_identical() {
        #[derive(Debug, PartialEq, Eq)]
        struct RuntimeFactoryInput {
            executable: OsString,
            model_config: CodexModelConfig,
            connection_mode: &'static str,
        }

        let certified_executable = OsString::from(
            r"C:\Users\morefunfun11\AppData\Local\codex-baselines\0.149.0\codex.exe",
        );
        let cases = [
            (
                "automatic certified baseline",
                CodexExecutableSelection {
                    executable: certified_executable.clone(),
                    source: CodexExecutableSource::CertifiedBaseline,
                },
                CodexExecutableSource::CertifiedBaseline,
            ),
            (
                "explicit override of the same certified executable",
                CodexExecutableSelection {
                    executable: certified_executable.clone(),
                    source: CodexExecutableSource::Override,
                },
                CodexExecutableSource::Override,
            ),
        ];

        let observed = cases.map(|(name, selection, expected_source)| {
            let prepared =
                prepare_codex_connection(move || Ok(selection), CodexModelConfig::Inherit)
                    .unwrap_or_else(|error| panic!("{name}: {error:?}"));
            assert_eq!(prepared.source, expected_source, "{name}");
            let observed_by_factory = Arc::new(Mutex::new(None));
            let factory_input = Arc::clone(&observed_by_factory);
            let (runtime, source) = futures::executor::block_on(connect_prepared_codex(
                prepared,
                move |prepared| async move {
                    *factory_input
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner) =
                        Some(RuntimeFactoryInput {
                            executable: prepared.executable,
                            model_config: prepared.model_config,
                            connection_mode: "tool_bridge",
                        });
                    Ok::<_, FrontendError>(())
                },
            ))
            .unwrap_or_else(|error| panic!("{name}: {error:?}"));

            assert_eq!(runtime, ());
            assert_eq!(source, expected_source, "{name}");
            observed_by_factory
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .take()
                .unwrap_or_else(|| panic!("{name}: factory was not invoked"))
        });

        assert_eq!(observed[0], observed[1]);
        assert_eq!(observed[0].executable, certified_executable);
        assert_eq!(observed[0].model_config, CodexModelConfig::Inherit);
        assert_eq!(observed[0].connection_mode, "tool_bridge");
    }

    #[test]
    fn desktop_connect_preparation_preserves_path_fallback_input() {
        let prepared = prepare_codex_connection(
            || {
                Ok(CodexExecutableSelection {
                    executable: OsString::from("codex"),
                    source: CodexExecutableSource::Path,
                })
            },
            CodexModelConfig::Inherit,
        )
        .expect("PATH fallback should prepare");
        assert_eq!(prepared.executable, OsString::from("codex"));
        assert_eq!(prepared.source, CodexExecutableSource::Path);
    }

    #[test]
    fn desktop_connect_does_not_invoke_the_runtime_factory_for_invalid_baseline() {
        let invoked = Arc::new(AtomicBool::new(false));
        let invoked_by_factory = Arc::clone(&invoked);
        let result = futures::executor::block_on(resolve_prepare_and_connect_codex(
            || Err(BaselineError::Invalid),
            CodexModelConfig::Inherit,
            move |_| async move {
                invoked_by_factory.store(true, Ordering::Relaxed);
                Ok::<_, FrontendError>(())
            },
        ));

        assert_eq!(result, Err(FrontendError::CodexBaselineInvalid));
        assert!(!invoked.load(Ordering::Relaxed));
    }

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
        let status = current_app_status(&ConnectionState::NotConnected, false, 0, 0);

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
        assert_eq!(status.model_configuration_status, "inactive");

        let serialized = serde_json::to_string(&status).expect("status serializes");
        assert!(!serialized.contains('\\'));
        assert!(!serialized.contains('/'));
    }

    #[test]
    fn status_reflects_connection_transitions_without_exposing_runtime_details() {
        let connecting = current_app_status(&ConnectionState::Connecting, false, 0, 0);
        assert_eq!(connecting.runtime_status, "not connected");
        assert_eq!(connecting.codex_status, "connecting");
        assert_eq!(connecting.codex_version, None);
        assert_eq!(connecting.codex_error, None);

        let error = current_app_status(
            &ConnectionState::Error(FrontendError::CodexConnectionFailed),
            false,
            0,
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
    fn unsupported_codex_version_frontend_error_has_one_adapter_origin() {
        let mismatch = CodexAdapterError::VersionMismatch {
            expected: "codex-cli 0.149.0",
            actual: "codex-cli 0.148.0".to_owned(),
        };
        assert_eq!(
            frontend_error(&mismatch),
            FrontendError::UnsupportedCodexVersion
        );
        for error in [
            CodexAdapterError::SchemaMismatch {
                missing: "method".to_owned(),
            },
            CodexAdapterError::ProcessStartup {
                path: PathBuf::from(r"C:\\private\\codex.exe"),
                source: std::io::Error::other("not started"),
            },
        ] {
            assert_ne!(
                frontend_error(&error),
                FrontendError::UnsupportedCodexVersion
            );
        }
    }

    #[test]
    fn codex_source_presentation_is_closed_and_never_contains_host_details() {
        let source = serde_json::to_string(&CodexExecutableSourcePresentation::CertifiedBaseline)
            .expect("source serializes");
        assert_eq!(source, "\"certified_baseline\"");
        for forbidden in ["\\", "/", ":", "sha", "manifest", "package"] {
            assert!(!source.contains(forbidden));
        }
    }

    #[test]
    fn repository_status_is_dynamic_without_exposing_repository_details() {
        let status = current_app_status(&ConnectionState::NotConnected, true, 1, 0);
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

    #[test]
    fn desktop_model_state_defaults_to_inherit_without_a_model() {
        let state = DesktopModelState::default();
        assert_eq!(state.selection.provider, DesktopModelProvider::Inherit);
        assert_eq!(state.selection.model, None);
        assert_eq!(state.generation, 0);
    }

    #[test]
    fn desktop_model_selection_maps_only_closed_provider_choices() {
        let inherit = DesktopModelSelection::default().codex_model_config();
        assert_eq!(inherit, Ok(CodexModelConfig::Inherit));
        for (provider, expected) in [
            (DesktopModelProvider::OpenAi, CodexModelProvider::OpenAi),
            (DesktopModelProvider::Ollama, CodexModelProvider::Ollama),
            (DesktopModelProvider::LmStudio, CodexModelProvider::LmStudio),
            (
                DesktopModelProvider::LlamaCpp,
                CodexModelProvider::LlamaCpp(CodexLlamaCppProvider::default_local()),
            ),
        ] {
            let selection = DesktopModelSelection {
                provider,
                model: Some("exact-model".to_owned()),
            };
            assert_eq!(
                selection.codex_model_config(),
                Ok(CodexModelConfig::Explicit(
                    rah_runtime_codex::CodexModelSelection::new("exact-model", expected)
                        .expect("test selection")
                ))
            );
        }
    }

    #[test]
    fn desktop_model_selection_rejects_invalid_or_inherit_model_values() {
        for selection in [
            DesktopModelSelection {
                provider: DesktopModelProvider::OpenAi,
                model: None,
            },
            DesktopModelSelection {
                provider: DesktopModelProvider::Ollama,
                model: Some("   ".to_owned()),
            },
            DesktopModelSelection {
                provider: DesktopModelProvider::Inherit,
                model: Some("not-allowed".to_owned()),
            },
        ] {
            assert_eq!(
                selection.codex_model_config(),
                Err(FrontendError::ModelConfigurationInvalid)
            );
        }
    }

    #[test]
    fn model_configuration_presentation_is_closed_and_sanitized() {
        let presentation = ModelConfigurationPresentation {
            provider: DesktopModelProvider::LlamaCpp,
            model: Some("rah-local-model".to_owned()),
            status: "active",
        };
        let serialized = serde_json::to_string(&presentation).expect("presentation serializes");
        assert_eq!(
            serialized,
            r#"{"provider":"llama_cpp","model":"rah-local-model","status":"active"}"#
        );
        for forbidden in [
            "base_url",
            "env_key",
            "credential",
            "api_key",
            "authorization",
            "executable",
            "provider_map",
        ] {
            assert!(!serialized.contains(forbidden));
        }
    }

    #[test]
    fn model_selection_changes_generation_only_when_effective_selection_changes() {
        let mut state = DesktopModelState::default();
        let selection = DesktopModelSelection {
            provider: DesktopModelProvider::LlamaCpp,
            model: Some("rah-local-model".to_owned()),
        };
        assert_eq!(
            apply_model_selection(&mut state, ChatState::Idle, selection.clone()),
            Ok(())
        );
        assert_eq!(state.generation, 1);
        assert_eq!(
            apply_model_selection(&mut state, ChatState::Idle, selection),
            Ok(())
        );
        assert_eq!(state.generation, 1);
    }

    #[test]
    fn model_selection_is_rejected_while_chat_is_running() {
        let mut state = DesktopModelState::default();
        assert_eq!(
            apply_model_selection(
                &mut state,
                ChatState::Running,
                DesktopModelSelection {
                    provider: DesktopModelProvider::OpenAi,
                    model: Some("test-model".to_owned()),
                },
            ),
            Err(FrontendError::ModelConfigurationBusy)
        );
        assert_eq!(state.generation, 0);
    }

    #[test]
    fn model_and_repository_connection_generations_are_independent() {
        assert_eq!(model_configuration_status(None, 0), "inactive");
        assert_eq!(model_configuration_status(Some(4), 4), "active");
        assert_eq!(model_configuration_status(Some(4), 5), "reconnect required");
        assert_eq!(repository_tool_authority(true, Some(4), 4), "active");
        assert_eq!(model_configuration_status(Some(4), 5), "reconnect required");
        assert_eq!(
            repository_tool_authority(true, Some(4), 5),
            "reconnect required"
        );
        assert_eq!(model_configuration_status(Some(5), 5), "active");
    }

    #[test]
    fn conversation_builds_and_commits_only_complete_alternating_pairs() {
        let mut conversation = DesktopConversationState::default();
        let identity = ConversationContextIdentity {
            repository_generation: 1,
            model_generation: 5,
        };
        assert_eq!(conversation.reconcile(identity), None);
        assert_eq!(
            conversation.request_messages("one"),
            Ok(vec![message(MessageRole::User, "one")])
        );
        assert_eq!(
            conversation.commit(
                0,
                "one".to_owned(),
                message(MessageRole::Assistant, "answer one")
            ),
            Ok(())
        );
        assert_eq!(
            conversation.request_messages("two"),
            Ok(vec![
                message(MessageRole::User, "one"),
                message(MessageRole::Assistant, "answer one"),
                message(MessageRole::User, "two"),
            ])
        );
        assert_eq!(
            conversation.commit(
                0,
                "two".to_owned(),
                message(MessageRole::Assistant, "answer two")
            ),
            Ok(())
        );
        assert_eq!(
            conversation.history,
            vec![
                message(MessageRole::User, "one"),
                message(MessageRole::Assistant, "answer one"),
                message(MessageRole::User, "two"),
                message(MessageRole::Assistant, "answer two"),
            ]
        );
    }

    #[test]
    fn resumed_conversation_imports_exact_pairs_and_next_request_appends_prompt() {
        let identity = ConversationContextIdentity {
            repository_generation: 7,
            model_generation: 9,
        };
        let mut conversation = DesktopConversationState::default();
        conversation
            .resume(
                identity,
                vec![
                    ResumePair {
                        user: "one".into(),
                        assistant: "answer one".into(),
                    },
                    ResumePair {
                        user: "two".into(),
                        assistant: "answer two".into(),
                    },
                ],
            )
            .unwrap();
        assert_eq!(conversation.identity, Some(identity));
        assert_eq!(
            conversation.request_messages("three").unwrap(),
            vec![
                message(MessageRole::User, "one"),
                message(MessageRole::Assistant, "answer one"),
                message(MessageRole::User, "two"),
                message(MessageRole::Assistant, "answer two"),
                message(MessageRole::User, "three"),
            ]
        );
        assert_eq!(
            conversation.resume(
                identity,
                vec![ResumePair {
                    user: "x".into(),
                    assistant: "y".into()
                }]
            ),
            Err(FrontendError::ConversationResumeUnavailable)
        );
    }

    #[test]
    fn resume_replay_limits_are_inclusive_and_context_changes_clear_it() {
        let identity = ConversationContextIdentity {
            repository_generation: 1,
            model_generation: 1,
        };
        let mut exact = DesktopConversationState::default();
        exact
            .resume(
                identity,
                (0..4)
                    .map(|_| ResumePair {
                        user: "u".into(),
                        assistant: "a".into(),
                    })
                    .collect(),
            )
            .unwrap();
        assert_eq!(exact.history.len(), MAX_CONVERSATION_REPLAY_MESSAGES);
        assert_eq!(
            exact.reconcile(ConversationContextIdentity {
                repository_generation: 2,
                model_generation: 1,
            }),
            Some(ConversationContextChange::Repository)
        );
        assert!(exact.history.is_empty());
        let mut oversized = DesktopConversationState::default();
        assert_eq!(
            oversized.resume(
                identity,
                (0..5)
                    .map(|_| ResumePair {
                        user: "u".into(),
                        assistant: "a".into()
                    })
                    .collect(),
            ),
            Err(FrontendError::ConversationResumeTooLarge)
        );

        let exact_bytes = "x".repeat(MAX_CONVERSATION_REPLAY_BYTES / 8);
        let mut byte_boundary = DesktopConversationState::default();
        byte_boundary
            .resume(
                identity,
                (0..4)
                    .map(|_| ResumePair {
                        user: exact_bytes.clone(),
                        assistant: exact_bytes.clone(),
                    })
                    .collect(),
            )
            .unwrap();
        assert_eq!(
            byte_boundary
                .history
                .iter()
                .map(|message| message.content.len())
                .sum::<usize>(),
            MAX_CONVERSATION_REPLAY_BYTES
        );
        let mut byte_overflow = DesktopConversationState::default();
        assert_eq!(
            byte_overflow.resume(
                identity,
                vec![ResumePair {
                    user: "x".repeat(MAX_CONVERSATION_REPLAY_BYTES),
                    assistant: "x".into(),
                }],
            ),
            Err(FrontendError::ConversationResumeTooLarge)
        );
    }

    #[test]
    fn conversation_never_commits_failed_cancelled_start_or_incomplete_turns() {
        let mut conversation = DesktopConversationState::default();
        conversation.reconcile(ConversationContextIdentity {
            repository_generation: 1,
            model_generation: 5,
        });
        let before = conversation.history.clone();
        assert_eq!(
            conversation.request_messages("pending"),
            Ok(vec![message(MessageRole::User, "pending")])
        );
        assert_eq!(conversation.history, before);
        assert_eq!(
            conversation.commit(
                0,
                "pending".to_owned(),
                message(MessageRole::Tool, "tool output")
            ),
            Err(())
        );
        assert_eq!(conversation.history, before);
    }

    #[test]
    fn conversation_context_changes_clear_history_with_closed_reasons_and_same_context_keeps_it() {
        let mut conversation = DesktopConversationState::default();
        let original = ConversationContextIdentity {
            repository_generation: 1,
            model_generation: 5,
        };
        conversation.reconcile(original);
        conversation
            .commit(
                0,
                "one".to_owned(),
                message(MessageRole::Assistant, "answer"),
            )
            .unwrap();
        assert_eq!(conversation.reconcile(original), None);
        assert_eq!(conversation.history.len(), 2);
        assert_eq!(
            conversation.reconcile(ConversationContextIdentity {
                repository_generation: 2,
                model_generation: 5
            }),
            Some(ConversationContextChange::Repository)
        );
        assert!(conversation.history.is_empty());
        conversation
            .commit(
                1,
                "two".to_owned(),
                message(MessageRole::Assistant, "answer"),
            )
            .unwrap();
        assert_eq!(
            conversation.reconcile(ConversationContextIdentity {
                repository_generation: 2,
                model_generation: 6
            }),
            Some(ConversationContextChange::ModelConfiguration)
        );
        assert_eq!(
            conversation.reconcile(ConversationContextIdentity {
                repository_generation: 3,
                model_generation: 7
            }),
            Some(ConversationContextChange::RepositoryAndModel)
        );
    }

    #[test]
    fn new_conversation_clears_history_and_invalidates_stale_completion() {
        let mut conversation = DesktopConversationState::default();
        conversation.reconcile(ConversationContextIdentity {
            repository_generation: 1,
            model_generation: 5,
        });
        conversation
            .commit(
                0,
                "one".to_owned(),
                message(MessageRole::Assistant, "answer"),
            )
            .unwrap();
        conversation.start_new();
        assert!(conversation.history.is_empty());
        assert_eq!(
            conversation.commit(
                0,
                "stale".to_owned(),
                message(MessageRole::Assistant, "answer")
            ),
            Err(())
        );
    }

    #[test]
    fn clear_conversation_uses_fresh_replay_context_and_closed_errors() {
        let mut conversation = DesktopConversationState::default();
        conversation.reconcile(ConversationContextIdentity {
            repository_generation: 1,
            model_generation: 5,
        });
        conversation
            .commit(
                0,
                "old".to_owned(),
                message(MessageRole::Assistant, "answer"),
            )
            .unwrap();
        conversation.start_new();

        assert_eq!(
            conversation.request_messages("new"),
            Ok(vec![message(MessageRole::User, "new")])
        );
        assert_eq!(clear_conversation_allowed(ChatState::Idle), Ok(()));
        assert_eq!(
            clear_conversation_allowed(ChatState::Running),
            Err(FrontendError::ConversationHistoryBusy)
        );
        let serialized = serde_json::to_string(&FrontendError::ConversationHistoryClearFailed)
            .expect("clear error serializes");
        assert_eq!(serialized, r#""conversation_history_clear_failed""#);
        assert!(!serialized.contains("path"));
    }

    #[test]
    fn conversation_replay_limits_are_closed_and_inclusive() {
        let mut conversation = DesktopConversationState {
            history: (0..MAX_CONVERSATION_REPLAY_MESSAGES)
                .map(|_| message(MessageRole::User, "x"))
                .collect(),
            ..Default::default()
        };
        assert!(conversation.request_messages("next").is_ok());
        conversation
            .history
            .push(message(MessageRole::Assistant, "x"));
        assert_eq!(
            conversation.request_messages("next"),
            Err(FrontendError::ConversationContextLimit)
        );
        let mut byte_limited = DesktopConversationState {
            history: vec![message(
                MessageRole::User,
                &"x".repeat(MAX_CONVERSATION_REPLAY_BYTES),
            )],
            ..Default::default()
        };
        assert!(byte_limited.request_messages("next").is_ok());
        byte_limited
            .history
            .push(message(MessageRole::Assistant, "x"));
        assert_eq!(
            byte_limited.request_messages("next"),
            Err(FrontendError::ConversationContextLimit)
        );
    }

    #[test]
    fn send_chat_result_serializes_only_the_closed_context_change_contract() {
        let serialized = serde_json::to_string(&SendChatResult {
            context_change: Some(ConversationContextChange::RepositoryAndModel),
        })
        .unwrap();
        assert_eq!(
            serialized,
            r#"{"contextChange":"repository_and_model_changed"}"#
        );
        for forbidden in [
            "generation",
            "session",
            "thread",
            "path",
            "endpoint",
            "credential",
            "token",
        ] {
            assert!(!serialized.contains(forbidden));
        }
    }
}
