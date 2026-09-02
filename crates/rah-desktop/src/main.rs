#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(target_os = "windows")]
mod codex_baseline;
#[cfg(target_os = "windows")]
mod conversation_persistence;
#[cfg(target_os = "windows")]
mod desktop_preferences;
#[cfg(target_os = "windows")]
mod git_discovery;

#[cfg(target_os = "windows")]
use codex_baseline::{BaselineError, CodexExecutableSource, resolve as resolve_codex_executable};
#[cfg(target_os = "windows")]
use conversation_persistence::{
    Persistence, Presentation as ConversationTranscriptPresentation, ResumeError, ResumePair,
    SeparatorReason, Warning as ConversationPersistenceWarning,
};
#[cfg(target_os = "windows")]
use desktop_preferences::{Preferences, Warning as PreferencesWarning};
#[cfg(target_os = "windows")]
use futures::StreamExt;
#[cfg(target_os = "windows")]
use rah_protocol::{
    AgentEvent, AgentInput, AgentOptions, AgentRequest, Message, MessageRole, PermissionLevel,
    RequestId, SessionId, ToolContent, ToolInput,
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
    EchoTool, FsReadTool, GitStageTool, GitUnstageTool, RepositoryCommitControl,
    RepositoryCommitReview, RepositoryCommitTool, RepositoryDiffStagedTool, RepositoryDiffTool,
    RepositoryFileCreationTool, RepositoryFileDeletionAuthority, RepositoryFileDeletionTool,
    RepositoryFileInfoTool, RepositoryMultiFileEditTool, RepositoryStatusTool,
    RepositoryWorktreePatchTool, Tool, ToolContext, ToolError, ToolRegistry,
};
#[cfg(target_os = "windows")]
use serde::{Deserialize, Serialize};
#[cfg(target_os = "windows")]
use sha2::{Digest, Sha256};
#[cfg(all(test, target_os = "windows"))]
use std::sync::OnceLock;
#[cfg(target_os = "windows")]
use std::{
    collections::HashMap,
    future::Future,
    net::IpAddr,
    path::{Path, PathBuf},
    process::ExitCode,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, SystemTime},
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
const READINESS_CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
#[cfg(target_os = "windows")]
const READINESS_TOTAL_TIMEOUT: Duration = Duration::from_secs(5);
#[cfg(target_os = "windows")]
const READINESS_BODY_LIMIT: usize = 4 * 1024;
#[cfg(target_os = "windows")]
const CANCEL_GRACEFUL_TIMEOUT: Duration = Duration::from_secs(2);
#[cfg(target_os = "windows")]
const CANCEL_HARD_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(2);
/// Empty, Desktop-owned child directory used when no project is selected.
#[cfg(target_os = "windows")]
const NEUTRAL_WORKSPACE_DIRECTORY: &str = "codex-neutral-workspace";
#[cfg(target_os = "windows")]
const COMMIT_IDENTITY_MAX_BYTES: usize = 1024;

/// Test-only evidence that constructing Desktop state does not activate optional authorities.
#[cfg(all(test, target_os = "windows"))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct StartupActivationCounters {
    codex_resolver: u64,
    git_resolver: u64,
    codex_runtime_construction: u64,
    readiness_probe: u64,
    tool_registry: u64,
    repository_composition: u64,
    conversation_resume: u64,
}

#[cfg(all(test, target_os = "windows"))]
static STARTUP_ACTIVATION_COUNTERS: OnceLock<Mutex<StartupActivationCounters>> = OnceLock::new();
#[cfg(all(test, target_os = "windows"))]
thread_local! {
    static STARTUP_COUNTER_TRACKING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(all(test, target_os = "windows"))]
fn startup_counter_tracking() -> bool {
    STARTUP_COUNTER_TRACKING.with(std::cell::Cell::get)
}

#[cfg(all(test, target_os = "windows"))]
fn startup_activation_counters() -> &'static Mutex<StartupActivationCounters> {
    STARTUP_ACTIVATION_COUNTERS.get_or_init(|| Mutex::new(StartupActivationCounters::default()))
}

#[cfg(all(test, target_os = "windows"))]
fn reset_startup_activation_counters() {
    *startup_activation_counters()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = StartupActivationCounters::default();
    STARTUP_COUNTER_TRACKING.with(|tracking| tracking.set(true));
}

#[cfg(all(test, target_os = "windows"))]
fn startup_activation_snapshot() -> StartupActivationCounters {
    let snapshot = *startup_activation_counters()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    STARTUP_COUNTER_TRACKING.with(|tracking| tracking.set(false));
    snapshot
}

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
    CancelRequested,
}

/// The one-shot, generation-scoped authority for a desktop turn's terminal result.
/// Keeping this separate from the runtime means late adapter events cannot win twice.
#[cfg(target_os = "windows")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TerminalOwnership {
    generation: u64,
    claimed: bool,
}

#[cfg(target_os = "windows")]
impl TerminalOwnership {
    fn new(generation: u64) -> Self {
        Self {
            generation,
            claimed: false,
        }
    }

    fn claim(&mut self, generation: u64) -> bool {
        if self.generation != generation || self.claimed {
            return false;
        }
        self.claimed = true;
        true
    }

    fn is_unclaimed(&self, generation: u64) -> bool {
        self.generation == generation && !self.claimed
    }
}

#[cfg(target_os = "windows")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GracefulCancelOutcome {
    Completed,
    Failed,
    TimedOut,
}

#[cfg(target_os = "windows")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HardShutdownOutcome {
    Completed,
    Failed,
    TimedOut,
}

#[cfg(target_os = "windows")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CancelRecoveryOutcome {
    Graceful(GracefulCancelOutcome),
    Hard(HardShutdownOutcome),
    Stale,
}

#[cfg(target_os = "windows")]
async fn await_graceful_cancel<F, E>(future: F, timeout: Duration) -> GracefulCancelOutcome
where
    F: Future<Output = Result<(), E>>,
{
    match tokio::time::timeout(timeout, future).await {
        Ok(Ok(())) => GracefulCancelOutcome::Completed,
        Ok(Err(_)) => GracefulCancelOutcome::Failed,
        Err(_) => GracefulCancelOutcome::TimedOut,
    }
}

#[cfg(target_os = "windows")]
async fn await_hard_shutdown<F, E>(future: F, timeout: Duration) -> HardShutdownOutcome
where
    F: Future<Output = Result<(), E>>,
{
    match tokio::time::timeout(timeout, future).await {
        Ok(Ok(())) => HardShutdownOutcome::Completed,
        Ok(Err(_)) => HardShutdownOutcome::Failed,
        Err(_) => HardShutdownOutcome::TimedOut,
    }
}

/// Calls `shutdown` only after graceful cancellation requires hard recovery.
#[cfg(target_os = "windows")]
async fn await_cancel_recovery<F, E, P, S, H, HE>(
    graceful: F,
    graceful_timeout: Duration,
    begin_hard_recovery: P,
    shutdown: S,
    hard_shutdown_timeout: Duration,
) -> CancelRecoveryOutcome
where
    F: Future<Output = Result<(), E>>,
    P: FnOnce(GracefulCancelOutcome) -> bool,
    S: FnOnce() -> H,
    H: Future<Output = Result<(), HE>>,
{
    let graceful = await_graceful_cancel(graceful, graceful_timeout).await;
    if graceful == GracefulCancelOutcome::Completed {
        CancelRecoveryOutcome::Graceful(graceful)
    } else if !begin_hard_recovery(graceful) {
        CancelRecoveryOutcome::Stale
    } else {
        CancelRecoveryOutcome::Hard(await_hard_shutdown(shutdown(), hard_shutdown_timeout).await)
    }
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
    active_chat: Mutex<Option<ActiveChat>>,
    next_chat_generation: Mutex<u64>,
    repository: Mutex<Option<Arc<DesktopRepository>>>,
    repository_generation: Mutex<u64>,
    repository_workflow: Mutex<RepositoryWorkflowState>,
    commit_identity: Mutex<Option<DesktopCommitIdentity>>,
    commit_identity_generation: Mutex<u64>,
    commit_capability: Mutex<Option<DesktopCommitCapability>>,
    /// An app-owned non-project directory used only when no repository is selected.
    neutral_workspace: Option<PathBuf>,
    model: Mutex<DesktopModelState>,
    preferences: Mutex<Preferences>,
    model_apply_persistence: Mutex<()>,
    conversation: Mutex<DesktopConversationState>,
    persistence: Mutex<Persistence>,
    close_started: AtomicBool,
}

#[cfg(target_os = "windows")]
impl DesktopAppState {
    fn persistence_namespace(&self) -> String {
        let repository = self
            .repository
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        repository.as_ref().map_or_else(
            || "neutral-v1".to_owned(),
            |repository| repository_persistence_key(&repository.root),
        )
    }

    fn select_persistence_namespace(&self) {
        self.persistence
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .select_namespace(self.persistence_namespace());
    }

    fn persist_separator(
        &self,
        reason: SeparatorReason,
    ) -> Result<(), ConversationPersistenceWarning> {
        self.select_persistence_namespace();
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
        self.select_persistence_namespace();
        self.persistence
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .append_pair(prompt, assistant)
    }

    fn new(storage_directory: PathBuf) -> Self {
        let neutral_workspace = neutral_workspace(&storage_directory);
        let (preferences, selection) = Preferences::start(storage_directory.clone());
        let identity = preferences.identity();
        let mut persistence = Persistence::start(storage_directory);
        // Startup has no selected repository. Bind presentation and any future
        // writes to the neutral namespace immediately; legacy global records
        // remain preserved but never become visible in this namespace.
        persistence.select_namespace("neutral-v1".to_owned());
        Self {
            connection: Mutex::new(ConnectionState::NotConnected),
            chat: Mutex::new(ChatState::Idle),
            active_chat: Mutex::new(None),
            next_chat_generation: Mutex::new(0),
            repository: Mutex::new(None),
            repository_generation: Mutex::new(0),
            repository_workflow: Mutex::new(RepositoryWorkflowState::default()),
            commit_identity: Mutex::new(identity),
            commit_identity_generation: Mutex::new(0),
            commit_capability: Mutex::new(None),
            neutral_workspace,
            model: Mutex::new(DesktopModelState {
                selection,
                generation: 0,
                readiness: ReadinessState::NotTested,
            }),
            preferences: Mutex::new(preferences),
            model_apply_persistence: Mutex::new(()),
            conversation: Mutex::new(DesktopConversationState::default()),
            persistence: Mutex::new(persistence),
            close_started: AtomicBool::new(false),
        }
    }
}

/// Durable repository identity is derived only from the canonical root selected
/// by the host. It is intentionally opaque in persistence metadata.
#[cfg(target_os = "windows")]
fn repository_persistence_key(root: &Path) -> String {
    use std::os::windows::ffi::OsStrExt;
    let mut digest = Sha256::new();
    digest.update(b"rah-desktop-repository-v1\0");
    for unit in root.as_os_str().encode_wide() {
        digest.update(unit.to_le_bytes());
    }
    format!("repo-sha256:{:x}", digest.finalize())
}

#[cfg(target_os = "windows")]
fn emit_preferences_warning(app: &AppHandle, warning: PreferencesWarning) {
    if let Err(error) = app.emit(
        "desktop_preferences_warning",
        match warning {
            PreferencesWarning::RestoreFailed => "preferences_restore_failed",
            PreferencesWarning::SaveFailed => "preferences_save_failed",
        },
    ) {
        tracing::warn!(error = %error, "failed to emit desktop preferences warning");
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
#[allow(dead_code)]
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
        #[cfg(test)]
        if startup_counter_tracking() {
            startup_activation_counters()
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .conversation_resume += 1;
        }
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
    fn start_chat(&self) -> Result<u64, FrontendError> {
        {
            let mut chat = self
                .chat
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            begin_chat(&mut chat)?;
        }
        let mut next_generation = self
            .next_chat_generation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *next_generation = next_generation.wrapping_add(1);
        let generation = *next_generation;
        let mut active = self
            .active_chat
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *active = Some(ActiveChat {
            generation,
            runtime: None,
            session_id: None,
            terminal: TerminalOwnership::new(generation),
        });
        Ok(generation)
    }

    fn register_chat_session(
        &self,
        generation: u64,
        runtime: Arc<CodexRuntime>,
        session_id: SessionId,
    ) -> bool {
        let mut active = self
            .active_chat
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(chat) = active.as_mut() else {
            return false;
        };
        if !chat.terminal.is_unclaimed(generation)
            || chat.runtime.is_some()
            || chat.session_id.is_some()
        {
            return false;
        }
        chat.runtime = Some(runtime);
        chat.session_id = Some(session_id);
        true
    }

    fn active_chat(&self) -> Result<(u64, Arc<CodexRuntime>, SessionId), FrontendError> {
        let active = self
            .active_chat
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match active.as_ref() {
            Some(
                chat @ ActiveChat {
                    runtime: Some(runtime),
                    session_id: Some(session_id),
                    ..
                },
            ) if chat.terminal.is_unclaimed(chat.generation) => {
                Ok((chat.generation, Arc::clone(runtime), session_id.clone()))
            }
            _ => Err(FrontendError::ChatNotRunning),
        }
    }

    fn request_cancel(
        &self,
        generation: u64,
        runtime: &Arc<CodexRuntime>,
        session_id: &SessionId,
    ) -> bool {
        let active = self
            .active_chat
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let matches = active.as_ref().is_some_and(|chat| {
            chat.terminal.is_unclaimed(generation)
                && chat.session_id.as_ref() == Some(session_id)
                && chat
                    .runtime
                    .as_ref()
                    .is_some_and(|current| same_arc(current, runtime))
        });
        drop(active);
        if matches {
            *self
                .chat
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = ChatState::CancelRequested;
        }
        matches
    }

    fn claim_terminal(
        &self,
        generation: u64,
        runtime: &Arc<CodexRuntime>,
        session_id: &SessionId,
    ) -> bool {
        let mut active = self
            .active_chat
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(chat) = active.as_mut() else {
            return false;
        };
        if !chat.terminal.is_unclaimed(generation)
            || chat.session_id.as_ref() != Some(session_id)
            || !chat
                .runtime
                .as_ref()
                .is_some_and(|current| same_arc(current, runtime))
        {
            return false;
        }
        let claimed = chat.terminal.claim(generation);
        drop(active);
        *self
            .chat
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = ChatState::Idle;
        claimed
    }

    fn is_current_chat(
        &self,
        generation: u64,
        runtime: &Arc<CodexRuntime>,
        session_id: &SessionId,
    ) -> bool {
        self.active_chat
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .is_some_and(|chat| {
                chat.terminal.is_unclaimed(generation)
                    && chat.session_id.as_ref() == Some(session_id)
                    && chat
                        .runtime
                        .as_ref()
                        .is_some_and(|current| same_arc(current, runtime))
            })
    }

    fn claim_start_failure(&self, generation: u64) -> bool {
        let mut active = self
            .active_chat
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if !active
            .as_ref()
            .is_some_and(|chat| chat.terminal.is_unclaimed(generation) && chat.runtime.is_none())
        {
            return false;
        }
        *active = None;
        drop(active);
        *self
            .chat
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = ChatState::Idle;
        true
    }

    fn finish_chat(&self, generation: u64) {
        {
            let mut active = self
                .active_chat
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if !active
                .as_mut()
                .is_some_and(|chat| chat.terminal.claim(generation))
            {
                return;
            }
        }
        *self
            .chat
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = ChatState::Idle;
    }
}

#[cfg(target_os = "windows")]
struct ActiveChat {
    generation: u64,
    runtime: Option<Arc<CodexRuntime>>,
    session_id: Option<SessionId>,
    terminal: TerminalOwnership,
}

#[cfg(target_os = "windows")]
fn same_arc<T>(left: &Arc<T>, right: &Arc<T>) -> bool {
    Arc::ptr_eq(left, right)
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

    fn begin_hard_recovery(&self, runtime: &Arc<CodexRuntime>) -> bool {
        let mut connection = self
            .connection
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let matches = matches!(&*connection, ConnectionState::Connected { runtime: current, .. } if same_arc(current, runtime));
        if matches {
            *connection = ConnectionState::Disconnecting;
        }
        matches
    }

    fn finish_hard_recovery(&self, success: bool) {
        let mut connection = self
            .connection
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if matches!(*connection, ConnectionState::Disconnecting) {
            *connection = if success {
                ConnectionState::NotConnected
            } else {
                ConnectionState::Error(FrontendError::CodexConnectionFailed)
            };
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
    deletion_authority: Option<RepositoryFileDeletionAuthority>,
}

#[cfg(target_os = "windows")]
impl DesktopRepository {
    #[cfg(test)]
    fn new(
        git_executable: &std::path::Path,
        repository_root: &std::path::Path,
    ) -> Result<Self, ToolError> {
        Self::new_with_deletion_authority(git_executable, repository_root, None)
    }

    fn new_with_deletion_authority(
        git_executable: &std::path::Path,
        repository_root: &std::path::Path,
        deletion_authority: Option<RepositoryFileDeletionAuthority>,
    ) -> Result<Self, ToolError> {
        #[cfg(test)]
        if startup_counter_tracking() {
            startup_activation_counters()
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .repository_composition += 1;
        }
        let repository_root =
            repository_root
                .canonicalize()
                .map_err(|error| ToolError::Execution {
                    message: error.to_string(),
                })?;
        let status = RepositoryStatusTool::new(git_executable, &repository_root)?;
        let worktree_diff = RepositoryDiffTool::new(git_executable, &repository_root)?;
        let staged_diff = RepositoryDiffStagedTool::new(git_executable, &repository_root)?;
        if let Some(authority) = &deletion_authority
            && !authority.matches_resources(git_executable, &repository_root)
        {
            return Err(ToolError::Execution {
                message: "deletion authority does not match selected repository".to_owned(),
            });
        }
        Ok(Self {
            display_path: repository_root.display().to_string(),
            git_executable: git_executable.to_path_buf(),
            root: repository_root,
            status: Arc::new(status),
            worktree_diff: Arc::new(worktree_diff),
            staged_diff: Arc::new(staged_diff),
            deletion_authority,
        })
    }
}

/// Explicit human author/committer preference. It is configuration only and
/// never constitutes reviewed-snapshot authority.
#[cfg(target_os = "windows")]
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct DesktopCommitIdentity {
    name: String,
    email: String,
}

#[cfg(target_os = "windows")]
impl DesktopCommitIdentity {
    fn validate(&self) -> Result<(), ()> {
        for value in [&self.name, &self.email] {
            if value.is_empty()
                || value.trim().is_empty()
                || value.len() > COMMIT_IDENTITY_MAX_BYTES
                || value
                    .chars()
                    .any(|character| character == '\0' || character.is_control())
            {
                return Err(());
            }
        }
        Ok(())
    }
}

#[cfg(target_os = "windows")]
struct DesktopCommitCapability {
    repository_generation: u64,
    model_generation: u64,
    identity_generation: u64,
    _tool: Arc<RepositoryCommitTool>,
    control: Arc<RepositoryCommitControl>,
}

/// Ephemeral, Rust-owned selectors for one observed index mutation. They are
/// regenerated on every repository observation and are never persisted.
#[cfg(target_os = "windows")]
#[derive(Default)]
struct RepositoryWorkflowState {
    observation_generation: u64,
    next_action: u64,
    actions: HashMap<String, RepositoryIndexAction>,
    review: Option<StagedReviewDescriptor>,
    commit_review: Option<RepositoryCommitReview>,
    review_selector: Option<String>,
    authorization: CommitAuthorizationPresentation,
}

#[cfg(target_os = "windows")]
#[derive(Clone, Copy, PartialEq, Eq)]
enum RepositoryIndexActionKind {
    Stage,
    Unstage,
}

#[cfg(target_os = "windows")]
struct RepositoryIndexAction {
    kind: RepositoryIndexActionKind,
    repository_generation: u64,
    observation_generation: u64,
    target: PathBuf,
    target_observation: TargetObservation,
}

#[cfg(target_os = "windows")]
#[derive(Clone, PartialEq, Eq)]
struct TargetObservation {
    canonical_path: PathBuf,
    length: u64,
    modified: Option<SystemTime>,
    content_digest: [u8; 32],
}

#[cfg(target_os = "windows")]
#[allow(dead_code)] // Task 148 consumes this Rust-only review binding.
#[derive(Clone)]
struct StagedReviewDescriptor {
    repository_generation: u64,
    observation_generation: u64,
    digest: String,
    complete: bool,
    binary_supported: bool,
}

#[cfg(target_os = "windows")]
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum CommitAuthorizationPresentation {
    IdentityNotConfigured,
    ConnectionRequired,
    #[default]
    ReviewRequired,
    ReadyToAuthorize,
    AuthorizedPending,
    ReviewStale,
    AuthorizationFailed,
    AuthorizationRevoked,
}

#[cfg(target_os = "windows")]
#[cfg(target_os = "windows")]
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum FrontendError {
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
    CodexReconnectRequired,
    ChatNotRunning,
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
    RepositoryActionInvalid,
    RepositoryActionStale,
    ModelConfigurationInvalid,
    ModelConfigurationBusy,
    CommitIdentityInvalid,
    CommitIdentitySaveFailed,
    CommitAuthorizationUnavailable,
    CommitAuthorizationStale,
    CommitAuthorizationFailed,
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
pub(crate) enum DesktopModelProvider {
    Inherit,
    #[serde(rename = "openai")]
    OpenAi,
    Ollama,
    LmStudio,
    LlamaCpp,
}

/// Closed, Desktop-private scheme selection for an already-running llama.cpp server.
#[cfg(target_os = "windows")]
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ProviderScheme {
    Http,
    Https,
}

#[cfg(target_os = "windows")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ProviderHost {
    Ip(IpAddr),
    Dns(String),
}

/// A normalized initial endpoint selected only by the Desktop host.
#[cfg(target_os = "windows")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ProviderEndpoint {
    pub(crate) scheme: ProviderScheme,
    pub(crate) host: ProviderHost,
    pub(crate) port: u16,
}

#[cfg(target_os = "windows")]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProviderEndpointInput {
    scheme: ProviderScheme,
    host: String,
    port: u16,
}

#[cfg(target_os = "windows")]
impl ProviderEndpoint {
    fn parse(input: ProviderEndpointInput) -> Result<Self, FrontendError> {
        if input.port == 0 || input.host.is_empty() || input.host.trim() != input.host {
            return Err(FrontendError::ModelConfigurationInvalid);
        }
        let host = match input.host.parse::<IpAddr>() {
            Ok(ip) => ProviderHost::Ip(ip),
            Err(_) if is_ipv4_shaped(&input.host) => {
                return Err(FrontendError::ModelConfigurationInvalid);
            }
            Err(_) => ProviderHost::Dns(normalize_dns_hostname(&input.host)?),
        };
        Ok(Self {
            scheme: input.scheme,
            host,
            port: input.port,
        })
    }

    fn base_url(&self) -> String {
        let scheme = match self.scheme {
            ProviderScheme::Http => "http",
            ProviderScheme::Https => "https",
        };
        let host = match &self.host {
            ProviderHost::Ip(IpAddr::V4(ip)) => ip.to_string(),
            ProviderHost::Ip(IpAddr::V6(ip)) => format!("[{ip}]"),
            ProviderHost::Dns(host) => host.clone(),
        };
        format!("{scheme}://{host}:{}/v1", self.port)
    }

    fn insecure_transport(&self) -> bool {
        self.scheme == ProviderScheme::Http
            && !matches!(&self.host, ProviderHost::Ip(ip) if ip.is_loopback())
    }
}

#[cfg(target_os = "windows")]
fn is_ipv4_shaped(host: &str) -> bool {
    host.contains('.')
        && host
            .bytes()
            .all(|byte| byte.is_ascii_digit() || byte == b'.')
}

#[cfg(target_os = "windows")]
fn normalize_dns_hostname(host: &str) -> Result<String, FrontendError> {
    let hostname = host.strip_suffix('.').unwrap_or(host);
    if hostname.is_empty() || hostname.len() > 253 || !hostname.is_ascii() {
        return Err(FrontendError::ModelConfigurationInvalid);
    }
    for label in hostname.split('.') {
        if label.is_empty()
            || label.len() > 63
            || label.starts_with('-')
            || label.ends_with('-')
            || !label
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        {
            return Err(FrontendError::ModelConfigurationInvalid);
        }
    }
    Ok(hostname.to_ascii_lowercase())
}

#[cfg(target_os = "windows")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DesktopModelSelection {
    pub(crate) provider: DesktopModelProvider,
    pub(crate) model: Option<String>,
    pub(crate) llama_cpp_endpoint: Option<ProviderEndpoint>,
}

#[cfg(target_os = "windows")]
impl Default for DesktopModelSelection {
    fn default() -> Self {
        Self {
            provider: DesktopModelProvider::Inherit,
            model: None,
            llama_cpp_endpoint: None,
        }
    }
}

#[cfg(target_os = "windows")]
impl DesktopModelSelection {
    pub(crate) fn validate(&self) -> Result<(), FrontendError> {
        match self.provider {
            DesktopModelProvider::Inherit
                if self.model.is_none() && self.llama_cpp_endpoint.is_none() =>
            {
                Ok(())
            }
            DesktopModelProvider::OpenAi
            | DesktopModelProvider::Ollama
            | DesktopModelProvider::LmStudio
                if self.llama_cpp_endpoint.is_none() =>
            {
                validate_model_identifier(
                    self.model
                        .as_deref()
                        .ok_or(FrontendError::ModelConfigurationInvalid)?,
                )
            }
            DesktopModelProvider::LlamaCpp if self.llama_cpp_endpoint.is_some() => {
                validate_model_identifier(
                    self.model
                        .as_deref()
                        .ok_or(FrontendError::ModelConfigurationInvalid)?,
                )
            }
            _ => Err(FrontendError::ModelConfigurationInvalid),
        }
    }
    fn codex_model_config(&self) -> Result<CodexModelConfig, FrontendError> {
        self.validate()?;
        let provider = match self.provider {
            DesktopModelProvider::Inherit => {
                return if self.model.is_none() && self.llama_cpp_endpoint.is_none() {
                    Ok(CodexModelConfig::Inherit)
                } else {
                    Err(FrontendError::ModelConfigurationInvalid)
                };
            }
            DesktopModelProvider::OpenAi if self.llama_cpp_endpoint.is_none() => {
                CodexModelProvider::OpenAi
            }
            DesktopModelProvider::Ollama if self.llama_cpp_endpoint.is_none() => {
                CodexModelProvider::Ollama
            }
            DesktopModelProvider::LmStudio if self.llama_cpp_endpoint.is_none() => {
                CodexModelProvider::LmStudio
            }
            DesktopModelProvider::LlamaCpp => {
                let endpoint = self
                    .llama_cpp_endpoint
                    .as_ref()
                    .ok_or(FrontendError::ModelConfigurationInvalid)?;
                CodexModelProvider::LlamaCpp(
                    CodexLlamaCppProvider::new(endpoint.base_url(), None)
                        .map_err(|_| FrontendError::ModelConfigurationInvalid)?,
                )
            }
            _ => return Err(FrontendError::ModelConfigurationInvalid),
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
pub(crate) fn validate_model_identifier(value: &str) -> Result<(), FrontendError> {
    if value.is_empty()
        || value.len() > 256
        || value.contains('\0')
        || value.chars().any(|c| c.is_control())
        || value.chars().next().is_some_and(char::is_whitespace)
        || value.chars().next_back().is_some_and(char::is_whitespace)
    {
        Err(FrontendError::ModelConfigurationInvalid)
    } else {
        Ok(())
    }
}

#[cfg(target_os = "windows")]
#[derive(Debug, Default)]
struct DesktopModelState {
    selection: DesktopModelSelection,
    generation: u64,
    readiness: ReadinessState,
}

/// Diagnostic state for the sole explicit Desktop llama.cpp readiness action.
#[cfg(target_os = "windows")]
#[derive(Clone, Copy, Debug, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)] // reqwest 0.13 has no stable TLS-only classifier in this closed client.
enum ReadinessState {
    #[default]
    NotTested,
    Checking,
    Ready,
    Loading,
    Unreachable,
    TlsFailure,
    CheckFailed,
}

#[cfg(target_os = "windows")]
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct ModelConfigurationPresentation {
    provider: DesktopModelProvider,
    model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    endpoint: Option<ProviderEndpointPresentation>,
    insecure_transport: bool,
    readiness: ReadinessState,
    status: &'static str,
}

#[cfg(target_os = "windows")]
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct ProviderEndpointPresentation {
    scheme: ProviderScheme,
    host: String,
    port: u16,
    normalized: String,
}

#[cfg(target_os = "windows")]
impl From<&ProviderEndpoint> for ProviderEndpointPresentation {
    fn from(endpoint: &ProviderEndpoint) -> Self {
        let host = match &endpoint.host {
            ProviderHost::Ip(ip) => ip.to_string(),
            ProviderHost::Dns(host) => host.clone(),
        };
        Self {
            scheme: endpoint.scheme,
            host,
            port: endpoint.port,
            normalized: endpoint.base_url(),
        }
    }
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
        #[serde(skip_serializing_if = "Option::is_none")]
        commit: Option<CommitActivityPresentation>,
    },
}

/// The only commit result details that Desktop may present. The underlying tool
/// output remains bridge-private and can never expose authorization material.
#[cfg(target_os = "windows")]
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct CommitActivityPresentation {
    status: CommitActivityStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    commit_oid: Option<String>,
}

#[cfg(target_os = "windows")]
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum CommitActivityStatus {
    InvalidInput,
    PreconditionFailed,
    KnownNoEffect,
    CommittedVerified,
    Uncertain,
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
        CodexAdapterError::WorkspaceContext { .. }
        | CodexAdapterError::InvalidModelProviderConfig { .. } => {
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
fn neutral_workspace(storage_directory: &Path) -> Option<PathBuf> {
    let workspace = storage_directory.join(NEUTRAL_WORKSPACE_DIRECTORY);
    std::fs::create_dir_all(&workspace).ok()?;
    workspace.canonicalize().ok()
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
fn connection_context_is_current(
    connection_repository_generation: u64,
    connection_model_generation: u64,
    current_repository_generation: u64,
    current_model_generation: u64,
) -> bool {
    connection_repository_generation == current_repository_generation
        && connection_model_generation == current_model_generation
}

#[cfg(target_os = "windows")]
#[tauri::command]
fn app_status(state: State<'_, DesktopAppState>) -> AppStatus {
    state.status()
}

#[cfg(target_os = "windows")]
#[tauri::command]
fn model_configuration(state: State<'_, DesktopAppState>) -> ModelConfigurationPresentation {
    let (selection, generation, readiness) = {
        let model = state
            .model
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        (model.selection.clone(), model.generation, model.readiness)
    };
    let connection = state
        .connection
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    ModelConfigurationPresentation {
        provider: selection.provider,
        model: selection.model,
        endpoint: selection
            .llama_cpp_endpoint
            .as_ref()
            .map(ProviderEndpointPresentation::from),
        insecure_transport: selection
            .llama_cpp_endpoint
            .as_ref()
            .is_some_and(ProviderEndpoint::insecure_transport),
        readiness,
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
fn desktop_preferences_warning(state: State<'_, DesktopAppState>) -> Option<&'static str> {
    state
        .preferences
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .take_warning()
        .map(|warning| match warning {
            PreferencesWarning::RestoreFailed => "preferences_restore_failed",
            PreferencesWarning::SaveFailed => "preferences_save_failed",
        })
}

#[cfg(target_os = "windows")]
#[tauri::command]
fn set_model_configuration(
    app: AppHandle,
    state: State<'_, DesktopAppState>,
    provider: DesktopModelProvider,
    model: Option<String>,
    llama_cpp_endpoint: Option<ProviderEndpointInput>,
) -> Result<(), FrontendError> {
    let _ordering = state
        .model_apply_persistence
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let chat = *state
        .chat
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let llama_cpp_endpoint = llama_cpp_endpoint
        .map(ProviderEndpoint::parse)
        .transpose()?;
    let selection = DesktopModelSelection {
        provider,
        model,
        llama_cpp_endpoint,
    };
    let mut current = state
        .model
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    apply_model_selection(&mut current, chat, selection.clone())?;
    drop(current);
    if state
        .preferences
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .save(&selection)
        .is_err()
    {
        emit_preferences_warning(&app, PreferencesWarning::SaveFailed);
    }
    *state
        .commit_capability
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
    *state
        .repository_workflow
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = RepositoryWorkflowState::default();
    Ok(())
}

#[cfg(target_os = "windows")]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CommitIdentityPresentation {
    configured: bool,
}

#[cfg(target_os = "windows")]
#[tauri::command]
fn commit_identity(state: State<'_, DesktopAppState>) -> CommitIdentityPresentation {
    CommitIdentityPresentation {
        configured: state
            .commit_identity
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_some(),
    }
}

#[cfg(target_os = "windows")]
#[tauri::command]
fn set_commit_identity(
    app: AppHandle,
    state: State<'_, DesktopAppState>,
    name: String,
    email: String,
) -> Result<(), FrontendError> {
    let identity = DesktopCommitIdentity { name, email };
    identity
        .validate()
        .map_err(|_| FrontendError::CommitIdentityInvalid)?;
    let selection = state
        .model
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .selection
        .clone();
    state
        .preferences
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .save_identity(&selection, identity.clone())
        .map_err(|_| {
            emit_preferences_warning(&app, PreferencesWarning::SaveFailed);
            FrontendError::CommitIdentitySaveFailed
        })?;
    *state
        .commit_identity
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(identity);
    *state
        .commit_identity_generation
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) += 1;
    *state
        .commit_capability
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
    *state
        .repository_workflow
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = RepositoryWorkflowState::default();
    Ok(())
}

#[cfg(target_os = "windows")]
#[tauri::command]
fn reset_model_preferences(
    app: AppHandle,
    state: State<'_, DesktopAppState>,
) -> Result<(), FrontendError> {
    let _ordering = state
        .model_apply_persistence
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let chat = *state
        .chat
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut current = state
        .model
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    apply_model_selection(&mut current, chat, DesktopModelSelection::default())?;
    drop(current);
    if state
        .preferences
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .reset()
        .is_err()
    {
        emit_preferences_warning(&app, PreferencesWarning::SaveFailed);
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn apply_model_selection(
    current: &mut DesktopModelState,
    chat: ChatState,
    selection: DesktopModelSelection,
) -> Result<(), FrontendError> {
    if chat != ChatState::Idle {
        return Err(FrontendError::ModelConfigurationBusy);
    }
    selection.codex_model_config()?;
    if current.selection != selection {
        current.selection = selection;
        current.generation += 1;
        current.readiness = ReadinessState::NotTested;
    }
    Ok(())
}

/// Desktop-private, explicit GET-only check of a currently selected llama.cpp endpoint.
#[cfg(target_os = "windows")]
struct LlamaCppReadinessProbe;

#[cfg(target_os = "windows")]
impl LlamaCppReadinessProbe {
    async fn check(endpoint: &ProviderEndpoint) -> ReadinessState {
        #[cfg(test)]
        if startup_counter_tracking() {
            startup_activation_counters()
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .readiness_probe += 1;
        }
        let client = match reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .retry(reqwest::retry::never())
            .connect_timeout(READINESS_CONNECT_TIMEOUT)
            .timeout(READINESS_TOTAL_TIMEOUT)
            .build()
        {
            Ok(client) => client,
            Err(error) => {
                tracing::warn!(error = %error, "failed to construct readiness client");
                return ReadinessState::CheckFailed;
            }
        };
        let response = match client
            .get(format!("{}/health", endpoint.base_url()))
            .send()
            .await
        {
            Ok(response) => response,
            Err(error) => return readiness_transport_error(&error),
        };
        let status = response.status();
        if response
            .content_length()
            .is_some_and(|length| length > READINESS_BODY_LIMIT as u64)
        {
            return ReadinessState::CheckFailed;
        }
        let mut bytes = 0usize;
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(chunk) => {
                    bytes = match bytes.checked_add(chunk.len()) {
                        Some(bytes) if bytes <= READINESS_BODY_LIMIT => bytes,
                        _ => return ReadinessState::CheckFailed,
                    };
                }
                Err(error) => return readiness_transport_error(&error),
            }
        }
        match status.as_u16() {
            200 => ReadinessState::Ready,
            503 => ReadinessState::Loading,
            _ => ReadinessState::CheckFailed,
        }
    }
}

#[cfg(target_os = "windows")]
fn readiness_transport_error(error: &reqwest::Error) -> ReadinessState {
    // reqwest exposes connection and timeout facts without relying on unstable error text.
    if error.is_timeout() || error.is_connect() {
        ReadinessState::Unreachable
    } else {
        // The selected client does not expose a stable TLS-only classifier here.
        ReadinessState::CheckFailed
    }
}

#[cfg(target_os = "windows")]
#[tauri::command]
fn test_llama_cpp_endpoint(
    app: AppHandle,
    state: State<'_, DesktopAppState>,
) -> Result<(), FrontendError> {
    let (endpoint, generation) = {
        let mut model = state
            .model
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if model.selection.provider != DesktopModelProvider::LlamaCpp {
            return Err(FrontendError::ModelConfigurationInvalid);
        }
        let endpoint = model
            .selection
            .llama_cpp_endpoint
            .clone()
            .ok_or(FrontendError::ModelConfigurationInvalid)?;
        if model.readiness == ReadinessState::Checking {
            return Ok(());
        }
        model.readiness = ReadinessState::Checking;
        (endpoint, model.generation)
    };
    tauri::async_runtime::spawn(async move {
        let result = LlamaCppReadinessProbe::check(&endpoint).await;
        publish_readiness_result(
            app.state::<DesktopAppState>().inner(),
            generation,
            &endpoint,
            result,
        );
    });
    Ok(())
}

/// Publishes only the result belonging to the exact selected endpoint generation.
#[cfg(target_os = "windows")]
fn publish_readiness_result(
    state: &DesktopAppState,
    generation: u64,
    endpoint: &ProviderEndpoint,
    result: ReadinessState,
) {
    let mut model = state
        .model
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if model.generation == generation
        && model.selection.provider == DesktopModelProvider::LlamaCpp
        && model.selection.llama_cpp_endpoint.as_ref() == Some(endpoint)
    {
        model.readiness = result;
    }
}

#[cfg(target_os = "windows")]
fn selected_git_executable() -> Result<std::path::PathBuf, FrontendError> {
    git_discovery::resolve()
        .map(|selection| selection.into_candidate())
        .map_err(|_| FrontendError::GitUnavailable)
}

/// Private diagnostic classification for the fixed repository-observer bundle.
///
/// These values deliberately never cross the Desktop IPC boundary: they identify
/// an observer layer without disclosing paths, executable details, stderr, or
/// repository configuration.
#[cfg(target_os = "windows")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RepositoryObservationStage {
    StatusExecutionOrRevalidation,
    WorktreeDiffExecution,
    StagedDiffExecution,
    NormalizedOutput,
}

#[cfg(target_os = "windows")]
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct RepositorySnapshot {
    path: String,
    status_entries: Vec<RepositoryStatusEntry>,
    worktree_diff: Vec<RepositoryDiffFile>,
    staged_diff: Vec<RepositoryDiffFile>,
    review: StagedReviewPresentation,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    stage_action_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    staging_note: Option<&'static str>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    unstage_action_id: Option<String>,
}

/// Stable, host-only review content used to bind a staged-review observation.
/// Presentation selectors deliberately cannot affect this representation.
#[cfg(target_os = "windows")]
#[derive(Serialize)]
struct CanonicalStagedReview<'a> {
    files: Vec<CanonicalStagedReviewFile<'a>>,
}

#[cfg(target_os = "windows")]
#[derive(Serialize)]
struct CanonicalStagedReviewFile<'a> {
    old_path: &'a Option<String>,
    new_path: &'a Option<String>,
    change_kind: &'a str,
    binary: bool,
    added_lines: Option<u64>,
    deleted_lines: Option<u64>,
    patch: &'a Option<String>,
}

#[cfg(target_os = "windows")]
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(
    tag = "state",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
enum StagedReviewPresentation {
    NoStagedChanges,
    ReviewAvailable {
        #[serde(skip_serializing_if = "Option::is_none")]
        review_id: Option<String>,
        can_authorize: bool,
        authorization_state: CommitAuthorizationPresentation,
    },
    ReviewBinaryUnsupported,
    ReviewUnavailable {
        reason: &'static str,
    },
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
                stage_action_id: None,
                staging_note: None,
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
                        unstage_action_id: None,
                    })
                })
                .collect()
        };
    let staged_diff = convert_diff(staged_diff)?;
    let review = if staged_diff.is_empty() {
        StagedReviewPresentation::NoStagedChanges
    } else if staged_diff.iter().any(|file| file.binary) {
        StagedReviewPresentation::ReviewBinaryUnsupported
    } else {
        StagedReviewPresentation::ReviewAvailable {
            review_id: None,
            can_authorize: false,
            authorization_state: CommitAuthorizationPresentation::ReviewRequired,
        }
    };
    Ok(RepositorySnapshot {
        path,
        status_entries,
        worktree_diff: convert_diff(worktree_diff)?,
        staged_diff,
        review,
    })
}

#[cfg(target_os = "windows")]
async fn desktop_repository_snapshot_with_review(
    repository: &DesktopRepository,
    commit_control: Option<Arc<RepositoryCommitControl>>,
) -> Result<(RepositorySnapshot, Option<RepositoryCommitReview>), RepositoryObservationStage> {
    let input = ToolInput(serde_json::json!({}));
    let status = repository
        .status
        .execute(input.clone(), ToolContext::default())
        .await
        .map_err(|_| RepositoryObservationStage::StatusExecutionOrRevalidation)?;
    let worktree_diff = repository
        .worktree_diff
        .execute(input.clone(), ToolContext::default())
        .await
        .map_err(|_| RepositoryObservationStage::WorktreeDiffExecution)?;
    let (staged_diff, review) = if let Some(control) = commit_control {
        let (presentation, review) = control
            .review_current_staged_snapshot()
            .await
            .map_err(|_| RepositoryObservationStage::StagedDiffExecution)?;
        (presentation, review)
    } else {
        (
            repository
                .staged_diff
                .execute(input, ToolContext::default())
                .await
                .map_err(|_| RepositoryObservationStage::StagedDiffExecution)?,
            None,
        )
    };
    desktop_snapshot(
        repository.display_path.clone(),
        status,
        worktree_diff,
        staged_diff,
    )
    .map(|snapshot| (snapshot, review))
    .map_err(|_| RepositoryObservationStage::NormalizedOutput)
}

#[cfg(all(test, target_os = "windows"))]
async fn desktop_repository_snapshot(
    repository: &DesktopRepository,
) -> Result<RepositorySnapshot, RepositoryObservationStage> {
    desktop_repository_snapshot_with_review(repository, None)
        .await
        .map(|(snapshot, _)| snapshot)
}

#[cfg(target_os = "windows")]
fn observe_regular_target(root: &Path, relative: &str) -> Option<TargetObservation> {
    let relative = Path::new(relative);
    if relative.is_absolute()
        || relative.components().any(|part| {
            matches!(
                part,
                std::path::Component::ParentDir
                    | std::path::Component::RootDir
                    | std::path::Component::Prefix(_)
            )
        })
    {
        return None;
    }
    let candidate = root.join(relative);
    let link = std::fs::symlink_metadata(&candidate).ok()?;
    if link.file_type().is_symlink() {
        return None;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if link.file_attributes() & 0x400 != 0 {
            return None;
        }
    }
    let canonical_path = candidate.canonicalize().ok()?;
    if !canonical_path.starts_with(root) {
        return None;
    }
    let metadata = std::fs::metadata(&canonical_path).ok()?;
    if !metadata.is_file() {
        return None;
    }
    let content_digest = Sha256::digest(std::fs::read(&canonical_path).ok()?).into();
    Some(TargetObservation {
        canonical_path,
        length: metadata.len(),
        modified: metadata.modified().ok(),
        content_digest,
    })
}

#[cfg(target_os = "windows")]
fn target_is_current(expected: &TargetObservation) -> bool {
    let Ok(link) = std::fs::symlink_metadata(&expected.canonical_path) else {
        return false;
    };
    if link.file_type().is_symlink() {
        return false;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if link.file_attributes() & 0x400 != 0 {
            return false;
        }
    }
    let Ok(canonical_path) = expected.canonical_path.canonicalize() else {
        return false;
    };
    let Ok(metadata) = std::fs::metadata(&canonical_path) else {
        return false;
    };
    metadata.is_file()
        && canonical_path == expected.canonical_path
        && metadata.len() == expected.length
        && metadata.modified().ok() == expected.modified
        && std::fs::read(&canonical_path)
            .ok()
            .map(|bytes| Sha256::digest(bytes).into())
            == Some(expected.content_digest)
}

#[cfg(target_os = "windows")]
fn staged_review_digest(files: &[RepositoryDiffFile]) -> String {
    let canonical = CanonicalStagedReview {
        files: files
            .iter()
            .map(|file| CanonicalStagedReviewFile {
                old_path: &file.old_path,
                new_path: &file.new_path,
                change_kind: &file.change_kind,
                binary: file.binary,
                added_lines: file.added_lines,
                deleted_lines: file.deleted_lines,
                patch: &file.patch,
            })
            .collect(),
    };
    let bytes = serde_json::to_vec(&canonical).unwrap_or_default();
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(target_os = "windows")]
fn install_repository_workflow(
    state: &DesktopAppState,
    repository: &DesktopRepository,
    repository_generation: u64,
    mut snapshot: RepositorySnapshot,
    commit_review: Option<RepositoryCommitReview>,
    identity_generation: u64,
) -> RepositorySnapshot {
    let mut workflow = state
        .repository_workflow
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    workflow.observation_generation += 1;
    workflow.actions.clear();
    workflow.review = None;
    workflow.commit_review = None;
    workflow.review_selector = None;
    workflow.authorization = CommitAuthorizationPresentation::AuthorizationRevoked;
    let observation_generation = workflow.observation_generation;
    let review_digest = matches!(
        snapshot.review,
        StagedReviewPresentation::ReviewAvailable { .. }
    )
    .then(|| staged_review_digest(&snapshot.staged_diff));
    for entry in &mut snapshot.status_entries {
        let Some(target) = observe_regular_target(&repository.root, &entry.path) else {
            if !entry.tracked && entry.worktree_state == "untracked" {
                entry.staging_note =
                    Some("Untracked — staging not supported by current bounded Desktop authority");
            }
            continue;
        };
        let stage = entry.tracked
            && entry.worktree_state != "unmodified"
            && entry.conflict_state == "none"
            && GitStageTool::new(
                &repository.git_executable,
                &repository.root,
                &entry.path,
                &target.canonical_path,
            )
            .is_ok();
        if stage {
            workflow.next_action += 1;
            let action_id = format!(
                "index-{repository_generation}-{observation_generation}-{}",
                workflow.next_action
            );
            workflow.actions.insert(
                action_id.clone(),
                RepositoryIndexAction {
                    kind: RepositoryIndexActionKind::Stage,
                    repository_generation,
                    observation_generation,
                    target: target.canonical_path.clone(),
                    target_observation: target.clone(),
                },
            );
            entry.stage_action_id = Some(action_id);
        }
        let unstage = entry.tracked
            && entry.index_state == "modified"
            && entry.conflict_state == "none"
            && GitUnstageTool::new(
                &repository.git_executable,
                &repository.root,
                &entry.path,
                &target.canonical_path,
            )
            .is_ok();
        if unstage {
            workflow.next_action += 1;
            let action_id = format!(
                "index-{repository_generation}-{observation_generation}-{}",
                workflow.next_action
            );
            workflow.actions.insert(
                action_id.clone(),
                RepositoryIndexAction {
                    kind: RepositoryIndexActionKind::Unstage,
                    repository_generation,
                    observation_generation,
                    target: target.canonical_path.clone(),
                    target_observation: target,
                },
            );
            for file in &mut snapshot.staged_diff {
                if file.new_path.as_deref() == Some(entry.path.as_str()) {
                    file.unstage_action_id = Some(action_id.clone());
                }
            }
        }
    }
    if let Some(digest) = review_digest {
        workflow.review = Some(StagedReviewDescriptor {
            repository_generation,
            observation_generation,
            digest,
            complete: true,
            binary_supported: true,
        });
    }
    if let Some(commit_review) = commit_review {
        workflow.next_action += 1;
        let selector = format!(
            "review-{repository_generation}-{observation_generation}-{identity_generation}-{}",
            workflow.next_action
        );
        workflow.commit_review = Some(commit_review);
        workflow.review_selector = Some(selector.clone());
        workflow.authorization = CommitAuthorizationPresentation::ReadyToAuthorize;
        snapshot.review = StagedReviewPresentation::ReviewAvailable {
            review_id: Some(selector),
            can_authorize: true,
            authorization_state: workflow.authorization,
        };
    }
    snapshot
}

#[cfg(target_os = "windows")]
async fn refresh_repository_workflow(
    state: &DesktopAppState,
) -> Result<RepositorySnapshot, FrontendError> {
    let (repository, generation) = (
        state
            .repository
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone(),
        *state
            .repository_generation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
    );
    let Some(repository) = repository else {
        return Err(FrontendError::RepositoryNotSelected);
    };
    let (control, identity_generation) = {
        let model_generation = state
            .model
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .generation;
        let connected_context_current = matches!(
            &*state
                .connection
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
            ConnectionState::Connected {
                repository_generation,
                model_generation: connected_model_generation,
                ..
            } if *repository_generation == generation
                && *connected_model_generation == model_generation
        );
        let identity_generation = *state
            .commit_identity_generation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let capability = state
            .commit_capability
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        (
            capability
                .as_ref()
                .filter(|capability| {
                    connected_context_current
                        && capability.repository_generation == generation
                        && capability.model_generation == model_generation
                        && capability.identity_generation == identity_generation
                })
                .map(|capability| Arc::clone(&capability.control)),
            identity_generation,
        )
    };
    // Task 148 intentionally treats every explicit observation refresh as an
    // authorization boundary, even if the staged presentation is identical.
    if let Some(control) = &control {
        control.clear_authorization().await;
    }
    let result = desktop_repository_snapshot_with_review(&repository, control).await;
    let (snapshot, review) = match result {
        Ok(snapshot) => snapshot,
        Err(_) => (
            RepositorySnapshot {
                path: repository.display_path.clone(),
                status_entries: Vec::new(),
                worktree_diff: Vec::new(),
                staged_diff: Vec::new(),
                review: StagedReviewPresentation::ReviewUnavailable {
                    reason: "bounded staged observation failed",
                },
            },
            None,
        ),
    };
    if *state
        .repository_generation
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        != generation
    {
        return Err(FrontendError::RepositoryObservationFailed);
    }
    Ok(install_repository_workflow(
        state,
        &repository,
        generation,
        snapshot,
        review,
        identity_generation,
    ))
}

#[cfg(target_os = "windows")]
async fn revoke_repository_commit_context(state: &DesktopAppState) {
    let capability = state
        .commit_capability
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .take();
    if let Some(capability) = capability {
        // Dropping Desktop's capability must not rely on every Arc holder
        // disappearing before the one-shot approval becomes unusable.
        capability.control.clear_authorization().await;
    }
    *state
        .repository_workflow
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = RepositoryWorkflowState::default();
}

#[cfg(target_os = "windows")]
async fn invalidate_repository_commit_review(state: &DesktopAppState) {
    let control = state
        .commit_capability
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .as_ref()
        .map(|capability| Arc::clone(&capability.control));
    if let Some(control) = control {
        control.clear_authorization().await;
    }
    let mut workflow = state
        .repository_workflow
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    workflow.actions.clear();
    workflow.review = None;
    workflow.commit_review = None;
    workflow.review_selector = None;
    workflow.authorization = CommitAuthorizationPresentation::AuthorizationRevoked;
}

#[cfg(target_os = "windows")]
fn replace_selected_repository(state: &DesktopAppState, repository: DesktopRepository) {
    *state
        .commit_capability
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
    *state
        .repository
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(Arc::new(repository));
    *state
        .repository_generation
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) += 1;
    *state
        .repository_workflow
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = RepositoryWorkflowState::default();
    state.select_persistence_namespace();
    // Persisted presentation is repository-owned, while replay remains an
    // explicit Resume action. Never carry the previous repository's model
    // context across this boundary.
    state
        .conversation
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .start_new();
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
    let deletion_authority =
        RepositoryFileDeletionAuthority::new(&git, &path).map_err(|error| {
            tracing::warn!(error = %error, "selected repository cannot receive deletion authority");
            FrontendError::RepositoryInvalid
        })?;
    let repository =
        DesktopRepository::new_with_deletion_authority(&git, &path, Some(deletion_authority))
            .map_err(|error| {
                tracing::warn!(error = %error, "selected repository is invalid");
                FrontendError::RepositoryInvalid
            })?;
    repository_selection_allowed(
        *state
            .chat
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
    )?;
    replace_selected_repository(state.inner(), repository);
    Ok(())
}

#[cfg(target_os = "windows")]
#[tauri::command]
async fn repository_snapshot(
    state: State<'_, DesktopAppState>,
) -> Result<RepositorySnapshot, FrontendError> {
    refresh_repository_workflow(state.inner()).await
}

#[cfg(target_os = "windows")]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CommitAuthorizationResult {
    authorization_state: CommitAuthorizationPresentation,
}

#[cfg(target_os = "windows")]
async fn authorize_repository_commit_review(
    state: &DesktopAppState,
    review_id: &str,
) -> Result<CommitAuthorizationResult, FrontendError> {
    let repository_generation = *state
        .repository_generation
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let model_generation = state
        .model
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .generation;
    let identity_generation = *state
        .commit_identity_generation
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let control = state
        .commit_capability
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .as_ref()
        .filter(|capability| {
            capability.repository_generation == repository_generation
                && capability.model_generation == model_generation
                && capability.identity_generation == identity_generation
        })
        .map(|capability| Arc::clone(&capability.control))
        .ok_or(FrontendError::CommitAuthorizationUnavailable)?;
    // Every explicit human attempt supersedes any earlier one-shot approval,
    // including stale or duplicate selector submissions.
    control.clear_authorization().await;
    let review = {
        let mut workflow = state
            .repository_workflow
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let valid = workflow.review.as_ref().is_some_and(|review| {
            review.repository_generation == repository_generation
                && review.observation_generation == workflow.observation_generation
                && review.complete
                && review.binary_supported
        }) && workflow.review_selector.as_deref() == Some(review_id);
        if !valid {
            workflow.authorization = CommitAuthorizationPresentation::ReviewStale;
            return Err(FrontendError::CommitAuthorizationStale);
        }
        match workflow.commit_review.take() {
            Some(review) => review,
            None => {
                workflow.authorization = CommitAuthorizationPresentation::ReviewStale;
                return Err(FrontendError::CommitAuthorizationStale);
            }
        }
    };
    match control.authorize_reviewed_snapshot(&review).await {
        Ok(()) => {
            let mut workflow = state
                .repository_workflow
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            workflow.authorization = CommitAuthorizationPresentation::AuthorizedPending;
            Ok(CommitAuthorizationResult {
                authorization_state: workflow.authorization,
            })
        }
        Err(_) => {
            let mut workflow = state
                .repository_workflow
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            workflow.authorization = CommitAuthorizationPresentation::ReviewStale;
            Err(FrontendError::CommitAuthorizationFailed)
        }
    }
}

#[cfg(target_os = "windows")]
#[tauri::command]
async fn repository_authorize_commit_review(
    state: State<'_, DesktopAppState>,
    review_id: String,
) -> Result<CommitAuthorizationResult, FrontendError> {
    authorize_repository_commit_review(state.inner(), &review_id).await
}

#[cfg(target_os = "windows")]
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct RepositoryIndexActionResult {
    status: &'static str,
    changed: bool,
    staged: bool,
    unstaged: bool,
    no_op: bool,
    partial: bool,
}

#[cfg(target_os = "windows")]
async fn repository_index_action(
    state: &DesktopAppState,
    action_id: String,
    kind: RepositoryIndexActionKind,
) -> Result<RepositoryIndexActionResult, FrontendError> {
    let control = state
        .commit_capability
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .as_ref()
        .map(|capability| Arc::clone(&capability.control));
    if let Some(control) = control {
        control.clear_authorization().await;
    }
    let generation = *state
        .repository_generation
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let action = {
        let mut workflow = state
            .repository_workflow
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let action = workflow
            .actions
            .remove(&action_id)
            .ok_or(FrontendError::RepositoryActionInvalid)?;
        // Every attempt consumes the complete observed catalog: no action can
        // survive a possible index effect, including known failure/uncertainty.
        workflow.actions.clear();
        workflow.review = None;
        if action.kind != kind
            || action.repository_generation != generation
            || action.observation_generation != workflow.observation_generation
        {
            return Err(FrontendError::RepositoryActionStale);
        }
        action
    };
    let repository = state
        .repository
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
        .ok_or(FrontendError::RepositoryNotSelected)?;
    if !target_is_current(&action.target_observation) {
        let _ = refresh_repository_workflow(state).await;
        return Err(FrontendError::RepositoryActionStale);
    }
    let result = match kind {
        RepositoryIndexActionKind::Stage => match GitStageTool::new(
            &repository.git_executable,
            &repository.root,
            action.target.to_string_lossy(),
            &action.target,
        ) {
            Ok(tool) => {
                tool.execute(ToolInput(serde_json::json!({})), ToolContext::default())
                    .await
            }
            Err(error) => Err(error),
        },
        RepositoryIndexActionKind::Unstage => match GitUnstageTool::new(
            &repository.git_executable,
            &repository.root,
            action.target.to_string_lossy(),
            &action.target,
        ) {
            Ok(tool) => {
                tool.execute(ToolInput(serde_json::json!({})), ToolContext::default())
                    .await
            }
            Err(error) => Err(error),
        },
    };
    let _ = refresh_repository_workflow(state).await;
    let output = result.map_err(|_| FrontendError::RepositoryObservationFailed)?;
    let value = observer_json(output)?;
    let status = value
        .get("status")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("failed_known");
    Ok(RepositoryIndexActionResult {
        status: match status {
            "ok" => "ok",
            "uncertain" => "uncertain",
            "policy_violation" => "policy_violation",
            _ => "failed_known",
        },
        changed: value
            .get("changed")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
        staged: value
            .get("staged")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
        unstaged: value
            .get("unstaged")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
        no_op: value
            .get("no_op")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
        partial: value
            .get("partial")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
    })
}

#[cfg(target_os = "windows")]
#[tauri::command]
async fn repository_stage_action(
    state: State<'_, DesktopAppState>,
    action_id: String,
) -> Result<RepositoryIndexActionResult, FrontendError> {
    repository_index_action(state.inner(), action_id, RepositoryIndexActionKind::Stage).await
}

#[cfg(target_os = "windows")]
#[tauri::command]
async fn repository_unstage_action(
    state: State<'_, DesktopAppState>,
    action_id: String,
) -> Result<RepositoryIndexActionResult, FrontendError> {
    repository_index_action(state.inner(), action_id, RepositoryIndexActionKind::Unstage).await
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
    commit_tool: Option<Arc<RepositoryCommitTool>>,
) -> Result<Arc<ToolRegistry>, ToolError> {
    #[cfg(test)]
    if startup_counter_tracking() {
        startup_activation_counters()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .tool_registry += 1;
    }
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
        if let Some(authority) = &repository.deletion_authority {
            registry.register(Arc::new(RepositoryFileDeletionTool::from_authority(
                authority.clone(),
            )))?;
        }
        if let Some(commit_tool) = commit_tool {
            registry.register(commit_tool)?;
        }
    }
    append_live_evidence(serde_json::json!({
        "event": "desktop_registry_composed",
        "selected_repository": repository.is_some(),
        "deletion_authority_present": repository
            .is_some_and(|value| value.deletion_authority.is_some()),
        "registry_contains_repo_delete_file": registry
            .definitions()
            .iter()
            .any(|definition| definition.name.as_str() == "repo.delete-file"),
        "relevant_tool_names": registry
            .definitions()
            .into_iter()
            .filter(|definition| definition.name.as_str().starts_with("repo."))
            .map(|definition| definition.name.to_string())
            .collect::<Vec<_>>(),
    }));
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
    #[cfg(test)]
    if startup_counter_tracking() {
        startup_activation_counters()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .codex_resolver += 1;
    }
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
    #[cfg(test)]
    if startup_counter_tracking() {
        startup_activation_counters()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .codex_runtime_construction += 1;
    }
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

    let (repository, repository_generation, neutral_workspace) = {
        let repository = state
            .repository
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        let generation = *state
            .repository_generation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        (repository, generation, state.neutral_workspace.clone())
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
    let identity = state
        .commit_identity
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    let identity_generation = *state
        .commit_identity_generation
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let commit_capability = match (repository.as_deref(), identity) {
        (Some(repository), Some(identity)) => RepositoryCommitTool::compose(
            &repository.git_executable,
            &repository.root,
            identity.name,
            identity.email,
        )
        .ok()
        .map(|(tool, control)| (Arc::new(tool), Arc::new(control))),
        _ => None,
    };
    let commit_tool = commit_capability.as_ref().map(|(tool, _)| Arc::clone(tool));
    match resolve_prepare_and_connect_codex(
        resolve_codex_executable,
        model_config,
        |prepared| async move {
            let registry =
                desktop_tool_registry(repository.as_deref(), commit_tool).map_err(|error| {
                    tracing::error!(error = %error, "failed to construct desktop tool registry");
                    FrontendError::ToolRegistryFailed
                })?;
            append_live_evidence(serde_json::json!({
                "event": "desktop_connection_context",
                "repository_generation": repository_generation,
                "selected_repository": repository.is_some(),
                "deletion_authority_present": repository
                    .as_ref()
                    .is_some_and(|value| value.deletion_authority.is_some()),
                "bridge_enabled": true,
            }));
            CodexRuntime::connect_tool_bridge_with_model_config_and_workspace(
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
                if let Some(repository) = repository.as_deref() {
                    repository.root.as_path()
                } else {
                    neutral_workspace
                        .as_deref()
                        .ok_or(FrontendError::CodexConnectionFailed)?
                },
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
            drop(connection);
            if let Some((tool, control)) = commit_capability {
                *state
                    .commit_capability
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) =
                    Some(DesktopCommitCapability {
                        repository_generation,
                        model_generation,
                        identity_generation,
                        _tool: tool,
                        control,
                    });
            }
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
        != ChatState::Idle
    {
        return Err(FrontendError::ChatAlreadyRunning);
    }
    let runtime = {
        revoke_repository_commit_context(state.inner()).await;
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
    if *chat != ChatState::Idle {
        return Err(FrontendError::ChatAlreadyRunning);
    }
    *chat = ChatState::Running;
    Ok(())
}

#[cfg(target_os = "windows")]
fn repository_selection_allowed(chat: ChatState) -> Result<(), FrontendError> {
    if chat != ChatState::Idle {
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
                "repo.patch"
                    | "repo.create-file"
                    | "repo.edit-files"
                    | "repo.delete-file"
                    | "repo.commit"
            );
            let commit = (tool == "repo.commit")
                .then(|| commit_activity_presentation(output))
                .flatten();
            (
                ActivityEvent::Finished {
                    tool,
                    result: if output.is_error {
                        ActivityResult::Failed
                    } else {
                        ActivityResult::Success
                    },
                    commit,
                },
                refresh,
            )
        }),
        _ => None,
    }
}

#[cfg(target_os = "windows")]
fn commit_activity_presentation(
    output: &rah_protocol::ToolOutput,
) -> Option<CommitActivityPresentation> {
    let [ToolContent::Text(text)] = output.content.as_slice() else {
        return None;
    };
    let value = serde_json::from_str::<serde_json::Value>(text).ok()?;
    if !value
        .as_object()?
        .keys()
        .all(|key| key == "status" || key == "commit_oid")
    {
        return None;
    }
    let status = match value.get("status")?.as_str()? {
        "invalid_input" => CommitActivityStatus::InvalidInput,
        "precondition_failed" => CommitActivityStatus::PreconditionFailed,
        "known_no_effect" => CommitActivityStatus::KnownNoEffect,
        "committed_verified" => CommitActivityStatus::CommittedVerified,
        "uncertain" => CommitActivityStatus::Uncertain,
        _ => return None,
    };
    if output.is_error == matches!(status, CommitActivityStatus::CommittedVerified) {
        return None;
    }
    let commit_oid = value.get("commit_oid").and_then(serde_json::Value::as_str);
    let commit_oid = match status {
        CommitActivityStatus::CommittedVerified => {
            let oid = commit_oid?;
            if (oid.len() == 40 || oid.len() == 64)
                && oid.bytes().all(|byte| byte.is_ascii_hexdigit())
            {
                Some(oid.to_owned())
            } else {
                return None;
            }
        }
        _ if commit_oid.is_none() => None,
        _ => return None,
    };
    Some(CommitActivityPresentation { status, commit_oid })
}

#[cfg(target_os = "windows")]
fn emit_activity_event(app: &AppHandle, event: ActivityEvent) {
    if let Err(error) = app.emit("activity_event", event) {
        tracing::warn!(error = %error, "failed to emit desktop activity event");
    }
}

#[cfg(target_os = "windows")]
fn append_live_evidence(record: serde_json::Value) {
    use std::io::Write;

    let Some(path) = std::env::var_os("RAH_LIVE_EVIDENCE_PATH") else {
        return;
    };
    let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    else {
        return;
    };
    let Ok(line) = serde_json::to_string(&record) else {
        return;
    };
    let _ = writeln!(file, "{line}");
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
    chat_generation: u64,
) {
    let handle = match runtime.start(request).await {
        Ok(handle) => handle,
        Err(error) => {
            tracing::warn!(error = %error, "desktop chat turn failed to start");
            if app
                .state::<DesktopAppState>()
                .claim_start_failure(chat_generation)
            {
                emit_chat_event(
                    &app,
                    ChatEvent::Failed {
                        code: FrontendError::ChatStartFailed,
                    },
                );
            }
            return;
        }
    };

    if !app.state::<DesktopAppState>().register_chat_session(
        chat_generation,
        Arc::clone(&runtime),
        handle.session_id().clone(),
    ) {
        tracing::warn!("desktop chat session was no longer current before streaming began");
        return;
    }

    let session_id = handle.session_id().clone();
    emit_chat_event(&app, ChatEvent::Started);
    let mut terminal = false;
    let mut tool_calls = HashMap::new();
    let mut events = handle.into_events();
    while let Some(event) = events.next().await {
        if let Some((activity, refresh_repository)) = activity_event(&event, &mut tool_calls) {
            if refresh_repository {
                invalidate_repository_commit_review(app.state::<DesktopAppState>().inner()).await;
            }
            emit_activity_event(&app, activity);
            if refresh_repository {
                append_live_evidence(serde_json::json!({
                    "event": "repository_refresh",
                    "reason": "repo.delete-file",
                }));
                emit_repository_refresh(&app);
            }
        }
        match event {
            AgentEvent::ModelDelta { delta, .. } => {
                if app.state::<DesktopAppState>().is_current_chat(
                    chat_generation,
                    &runtime,
                    &session_id,
                ) {
                    emit_chat_event(&app, ChatEvent::Delta { text: delta });
                }
            }
            AgentEvent::Completed { output, .. } => {
                if !app.state::<DesktopAppState>().claim_terminal(
                    chat_generation,
                    &runtime,
                    &session_id,
                ) {
                    return;
                }
                let assistant_text = output.message.content.clone();
                append_live_evidence(serde_json::json!({
                    "event": "desktop_completed",
                    "final_text": assistant_text,
                    "marker_observed": assistant_text.contains("RAH_REPO_DELETE_FILE_LIVE_OK"),
                }));
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
            AgentEvent::Failed { message, .. } => {
                // The frontend receives only a closed error code. Retain the
                // adapter-provided stage privately for live diagnosis.
                tracing::warn!(stage = "post-start runtime/event failure", error = %message, "desktop chat turn failed after start");
                if !app.state::<DesktopAppState>().claim_terminal(
                    chat_generation,
                    &runtime,
                    &session_id,
                ) {
                    return;
                }
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
                if !app.state::<DesktopAppState>().claim_terminal(
                    chat_generation,
                    &runtime,
                    &session_id,
                ) {
                    return;
                }
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
    if !terminal
        && app
            .state::<DesktopAppState>()
            .claim_terminal(chat_generation, &runtime, &session_id)
    {
        emit_chat_event(
            &app,
            ChatEvent::Failed {
                code: FrontendError::ChatRuntimeFailed,
            },
        );
    }
}

#[cfg(target_os = "windows")]
#[tauri::command]
async fn send_chat(
    prompt: String,
    app: AppHandle,
    state: State<'_, DesktopAppState>,
) -> Result<SendChatResult, FrontendError> {
    validate_prompt(&prompt)?;
    let chat_generation = state.start_chat()?;
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
                state.finish_chat(chat_generation);
                return Err(FrontendError::CodexNotConnected);
            }
        }
    };
    let current_repository_generation = *state
        .repository_generation
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let current_model_generation = state
        .model
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .generation;
    if !connection_context_is_current(
        identity.repository_generation,
        identity.model_generation,
        current_repository_generation,
        current_model_generation,
    ) {
        state.finish_chat(chat_generation);
        return Err(FrontendError::CodexReconnectRequired);
    }
    let (request, conversation_epoch, context_change) = {
        let mut conversation = state
            .conversation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let context_change = conversation.reconcile(identity);
        let messages = match conversation.request_messages(&prompt) {
            Ok(messages) => messages,
            Err(error) => {
                state.finish_chat(chat_generation);
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
    tauri::async_runtime::spawn(run_chat(
        app,
        runtime,
        request,
        prompt,
        conversation_epoch,
        chat_generation,
    ));
    Ok(SendChatResult { context_change })
}

#[cfg(target_os = "windows")]
#[tauri::command]
async fn cancel_chat(
    app: AppHandle,
    state: State<'_, DesktopAppState>,
) -> Result<(), FrontendError> {
    let (generation, runtime, session_id) = state.active_chat()?;
    if !state.request_cancel(generation, &runtime, &session_id) {
        return Err(FrontendError::ChatNotRunning);
    }
    let outcome = await_cancel_recovery(
        runtime.cancel(session_id.clone()),
        CANCEL_GRACEFUL_TIMEOUT,
        |_| {
            state.claim_terminal(generation, &runtime, &session_id)
                && state.begin_hard_recovery(&runtime)
        },
        || runtime.shutdown(),
        CANCEL_HARD_SHUTDOWN_TIMEOUT,
    )
    .await;
    match outcome {
        CancelRecoveryOutcome::Graceful(GracefulCancelOutcome::Completed) => {
            // Either the adapter terminal event won first, or successful cancellation does.
            // In both orders there is exactly one terminal owner and no completed persistence
            // after cancellation owns the generation.
            if state.claim_terminal(generation, &runtime, &session_id) {
                emit_chat_event(
                    &app,
                    ChatEvent::Cancelled {
                        code: FrontendError::ChatCancelled,
                    },
                );
            }
        }
        CancelRecoveryOutcome::Hard(hard) => {
            if hard != HardShutdownOutcome::Completed {
                tracing::warn!("bounded hard Codex shutdown did not complete successfully");
            }
            state.finish_hard_recovery(hard == HardShutdownOutcome::Completed);
            // This is a Desktop recovery outcome, not a claim that a remote provider request
            // rolled back.
            emit_chat_event(
                &app,
                ChatEvent::Failed {
                    code: FrontendError::ChatRuntimeFailed,
                },
            );
        }
        CancelRecoveryOutcome::Stale
        | CancelRecoveryOutcome::Graceful(
            GracefulCancelOutcome::Failed | GracefulCancelOutcome::TimedOut,
        ) => {}
    }
    Ok(())
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
        != ChatState::Idle
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
    state.select_persistence_namespace();
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
    if chat != ChatState::Idle {
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
    state.select_persistence_namespace();
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
    state.select_persistence_namespace();
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
            commit_identity,
            desktop_preferences_warning,
            set_model_configuration,
            set_commit_identity,
            reset_model_preferences,
            test_llama_cpp_endpoint,
            choose_repository,
            connect_codex,
            disconnect_codex,
            repository_snapshot,
            repository_authorize_commit_review,
            repository_stage_action,
            repository_unstage_action,
            send_chat,
            cancel_chat,
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
        ActivityEvent, ActivityResult, CancelRecoveryOutcome, ChatEvent, ChatState,
        CodexExecutableSourcePresentation, CommitAuthorizationPresentation, ConnectRequest,
        ConnectionState, ConversationContextChange, ConversationContextIdentity, DESKTOP_TOOL_NAME,
        DesktopAppState, DesktopCommitCapability, DesktopCommitIdentity, DesktopConversationState,
        DesktopModelProvider, DesktopModelSelection, DesktopModelState, DesktopRepository,
        FrontendError, GracefulCancelOutcome, HardShutdownOutcome, LlamaCppReadinessProbe,
        MAX_CONVERSATION_REPLAY_BYTES, MAX_CONVERSATION_REPLAY_MESSAGES, MAX_PROMPT_BYTES,
        ModelConfigurationPresentation, NEUTRAL_WORKSPACE_DIRECTORY, Preferences,
        PreferencesWarning, ProviderEndpoint, ProviderEndpointInput, ProviderEndpointPresentation,
        ProviderScheme, READINESS_BODY_LIMIT, READINESS_TOTAL_TIMEOUT, ReadinessState,
        RepositoryIndexActionKind, RepositoryObservationStage, ResumePair, SendChatResult,
        StagedReviewPresentation, TerminalOwnership, activity_event, apply_model_selection,
        authorize_repository_commit_review, await_cancel_recovery, await_graceful_cancel,
        await_hard_shutdown, begin_chat, clear_conversation_allowed, commit_activity_presentation,
        connect_prepared_codex, connection_context_is_current, current_app_status,
        desktop_repository_snapshot, desktop_repository_snapshot_with_review,
        desktop_tool_registry, frontend_error, install_repository_workflow,
        invalidate_repository_commit_review, model_configuration_status, prepare_codex_connection,
        publish_readiness_result, refresh_repository_workflow, replace_selected_repository,
        repository_index_action, repository_selection_allowed, repository_tool_authority,
        request_connect, resolve_codex_executable, resolve_prepare_and_connect_codex,
        revoke_repository_commit_context, same_arc, selected_git_executable, validate_prompt,
    };
    use futures::StreamExt;
    use rah_protocol::{
        AgentEvent, AgentInput, AgentOptions, AgentRequest, Message, MessageRole, PermissionLevel,
        RequestId, SessionId, ToolCall, ToolCallId, ToolContent, ToolInput, ToolName, ToolOutput,
    };
    use rah_runtime::AgentRuntime;
    use rah_runtime_codex::{
        CodexAdapterError, CodexLlamaCppProvider, CodexModelConfig, CodexModelProvider,
        CodexRuntime,
    };
    use rah_tools::{RepositoryCommitTool, RepositoryFileDeletionAuthority, ToolContext};
    use sha2::{Digest, Sha256};
    use std::{
        collections::HashMap,
        ffi::OsString,
        fs,
        io::{Read, Write},
        net::{Ipv4Addr, SocketAddrV4, TcpListener},
        path::PathBuf,
        process::Command,
        sync::{
            Arc, Mutex,
            atomic::{AtomicBool, AtomicU64, Ordering},
        },
        thread,
        time::{Duration, SystemTime, UNIX_EPOCH},
    };

    #[derive(Debug)]
    struct RuntimeMarker;

    fn default_test_endpoint() -> ProviderEndpoint {
        ProviderEndpoint::parse(ProviderEndpointInput {
            scheme: ProviderScheme::Http,
            host: "127.0.0.1".to_owned(),
            port: 8080,
        })
        .expect("default endpoint is valid")
    }

    struct ReadinessTestServer {
        endpoint: ProviderEndpoint,
        requests: std::sync::mpsc::Receiver<String>,
        join: thread::JoinHandle<()>,
    }

    impl ReadinessTestServer {
        fn start(response: String, requests_to_serve: usize) -> Self {
            let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0))
                .expect("readiness test listener binds loopback");
            listener
                .set_nonblocking(false)
                .expect("readiness listener is blocking");
            let port = listener
                .local_addr()
                .expect("readiness listener has address")
                .port();
            let endpoint = ProviderEndpoint::parse(ProviderEndpointInput {
                scheme: ProviderScheme::Http,
                host: "127.0.0.1".to_owned(),
                port,
            })
            .expect("loopback endpoint is valid");
            let (sender, requests) = std::sync::mpsc::channel();
            let join = thread::spawn(move || {
                for _ in 0..requests_to_serve {
                    let (mut stream, _) = listener.accept().expect("readiness request arrives");
                    stream
                        .set_read_timeout(Some(Duration::from_secs(2)))
                        .expect("request read timeout configures");
                    let mut request = Vec::new();
                    let mut buffer = [0_u8; 512];
                    while !request.windows(4).any(|window| window == b"\r\n\r\n") {
                        let read = stream.read(&mut buffer).expect("request reads");
                        if read == 0 {
                            break;
                        }
                        request.extend_from_slice(&buffer[..read]);
                    }
                    sender
                        .send(String::from_utf8(request).expect("request is HTTP text"))
                        .expect("request observation is received");
                    stream
                        .write_all(response.as_bytes())
                        .expect("readiness response writes");
                }
            });
            Self {
                endpoint,
                requests,
                join,
            }
        }

        fn request(&self) -> String {
            self.requests
                .recv_timeout(Duration::from_secs(2))
                .expect("exactly one readiness request")
        }

        fn finish(self) {
            self.join.join().expect("readiness server exits");
        }
    }

    fn readiness_check(endpoint: &ProviderEndpoint) -> ReadinessState {
        tauri::async_runtime::block_on(LlamaCppReadinessProbe::check(endpoint))
    }

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

        fn deletion_repository(&self) -> DesktopRepository {
            let git = Self::native_git();
            let authority = RepositoryFileDeletionAuthority::new(&git, &self.0)
                .expect("host deletion authority should construct");
            DesktopRepository::new_with_deletion_authority(&git, &self.0, Some(authority))
                .expect("selected deletion repository should construct")
        }

        fn native_git() -> PathBuf {
            let output = Command::new("where.exe")
                .arg("git.exe")
                .output()
                .expect("where.exe should locate native Git");
            assert!(
                output.status.success(),
                "native Git must be installed for this test"
            );
            let path = String::from_utf8(output.stdout).expect("Git path is UTF-8");
            fs::canonicalize(
                path.lines()
                    .next()
                    .expect("where.exe should return one Git path"),
            )
            .expect("Git path should canonicalize")
        }

        fn git_repository(state: GitRepositoryState) -> Self {
            let repository = Self::new();
            let git = Self::native_git();
            fs::remove_dir_all(repository.0.join(".git"))
                .expect("placeholder metadata should be removed before git init");
            fs::remove_file(repository.0.join("inside.txt"))
                .expect("placeholder file should be removed before git init");
            let run = |arguments: &[&str]| {
                let output = Command::new(&git)
                    .args(arguments)
                    .current_dir(&repository.0)
                    .output()
                    .expect("native Git command should start");
                assert!(
                    output.status.success(),
                    "native Git fixture command should succeed"
                );
            };
            run(&["init", "--quiet"]);
            run(&["config", "user.email", "rah-desktop-test@example.invalid"]);
            run(&["config", "user.name", "RAH Desktop Test"]);
            fs::create_dir_all(repository.0.join("nested"))
                .expect("nested fixture directory should be created");
            fs::write(repository.0.join("tracked.txt"), "base\n")
                .expect("tracked fixture should be written");
            fs::write(
                repository.0.join("nested").join("ordinary.txt"),
                "ordinary\n",
            )
            .expect("nested tracked fixture should be written");
            run(&["add", "tracked.txt", "nested/ordinary.txt"]);
            run(&["commit", "--quiet", "-m", "fixture"]);
            match state {
                GitRepositoryState::Clean => {}
                GitRepositoryState::Untracked => {
                    fs::write(repository.0.join("untracked.txt"), "untracked\n")
                        .expect("untracked fixture should be written");
                }
                GitRepositoryState::Modified => {
                    fs::write(
                        repository.0.join("nested").join("ordinary.txt"),
                        "modified\n",
                    )
                    .expect("modified fixture should be written");
                }
                GitRepositoryState::Staged => {
                    fs::write(repository.0.join("tracked.txt"), "staged\n")
                        .expect("staged fixture should be written");
                    run(&["add", "tracked.txt"]);
                }
            }
            repository
        }
    }

    impl Drop for TestRepository {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[derive(Clone, Copy)]
    enum GitRepositoryState {
        Clean,
        Untracked,
        Modified,
        Staged,
    }

    #[tokio::test(flavor = "current_thread")]
    async fn repository_snapshot_matrix_isolated_repositories_and_replacements() {
        let repository_a = TestRepository::git_repository(GitRepositoryState::Clean);
        let repository_b = TestRepository::git_repository(GitRepositoryState::Untracked);
        let repository_c = TestRepository::git_repository(GitRepositoryState::Staged);
        let repository_d = TestRepository::git_repository(GitRepositoryState::Modified);
        let git = TestRepository::native_git();

        let a = DesktopRepository::new(&git, &repository_a.0).expect("A constructs");
        let b = DesktopRepository::new(&git, &repository_b.0).expect("B constructs");
        let c = DesktopRepository::new(&git, &repository_c.0).expect("C constructs");
        let snapshot_a = desktop_repository_snapshot(&a)
            .await
            .expect("A snapshot succeeds");
        assert_eq!(snapshot_a.status_entries.len(), 0, "{snapshot_a:#?}");
        assert!(snapshot_a.worktree_diff.is_empty());
        assert!(snapshot_a.staged_diff.is_empty());
        let snapshot_b = desktop_repository_snapshot(&b)
            .await
            .expect("B snapshot succeeds");
        assert_eq!(snapshot_b.status_entries.len(), 1);
        assert!(snapshot_b.worktree_diff.is_empty());
        assert!(snapshot_b.staged_diff.is_empty());
        let snapshot_c = desktop_repository_snapshot(&c)
            .await
            .expect("C snapshot succeeds");
        assert_eq!(snapshot_c.status_entries.len(), 1);
        assert!(snapshot_c.worktree_diff.is_empty());
        assert_eq!(snapshot_c.staged_diff.len(), 1);
        let d = DesktopRepository::new(&git, &repository_d.0).expect("D constructs");
        let snapshot_d = desktop_repository_snapshot(&d)
            .await
            .expect("D snapshot succeeds");
        assert_eq!(snapshot_d.status_entries.len(), 1);
        assert_eq!(snapshot_d.worktree_diff.len(), 1);
        assert!(snapshot_d.staged_diff.is_empty());

        let storage = TestRepository::new();
        let state = DesktopAppState::new(storage.0.clone());
        replace_selected_repository(&state, a);
        let first_a = state
            .repository
            .lock()
            .unwrap()
            .clone()
            .expect("A is selected");
        assert_eq!(*state.repository_generation.lock().unwrap(), 1);
        assert!(desktop_repository_snapshot(&first_a).await.is_ok());

        replace_selected_repository(&state, b);
        let selected_b = state
            .repository
            .lock()
            .unwrap()
            .clone()
            .expect("B is selected");
        assert!(!Arc::ptr_eq(&first_a, &selected_b));
        assert_eq!(*state.repository_generation.lock().unwrap(), 2);
        assert_eq!(
            desktop_repository_snapshot(&selected_b)
                .await
                .unwrap()
                .status_entries
                .len(),
            1
        );

        let replacement_a =
            DesktopRepository::new(&git, &repository_a.0).expect("replacement A constructs");
        replace_selected_repository(&state, replacement_a);
        let second_a = state
            .repository
            .lock()
            .unwrap()
            .clone()
            .expect("replacement A is selected");
        assert!(!Arc::ptr_eq(&first_a, &second_a));
        assert_eq!(*state.repository_generation.lock().unwrap(), 3);
        assert!(desktop_repository_snapshot(&second_a).await.is_ok());

        replace_selected_repository(&state, c);
        let selected_c = state
            .repository
            .lock()
            .unwrap()
            .clone()
            .expect("C is selected");
        assert_eq!(*state.repository_generation.lock().unwrap(), 4);
        assert_eq!(
            desktop_repository_snapshot(&selected_c)
                .await
                .unwrap()
                .staged_diff
                .len(),
            1
        );

        let fresh_state = DesktopAppState::new(storage.0.clone());
        let fresh_b = DesktopRepository::new(&git, &repository_b.0).expect("fresh B constructs");
        replace_selected_repository(&fresh_state, fresh_b);
        let fresh_b = fresh_state
            .repository
            .lock()
            .unwrap()
            .clone()
            .expect("fresh B selected");
        assert_eq!(*fresh_state.repository_generation.lock().unwrap(), 1);
        assert_eq!(
            desktop_repository_snapshot(&fresh_b)
                .await
                .unwrap()
                .status_entries
                .len(),
            1
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn restored_identity_and_fresh_bound_review_serialize_authorize_presentation() {
        let storage = TestRepository::new();
        let identity = DesktopCommitIdentity {
            name: "RAH Host".to_owned(),
            email: "rah-host@example.invalid".to_owned(),
        };
        let initial = DesktopAppState::new(storage.0.clone());
        let selection = initial.model.lock().unwrap().selection.clone();
        initial
            .preferences
            .lock()
            .unwrap()
            .save_identity(&selection, identity.clone())
            .expect("identity persists before restart");
        drop(initial);

        let state = DesktopAppState::new(storage.0.clone());
        assert_eq!(
            *state.commit_identity.lock().unwrap(),
            Some(identity.clone())
        );
        assert!(state.commit_capability.lock().unwrap().is_none());

        let repository = TestRepository::git_repository(GitRepositoryState::Staged);
        let git = TestRepository::native_git();
        replace_selected_repository(
            &state,
            DesktopRepository::new(&git, &repository.0).expect("staged repository constructs"),
        );
        let repository_generation = *state.repository_generation.lock().unwrap();
        let model_generation = state.model.lock().unwrap().generation;
        let identity_generation = *state.commit_identity_generation.lock().unwrap();
        let selected = state
            .repository
            .lock()
            .unwrap()
            .clone()
            .expect("repository remains selected");
        let (tool, control) = RepositoryCommitTool::compose(
            &selected.git_executable,
            &selected.root,
            identity.name,
            identity.email,
        )
        .expect("current paired commit capability constructs");
        *state.commit_capability.lock().unwrap() = Some(DesktopCommitCapability {
            repository_generation,
            model_generation,
            identity_generation,
            _tool: Arc::new(tool),
            control: Arc::new(control),
        });

        let control = state
            .commit_capability
            .lock()
            .unwrap()
            .as_ref()
            .map(|capability| Arc::clone(&capability.control));
        let (snapshot, review) = desktop_repository_snapshot_with_review(&selected, control)
            .await
            .expect("fresh bound staged review");
        let snapshot = install_repository_workflow(
            &state,
            &selected,
            repository_generation,
            snapshot,
            review,
            identity_generation,
        );
        let StagedReviewPresentation::ReviewAvailable {
            review_id,
            can_authorize,
            authorization_state,
        } = &snapshot.review
        else {
            panic!("staged textual review must be available");
        };
        assert!(review_id.is_some());
        assert!(*can_authorize);
        assert_eq!(
            *authorization_state,
            super::CommitAuthorizationPresentation::ReadyToAuthorize
        );
        let serialized = serde_json::to_value(&snapshot).expect("snapshot serializes");
        assert_eq!(serialized["review"]["state"], "review_available");
        assert_eq!(serialized["review"]["canAuthorize"], true);
        assert!(serialized["review"]["reviewId"].is_string());
        assert_eq!(
            serialized["review"]["authorizationState"],
            "ready_to_authorize"
        );
        assert!(serialized["review"]["can_authorize"].is_null());
        assert!(serialized["review"]["review_id"].is_null());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn binary_staged_review_is_not_authorizable_and_forged_selector_has_no_effect() {
        let repository = TestRepository::git_repository(GitRepositoryState::Clean);
        let storage = TestRepository::new();
        let state = DesktopAppState::new(storage.0.clone());
        let identity = DesktopCommitIdentity {
            name: "RAH Host".to_owned(),
            email: "rah-host@example.invalid".to_owned(),
        };
        *state.commit_identity.lock().unwrap() = Some(identity.clone());
        replace_selected_repository(
            &state,
            DesktopRepository::new(&TestRepository::native_git(), &repository.0)
                .expect("binary repository constructs"),
        );
        fs::write(repository.0.join("binary.dat"), [0_u8, 159, 146, 150])
            .expect("binary fixture writes");
        let git = TestRepository::native_git();
        assert!(
            Command::new(&git)
                .args(["add", "binary.dat"])
                .current_dir(&repository.0)
                .status()
                .expect("binary fixture stages")
                .success()
        );
        let repository_generation = *state.repository_generation.lock().unwrap();
        let model_generation = state.model.lock().unwrap().generation;
        let identity_generation = *state.commit_identity_generation.lock().unwrap();
        let selected = state
            .repository
            .lock()
            .unwrap()
            .clone()
            .expect("repository selected");
        let (tool, control) = RepositoryCommitTool::compose(
            &selected.git_executable,
            &selected.root,
            identity.name,
            identity.email,
        )
        .expect("current paired commit capability constructs");
        let control = Arc::new(control);
        *state.commit_capability.lock().unwrap() = Some(DesktopCommitCapability {
            repository_generation,
            model_generation,
            identity_generation,
            _tool: Arc::new(tool),
            control: Arc::clone(&control),
        });
        let git_output = |arguments: &[&str]| {
            Command::new(&git)
                .args(arguments)
                .current_dir(&repository.0)
                .output()
                .expect("Git state command should start")
        };
        let head_before = git_output(&["rev-parse", "HEAD"]);
        let refs_before = git_output(&["show-ref", "--head"]);
        let worktree_before = fs::read(repository.0.join("binary.dat")).expect("worktree reads");

        let (snapshot, review) =
            desktop_repository_snapshot_with_review(&selected, Some(Arc::clone(&control)))
                .await
                .expect("binary staged observation succeeds");
        assert!(snapshot.staged_diff.iter().any(|file| file.binary));
        assert!(review.is_none());
        let index_before = fs::read(repository.0.join(".git/index")).expect("index reads");
        let snapshot = install_repository_workflow(
            &state,
            &selected,
            repository_generation,
            snapshot,
            review,
            identity_generation,
        );
        assert!(matches!(
            snapshot.review,
            StagedReviewPresentation::ReviewBinaryUnsupported
        ));
        {
            let workflow = state.repository_workflow.lock().unwrap();
            assert!(workflow.commit_review.is_none());
            assert!(workflow.review_selector.is_none());
            assert_ne!(
                workflow.authorization,
                CommitAuthorizationPresentation::ReadyToAuthorize
            );
        }
        assert!(matches!(
            authorize_repository_commit_review(&state, "review-forged").await,
            Err(FrontendError::CommitAuthorizationStale)
        ));
        assert!(!control.has_pending_authorization().await);
        assert_eq!(
            git_output(&["rev-parse", "HEAD"]).stdout,
            head_before.stdout
        );
        assert_eq!(
            git_output(&["show-ref", "--head"]).stdout,
            refs_before.stdout
        );
        assert_eq!(
            fs::read(repository.0.join(".git/index")).unwrap(),
            index_before
        );
        assert_eq!(
            fs::read(repository.0.join("binary.dat")).unwrap(),
            worktree_before
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn authorize_then_refresh_revokes_pending_and_rotates_review_selector_without_commit() {
        let repository = TestRepository::git_repository(GitRepositoryState::Staged);
        let storage = TestRepository::new();
        let state = DesktopAppState::new(storage.0.clone());
        let identity = DesktopCommitIdentity {
            name: "RAH Host".to_owned(),
            email: "rah-host@example.invalid".to_owned(),
        };
        *state.commit_identity.lock().unwrap() = Some(identity.clone());
        replace_selected_repository(
            &state,
            DesktopRepository::new(&TestRepository::native_git(), &repository.0)
                .expect("staged repository constructs"),
        );
        let repository_generation = *state.repository_generation.lock().unwrap();
        let model_generation = state.model.lock().unwrap().generation;
        let identity_generation = *state.commit_identity_generation.lock().unwrap();
        let selected = state
            .repository
            .lock()
            .unwrap()
            .clone()
            .expect("repository selected");
        let (tool, control) = RepositoryCommitTool::compose(
            &selected.git_executable,
            &selected.root,
            identity.name,
            identity.email,
        )
        .expect("current paired commit capability constructs");
        let control = Arc::new(control);
        *state.commit_capability.lock().unwrap() = Some(DesktopCommitCapability {
            repository_generation,
            model_generation,
            identity_generation,
            _tool: Arc::new(tool),
            control: Arc::clone(&control),
        });
        let (snapshot, review) =
            desktop_repository_snapshot_with_review(&selected, Some(Arc::clone(&control)))
                .await
                .expect("fresh bound staged review");
        let first = install_repository_workflow(
            &state,
            &selected,
            repository_generation,
            snapshot,
            review,
            identity_generation,
        );
        let StagedReviewPresentation::ReviewAvailable {
            review_id: Some(old_review_id),
            can_authorize: true,
            authorization_state: super::CommitAuthorizationPresentation::ReadyToAuthorize,
        } = first.review
        else {
            panic!("fresh review must be ready to authorize: {first:#?}");
        };
        let git_output = |arguments: &[&str]| {
            Command::new(TestRepository::native_git())
                .args(arguments)
                .current_dir(&repository.0)
                .output()
                .expect("Git state command should start")
        };
        let head_before = git_output(&["rev-parse", "HEAD"]);
        let refs_before = git_output(&["show-ref", "--head"]);
        let index_before = fs::read(repository.0.join(".git/index")).expect("index reads");
        let worktree_before = fs::read(repository.0.join("tracked.txt")).expect("worktree reads");
        let count_before = git_output(&["rev-list", "--count", "HEAD"]);

        let authorized = authorize_repository_commit_review(&state, &old_review_id)
            .await
            .expect("current review authorizes");
        assert_eq!(
            authorized.authorization_state,
            super::CommitAuthorizationPresentation::AuthorizedPending
        );
        assert!(control.has_pending_authorization().await);
        assert_eq!(
            state.repository_workflow.lock().unwrap().authorization,
            super::CommitAuthorizationPresentation::AuthorizedPending
        );

        invalidate_repository_commit_review(&state).await;
        assert!(!control.has_pending_authorization().await);
        assert!(
            state
                .repository_workflow
                .lock()
                .unwrap()
                .commit_review
                .is_none()
        );
        let (snapshot, review) =
            desktop_repository_snapshot_with_review(&selected, Some(Arc::clone(&control)))
                .await
                .expect("fresh bound staged review after revocation");
        let refreshed = install_repository_workflow(
            &state,
            &selected,
            repository_generation,
            snapshot,
            review,
            identity_generation,
        );
        let StagedReviewPresentation::ReviewAvailable {
            review_id: Some(new_review_id),
            can_authorize: true,
            authorization_state: super::CommitAuthorizationPresentation::ReadyToAuthorize,
        } = refreshed.review
        else {
            panic!("refresh must create a new ready review: {refreshed:#?}");
        };
        assert_ne!(old_review_id, new_review_id);
        assert!(!control.has_pending_authorization().await);
        assert!(matches!(
            authorize_repository_commit_review(&state, &old_review_id).await,
            Err(FrontendError::CommitAuthorizationStale)
        ));
        assert_eq!(
            git_output(&["rev-parse", "HEAD"]).stdout,
            head_before.stdout
        );
        assert_eq!(
            git_output(&["show-ref", "--head"]).stdout,
            refs_before.stdout
        );
        assert_eq!(
            fs::read(repository.0.join(".git/index")).unwrap(),
            index_before
        );
        assert_eq!(
            fs::read(repository.0.join("tracked.txt")).unwrap(),
            worktree_before
        );
        assert_eq!(
            git_output(&["rev-list", "--count", "HEAD"]).stdout,
            count_before.stdout
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn disconnect_revokes_pending_authorization_and_never_restores_old_review() {
        let repository = TestRepository::git_repository(GitRepositoryState::Staged);
        let storage = TestRepository::new();
        let state = DesktopAppState::new(storage.0.clone());
        let identity = DesktopCommitIdentity {
            name: "RAH Host".to_owned(),
            email: "rah-host@example.invalid".to_owned(),
        };
        *state.commit_identity.lock().unwrap() = Some(identity.clone());
        replace_selected_repository(
            &state,
            DesktopRepository::new(&TestRepository::native_git(), &repository.0)
                .expect("staged repository constructs"),
        );
        let repository_generation = *state.repository_generation.lock().unwrap();
        let model_generation = state.model.lock().unwrap().generation;
        let identity_generation = *state.commit_identity_generation.lock().unwrap();
        let selected = state
            .repository
            .lock()
            .unwrap()
            .clone()
            .expect("repository selected");
        let (tool, control) = RepositoryCommitTool::compose(
            &selected.git_executable,
            &selected.root,
            identity.name.clone(),
            identity.email.clone(),
        )
        .expect("current paired commit capability constructs");
        let control = Arc::new(control);
        *state.commit_capability.lock().unwrap() = Some(DesktopCommitCapability {
            repository_generation,
            model_generation,
            identity_generation,
            _tool: Arc::new(tool),
            control: Arc::clone(&control),
        });
        let (snapshot, review) =
            desktop_repository_snapshot_with_review(&selected, Some(Arc::clone(&control)))
                .await
                .expect("fresh bound review");
        let first = install_repository_workflow(
            &state,
            &selected,
            repository_generation,
            snapshot,
            review,
            identity_generation,
        );
        let StagedReviewPresentation::ReviewAvailable {
            review_id: Some(old_review_id),
            can_authorize: true,
            authorization_state: super::CommitAuthorizationPresentation::ReadyToAuthorize,
        } = first.review
        else {
            panic!("fresh review must be ready to authorize: {first:#?}");
        };
        let git_output = |arguments: &[&str]| {
            Command::new(TestRepository::native_git())
                .args(arguments)
                .current_dir(&repository.0)
                .output()
                .expect("Git state command should start")
        };
        let head_before = git_output(&["rev-parse", "HEAD"]);
        let refs_before = git_output(&["show-ref", "--head"]);
        let index_before = fs::read(repository.0.join(".git/index")).expect("index reads");
        let worktree_before = fs::read(repository.0.join("tracked.txt")).expect("worktree reads");
        let count_before = git_output(&["rev-list", "--count", "HEAD"]);

        authorize_repository_commit_review(&state, &old_review_id)
            .await
            .expect("authorize");
        assert!(control.has_pending_authorization().await);
        revoke_repository_commit_context(&state).await;

        assert!(
            !control.has_pending_authorization().await,
            "disconnect clears the control, not merely presentation"
        );
        assert!(state.commit_capability.lock().unwrap().is_none());
        {
            let workflow = state.repository_workflow.lock().unwrap();
            assert!(workflow.commit_review.is_none());
            assert!(workflow.review_selector.is_none());
            assert_ne!(
                workflow.authorization,
                super::CommitAuthorizationPresentation::AuthorizedPending
            );
        }
        assert!(matches!(
            authorize_repository_commit_review(&state, &old_review_id).await,
            Err(FrontendError::CommitAuthorizationUnavailable)
        ));
        let (snapshot, review) = desktop_repository_snapshot_with_review(&selected, None)
            .await
            .expect("redacted snapshot observation");
        let disconnected = install_repository_workflow(
            &state,
            &selected,
            repository_generation,
            snapshot,
            review,
            identity_generation,
        );
        let serialized = serde_json::to_value(&disconnected).expect("snapshot serializes");
        assert_ne!(
            serialized["review"]["authorizationState"],
            "authorized_pending"
        );
        let StagedReviewPresentation::ReviewAvailable {
            review_id,
            can_authorize,
            authorization_state,
        } = disconnected.review
        else {
            panic!("staged review remains observable but un-authorizable");
        };
        assert!(review_id.is_none());
        assert!(!can_authorize);
        assert_ne!(
            authorization_state,
            super::CommitAuthorizationPresentation::AuthorizedPending
        );

        let (tool, fresh_control) = RepositoryCommitTool::compose(
            &selected.git_executable,
            &selected.root,
            identity.name,
            identity.email,
        )
        .expect("reconnect creates fresh capability");
        let fresh_control = Arc::new(fresh_control);
        *state.commit_capability.lock().unwrap() = Some(DesktopCommitCapability {
            repository_generation,
            model_generation,
            identity_generation,
            _tool: Arc::new(tool),
            control: Arc::clone(&fresh_control),
        });
        assert!(!fresh_control.has_pending_authorization().await);
        let (snapshot, review) =
            desktop_repository_snapshot_with_review(&selected, Some(Arc::clone(&fresh_control)))
                .await
                .expect("fresh reconnect review");
        let reconnected = install_repository_workflow(
            &state,
            &selected,
            repository_generation,
            snapshot,
            review,
            identity_generation,
        );
        let StagedReviewPresentation::ReviewAvailable {
            review_id: Some(new_review_id),
            can_authorize: true,
            authorization_state: super::CommitAuthorizationPresentation::ReadyToAuthorize,
        } = reconnected.review
        else {
            panic!("reconnect requires a fresh review");
        };
        assert_ne!(old_review_id, new_review_id);
        assert!(!fresh_control.has_pending_authorization().await);
        assert_eq!(
            git_output(&["rev-parse", "HEAD"]).stdout,
            head_before.stdout
        );
        assert_eq!(
            git_output(&["show-ref", "--head"]).stdout,
            refs_before.stdout
        );
        assert_eq!(
            fs::read(repository.0.join(".git/index")).unwrap(),
            index_before
        );
        assert_eq!(
            fs::read(repository.0.join("tracked.txt")).unwrap(),
            worktree_before
        );
        assert_eq!(
            git_output(&["rev-list", "--count", "HEAD"]).stdout,
            count_before.stdout
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn observed_single_target_stage_and_unstage_refresh_and_consume_selectors() {
        let repository = TestRepository::git_repository(GitRepositoryState::Modified);
        let storage = TestRepository::new();
        let state = DesktopAppState::new(storage.0.clone());
        replace_selected_repository(
            &state,
            DesktopRepository::new(&TestRepository::native_git(), &repository.0)
                .expect("native Git repository constructs"),
        );
        let snapshot = refresh_repository_workflow(&state)
            .await
            .expect("modified snapshot");
        let stage = snapshot
            .status_entries
            .iter()
            .find_map(|entry| entry.stage_action_id.clone())
            .unwrap_or_else(|| panic!("tracked modification is stage eligible: {snapshot:#?}"));
        let before = fs::read(repository.0.join("nested/ordinary.txt")).unwrap();
        let staged =
            repository_index_action(&state, stage.clone(), RepositoryIndexActionKind::Stage)
                .await
                .expect("one stage action");
        assert_eq!(staged.status, "ok");
        assert!(staged.staged);
        assert_eq!(
            fs::read(repository.0.join("nested/ordinary.txt")).unwrap(),
            before
        );
        assert_eq!(
            repository_index_action(&state, stage, RepositoryIndexActionKind::Stage).await,
            Err(FrontendError::RepositoryActionInvalid)
        );
        let snapshot = refresh_repository_workflow(&state)
            .await
            .expect("staged snapshot");
        assert!(matches!(
            snapshot.review,
            super::StagedReviewPresentation::ReviewAvailable { .. }
        ));
        let unstage = snapshot
            .staged_diff
            .iter()
            .find_map(|file| file.unstage_action_id.clone())
            .expect("staged tracked modification is unstage eligible");
        let unstaged = repository_index_action(&state, unstage, RepositoryIndexActionKind::Unstage)
            .await
            .expect("one unstage action");
        assert_eq!(unstaged.status, "ok");
        assert!(unstaged.unstaged);
        assert_eq!(
            fs::read(repository.0.join("nested/ordinary.txt")).unwrap(),
            before
        );
        let snapshot = refresh_repository_workflow(&state)
            .await
            .expect("unstaged snapshot");
        assert!(matches!(
            snapshot.review,
            super::StagedReviewPresentation::NoStagedChanges
        ));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn old_repository_selector_cannot_resolve_to_new_repository_action() {
        let repository_a = TestRepository::git_repository(GitRepositoryState::Modified);
        let repository_b = TestRepository::git_repository(GitRepositoryState::Modified);
        let storage = TestRepository::new();
        let state = DesktopAppState::new(storage.0.clone());
        let git = TestRepository::native_git();

        replace_selected_repository(
            &state,
            DesktopRepository::new(&git, &repository_a.0).expect("A constructs"),
        );
        let old_action_id = refresh_repository_workflow(&state)
            .await
            .expect("A observation succeeds")
            .status_entries
            .into_iter()
            .find_map(|entry| entry.stage_action_id)
            .expect("A exposes one stage action");

        replace_selected_repository(
            &state,
            DesktopRepository::new(&git, &repository_b.0).expect("B constructs"),
        );
        let new_action_id = refresh_repository_workflow(&state)
            .await
            .expect("B observation succeeds")
            .status_entries
            .into_iter()
            .find_map(|entry| entry.stage_action_id)
            .expect("B exposes one stage action");
        assert_ne!(old_action_id, new_action_id);

        let git_output = |arguments: &[&str]| {
            Command::new(&git)
                .args(arguments)
                .current_dir(&repository_b.0)
                .output()
                .expect("Git state command should start")
        };
        let head_before = git_output(&["rev-parse", "HEAD"]);
        let refs_before = git_output(&["show-ref", "--head"]);
        let index_before = fs::read(repository_b.0.join(".git/index")).expect("B index reads");
        let worktree_before =
            fs::read(repository_b.0.join("nested/ordinary.txt")).expect("B worktree reads");

        assert!(matches!(
            repository_index_action(&state, old_action_id, RepositoryIndexActionKind::Stage).await,
            Err(FrontendError::RepositoryActionInvalid | FrontendError::RepositoryActionStale)
        ));
        assert_eq!(
            git_output(&["rev-parse", "HEAD"]).stdout,
            head_before.stdout
        );
        assert_eq!(
            git_output(&["show-ref", "--head"]).stdout,
            refs_before.stdout
        );
        assert_eq!(
            fs::read(repository_b.0.join(".git/index")).unwrap(),
            index_before
        );
        assert_eq!(
            fs::read(repository_b.0.join("nested/ordinary.txt")).unwrap(),
            worktree_before
        );

        let result =
            repository_index_action(&state, new_action_id, RepositoryIndexActionKind::Stage)
                .await
                .expect("B action works once");
        assert!(result.staged);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn review_digest_is_stable_for_unchanged_index_and_ignores_action_selectors() {
        let repository = TestRepository::git_repository(GitRepositoryState::Staged);
        let storage = TestRepository::new();
        let state = DesktopAppState::new(storage.0.clone());
        replace_selected_repository(
            &state,
            DesktopRepository::new(&TestRepository::native_git(), &repository.0)
                .expect("staged repository constructs"),
        );
        let first = refresh_repository_workflow(&state)
            .await
            .expect("first review");
        let first_action = first.staged_diff[0].unstage_action_id.clone();
        let digest_a = state
            .repository_workflow
            .lock()
            .unwrap()
            .review
            .as_ref()
            .expect("first review descriptor")
            .digest
            .clone();

        let second = refresh_repository_workflow(&state)
            .await
            .expect("second review");
        let second_action = second.staged_diff[0].unstage_action_id.clone();
        let digest_b = state
            .repository_workflow
            .lock()
            .unwrap()
            .review
            .as_ref()
            .expect("second review descriptor")
            .digest
            .clone();
        assert_ne!(first_action, second_action);
        assert_eq!(digest_a, digest_b);

        fs::write(
            repository.0.join("tracked.txt"),
            "different staged content\n",
        )
        .expect("staged target updates");
        let output = Command::new(TestRepository::native_git())
            .args(["add", "tracked.txt"])
            .current_dir(&repository.0)
            .output()
            .expect("Git add starts");
        assert!(output.status.success());
        refresh_repository_workflow(&state)
            .await
            .expect("changed review");
        let digest_c = state
            .repository_workflow
            .lock()
            .unwrap()
            .review
            .as_ref()
            .expect("changed review descriptor")
            .digest
            .clone();
        assert_ne!(digest_a, digest_c);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn same_size_target_change_rejects_displayed_stage_action() {
        let repository = TestRepository::git_repository(GitRepositoryState::Modified);
        let storage = TestRepository::new();
        let state = DesktopAppState::new(storage.0.clone());
        replace_selected_repository(
            &state,
            DesktopRepository::new(&TestRepository::native_git(), &repository.0)
                .expect("modified repository constructs"),
        );
        let action_id = refresh_repository_workflow(&state)
            .await
            .expect("observation succeeds")
            .status_entries
            .into_iter()
            .find_map(|entry| entry.stage_action_id)
            .expect("stage action exists");
        let target = repository.0.join("nested/ordinary.txt");
        assert_eq!(fs::read(&target).unwrap().len(), b"changed!\n".len());
        fs::write(&target, "changed!\n").expect("same-size target rewrite");

        assert_eq!(
            repository_index_action(&state, action_id, RepositoryIndexActionKind::Stage).await,
            Err(FrontendError::RepositoryActionStale)
        );
        let output = Command::new(TestRepository::native_git())
            .args(["diff", "--cached", "--quiet"])
            .current_dir(&repository.0)
            .output()
            .expect("Git index check starts");
        assert!(
            output.status.success(),
            "stale action must not stage the target"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn repository_snapshot_classifies_first_observer_execution_failure() {
        let fixture = TestRepository::new();
        let repository = fixture.desktop_repository();
        assert_eq!(
            desktop_repository_snapshot(&repository).await,
            Err(RepositoryObservationStage::StatusExecutionOrRevalidation)
        );
    }

    #[test]
    fn hardened_git_environment_requires_host_pinned_safe_directory_for_foreign_owner_diagnostic() {
        let repository = TestRepository::git_repository(GitRepositoryState::Clean);
        let git = TestRepository::native_git();
        let status = || {
            Command::new(&git)
                .args([
                    "--no-pager",
                    "status",
                    "--porcelain=v2",
                    "-z",
                    "--untracked-files=normal",
                    "--ignored=no",
                    "--no-renames",
                    "--ignore-submodules=all",
                ])
                .current_dir(&repository.0)
                .env_clear()
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .env("GIT_CONFIG_GLOBAL", "NUL")
                .env("GIT_CONFIG_COUNT", "2")
                .env("GIT_CONFIG_KEY_0", "core.fsmonitor")
                .env("GIT_CONFIG_VALUE_0", "false")
                .env("GIT_CONFIG_KEY_1", "core.untrackedCache")
                .env("GIT_CONFIG_VALUE_1", "false")
                .env("GIT_OPTIONAL_LOCKS", "0")
                .env("GIT_TERMINAL_PROMPT", "0")
                .env("GIT_TEST_ASSUME_DIFFERENT_OWNER", "1")
                .output()
                .expect("hardened native Git status should start")
        };
        assert!(
            !status().status.success(),
            "the diagnostic must reproduce Git's protected ownership refusal"
        );

        let output = Command::new(&git)
            .args([
                "--no-pager",
                "status",
                "--porcelain=v2",
                "-z",
                "--untracked-files=normal",
                "--ignored=no",
                "--no-renames",
                "--ignore-submodules=all",
            ])
            .current_dir(&repository.0)
            .env_clear()
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "NUL")
            .env("GIT_CONFIG_COUNT", "3")
            .env("GIT_CONFIG_KEY_0", "core.fsmonitor")
            .env("GIT_CONFIG_VALUE_0", "false")
            .env("GIT_CONFIG_KEY_1", "core.untrackedCache")
            .env("GIT_CONFIG_VALUE_1", "false")
            .env("GIT_CONFIG_KEY_2", "safe.directory")
            .env("GIT_CONFIG_VALUE_2", &repository.0)
            .env("GIT_OPTIONAL_LOCKS", "0")
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_TEST_ASSUME_DIFFERENT_OWNER", "1")
            .output()
            .expect("host-pinned safe-directory diagnostic should start");
        assert!(
            output.status.success(),
            "only the exact host-selected root should restore this observation"
        );
    }

    #[test]
    fn terminal_failure_and_cancellation_restore_desktop_chat_controls() {
        let storage = TestRepository::new();
        let state = DesktopAppState::new(storage.0.clone());

        let failed_turn = state.start_chat().expect("failed turn reserves chat state");
        assert_eq!(
            *state
                .chat
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
            ChatState::Running
        );
        state.finish_chat(failed_turn);
        assert_eq!(
            *state
                .chat
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
            ChatState::Idle
        );

        let cancelled_turn = state
            .start_chat()
            .expect("cancelled turn reserves chat state");
        state.finish_chat(cancelled_turn);
        assert_eq!(
            *state
                .chat
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
            ChatState::Idle
        );
        assert!(matches!(
            state.active_chat(),
            Err(FrontendError::ChatNotRunning)
        ));
    }

    #[test]
    fn stale_terminal_cannot_clear_a_later_desktop_turn() {
        let storage = TestRepository::new();
        let state = DesktopAppState::new(storage.0.clone());
        let first = state.start_chat().expect("first turn");
        state.finish_chat(first);
        let second = state.start_chat().expect("second turn");

        state.finish_chat(first);
        assert_eq!(
            *state
                .chat
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
            ChatState::Running
        );
        state.finish_chat(second);
    }

    #[test]
    fn terminal_ownership_allows_exactly_one_winner_per_generation() {
        let mut completed = TerminalOwnership::new(7);
        assert!(completed.claim(7), "completed wins its generation");
        assert!(!completed.claim(7), "cancel cannot claim after completed");

        let mut cancelled = TerminalOwnership::new(8);
        assert!(cancelled.claim(8), "cancel wins its generation");
        assert!(!cancelled.claim(8), "late completed is rejected");

        let mut duplicate = TerminalOwnership::new(9);
        assert!(duplicate.claim(9), "first failed terminal wins");
        assert!(!duplicate.claim(9), "duplicate failed is rejected");
        assert!(!duplicate.claim(9), "duplicate cancelled is rejected");
        assert!(!duplicate.claim(9), "duplicate completed is rejected");
    }

    #[test]
    fn old_terminal_ownership_cannot_affect_a_new_generation() {
        let mut current = TerminalOwnership::new(11);
        assert!(!current.claim(10));
        assert!(current.is_unclaimed(11));
        assert!(current.claim(11));
        assert!(!current.claim(10));
    }

    #[test]
    fn completed_ownership_is_the_only_path_that_commits_a_pair() {
        let identity = ConversationContextIdentity {
            repository_generation: 1,
            model_generation: 1,
        };
        let mut completed = DesktopConversationState::default();
        completed.reconcile(identity);
        let mut owner = TerminalOwnership::new(1);
        assert!(owner.claim(1));
        completed
            .commit(0, "user".into(), message(MessageRole::Assistant, "answer"))
            .expect("completed winner commits once");
        assert_eq!(completed.history.len(), 2);

        for generation in [2, 3, 4] {
            let mut conversation = DesktopConversationState::default();
            conversation.reconcile(identity);
            let mut owner = TerminalOwnership::new(generation);
            assert!(owner.claim(generation));
            // Cancelled, failed, and hard-recovery winners deliberately have no commit path;
            // model deltas are presentation-only and cannot become a completed response.
            assert!(conversation.history.is_empty());
            assert!(!owner.claim(generation), "late completion is rejected");
        }
    }

    #[test]
    fn runtime_identity_uses_arc_pointer_identity_without_a_codex_runtime() {
        let first = Arc::new(RuntimeMarker);
        let same = Arc::clone(&first);
        let replacement = Arc::new(RuntimeMarker);
        assert!(same_arc(&first, &same));
        assert!(!same_arc(&first, &replacement));
    }

    #[tokio::test(start_paused = true)]
    async fn graceful_cancel_outcomes_are_bounded_without_starting_shutdown() {
        assert_eq!(
            await_graceful_cancel(
                futures::future::ready(Ok::<(), ()>(())),
                Duration::from_secs(2)
            )
            .await,
            GracefulCancelOutcome::Completed
        );
        assert_eq!(
            await_graceful_cancel(
                futures::future::ready(Err::<(), ()>(())),
                Duration::from_secs(2)
            )
            .await,
            GracefulCancelOutcome::Failed
        );
        assert_eq!(
            await_graceful_cancel(
                futures::future::pending::<Result<(), ()>>(),
                Duration::from_secs(2)
            )
            .await,
            GracefulCancelOutcome::TimedOut
        );
    }

    #[tokio::test(start_paused = true)]
    async fn hard_recovery_is_lazy_and_has_complete_error_and_timeout_outcomes() {
        let shutdown_calls = Arc::new(AtomicU64::new(0));
        let completed = await_cancel_recovery(
            futures::future::ready(Ok::<(), ()>(())),
            Duration::from_secs(2),
            |_| true,
            {
                let shutdown_calls = Arc::clone(&shutdown_calls);
                move || {
                    shutdown_calls.fetch_add(1, Ordering::SeqCst);
                    futures::future::ready(Ok::<(), ()>(()))
                }
            },
            Duration::from_secs(2),
        )
        .await;
        assert_eq!(
            completed,
            CancelRecoveryOutcome::Graceful(GracefulCancelOutcome::Completed)
        );
        assert_eq!(shutdown_calls.load(Ordering::SeqCst), 0);

        let succeeded = await_cancel_recovery(
            futures::future::pending::<Result<(), ()>>(),
            Duration::from_secs(2),
            |_| true,
            {
                let shutdown_calls = Arc::clone(&shutdown_calls);
                move || {
                    shutdown_calls.fetch_add(1, Ordering::SeqCst);
                    futures::future::ready(Ok::<(), ()>(()))
                }
            },
            Duration::from_secs(2),
        )
        .await;
        assert_eq!(
            succeeded,
            CancelRecoveryOutcome::Hard(HardShutdownOutcome::Completed)
        );
        assert_eq!(shutdown_calls.load(Ordering::SeqCst), 1);

        let failed = await_hard_shutdown(
            futures::future::ready(Err::<(), ()>(())),
            Duration::from_secs(2),
        )
        .await;
        assert_eq!(failed, HardShutdownOutcome::Failed);
        let timed_out = await_hard_shutdown(
            futures::future::pending::<Result<(), ()>>(),
            Duration::from_secs(2),
        )
        .await;
        assert_eq!(timed_out, HardShutdownOutcome::TimedOut);
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

    #[tokio::test]
    #[ignore = "host-only Task 126 live runtime.start probe; requires the pinned Codex 0.149.0 executable and inherited Codex configuration"]
    async fn task_126_host_probe_uses_desktop_repository_runtime_construction() {
        let root = std::env::current_dir()
            .expect("current directory")
            .canonicalize()
            .expect("canonical selected repository root");
        let git = selected_git_executable().expect("host Git discovery");
        let repository = DesktopRepository::new(&git, &root)
            .expect("current directory is a selected repository");
        let prepared =
            prepare_codex_connection(resolve_codex_executable, CodexModelConfig::Inherit)
                .expect("pinned Codex executable resolves");
        let registry =
            desktop_tool_registry(Some(&repository), None).expect("desktop repository registry");
        let runtime = CodexRuntime::connect_tool_bridge_with_model_config_and_workspace(
            prepared.executable,
            registry,
            vec![
                PermissionLevel::None,
                PermissionLevel::Read,
                PermissionLevel::Execute,
            ],
            prepared.model_config,
            &root,
        )
        .await
        .expect("Desktop runtime connects");
        let handle = runtime
            .start(AgentRequest {
                request_id: RequestId::new(),
                input: AgentInput {
                    messages: vec![Message {
                        role: MessageRole::User,
                        content: "Task 126 runtime.start probe".to_owned(),
                    }],
                },
                options: AgentOptions::default(),
            })
            .await
            .expect("thread/start and turn/start create an AgentHandle");
        let first = handle.into_events().next().await;
        assert!(matches!(first, Some(AgentEvent::Started { .. })));
        runtime.shutdown().await.expect("host probe shutdown");
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
        let registry = desktop_tool_registry(None, None).expect("desktop registry should build");
        let definitions = registry.definitions();

        assert_eq!(definitions.len(), 1);
        assert_eq!(definitions[0].name.as_str(), DESKTOP_TOOL_NAME);
        assert_eq!(
            definitions[0].permission,
            rah_protocol::PermissionLevel::None
        );
    }

    #[test]
    fn host_composed_commit_tool_is_registered_with_execute_permission() {
        let fixture = TestRepository::new();
        let repository = fixture.desktop_repository();
        let (tool, _control) = RepositoryCommitTool::compose(
            &repository.git_executable,
            &repository.root,
            "RAH Host".to_owned(),
            "rah-host@example.invalid".to_owned(),
        )
        .expect("host commit composition should succeed");
        let registry = desktop_tool_registry(Some(&repository), Some(Arc::new(tool)))
            .expect("registry should retain the host-composed commit tool");
        let definition = registry
            .definitions()
            .into_iter()
            .find(|definition| definition.name.as_str() == "repo.commit")
            .expect("commit tool is registered");
        assert_eq!(definition.permission, PermissionLevel::Execute);
        assert_eq!(
            definition.input_schema,
            serde_json::json!({
                "type": "object", "properties": {"message": {"type": "string", "maxLength": 16 * 1024}},
                "required": ["message"], "additionalProperties": false
            })
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn desktop_delete_file_uses_host_authority_refreshes_and_does_not_stage() {
        let fixture = TestRepository::git_repository(GitRepositoryState::Clean);
        let repository = fixture.deletion_repository();
        let registry = desktop_tool_registry(Some(&repository), None)
            .expect("authorized Desktop registry should build");
        let definition = registry
            .definitions()
            .into_iter()
            .find(|definition| definition.name.as_str() == "repo.delete-file")
            .expect("host authority exposes deletion tool");
        assert_eq!(definition.permission, PermissionLevel::Execute);
        assert_eq!(
            definition.input_schema,
            serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "minLength": 1, "maxLength": 1024},
                    "expected_file_sha256": {"type": "string", "pattern": "^[0-9a-f]{64}$"},
                    "expected_file_byte_length": {"type": "integer", "minimum": 0, "maximum": 1024 * 1024}
                },
                "required": ["path", "expected_file_sha256", "expected_file_byte_length"],
                "additionalProperties": false
            })
        );
        let target = fixture.0.join("tracked.txt");
        let bytes = fs::read(&target).expect("tracked target should be readable");
        let index_before = Command::new(TestRepository::native_git())
            .args(["ls-files", "--stage"])
            .current_dir(&fixture.0)
            .output()
            .expect("index should be readable");
        let output = registry
            .execute(
                ToolCall {
                    id: ToolCallId::new(),
                    name: ToolName::new("repo.delete-file"),
                    input: ToolInput(serde_json::json!({
                        "path": "tracked.txt",
                        "expected_file_sha256": format!("{:x}", Sha256::digest(&bytes)),
                        "expected_file_byte_length": bytes.len()
                    })),
                },
                ToolContext::default(),
            )
            .await
            .expect("bridge dispatch should return a bounded result");
        assert!(!output.is_error);
        assert!(
            matches!(&output.content[0], ToolContent::Json(value) if value["status"] == "deleted_verified")
        );
        assert!(
            !target.exists(),
            "successful deletion is visible in worktree"
        );
        let index_after = Command::new(TestRepository::native_git())
            .args(["ls-files", "--stage"])
            .current_dir(&fixture.0)
            .output()
            .expect("index should remain readable");
        assert_eq!(index_after.stdout, index_before.stdout);
        let status = Command::new(TestRepository::native_git())
            .args(["status", "--porcelain=v1"])
            .current_dir(&fixture.0)
            .output()
            .expect("status should be readable");
        assert!(!String::from_utf8_lossy(&status.stdout).contains("D  tracked.txt"));
        assert!(String::from_utf8_lossy(&status.stdout).contains(" D tracked.txt"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn desktop_delete_file_requires_authority_and_rejects_stale_preimage_without_replay() {
        let fixture = TestRepository::git_repository(GitRepositoryState::Clean);
        let without_authority = fixture.desktop_repository();
        let registry = desktop_tool_registry(Some(&without_authority), None)
            .expect("registry without deletion authority should build");
        assert!(
            registry
                .definitions()
                .iter()
                .all(|definition| definition.name.as_str() != "repo.delete-file")
        );

        let repository = fixture.deletion_repository();
        let registry = desktop_tool_registry(Some(&repository), None)
            .expect("authorized registry should build");
        let bytes = fs::read(fixture.0.join("tracked.txt")).expect("target should be readable");
        let request = || ToolCall {
            id: ToolCallId::new(),
            name: ToolName::new("repo.delete-file"),
            input: ToolInput(serde_json::json!({
                "path": "tracked.txt",
                "expected_file_sha256": format!("{:x}", Sha256::digest(&bytes)),
                "expected_file_byte_length": bytes.len()
            })),
        };
        fs::write(fixture.0.join("tracked.txt"), b"newer user bytes\n")
            .expect("fixture should simulate a user edit");
        let first = registry
            .execute(request(), ToolContext::default())
            .await
            .expect("stale deletion should return a bounded result");
        assert!(first.is_error);
        assert!(
            matches!(&first.content[0], ToolContent::Json(value) if value["status"] == "precondition_failed")
        );
        assert_eq!(
            fs::read(fixture.0.join("tracked.txt")).unwrap(),
            b"newer user bytes\n"
        );
        let retry = registry
            .execute(request(), ToolContext::default())
            .await
            .expect("retry should return a bounded result");
        assert!(retry.is_error);
        assert_eq!(
            fs::read(fixture.0.join("tracked.txt")).unwrap(),
            b"newer user bytes\n"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn desktop_registry_commit_requires_its_paired_control_and_is_one_shot() {
        let fixture = TestRepository::git_repository(GitRepositoryState::Staged);
        let repository = DesktopRepository::new(&TestRepository::native_git(), &fixture.0)
            .expect("selected Git repository constructs");
        fs::write(fixture.0.join("tracked.txt"), b"reviewed index bytes\n").unwrap();
        assert!(
            Command::new(TestRepository::native_git())
                .args(["add", "tracked.txt"])
                .current_dir(&fixture.0)
                .status()
                .unwrap()
                .success()
        );
        fs::write(fixture.0.join("tracked.txt"), b"unstaged bytes remain\n").unwrap();
        fs::write(fixture.0.join("untracked.txt"), b"untracked bytes remain\n").unwrap();
        let (tool, control) = RepositoryCommitTool::compose(
            &repository.git_executable,
            &repository.root,
            "RAH Host".to_owned(),
            "rah-host@example.invalid".to_owned(),
        )
        .expect("Desktop commit capability composes");
        let registry = desktop_tool_registry(Some(&repository), Some(Arc::new(tool)))
            .expect("Desktop registry retains the composed tool");
        let git = TestRepository::native_git();
        let observe = |arguments: &[&str]| {
            Command::new(&git)
                .args(arguments)
                .current_dir(&fixture.0)
                .output()
                .unwrap()
        };
        let old_head = String::from_utf8(observe(&["rev-parse", "HEAD"]).stdout)
            .unwrap()
            .trim()
            .to_owned();
        let count_before = String::from_utf8(observe(&["rev-list", "--count", "HEAD"]).stdout)
            .unwrap()
            .trim()
            .parse::<u64>()
            .unwrap();
        let call = || ToolCall {
            id: ToolCallId::new(),
            name: ToolName::new("repo.commit"),
            input: ToolInput(serde_json::json!({"message": "reviewed desktop commit"})),
        };

        let unarmed = registry
            .execute(call(), ToolContext::default())
            .await
            .unwrap();
        assert!(unarmed.is_error);
        assert!(
            matches!(&unarmed.content[0], ToolContent::Text(text) if text.contains("precondition_failed"))
        );
        assert_eq!(
            String::from_utf8(observe(&["rev-parse", "HEAD"]).stdout)
                .unwrap()
                .trim(),
            old_head
        );

        let (_, review) = control.review_current_staged_snapshot().await.unwrap();
        control
            .authorize_reviewed_snapshot(&review.expect("text review"))
            .await
            .unwrap();
        let committed = registry
            .execute(call(), ToolContext::default())
            .await
            .unwrap();
        let presentation =
            commit_activity_presentation(&committed).expect("sanitized verified result");
        let new_head = String::from_utf8(observe(&["rev-parse", "HEAD"]).stdout)
            .unwrap()
            .trim()
            .to_owned();
        assert_eq!(presentation.commit_oid.as_deref(), Some(new_head.as_str()));
        assert_ne!(new_head, old_head);
        assert_eq!(
            String::from_utf8(observe(&["rev-parse", "HEAD^"]).stdout)
                .unwrap()
                .trim(),
            old_head
        );
        assert_eq!(
            String::from_utf8(observe(&["rev-list", "--count", "HEAD"]).stdout)
                .unwrap()
                .trim()
                .parse::<u64>()
                .unwrap(),
            count_before + 1
        );
        assert_eq!(
            observe(&["show", "HEAD:tracked.txt"]).stdout,
            b"reviewed index bytes\n"
        );
        assert_eq!(
            fs::read(fixture.0.join("tracked.txt")).unwrap(),
            b"unstaged bytes remain\n"
        );
        assert!(
            String::from_utf8(observe(&["status", "--porcelain=v1"]).stdout)
                .unwrap()
                .contains("?? untracked.txt")
        );

        let replay = registry
            .execute(call(), ToolContext::default())
            .await
            .unwrap();
        assert!(replay.is_error);
        assert!(
            matches!(&replay.content[0], ToolContent::Text(text) if text.contains("precondition_failed"))
        );
        assert_eq!(
            String::from_utf8(observe(&["rev-list", "--count", "HEAD"]).stdout)
                .unwrap()
                .trim()
                .parse::<u64>()
                .unwrap(),
            count_before + 1
        );
    }

    #[test]
    fn commit_activity_presentation_is_redacted_and_fail_closed() {
        let verified = ToolOutput {
            content: vec![ToolContent::Text(
                r#"{"status":"committed_verified","commit_oid":"0123456789abcdef0123456789abcdef01234567"}"#.to_owned(),
            )],
            is_error: false,
        };
        let presentation =
            commit_activity_presentation(&verified).expect("verified result is presentable");
        assert_eq!(
            presentation.status,
            super::CommitActivityStatus::CommittedVerified
        );
        assert_eq!(
            presentation.commit_oid.as_deref(),
            Some("0123456789abcdef0123456789abcdef01234567")
        );
        let malformed = ToolOutput {
            content: vec![ToolContent::Text(
                r#"{"status":"uncertain","detail":"private"}"#.to_owned(),
            )],
            is_error: true,
        };
        assert!(commit_activity_presentation(&malformed).is_none());
    }

    #[test]
    fn no_repository_uses_an_app_owned_neutral_workspace_not_its_storage_root() {
        let storage = TestRepository::new();
        fs::write(storage.0.join("AGENTS.md"), "SENTINEL_STORAGE_ROOT")
            .expect("storage sentinel should be written");
        let state = DesktopAppState::new(storage.0.clone());
        let neutral = state
            .neutral_workspace
            .as_ref()
            .expect("neutral workspace should be available");
        let canonical_storage =
            fs::canonicalize(&storage.0).expect("storage root should canonicalize");
        assert_eq!(neutral.parent(), Some(canonical_storage.as_path()));
        assert_eq!(
            neutral.file_name().and_then(|name| name.to_str()),
            Some(NEUTRAL_WORKSPACE_DIRECTORY)
        );
        assert!(!neutral.join("AGENTS.md").exists());
        assert_ne!(neutral, &storage.0);
    }

    #[test]
    fn stale_repository_or_model_connection_cannot_start_a_chat_turn() {
        assert!(connection_context_is_current(4, 8, 4, 8));
        assert!(!connection_context_is_current(4, 8, 5, 8));
        assert!(!connection_context_is_current(4, 8, 4, 9));
        assert!(!connection_context_is_current(4, 8, 5, 9));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn selected_repository_registry_has_only_the_intended_bounded_tools() {
        let fixture = TestRepository::new();
        fs::write(
            fixture.0.join("AGENTS.md"),
            "SENTINEL_REPOSITORY_INSTRUCTIONS_CANNOT_GRANT_AUTHORITY",
        )
        .expect("repository instruction sentinel should be written");
        let repository = fixture.desktop_repository();
        let registry =
            desktop_tool_registry(Some(&repository), None).expect("registry should build");
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
        assert!(desktop_tool_registry(Some(&repository), None).is_err());
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
            commit: None,
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
                    commit: None,
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
    fn verified_commit_activity_is_redacted_and_refreshes_the_repository() {
        let id = ToolCallId::new();
        let requested = AgentEvent::ToolRequested {
            session_id: SessionId::new(),
            tool_call: ToolCall {
                id: id.clone(),
                name: ToolName::new("repo.commit"),
                input: ToolInput(serde_json::json!({"message": "bounded commit"})),
            },
        };
        let finished = AgentEvent::ToolFinished {
            session_id: SessionId::new(),
            tool_call_id: id,
            output: ToolOutput {
                content: vec![ToolContent::Text(
                    r#"{"status":"committed_verified","commit_oid":"0123456789abcdef0123456789abcdef01234567"}"#.to_owned(),
                )],
                is_error: false,
            },
        };
        let mut calls = HashMap::new();
        assert!(activity_event(&requested, &mut calls).is_some());
        let (event, refresh) = activity_event(&finished, &mut calls).expect("commit finishes");
        assert!(refresh);
        let serialized = serde_json::to_string(&event).expect("commit activity serializes");
        assert_eq!(
            serialized,
            r#"{"kind":"tool_finished","tool":"repo.commit","result":"success","commit":{"status":"committed_verified","commitOid":"0123456789abcdef0123456789abcdef01234567"}}"#
        );
    }

    #[test]
    fn deleted_file_activity_refreshes_repository_without_frontend_authority() {
        let id = ToolCallId::new();
        let requested = AgentEvent::ToolRequested {
            session_id: SessionId::new(),
            tool_call: ToolCall {
                id: id.clone(),
                name: ToolName::new("repo.delete-file"),
                input: ToolInput(serde_json::json!({
                    "path": "tracked.txt",
                    "expected_file_sha256": "0".repeat(64),
                    "expected_file_byte_length": 1
                })),
            },
        };
        let finished = AgentEvent::ToolFinished {
            session_id: SessionId::new(),
            tool_call_id: id,
            output: ToolOutput {
                content: vec![ToolContent::Json(serde_json::json!({
                    "path": "tracked.txt",
                    "status": "deleted_verified",
                    "uncertain": false
                }))],
                is_error: false,
            },
        };
        let mut calls = HashMap::new();
        assert!(activity_event(&requested, &mut calls).is_some());
        let (event, refresh) = activity_event(&finished, &mut calls).expect("delete finishes");
        assert!(refresh);
        let serialized = serde_json::to_string(&event).expect("activity serializes");
        assert_eq!(
            serialized,
            r#"{"kind":"tool_finished","tool":"repo.delete-file","result":"success"}"#
        );
        assert!(!serialized.contains("sha256"));
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
    fn provider_endpoint_accepts_normalizes_and_serializes_only_the_closed_authority() {
        let dns = ProviderEndpoint::parse(ProviderEndpointInput {
            scheme: ProviderScheme::Https,
            host: "EXAMPLE.COM.".to_owned(),
            port: 443,
        })
        .expect("normalized DNS endpoint");
        assert_eq!(dns.base_url(), "https://example.com:443/v1");
        assert!(!dns.insecure_transport());
        let ipv6 = ProviderEndpoint::parse(ProviderEndpointInput {
            scheme: ProviderScheme::Http,
            host: "::1".to_owned(),
            port: 65535,
        })
        .expect("IPv6 endpoint");
        assert_eq!(ipv6.base_url(), "http://[::1]:65535/v1");
        assert!(!ipv6.insecure_transport());
        let lan_http = ProviderEndpoint::parse(ProviderEndpointInput {
            scheme: ProviderScheme::Http,
            host: "198.51.100.20".to_owned(),
            port: 1,
        })
        .expect("non-loopback endpoint");
        assert!(lan_http.insecure_transport());
    }

    #[test]
    fn llama_adapter_handoff_uses_the_synthesized_endpoint_without_credentials() {
        let selection = DesktopModelSelection {
            provider: DesktopModelProvider::LlamaCpp,
            model: Some("model".to_owned()),
            llama_cpp_endpoint: Some(
                ProviderEndpoint::parse(ProviderEndpointInput {
                    scheme: ProviderScheme::Https,
                    host: "2001:db8::1".to_owned(),
                    port: 443,
                })
                .expect("endpoint"),
            ),
        };
        let CodexModelConfig::Explicit(config) = selection.codex_model_config().expect("config")
        else {
            panic!("llama selection must be explicit");
        };
        let CodexModelProvider::LlamaCpp(provider) = config.provider() else {
            panic!("llama selection must use llama provider");
        };
        assert_eq!(provider.base_url(), "https://[2001:db8::1]:443/v1");
        assert_eq!(provider.credential_environment_variable(), None);
    }

    #[test]
    fn provider_endpoint_rejects_non_authority_syntax_and_malformed_hosts() {
        for (host, port) in [
            ("", 8080),
            (" 127.0.0.1", 8080),
            ("127.0.0.1 ", 8080),
            ("http://example.com", 8080),
            ("user@example.com", 8080),
            ("example.com/path", 8080),
            ("example.com?x", 8080),
            ("example.com#x", 8080),
            ("[::1]", 8080),
            ("example.com:8080", 8080),
            ("-example.com", 8080),
            ("example-.com", 8080),
            ("example..com", 8080),
            ("例子.com", 8080),
            ("999.999.999.999", 8080),
            ("127.0.0.1", 0),
        ] {
            assert_eq!(
                ProviderEndpoint::parse(ProviderEndpointInput {
                    scheme: ProviderScheme::Http,
                    host: host.to_owned(),
                    port
                }),
                Err(FrontendError::ModelConfigurationInvalid),
                "{host}:{port}",
            );
        }
        let long_label = format!("{}.example", "a".repeat(64));
        assert!(
            ProviderEndpoint::parse(ProviderEndpointInput {
                scheme: ProviderScheme::Https,
                host: long_label,
                port: 443
            })
            .is_err()
        );
        assert!(
            ProviderEndpoint::parse(ProviderEndpointInput {
                scheme: ProviderScheme::Https,
                host: "a".repeat(254),
                port: 443
            })
            .is_err()
        );
    }

    #[test]
    fn endpoint_normalization_controls_generation_and_llama_only_closure() {
        let mut state = DesktopModelState::default();
        let selection = |host: &str, scheme, port| DesktopModelSelection {
            provider: DesktopModelProvider::LlamaCpp,
            model: Some("model".to_owned()),
            llama_cpp_endpoint: Some(
                ProviderEndpoint::parse(ProviderEndpointInput {
                    scheme,
                    host: host.to_owned(),
                    port,
                })
                .expect("endpoint"),
            ),
        };
        apply_model_selection(
            &mut state,
            ChatState::Idle,
            selection("EXAMPLE.COM.", ProviderScheme::Http, 8080),
        )
        .expect("apply");
        assert_eq!(state.generation, 1);
        apply_model_selection(
            &mut state,
            ChatState::Idle,
            selection("example.com", ProviderScheme::Http, 8080),
        )
        .expect("equivalent apply");
        assert_eq!(state.generation, 1);
        apply_model_selection(
            &mut state,
            ChatState::Idle,
            selection("example.com", ProviderScheme::Https, 8080),
        )
        .expect("scheme apply");
        assert_eq!(state.generation, 2);
        for provider in [
            DesktopModelProvider::Inherit,
            DesktopModelProvider::OpenAi,
            DesktopModelProvider::Ollama,
            DesktopModelProvider::LmStudio,
        ] {
            let selection = DesktopModelSelection {
                provider,
                model: if provider == DesktopModelProvider::Inherit {
                    None
                } else {
                    Some("model".to_owned())
                },
                llama_cpp_endpoint: Some(default_test_endpoint()),
            };
            assert_eq!(
                selection.codex_model_config(),
                Err(FrontendError::ModelConfigurationInvalid)
            );
        }
        assert_eq!(
            DesktopModelSelection {
                provider: DesktopModelProvider::LlamaCpp,
                model: Some("model".to_owned()),
                llama_cpp_endpoint: None
            }
            .codex_model_config(),
            Err(FrontendError::ModelConfigurationInvalid)
        );
    }

    #[test]
    fn readiness_probe_uses_one_exact_get_request_and_maps_ready_and_loading() {
        for (status, expected) in [(200, ReadinessState::Ready), (503, ReadinessState::Loading)] {
            let server = ReadinessTestServer::start(
                format!("HTTP/1.1 {status} Test\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"),
                1,
            );
            assert_eq!(readiness_check(&server.endpoint), expected);
            let request = server.request();
            assert!(request.starts_with("GET /v1/health HTTP/1.1\r\n"));
            assert!(!request.contains("Authorization:"));
            assert!(!request.contains("Cookie:"));
            assert!(!request.contains("Referer:"));
            assert!(!request.contains('?'));
            server.finish();
        }
    }

    #[test]
    fn readiness_probe_does_not_follow_redirects() {
        let redirect_target = ReadinessTestServer::start(
            "HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_owned(),
            1,
        );
        let source = ReadinessTestServer::start(
            format!(
                "HTTP/1.1 302 Found\r\nLocation: {}/health\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                redirect_target.endpoint.base_url()
            ),
            1,
        );
        assert_eq!(
            readiness_check(&source.endpoint),
            ReadinessState::CheckFailed
        );
        assert!(source.request().starts_with("GET /v1/health HTTP/1.1\r\n"));
        assert!(
            redirect_target
                .requests
                .recv_timeout(Duration::from_millis(250))
                .is_err()
        );
        source.finish();
        drop(redirect_target.requests);
        drop(redirect_target.join);
    }

    #[test]
    fn readiness_probe_rejects_declared_and_streamed_oversize_bodies() {
        let declared = ReadinessTestServer::start(
            format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                READINESS_BODY_LIMIT + 1
            ),
            1,
        );
        assert_eq!(
            readiness_check(&declared.endpoint),
            ReadinessState::CheckFailed
        );
        assert!(
            declared
                .request()
                .starts_with("GET /v1/health HTTP/1.1\r\n")
        );
        declared.finish();

        let body = "x".repeat(READINESS_BODY_LIMIT + 1);
        let streamed = ReadinessTestServer::start(
            format!(
                "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n{:X}\r\n{}\r\n0\r\n\r\n",
                body.len(),
                body
            ),
            1,
        );
        assert_eq!(
            readiness_check(&streamed.endpoint),
            ReadinessState::CheckFailed
        );
        assert!(
            streamed
                .request()
                .starts_with("GET /v1/health HTTP/1.1\r\n")
        );
        streamed.finish();
    }

    #[test]
    fn readiness_probe_maps_other_status_and_refused_connection() {
        for status in [404, 500] {
            let server = ReadinessTestServer::start(
                format!("HTTP/1.1 {status} Test\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"),
                1,
            );
            assert_eq!(
                readiness_check(&server.endpoint),
                ReadinessState::CheckFailed
            );
            server.request();
            server.finish();
        }
        let unused = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0))
            .expect("temporary loopback port reserves");
        let port = unused.local_addr().expect("reserved port address").port();
        drop(unused);
        let refused = ProviderEndpoint::parse(ProviderEndpointInput {
            scheme: ProviderScheme::Http,
            host: "127.0.0.1".to_owned(),
            port,
        })
        .expect("refused endpoint is structurally valid");
        assert_eq!(readiness_check(&refused), ReadinessState::Unreachable);
    }

    #[test]
    fn readiness_probe_does_not_retry_a_transient_connection_failure() {
        let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0))
            .expect("loopback listener binds");
        let port = listener.local_addr().expect("listener address").port();
        let endpoint = ProviderEndpoint::parse(ProviderEndpointInput {
            scheme: ProviderScheme::Http,
            host: "127.0.0.1".to_owned(),
            port,
        })
        .expect("loopback endpoint is valid");
        let attempts = Arc::new(AtomicU64::new(0));
        let observed = Arc::clone(&attempts);
        let server = thread::spawn(move || {
            let (stream, _) = listener.accept().expect("first request arrives");
            observed.fetch_add(1, Ordering::SeqCst);
            drop(stream);
            listener
                .set_nonblocking(true)
                .expect("listener becomes nonblocking");
            let deadline = std::time::Instant::now() + Duration::from_millis(500);
            while std::time::Instant::now() < deadline {
                if let Ok((stream, _)) = listener.accept() {
                    observed.fetch_add(1, Ordering::SeqCst);
                    drop(stream);
                }
                thread::yield_now();
            }
        });
        assert_eq!(readiness_check(&endpoint), ReadinessState::CheckFailed);
        server.join().expect("transient server exits");
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn readiness_probe_has_a_bounded_total_timeout() {
        let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0))
            .expect("loopback listener binds");
        let port = listener.local_addr().expect("listener address").port();
        let endpoint = ProviderEndpoint::parse(ProviderEndpointInput {
            scheme: ProviderScheme::Http,
            host: "127.0.0.1".to_owned(),
            port,
        })
        .expect("loopback endpoint is valid");
        let server = thread::spawn(move || {
            let (stream, _) = listener.accept().expect("readiness request arrives");
            thread::sleep(READINESS_TOTAL_TIMEOUT + Duration::from_secs(1));
            drop(stream);
        });
        let started = std::time::Instant::now();
        assert_eq!(readiness_check(&endpoint), ReadinessState::Unreachable);
        assert!(started.elapsed() < READINESS_TOTAL_TIMEOUT + Duration::from_secs(1));
        server.join().expect("timeout server exits");
    }

    #[test]
    fn stale_readiness_result_cannot_overwrite_a_new_model_generation() {
        let storage = TestRepository::new();
        let state = DesktopAppState::new(storage.0.clone());
        let old_endpoint = default_test_endpoint();
        let new_endpoint = ProviderEndpoint::parse(ProviderEndpointInput {
            scheme: ProviderScheme::Http,
            host: "127.0.0.1".to_owned(),
            port: 8081,
        })
        .expect("replacement endpoint is valid");
        {
            let mut model = state.model.lock().expect("model lock");
            model.selection = DesktopModelSelection {
                provider: DesktopModelProvider::LlamaCpp,
                model: Some("old".to_owned()),
                llama_cpp_endpoint: Some(old_endpoint.clone()),
            };
            model.generation = 7;
            model.readiness = ReadinessState::Checking;
            model.selection = DesktopModelSelection {
                provider: DesktopModelProvider::LlamaCpp,
                model: Some("new".to_owned()),
                llama_cpp_endpoint: Some(new_endpoint.clone()),
            };
            model.generation = 8;
            model.readiness = ReadinessState::NotTested;
        }
        publish_readiness_result(&state, 7, &old_endpoint, ReadinessState::Ready);
        let model = state.model.lock().expect("model lock");
        assert_eq!(model.generation, 8);
        assert_eq!(
            model.selection.llama_cpp_endpoint.as_ref(),
            Some(&new_endpoint)
        );
        assert_eq!(model.readiness, ReadinessState::NotTested);
    }

    #[test]
    fn readiness_probe_has_no_configuration_or_runtime_effect() {
        let storage = TestRepository::new();
        let state = DesktopAppState::new(storage.0.clone());
        let server = ReadinessTestServer::start(
            "HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_owned(),
            1,
        );
        assert_eq!(readiness_check(&server.endpoint), ReadinessState::Ready);
        server.request();
        server.finish();
        let model = state.model.lock().expect("model lock");
        assert_eq!(model.generation, 0);
        assert_eq!(model.selection, DesktopModelSelection::default());
        assert_eq!(model.readiness, ReadinessState::NotTested);
        drop(model);
        assert!(matches!(
            *state.connection.lock().expect("connection lock"),
            ConnectionState::NotConnected
        ));
        assert!(state.repository.lock().expect("repository lock").is_none());
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
                CodexModelProvider::LlamaCpp(
                    CodexLlamaCppProvider::new("http://127.0.0.1:8080/v1", None)
                        .expect("test provider"),
                ),
            ),
        ] {
            let selection = DesktopModelSelection {
                provider,
                model: Some("exact-model".to_owned()),
                llama_cpp_endpoint: (provider == DesktopModelProvider::LlamaCpp)
                    .then(default_test_endpoint),
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
                llama_cpp_endpoint: None,
            },
            DesktopModelSelection {
                provider: DesktopModelProvider::Ollama,
                model: Some("   ".to_owned()),
                llama_cpp_endpoint: None,
            },
            DesktopModelSelection {
                provider: DesktopModelProvider::Inherit,
                model: Some("not-allowed".to_owned()),
                llama_cpp_endpoint: None,
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
            endpoint: Some(ProviderEndpointPresentation::from(&default_test_endpoint())),
            insecure_transport: false,
            readiness: ReadinessState::NotTested,
            status: "active",
        };
        let serialized = serde_json::to_string(&presentation).expect("presentation serializes");
        assert_eq!(
            serialized,
            r#"{"provider":"llama_cpp","model":"rah-local-model","endpoint":{"scheme":"http","host":"127.0.0.1","port":8080,"normalized":"http://127.0.0.1:8080/v1"},"insecureTransport":false,"readiness":"not_tested","status":"active"}"#
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
            llama_cpp_endpoint: Some(default_test_endpoint()),
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
                    llama_cpp_endpoint: None,
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

    #[test]
    fn preference_restore_initializes_only_inactive_desired_state() {
        let storage = TestRepository::new();
        let selection = DesktopModelSelection {
            provider: DesktopModelProvider::Ollama,
            model: Some("restored-model".to_owned()),
            llama_cpp_endpoint: None,
        };
        let mut preferences = Preferences::start(storage.0.clone()).0;
        preferences.save(&selection).unwrap();
        let state = DesktopAppState::new(storage.0.clone());
        let model = state.model.lock().unwrap();
        assert_eq!(model.selection, selection);
        assert_eq!(model.generation, 0);
        assert_eq!(model.readiness, ReadinessState::NotTested);
        assert!(matches!(
            *state.connection.lock().unwrap(),
            ConnectionState::NotConnected
        ));
        assert!(state.repository.lock().unwrap().is_none());
    }

    #[test]
    fn malformed_preference_restore_defaults_without_connection_or_transcript_mutation() {
        let storage = TestRepository::new();
        let preferences_path = storage.0.join("desktop-preferences.json");
        let transcript_path = storage.0.join("conversation-transcript.sqlite3");
        fs::write(&preferences_path, b"{").unwrap();
        let state = DesktopAppState::new(storage.0.clone());
        let model = state.model.lock().unwrap();
        assert_eq!(model.selection, DesktopModelSelection::default());
        assert_eq!(model.generation, 0);
        assert_eq!(model.readiness, ReadinessState::NotTested);
        assert!(matches!(
            *state.connection.lock().unwrap(),
            ConnectionState::NotConnected
        ));
        assert_eq!(fs::read(&preferences_path).unwrap(), b"{");
        assert!(transcript_path.exists());
    }

    #[test]
    fn apply_and_reset_save_failures_do_not_roll_back_current_desired_state() {
        use super::desktop_preferences::{TestFault, clear_test_state, set_test_fault, test_lock};
        let _guard = test_lock()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let storage = TestRepository::new();
        let durable = DesktopModelSelection {
            provider: DesktopModelProvider::OpenAi,
            model: Some("A".into()),
            llama_cpp_endpoint: None,
        };
        let applied = DesktopModelSelection {
            provider: DesktopModelProvider::Ollama,
            model: Some("B".into()),
            llama_cpp_endpoint: None,
        };
        let mut preferences = Preferences::start(storage.0.clone()).0;
        preferences.save(&durable).unwrap();
        let disk = fs::read(storage.0.join("desktop-preferences.json")).unwrap();
        let mut model = DesktopModelState {
            selection: durable,
            generation: 4,
            readiness: ReadinessState::Ready,
        };
        apply_model_selection(&mut model, ChatState::Idle, applied.clone()).unwrap();
        set_test_fault(
            storage.0.join("desktop-preferences.json"),
            TestFault::ReplaceOther,
        );
        assert_eq!(
            preferences.save(&applied),
            Err(PreferencesWarning::SaveFailed)
        );
        assert_eq!(model.selection, applied);
        assert_eq!(model.generation, 5);
        assert_eq!(model.readiness, ReadinessState::NotTested);
        assert_eq!(
            fs::read(storage.0.join("desktop-preferences.json")).unwrap(),
            disk
        );
        clear_test_state();
        let later = DesktopModelSelection {
            provider: DesktopModelProvider::LmStudio,
            model: Some("C".into()),
            llama_cpp_endpoint: None,
        };
        apply_model_selection(&mut model, ChatState::Idle, later.clone()).unwrap();
        preferences.save(&later).unwrap();
        assert_eq!(
            fs::read(storage.0.join("desktop-preferences.json")).unwrap(),
            super::desktop_preferences::test_canonical(&later).unwrap()
        );

        set_test_fault(
            storage.0.join("desktop-preferences.json"),
            TestFault::ReplaceOther,
        );
        apply_model_selection(
            &mut model,
            ChatState::Idle,
            DesktopModelSelection::default(),
        )
        .unwrap();
        assert_eq!(preferences.reset(), Err(PreferencesWarning::SaveFailed));
        assert_eq!(model.selection, DesktopModelSelection::default());
        assert_eq!(model.generation, 7);
        assert_eq!(
            fs::read(storage.0.join("desktop-preferences.json")).unwrap(),
            super::desktop_preferences::test_canonical(&later).unwrap()
        );
        clear_test_state();
    }

    #[test]
    fn reset_is_idle_only_and_changes_no_conversation_state() {
        let selection = DesktopModelSelection {
            provider: DesktopModelProvider::OpenAi,
            model: Some("model".into()),
            llama_cpp_endpoint: None,
        };
        for busy in [ChatState::Running, ChatState::CancelRequested] {
            let mut model = DesktopModelState {
                selection: selection.clone(),
                generation: 9,
                readiness: ReadinessState::Ready,
            };
            assert_eq!(
                apply_model_selection(&mut model, busy, DesktopModelSelection::default()),
                Err(FrontendError::ModelConfigurationBusy)
            );
            assert_eq!(model.selection, selection);
            assert_eq!(model.generation, 9);
        }
        let mut model = DesktopModelState::default();
        apply_model_selection(
            &mut model,
            ChatState::Idle,
            DesktopModelSelection::default(),
        )
        .unwrap();
        assert_eq!(model.generation, 0);
    }

    #[test]
    fn startup_preference_matrix_restores_only_inactive_state_without_activation() {
        use super::desktop_preferences::{clear_test_state, test_lock};
        let _guard = test_lock()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let valid = DesktopModelSelection {
            provider: DesktopModelProvider::LlamaCpp,
            model: Some("loopback".into()),
            llama_cpp_endpoint: Some(default_test_endpoint()),
        };
        for (stored, expected, warning) in [
            (None, DesktopModelSelection::default(), None),
            (
                Some(super::desktop_preferences::test_canonical(&valid).unwrap()),
                valid,
                None,
            ),
            (
                Some(b"{".to_vec()),
                DesktopModelSelection::default(),
                Some(PreferencesWarning::RestoreFailed),
            ),
            (
                Some(b"{\"version\":2,\"model\":{\"provider\":\"inherit\"}}".to_vec()),
                DesktopModelSelection::default(),
                None,
            ),
        ] {
            let storage = TestRepository::new();
            if let Some(bytes) = stored {
                fs::write(storage.0.join("desktop-preferences.json"), bytes).unwrap();
            }
            clear_test_state();
            super::reset_startup_activation_counters();
            let state = DesktopAppState::new(storage.0.clone());
            let model = state.model.lock().unwrap();
            assert_eq!(model.selection, expected);
            assert_eq!(model.generation, 0);
            assert_eq!(model.readiness, ReadinessState::NotTested);
            drop(model);
            assert!(matches!(
                *state.connection.lock().unwrap(),
                ConnectionState::NotConnected
            ));
            assert_eq!(
                state.preferences.lock().unwrap().take_warning(),
                warning,
                "only invalid existing files warn"
            );
            assert_eq!(
                super::startup_activation_snapshot(),
                super::StartupActivationCounters::default()
            );
            assert!(state.conversation.lock().unwrap().history.is_empty());
        }
        clear_test_state();
    }

    #[test]
    fn preference_write_matrix_excludes_all_non_apply_reset_events() {
        use super::desktop_preferences::{
            clear_test_state, reset_test_accounting, test_lock, test_write_accounting,
        };
        let _guard = test_lock()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let storage = TestRepository::new();
        let state = DesktopAppState::new(storage.0.clone());
        let preference_path = storage.0.join("desktop-preferences.json");
        reset_test_accounting();

        // Read/presentation, readiness results (including stale discard), status polling,
        // chat bookkeeping, repository generation, and conversation presentation/actions.
        let _ = state.status();
        let _ = model_configuration_status(None, 0);
        let _ = state.persistence.lock().unwrap().presentation();
        let endpoint = default_test_endpoint();
        for result in [
            ReadinessState::Checking,
            ReadinessState::Ready,
            ReadinessState::Loading,
            ReadinessState::Unreachable,
            ReadinessState::TlsFailure,
            ReadinessState::CheckFailed,
        ] {
            publish_readiness_result(&state, 0, &endpoint, result);
        }
        publish_readiness_result(&state, 1, &endpoint, ReadinessState::Ready);
        let generation = state.start_chat().unwrap();
        state.finish_chat(generation);
        *state.repository_generation.lock().unwrap() += 1;
        state.conversation.lock().unwrap().start_new();
        state
            .persist_completed_pair("user".into(), "assistant".into())
            .unwrap();
        let mut resumed = DesktopConversationState::default();
        resumed
            .resume(
                ConversationContextIdentity {
                    repository_generation: 1,
                    model_generation: 0,
                },
                vec![ResumePair {
                    user: "prior user".into(),
                    assistant: "prior assistant".into(),
                }],
            )
            .unwrap();
        let mut model = state.model.lock().unwrap();
        apply_model_selection(
            &mut model,
            ChatState::Idle,
            DesktopModelSelection {
                provider: DesktopModelProvider::OpenAi,
                model: Some("process-local".into()),
                llama_cpp_endpoint: None,
            },
        )
        .unwrap();
        drop(model);

        assert_eq!(
            test_write_accounting(&preference_path),
            super::desktop_preferences::TestWriteAccounting::default()
        );
        clear_test_state();
    }

    #[test]
    fn only_apply_and_reset_create_one_preference_transaction_and_identical_apply_skips_it() {
        use super::desktop_preferences::{
            clear_test_state, reset_test_accounting, test_lock, test_write_accounting,
        };
        let _guard = test_lock()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let storage = TestRepository::new();
        let path = storage.0.join("desktop-preferences.json");
        let mut preferences = Preferences::start(storage.0.clone()).0;
        let applied = DesktopModelSelection {
            provider: DesktopModelProvider::OpenAi,
            model: Some("applied".into()),
            llama_cpp_endpoint: None,
        };
        reset_test_accounting();
        preferences.save(&applied).unwrap();
        let apply = test_write_accounting(&path);
        assert_eq!(apply.destination_write_attempts, 1);
        assert_eq!(apply.temp_creations, 1);
        assert_eq!(apply.native_replacements_or_moves, 1);

        reset_test_accounting();
        preferences.save(&applied).unwrap();
        assert_eq!(test_write_accounting(&path), Default::default());

        reset_test_accounting();
        preferences.reset().unwrap();
        let reset = test_write_accounting(&path);
        assert_eq!(reset.destination_write_attempts, 1);
        assert_eq!(reset.temp_creations, 1);
        assert_eq!(reset.native_replacements_or_moves, 1);
        clear_test_state();
    }

    #[test]
    fn non_loopback_apply_changes_current_state_without_preference_filesystem_activity() {
        use super::desktop_preferences::{
            clear_test_state, reset_test_accounting, test_lock, test_write_accounting,
        };
        let _guard = test_lock()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let storage = TestRepository::new();
        let path = storage.0.join("desktop-preferences.json");
        let durable = DesktopModelSelection {
            provider: DesktopModelProvider::OpenAi,
            model: Some("A".into()),
            llama_cpp_endpoint: None,
        };
        let remote = DesktopModelSelection {
            provider: DesktopModelProvider::LlamaCpp,
            model: Some("B".into()),
            llama_cpp_endpoint: Some(
                ProviderEndpoint::parse(ProviderEndpointInput {
                    scheme: ProviderScheme::Http,
                    host: "192.168.1.10".into(),
                    port: 8080,
                })
                .unwrap(),
            ),
        };
        let mut preferences = Preferences::start(storage.0.clone()).0;
        preferences.save(&durable).unwrap();
        let before = fs::read(&path).unwrap();
        let mut model = DesktopModelState {
            selection: durable,
            generation: 4,
            readiness: ReadinessState::Ready,
        };
        reset_test_accounting();
        apply_model_selection(&mut model, ChatState::Idle, remote.clone()).unwrap();
        assert_eq!(
            preferences.save(&remote),
            Err(PreferencesWarning::SaveFailed)
        );
        assert_eq!(model.selection, remote);
        assert_eq!(model.generation, 5);
        assert_eq!(model.readiness, ReadinessState::NotTested);
        assert_eq!(fs::read(&path).unwrap(), before);
        assert_eq!(test_write_accounting(&path), Default::default());
        clear_test_state();
    }

    #[test]
    fn reset_and_restore_keep_preference_and_conversation_persistence_separate() {
        let storage = TestRepository::new();
        let preference_path = storage.0.join("desktop-preferences.json");
        let non_default = DesktopModelSelection {
            provider: DesktopModelProvider::Ollama,
            model: Some("non-default".into()),
            llama_cpp_endpoint: None,
        };
        let mut preferences = Preferences::start(storage.0.clone()).0;
        preferences.save(&non_default).unwrap();
        {
            let mut persistence = super::Persistence::start(storage.0.clone());
            persistence.select_namespace("neutral-v1".into());
            persistence
                .append_pair("user".into(), "assistant".into())
                .unwrap();
        }
        let state = DesktopAppState::new(storage.0.clone());
        let preference_after_startup = fs::read(&preference_path).unwrap();
        let presentation_after_startup =
            serde_json::to_vec(&state.persistence.lock().unwrap().presentation()).unwrap();
        assert_eq!(state.model.lock().unwrap().selection, non_default);
        // SQLite retains the selected neutral namespace independently from
        // model preferences and connection state.
        assert!(
            state
                .persistence
                .lock()
                .unwrap()
                .presentation()
                .records
                .len()
                >= 3
        );

        let mut model = state.model.lock().unwrap();
        apply_model_selection(
            &mut model,
            ChatState::Idle,
            DesktopModelSelection::default(),
        )
        .unwrap();
        drop(model);
        state.preferences.lock().unwrap().reset().unwrap();
        assert_eq!(
            serde_json::to_vec(&state.persistence.lock().unwrap().presentation()).unwrap(),
            presentation_after_startup
        );
        assert!(matches!(
            *state.connection.lock().unwrap(),
            ConnectionState::NotConnected
        ));
        assert_eq!(
            fs::read(&preference_path).unwrap(),
            super::desktop_preferences::test_canonical(&DesktopModelSelection::default()).unwrap()
        );

        let restored = DesktopAppState::new(storage.0.clone());
        assert_eq!(
            restored.model.lock().unwrap().selection,
            DesktopModelSelection::default()
        );
        assert_eq!(
            fs::read(&preference_path).unwrap(),
            super::desktop_preferences::test_canonical(&DesktopModelSelection::default()).unwrap()
        );
        assert_ne!(
            preference_after_startup,
            fs::read(&preference_path).unwrap()
        );
        assert!(
            restored
                .persistence
                .lock()
                .unwrap()
                .presentation()
                .records
                .len()
                >= 3
        );
    }

    #[test]
    fn preference_and_conversation_warning_domains_do_not_cross() {
        let preferences = [
            PreferencesWarning::RestoreFailed,
            PreferencesWarning::SaveFailed,
        ]
        .map(|warning| match warning {
            PreferencesWarning::RestoreFailed => "preferences_restore_failed",
            PreferencesWarning::SaveFailed => "preferences_save_failed",
        });
        assert_eq!(
            preferences,
            ["preferences_restore_failed", "preferences_save_failed"]
        );
        for warning in [
            super::ConversationPersistenceWarning::RestoreFailed,
            super::ConversationPersistenceWarning::SaveFailed,
        ] {
            let serialized = serde_json::to_string(&warning).unwrap();
            assert!(!preferences.iter().any(|name| serialized.contains(name)));
        }
    }
}
