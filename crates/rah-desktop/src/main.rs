#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(target_os = "windows")]
mod codex_baseline;
#[cfg(target_os = "windows")]
mod conversation_persistence;
#[cfg(target_os = "windows")]
mod desktop_preferences;
#[cfg(target_os = "windows")]
mod effective_authority;
#[cfg(target_os = "windows")]
mod git_discovery;
#[cfg(target_os = "windows")]
mod host_invocation;
#[cfg(target_os = "windows")]
mod provider_composition;
#[cfg(target_os = "windows")]
mod repository_membership;
#[cfg(target_os = "windows")]
mod trusted_profile_selection;

#[cfg(target_os = "windows")]
use codex_baseline::{BaselineError, CodexExecutableSource, resolve as resolve_codex_executable};
#[cfg(target_os = "windows")]
use conversation_persistence::{
    Persistence, Presentation as ConversationTranscriptPresentation, ResumeError, ResumePair,
    SeparatorReason, Warning as ConversationPersistenceWarning,
};
#[cfg(target_os = "windows")]
use desktop_preferences::{
    Preferences, RememberedTrustedProfilePath, Warning as PreferencesWarning,
};
#[cfg(target_os = "windows")]
use effective_authority::{
    ConfiguredSummary, ConnectionBinding, ConnectionBindingState, DesktopToolComposition,
    EffectiveAuthoritySnapshot, EffectiveToolEntry, ExternalToolDescriptor, RepositoryBinding,
    RepositoryIdentity, RepositoryKind, SnapshotStatus, SourceKind,
};
#[cfg(target_os = "windows")]
use futures::StreamExt;
#[cfg(target_os = "windows")]
use host_invocation::{
    BranchReview, CoordinatorState, DESKTOP_HOST_BRANCH_NAME_MAX_BYTES, HostConfirmRequest,
    HostInvocationCoordinator, HostInvocationKind, HostInvocationResponse, HostInvocationReview,
    HostInvocationUnavailableReason, HostPrepareBranchRequest, HostPrepareCreateFileRequest,
    HostPrepareDeleteFileRequest, HostPrepareMultiFileEditRequest, HostPreparePatchRequest,
    HostPrepareRenameFileRequest, HostReadRequest, PreparedBranchResponse,
    PreparedCreateFileResponse, PreparedDeleteFileResponse, PreparedHostInvocation,
    PreparedHostPayload, PreparedMultiFileEditResponse, PreparedPatchResponse,
    PreparedRenameFileResponse, host_descriptor_with_rename, host_kind, read_request,
    validate_bounded_string,
};
#[cfg(target_os = "windows")]
use provider_composition::{
    DesktopProviderActivation, ProviderActivationError, desktop_allowed_permissions,
    merge_tool_registries,
};
#[cfg(target_os = "windows")]
use rah_protocol::{
    AgentErrorCode, AgentEvent, AgentInput, AgentOptions, AgentRequest, Message, MessageRole,
    PermissionLevel, RequestId, SessionId, ToolCall, ToolCallId, ToolContent, ToolDefinition,
    ToolInput, ToolName, ToolOutput,
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
    AuthorizedDispatchError, AuthorizedDispatchRejection, EchoTool, FsReadTool, GitStageTool,
    GitUnstageTool, REPOSITORY_CREATE_BRANCH_TOOL_NAME, RepositoryAdmissionIdentity,
    RepositoryAdmissionRelation, RepositoryBranchCreationAuthority, RepositoryBranchCreationTool,
    RepositoryCommitControl, RepositoryCommitReview, RepositoryCommitTool,
    RepositoryCreateFilePreparationError, RepositoryCreateFilePreparationRequest,
    RepositoryDeleteFilePreparationError, RepositoryDeleteFilePreparationRequest,
    RepositoryDiffStagedTool, RepositoryDiffTool, RepositoryDirectoryCreationAuthority,
    RepositoryDirectoryCreationTool, RepositoryFileCreationTool, RepositoryFileDeletionAuthority,
    RepositoryFileDeletionTool, RepositoryFileInfoTool, RepositoryFileRenameAuthority,
    RepositoryFileRenameTool, RepositoryMultiFileEditPreparationError,
    RepositoryMultiFileEditPreparationRequest, RepositoryMultiFileEditPreparationTarget,
    RepositoryMultiFileEditTextReplacement, RepositoryMultiFileEditTool,
    RepositoryPatchPreparationError, RepositoryPatchPreparationRequest,
    RepositoryPatchResultClassification, RepositoryRenameFilePreparationError,
    RepositoryRenameFilePreparationRequest, RepositoryRenameFileProof, RepositoryStatusTool,
    RepositoryWorktreePatchTool, Tool, ToolContext, ToolError, ToolRegistry,
    authorize_tool_dispatch, authorized_tool_dispatch, classify_repository_patch_output,
};
#[cfg(target_os = "windows")]
use repository_membership::{RepositoryMemberId, WorkspaceMembershipState};
#[cfg(target_os = "windows")]
use serde::{Deserialize, Serialize};
#[cfg(target_os = "windows")]
use sha2::{Digest, Sha256};
#[cfg(target_os = "windows")]
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
use trusted_profile_selection::{
    DesktopTrustedProfileSelection, ProfileSelectionError, TrustedProfilePresentation,
    load_provider_only_profile,
};

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

#[cfg(target_os = "windows")]
static LIVE_EVIDENCE_PROCESS_SALT: OnceLock<String> = OnceLock::new();

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
        profile_generation: u64,
        connection_generation: u64,
        repository_fingerprint: Option<String>,
        composition: Arc<DesktopToolComposition>,
        allowed_permissions: Vec<PermissionLevel>,
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
    next_connection_generation: Mutex<u64>,
    /// Descriptive process-local membership; it retains no executable objects.
    workspace_membership: Mutex<WorkspaceMembershipState>,
    /// Serializes admission and activation publication transactions.
    membership_coordination: Mutex<()>,
    repository: Mutex<Option<Arc<DesktopRepository>>>,
    repository_generation: Mutex<u64>,
    repository_workflow: Mutex<RepositoryWorkflowState>,
    commit_identity: Mutex<Option<DesktopCommitIdentity>>,
    commit_identity_generation: Mutex<u64>,
    commit_capability: Mutex<Option<DesktopCommitCapability>>,
    /// Explicit host-selected Trusted Profile intent. Static selection never activates providers.
    trusted_profile: Mutex<Option<DesktopTrustedProfileSelection>>,
    trusted_profile_generation: Mutex<u64>,
    /// One atomic exclusion coordinator for model turns and explicit host work.
    host_invocation: Mutex<HostInvocationCoordinator>,
    /// One effective provider composition owned by the currently published connection.
    /// Kept outside `ConnectionState` so hard recovery can asynchronously reap providers
    /// after synchronously withdrawing the usable runtime state.
    provider_activation: Mutex<Option<DesktopProviderActivation>>,
    /// An app-owned non-project directory used only when no repository is selected.
    neutral_workspace: Option<PathBuf>,
    model: Mutex<DesktopModelState>,
    preferences: Mutex<Preferences>,
    preference_ordering: Mutex<()>,
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
            next_connection_generation: Mutex::new(0),
            workspace_membership: Mutex::new(WorkspaceMembershipState::new()),
            membership_coordination: Mutex::new(()),
            repository: Mutex::new(None),
            repository_generation: Mutex::new(0),
            repository_workflow: Mutex::new(RepositoryWorkflowState::default()),
            commit_identity: Mutex::new(identity),
            commit_identity_generation: Mutex::new(0),
            commit_capability: Mutex::new(None),
            trusted_profile: Mutex::new(None),
            trusted_profile_generation: Mutex::new(0),
            host_invocation: Mutex::new(HostInvocationCoordinator::default()),
            provider_activation: Mutex::new(None),
            neutral_workspace,
            model: Mutex::new(DesktopModelState {
                selection,
                generation: 0,
                readiness: ReadinessState::NotTested,
            }),
            preferences: Mutex::new(preferences),
            preference_ordering: Mutex::new(()),
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

/// Returns a process-local correlation identifier for a canonical repository.
///
/// The salt prevents this diagnostic value from being a durable or
/// dictionary-friendly path hash. It is correlation metadata only, not a
/// repository identity, authorization token, or security boundary.
#[cfg(target_os = "windows")]
fn repository_context_fingerprint(root: &Path) -> String {
    let salt = LIVE_EVIDENCE_PROCESS_SALT.get_or_init(|| {
        format!(
            "rah-desktop-live-evidence:{}:{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .map_or(0, |duration| duration.as_nanos())
        )
    });
    let mut digest = Sha256::new();
    digest.update(salt.as_bytes());
    digest.update([0]);
    digest.update(root.as_os_str().to_string_lossy().as_bytes());
    format!("repo-context:{:x}", digest.finalize())
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
            let mut coordinator = self
                .host_invocation
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            coordinator.reap_expired(std::time::Instant::now());
            coordinator
                .begin_model()
                .map_err(|_| FrontendError::HostInvocationBusy)?;
        }
        {
            let mut chat = self
                .chat
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Err(error) = begin_chat(&mut chat) {
                self.host_invocation
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .release_model();
                return Err(error);
            }
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
        self.host_invocation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .release_model();
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
        self.host_invocation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .release_model();
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
        self.host_invocation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .release_model();
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
        let profile_selected = self
            .trusted_profile
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_some();
        let provider_active = self
            .provider_activation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_some();
        let mut status = current_app_status(
            &connection,
            repository_selected,
            repository_generation,
            model_generation,
        );
        let current_profile_generation = *self
            .trusted_profile_generation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        status.profile_status = match (&*connection, profile_selected, provider_active) {
            (
                ConnectionState::Connected {
                    profile_generation, ..
                },
                _,
                _,
            ) if *profile_generation != current_profile_generation => "reconnect required",
            (ConnectionState::Connected { .. }, true, true) => "active",
            (ConnectionState::Connecting, true, _) => "activating",
            (_, true, _) => "configured; providers inactive",
            _ => "not loaded",
        };
        status
    }

    fn close_started(&self) -> bool {
        self.close_started
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }

    async fn shutdown_for_exit(&self) {
        self.host_invocation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clear_prepared();
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
        self.shutdown_provider_activation().await;
    }

    async fn shutdown_provider_activation(&self) {
        let activation = self
            .provider_activation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        if let Some(activation) = activation {
            activation.shutdown().await;
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
    directory_creation_authority: Option<RepositoryDirectoryCreationAuthority>,
    deletion_authority: Option<RepositoryFileDeletionAuthority>,
    rename_authority: Option<RepositoryFileRenameAuthority>,
    branch_creation_authority: Option<RepositoryBranchCreationAuthority>,
}

#[cfg(target_os = "windows")]
impl DesktopRepository {
    #[cfg(test)]
    fn new(
        git_executable: &std::path::Path,
        repository_root: &std::path::Path,
    ) -> Result<Self, ToolError> {
        Self::new_with_authorities(git_executable, repository_root, None, None, None, None)
    }

    fn new_with_authorities(
        git_executable: &std::path::Path,
        repository_root: &std::path::Path,
        directory_creation_authority: Option<RepositoryDirectoryCreationAuthority>,
        deletion_authority: Option<RepositoryFileDeletionAuthority>,
        rename_authority: Option<RepositoryFileRenameAuthority>,
        branch_creation_authority: Option<RepositoryBranchCreationAuthority>,
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
        if let Some(authority) = &directory_creation_authority
            && !authority.matches_repository_root(&repository_root)
        {
            return Err(ToolError::Execution {
                message: "directory creation authority does not match selected repository"
                    .to_owned(),
            });
        }
        if let Some(authority) = &rename_authority
            && !authority.matches_resources(git_executable, &repository_root)
        {
            return Err(ToolError::Execution {
                message: "rename authority does not match selected repository".to_owned(),
            });
        }
        if let Some(authority) = &branch_creation_authority
            && !authority.matches_resources(git_executable, &repository_root)
        {
            return Err(ToolError::Execution {
                message: "branch creation authority does not match selected repository".to_owned(),
            });
        }
        Ok(Self {
            display_path: repository_root.display().to_string(),
            git_executable: git_executable.to_path_buf(),
            root: repository_root,
            status: Arc::new(status),
            worktree_diff: Arc::new(worktree_diff),
            staged_diff: Arc::new(staged_diff),
            directory_creation_authority,
            deletion_authority,
            rename_authority,
            branch_creation_authority,
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
    ProfileInvalid,
    ProfileFirstPartyCapabilitiesUnsupported,
    ProfileActivationFailed,
    ProfileDialogFailed,
    NoRememberedTrustedProfile,
    TrustedProfilePreferenceSaveFailed,
    ProfileBusy,
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
    RepositoryAlreadyMember,
    RepositoryNestedMembershipConflict,
    RepositoryMemberStale,
    RepositoryMemberNotFound,
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
    HostInvocationBusy,
    HostInvocationNotConnected,
    HostInvocationNotEligible,
    HostInvocationPermissionDenied,
    HostInvocationStale,
    HostInvocationInvalidInput,
    HostInvocationTicketInvalid,
    HostInvocationInvalidTarget,
    HostInvocationPreconditionChanged,
    HostInvocationReviewTooLarge,
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
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)]
enum HostActivityState {
    Prepared,
    Started,
    ToolCompleted,
    ToolError,
    PartialEffect,
    RejectedNotEligible,
    RejectedPermission,
    RejectedStale,
    RejectedBusy,
    InvalidInput,
    CancelledBeforeStart,
    PossibleEffectUnknown,
}

#[cfg(target_os = "windows")]
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct HostActivityEvent {
    source: &'static str,
    invocation_id: String,
    tool: String,
    state: HostActivityState,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<ToolOutput>,
    #[serde(skip_serializing_if = "Option::is_none")]
    review: Option<HostInvocationReview>,
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
fn connection_activation_publication_is_current(captured: [u64; 4], current: [u64; 4]) -> bool {
    captured == current
}

#[cfg(target_os = "windows")]
fn current_host_generation_tuple(state: &DesktopAppState) -> [u64; 4] {
    [
        *state
            .repository_generation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
        state
            .model
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .generation,
        *state
            .trusted_profile_generation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
        *state
            .next_connection_generation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
    ]
}

#[cfg(target_os = "windows")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProviderPublicationRejectionReason {
    Superseded,
    Stale,
    DuplicateOwner,
}

#[cfg(target_os = "windows")]
struct PendingConnectedPublication {
    runtime: Arc<CodexRuntime>,
    activation: Option<DesktopProviderActivation>,
    source: CodexExecutableSource,
    repository_generation: u64,
    model_generation: u64,
    profile_generation: u64,
    connection_generation: u64,
    repository_fingerprint: Option<String>,
    composition: Arc<DesktopToolComposition>,
    allowed_permissions: Vec<PermissionLevel>,
}

#[cfg(target_os = "windows")]
struct RejectedProviderPublication {
    runtime: Arc<CodexRuntime>,
    activation: Option<DesktopProviderActivation>,
    reason: ProviderPublicationRejectionReason,
}

#[cfg(target_os = "windows")]
fn publish_connected_provider_state(
    state: &DesktopAppState,
    pending: PendingConnectedPublication,
) -> Result<(), Box<RejectedProviderPublication>> {
    let mut connection = state
        .connection
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if !matches!(*connection, ConnectionState::Connecting) {
        let PendingConnectedPublication {
            runtime,
            activation,
            ..
        } = pending;
        return Err(Box::new(RejectedProviderPublication {
            runtime,
            activation,
            reason: ProviderPublicationRejectionReason::Superseded,
        }));
    }

    let current_repository_generation = state
        .repository_generation
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let current_model = state
        .model
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let current_profile_generation = state
        .trusted_profile_generation
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let current_connection_generation = state
        .next_connection_generation
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if !connection_activation_publication_is_current(
        [
            pending.repository_generation,
            pending.model_generation,
            pending.profile_generation,
            pending.connection_generation,
        ],
        [
            *current_repository_generation,
            current_model.generation,
            *current_profile_generation,
            *current_connection_generation,
        ],
    ) {
        let PendingConnectedPublication {
            runtime,
            activation,
            ..
        } = pending;
        return Err(Box::new(RejectedProviderPublication {
            runtime,
            activation,
            reason: ProviderPublicationRejectionReason::Stale,
        }));
    }

    let mut published_provider = state
        .provider_activation
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if published_provider.is_some() {
        let PendingConnectedPublication {
            runtime,
            activation,
            ..
        } = pending;
        return Err(Box::new(RejectedProviderPublication {
            runtime,
            activation,
            reason: ProviderPublicationRejectionReason::DuplicateOwner,
        }));
    }

    let PendingConnectedPublication {
        runtime,
        activation,
        source,
        repository_generation,
        model_generation,
        profile_generation,
        connection_generation,
        repository_fingerprint,
        composition,
        allowed_permissions,
    } = pending;
    *published_provider = activation;
    *connection = ConnectionState::Connected {
        runtime,
        source,
        repository_generation,
        model_generation,
        profile_generation,
        connection_generation,
        repository_fingerprint,
        composition,
        allowed_permissions,
    };
    Ok(())
}

#[cfg(target_os = "windows")]
fn safe_repository_display_name(root: &Path) -> Option<String> {
    let name = root.file_name()?.to_str()?;
    if name.is_empty() || name.len() > 255 || name.chars().any(char::is_control) {
        return None;
    }
    Some(name.to_owned())
}

#[cfg(target_os = "windows")]
#[tauri::command]
fn get_effective_authority_snapshot(
    state: State<'_, DesktopAppState>,
) -> EffectiveAuthoritySnapshot {
    let repository = state
        .repository
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    let current_repository_generation = *state
        .repository_generation
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let current_model_generation = state
        .model
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .generation;
    let connection = state
        .connection
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let workflow = state
        .repository_workflow
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let profile_selection = state
        .trusted_profile
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    let selected = repository.is_some();
    let (connection_binding, composition, status) = match &*connection {
        ConnectionState::Connected {
            source,
            repository_generation,
            model_generation,
            profile_generation,
            connection_generation,
            composition,
            ..
        } => {
            let current_profile_generation = *state
                .trusted_profile_generation
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let current_connection_generation = *state
                .next_connection_generation
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let context_current = [
                *repository_generation,
                *model_generation,
                *profile_generation,
            ] == [
                current_repository_generation,
                current_model_generation,
                current_profile_generation,
            ];
            let publication_current = connection_activation_publication_is_current(
                [
                    *repository_generation,
                    *model_generation,
                    *profile_generation,
                    *connection_generation,
                ],
                [
                    current_repository_generation,
                    current_model_generation,
                    current_profile_generation,
                    current_connection_generation,
                ],
            );
            let repository_context_matches = selected || *repository_generation == 0;
            let current = context_current
                && publication_current
                && repository_context_matches
                && composition.registry.definitions().len() == composition.tools.len();
            let status = if current {
                SnapshotStatus::ConnectedCurrent
            } else if context_current {
                SnapshotStatus::Stale
            } else {
                SnapshotStatus::ReconnectRequired
            };
            (
                ConnectionBinding {
                    state: ConnectionBindingState::Connected,
                    runtime_kind: Some("codex"),
                    runtime_source: Some(effective_authority::source_label(*source)),
                    captured_repository_generation: Some(*repository_generation),
                    captured_model_generation: Some(*model_generation),
                    captured_connection_generation: Some(*connection_generation),
                    advertised: current,
                },
                Some(Arc::clone(composition)),
                status,
            )
        }
        ConnectionState::Connecting => (
            ConnectionBinding {
                state: ConnectionBindingState::Connecting,
                runtime_kind: None,
                runtime_source: None,
                captured_repository_generation: None,
                captured_model_generation: None,
                captured_connection_generation: None,
                advertised: false,
            },
            None,
            SnapshotStatus::Connecting,
        ),
        ConnectionState::Disconnecting => (
            ConnectionBinding {
                state: ConnectionBindingState::Disconnecting,
                runtime_kind: None,
                runtime_source: None,
                captured_repository_generation: None,
                captured_model_generation: None,
                captured_connection_generation: None,
                advertised: false,
            },
            None,
            SnapshotStatus::Stale,
        ),
        ConnectionState::Error(_) => (
            ConnectionBinding {
                state: ConnectionBindingState::Error,
                runtime_kind: None,
                runtime_source: None,
                captured_repository_generation: None,
                captured_model_generation: None,
                captured_connection_generation: None,
                advertised: false,
            },
            None,
            SnapshotStatus::Unavailable,
        ),
        ConnectionState::NotConnected => (
            ConnectionBinding {
                state: ConnectionBindingState::NotConnected,
                runtime_kind: None,
                runtime_source: None,
                captured_repository_generation: None,
                captured_model_generation: None,
                captured_connection_generation: None,
                advertised: false,
            },
            None,
            if selected {
                SnapshotStatus::Disconnected
            } else {
                SnapshotStatus::NoRepository
            },
        ),
    };
    let allowed_permissions = match &*connection {
        ConnectionState::Connected {
            allowed_permissions,
            ..
        } => allowed_permissions.clone(),
        _ => Vec::new(),
    };
    let connection_is_error = matches!(&*connection, ConnectionState::Error(_));
    drop(connection);
    let mut effective_tools = composition
        .as_ref()
        .map_or_else(Vec::new, |value| value.tools.clone());
    let coordinator_state = {
        let mut coordinator = state
            .host_invocation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        coordinator.reap_expired(std::time::Instant::now());
        coordinator.state()
    };
    let branch_authority_present = repository
        .as_ref()
        .is_some_and(|value| value.branch_creation_authority.is_some());
    for tool in &mut effective_tools {
        tool.advertised = connection_binding.advertised;
        tool.host_invocation = host_descriptor_with_rename(
            tool,
            status == SnapshotStatus::ConnectedCurrent,
            selected,
            allowed_permissions.contains(&tool.permission),
            branch_authority_present,
            composition
                .as_ref()
                .is_some_and(|value| value.repository_patch_preparer.is_some()),
            composition
                .as_ref()
                .is_some_and(|value| value.repository_multi_file_edit_preparer.is_some()),
            composition
                .as_ref()
                .is_some_and(|value| value.repository_create_file_preparer.is_some()),
            composition
                .as_ref()
                .is_some_and(|value| value.repository_delete_file_preparer.is_some()),
            composition
                .as_ref()
                .is_some_and(|value| value.repository_rename_file_preparer.is_some()),
            coordinator_state,
        );
    }
    let mut unavailable_capabilities = composition
        .as_ref()
        .map_or_else(Vec::new, |value| value.unavailable.clone());
    if composition.is_none()
        && let Some(profile) = profile_selection.as_ref()
    {
        let reason = if connection_is_error {
            effective_authority::UnavailableReason::ProviderUnavailable
        } else {
            effective_authority::UnavailableReason::ProviderNotEffective
        };
        unavailable_capabilities.extend(
            profile
                .external_tools()
                .iter()
                .map(|tool| effective_authority::external_unavailable(tool, reason)),
        );
    }
    let configured = profile_selection.as_ref().map_or(
        ConfiguredSummary {
            profile_source: Some(SourceKind::BuiltIn),
            configured_provider_count: 0,
            configured_capability_count: effective_tools.len() as u32,
        },
        |profile| ConfiguredSummary {
            profile_source: Some(SourceKind::TrustedProfile),
            configured_provider_count: profile.presentation().configured_provider_count,
            configured_capability_count: profile.presentation().expected_tool_count,
        },
    );
    let repository_binding = RepositoryBinding {
        selected,
        display_name: repository
            .as_deref()
            .and_then(|value| safe_repository_display_name(&value.root)),
        kind: if selected {
            RepositoryKind::SelectedRepository
        } else {
            RepositoryKind::None
        },
        current_generation: selected.then_some(current_repository_generation),
        captured_generation: connection_binding.captured_repository_generation,
        identity: if !selected {
            RepositoryIdentity::NotSelected
        } else if connection_binding.advertised {
            RepositoryIdentity::Current
        } else if connection_binding.captured_repository_generation.is_some() {
            RepositoryIdentity::Stale
        } else {
            RepositoryIdentity::Unknown
        },
    };
    EffectiveAuthoritySnapshot {
        schema_version: 1,
        status,
        repository: repository_binding,
        connection: connection_binding,
        configured,
        effective_tools,
        unavailable_capabilities,
        reviewed_commit: effective_authority::reviewed_commit(workflow.authorization, selected),
    }
}

#[cfg(target_os = "windows")]
#[derive(Clone)]
struct CurrentHostComposition {
    registry: Arc<ToolRegistry>,
    expected_definitions: Vec<ToolDefinition>,
    composition_identity: usize,
    allowed_permissions: Vec<PermissionLevel>,
    generations: [u64; 4],
    repository_identity: Option<String>,
    repository: Option<Arc<DesktopRepository>>,
    tools: Vec<EffectiveToolEntry>,
    repository_patch_preparer: Option<Arc<rah_tools::RepositoryPatchPreparer>>,
    repository_multi_file_edit_preparer: Option<Arc<rah_tools::RepositoryMultiFileEditPreparer>>,
    repository_create_file_preparer: Option<Arc<rah_tools::RepositoryCreateFilePreparer>>,
    repository_delete_file_preparer: Option<Arc<rah_tools::RepositoryDeleteFilePreparer>>,
    repository_rename_file_preparer: Option<Arc<rah_tools::RepositoryRenameFilePreparer>>,
}

#[cfg(target_os = "windows")]
fn current_host_composition(
    state: &DesktopAppState,
) -> Result<CurrentHostComposition, FrontendError> {
    let current_generations = current_host_generation_tuple(state);
    let repository = state
        .repository
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    let repository_identity = repository
        .as_ref()
        .map(|value| repository_context_fingerprint(&value.root));
    let connection = state
        .connection
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let ConnectionState::Connected {
        runtime: _,
        repository_generation,
        model_generation,
        profile_generation,
        connection_generation,
        repository_fingerprint,
        composition,
        allowed_permissions,
        ..
    } = &*connection
    else {
        return Err(FrontendError::HostInvocationNotConnected);
    };
    let generations = [
        *repository_generation,
        *model_generation,
        *profile_generation,
        *connection_generation,
    ];
    if generations != current_generations
        || repository_identity != *repository_fingerprint
        || composition.registry.definitions().len() != composition.tools.len()
        || composition.expected_definitions.len() != composition.tools.len()
    {
        return Err(FrontendError::HostInvocationStale);
    }
    Ok(CurrentHostComposition {
        registry: Arc::clone(&composition.registry),
        expected_definitions: composition.expected_definitions.clone(),
        composition_identity: Arc::as_ptr(&composition.registry) as usize,
        allowed_permissions: allowed_permissions.clone(),
        generations,
        repository_identity,
        repository,
        tools: composition.tools.clone(),
        repository_patch_preparer: composition.repository_patch_preparer.clone(),
        repository_multi_file_edit_preparer: composition
            .repository_multi_file_edit_preparer
            .clone(),
        repository_create_file_preparer: composition.repository_create_file_preparer.clone(),
        repository_delete_file_preparer: composition.repository_delete_file_preparer.clone(),
        repository_rename_file_preparer: composition.repository_rename_file_preparer.clone(),
    })
}

#[cfg(target_os = "windows")]
fn host_tool_definition(
    current: &CurrentHostComposition,
    name: &ToolName,
    coordinator_state: CoordinatorState,
) -> Result<ToolDefinition, FrontendError> {
    let effective = current
        .tools
        .iter()
        .find(|entry| entry.public_tool_name == name.as_str())
        .cloned()
        .ok_or(FrontendError::HostInvocationNotEligible)?;
    let entry = current
        .expected_definitions
        .iter()
        .find(|definition| definition.name == *name)
        .cloned()
        .ok_or(FrontendError::HostInvocationNotEligible)?;
    if host_kind(name.as_str()).is_none() {
        return Err(FrontendError::HostInvocationNotEligible);
    }
    let descriptor = host_descriptor_with_rename(
        &effective,
        true,
        current.repository.is_some(),
        current.allowed_permissions.contains(&entry.permission),
        current
            .repository
            .as_ref()
            .is_some_and(|value| value.branch_creation_authority.is_some()),
        current.repository_patch_preparer.is_some(),
        current.repository_multi_file_edit_preparer.is_some(),
        current.repository_create_file_preparer.is_some(),
        current.repository_delete_file_preparer.is_some(),
        current.repository_rename_file_preparer.is_some(),
        coordinator_state,
    );
    if !descriptor.eligible {
        return Err(match descriptor.unavailable_reason {
            Some(HostInvocationUnavailableReason::PermissionDenied) => {
                FrontendError::HostInvocationPermissionDenied
            }
            Some(HostInvocationUnavailableReason::ModelTurnActive)
            | Some(HostInvocationUnavailableReason::HostInvocationBusy) => {
                FrontendError::HostInvocationBusy
            }
            Some(HostInvocationUnavailableReason::RepositoryRequired)
            | Some(HostInvocationUnavailableReason::AuthorityNotGranted)
            | Some(HostInvocationUnavailableReason::NotSupported)
            | Some(HostInvocationUnavailableReason::ProviderNotSupported)
            | None => FrontendError::HostInvocationNotEligible,
            Some(
                HostInvocationUnavailableReason::NotConnectedCurrent
                | HostInvocationUnavailableReason::Stale,
            ) => FrontendError::HostInvocationStale,
        });
    }
    Ok(entry)
}

#[cfg(target_os = "windows")]
fn emit_host_activity(app: &AppHandle, event: HostActivityEvent) {
    if let Err(error) = app.emit("host_activity_event", event) {
        tracing::warn!(error = %error, "failed to emit explicit host activity");
    }
}

#[cfg(target_os = "windows")]
fn prepared_host_activity(
    activity_id: String,
    tool: String,
    review: Option<HostInvocationReview>,
) -> HostActivityEvent {
    HostActivityEvent {
        source: "host_explicit",
        invocation_id: activity_id,
        tool,
        state: HostActivityState::Prepared,
        result: None,
        review,
    }
}

#[cfg(target_os = "windows")]
fn host_call(name: ToolName, input: ToolInput) -> ToolCall {
    ToolCall {
        id: ToolCallId::new(),
        name,
        input,
    }
}

#[cfg(target_os = "windows")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DeleteFileResultClassification {
    DeletedVerified,
    KnownNoEffect,
    InvalidInput,
    PreconditionFailed,
    Uncertain,
    Malformed,
}

#[cfg(target_os = "windows")]
fn classify_repository_delete_file_output(
    output: &ToolOutput,
    expected_path: &str,
) -> DeleteFileResultClassification {
    let [ToolContent::Json(value)] = output.content.as_slice() else {
        return DeleteFileResultClassification::Malformed;
    };
    let Some(object) = value.as_object() else {
        return DeleteFileResultClassification::Malformed;
    };
    let Some(status) = object.get("status").and_then(serde_json::Value::as_str) else {
        return DeleteFileResultClassification::Malformed;
    };
    let uncertain = object.get("uncertain").and_then(serde_json::Value::as_bool);
    let path_matches = object
        .get("path")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|path| path == expected_path);
    let exact_keys = |keys: &[&str]| {
        object.len() == keys.len() && keys.iter().all(|key| object.contains_key(*key))
    };
    match status {
        "deleted_verified" => {
            if output.is_error
                || !exact_keys(&["status", "uncertain", "path"])
                || uncertain != Some(false)
                || !path_matches
            {
                DeleteFileResultClassification::Malformed
            } else {
                DeleteFileResultClassification::DeletedVerified
            }
        }
        "known_no_effect" => {
            if !output.is_error
                || !exact_keys(&["status", "uncertain", "path"])
                || uncertain != Some(false)
                || !path_matches
            {
                DeleteFileResultClassification::Malformed
            } else {
                DeleteFileResultClassification::KnownNoEffect
            }
        }
        "invalid_input" => {
            if !output.is_error || !exact_keys(&["status", "uncertain"]) || uncertain != Some(false)
            {
                DeleteFileResultClassification::Malformed
            } else {
                DeleteFileResultClassification::InvalidInput
            }
        }
        "precondition_failed" => {
            if !output.is_error
                || !exact_keys(&["status", "uncertain", "path"])
                || uncertain != Some(false)
                || !path_matches
            {
                DeleteFileResultClassification::Malformed
            } else {
                DeleteFileResultClassification::PreconditionFailed
            }
        }
        "uncertain" => {
            let valid_path = object.get("path").is_none() || path_matches;
            if !output.is_error
                || uncertain != Some(true)
                || !(exact_keys(&["status", "uncertain"])
                    || exact_keys(&["status", "uncertain", "path"]))
                || !valid_path
            {
                DeleteFileResultClassification::Malformed
            } else {
                DeleteFileResultClassification::Uncertain
            }
        }
        _ => DeleteFileResultClassification::Malformed,
    }
}

#[cfg(target_os = "windows")]
fn delete_file_host_terminal_state(
    classification: DeleteFileResultClassification,
) -> HostActivityState {
    match classification {
        DeleteFileResultClassification::DeletedVerified => HostActivityState::ToolCompleted,
        DeleteFileResultClassification::KnownNoEffect
        | DeleteFileResultClassification::InvalidInput
        | DeleteFileResultClassification::PreconditionFailed => HostActivityState::ToolError,
        DeleteFileResultClassification::Uncertain | DeleteFileResultClassification::Malformed => {
            HostActivityState::PossibleEffectUnknown
        }
    }
}

#[cfg(target_os = "windows")]
fn safe_delete_file_activity_result(classification: DeleteFileResultClassification) -> ToolOutput {
    let (status, is_error) = match classification {
        DeleteFileResultClassification::DeletedVerified => ("deleted_verified", false),
        DeleteFileResultClassification::KnownNoEffect => ("known_no_effect", true),
        DeleteFileResultClassification::InvalidInput => ("invalid_input", true),
        DeleteFileResultClassification::PreconditionFailed => ("precondition_failed", true),
        DeleteFileResultClassification::Uncertain | DeleteFileResultClassification::Malformed => {
            ("uncertain", true)
        }
    };
    ToolOutput {
        content: vec![ToolContent::Json(serde_json::json!({"status": status}))],
        is_error,
    }
}

#[cfg(target_os = "windows")]
fn rename_file_host_terminal_state(proof: RepositoryRenameFileProof) -> HostActivityState {
    match proof {
        RepositoryRenameFileProof::ReviewedSuccess => HostActivityState::ToolCompleted,
        RepositoryRenameFileProof::KnownNoEffect => HostActivityState::ToolError,
        RepositoryRenameFileProof::Uncertain => HostActivityState::PossibleEffectUnknown,
    }
}

#[cfg(target_os = "windows")]
fn safe_rename_file_activity_result(proof: RepositoryRenameFileProof) -> ToolOutput {
    let (status, is_error) = match proof {
        RepositoryRenameFileProof::ReviewedSuccess => ("renamed_verified", false),
        RepositoryRenameFileProof::KnownNoEffect => ("known_no_effect", true),
        RepositoryRenameFileProof::Uncertain => ("uncertain", true),
    };
    ToolOutput {
        content: vec![ToolContent::Json(serde_json::json!({"status": status}))],
        is_error,
    }
}

#[cfg(target_os = "windows")]
#[derive(Clone, Debug, PartialEq, Eq)]
struct CreateFileExpectedOutput {
    path: String,
    length: usize,
    sha256: String,
}

#[cfg(target_os = "windows")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CreateFileResultClassification {
    Ok,
    InvalidTarget,
    PreconditionFailed,
    CreateFailedKnown,
    WriteFailedKnown,
    Uncertain,
    Malformed,
}

#[cfg(target_os = "windows")]
fn classify_repository_create_file_output(
    output: &ToolOutput,
    expected: &CreateFileExpectedOutput,
) -> CreateFileResultClassification {
    let [ToolContent::Json(value)] = output.content.as_slice() else {
        return CreateFileResultClassification::Malformed;
    };
    let Some(object) = value.as_object() else {
        return CreateFileResultClassification::Malformed;
    };
    let exact_keys = |keys: &[&str]| {
        object.len() == keys.len() && keys.iter().all(|key| object.contains_key(*key))
    };
    let Some(status) = object.get("status").and_then(serde_json::Value::as_str) else {
        return CreateFileResultClassification::Malformed;
    };
    if status == "ok" {
        let length_matches = object
            .get("length")
            .and_then(serde_json::Value::as_u64)
            .and_then(|length| usize::try_from(length).ok())
            == Some(expected.length);
        let valid = !output.is_error
            && exact_keys(&["status", "path", "length", "sha256"])
            && object.get("path").and_then(serde_json::Value::as_str)
                == Some(expected.path.as_str())
            && length_matches
            && object.get("sha256").and_then(serde_json::Value::as_str)
                == Some(expected.sha256.as_str());
        return if valid {
            CreateFileResultClassification::Ok
        } else {
            CreateFileResultClassification::Malformed
        };
    }
    let classification = match status {
        "invalid_target" => CreateFileResultClassification::InvalidTarget,
        "precondition_failed" => CreateFileResultClassification::PreconditionFailed,
        "create_failed_known" => CreateFileResultClassification::CreateFailedKnown,
        "write_failed_known" => CreateFileResultClassification::WriteFailedKnown,
        "uncertain" => CreateFileResultClassification::Uncertain,
        _ => return CreateFileResultClassification::Malformed,
    };
    if output.is_error && exact_keys(&["status"]) {
        classification
    } else {
        CreateFileResultClassification::Malformed
    }
}

#[cfg(target_os = "windows")]
fn create_file_host_terminal_state(
    classification: CreateFileResultClassification,
) -> HostActivityState {
    match classification {
        CreateFileResultClassification::Ok => HostActivityState::ToolCompleted,
        CreateFileResultClassification::InvalidTarget
        | CreateFileResultClassification::PreconditionFailed
        | CreateFileResultClassification::CreateFailedKnown => HostActivityState::ToolError,
        CreateFileResultClassification::WriteFailedKnown => HostActivityState::PartialEffect,
        CreateFileResultClassification::Uncertain | CreateFileResultClassification::Malformed => {
            HostActivityState::PossibleEffectUnknown
        }
    }
}

#[cfg(target_os = "windows")]
fn safe_create_file_activity_result(
    classification: CreateFileResultClassification,
) -> Option<ToolOutput> {
    let status = match classification {
        CreateFileResultClassification::Ok => "ok",
        CreateFileResultClassification::InvalidTarget => "invalid_target",
        CreateFileResultClassification::PreconditionFailed => "precondition_failed",
        CreateFileResultClassification::CreateFailedKnown => "create_failed_known",
        CreateFileResultClassification::WriteFailedKnown => "write_failed_known",
        CreateFileResultClassification::Uncertain => "uncertain",
        CreateFileResultClassification::Malformed => return None,
    };
    Some(ToolOutput {
        content: vec![ToolContent::Json(serde_json::json!({"status": status}))],
        is_error: classification != CreateFileResultClassification::Ok,
    })
}

#[cfg(target_os = "windows")]
#[allow(clippy::too_many_arguments)]
async fn run_host_tool(
    app: AppHandle,
    registry: Arc<ToolRegistry>,
    expected_definition: ToolDefinition,
    allowed_permissions: Vec<PermissionLevel>,
    call: ToolCall,
    kind: HostInvocationKind,
    invocation_id: String,
    repository_identity: Option<String>,
    generations: [u64; 4],
    multi_file_target_order: Option<Vec<String>>,
    create_file_expected_output: Option<CreateFileExpectedOutput>,
    deletion_proof: Option<(
        Box<rah_tools::RepositoryDeleteFilePreparation>,
        Arc<rah_tools::RepositoryDeleteFilePreparer>,
    )>,
    rename_proof: Option<(
        Box<rah_tools::RepositoryRenameFilePreparation>,
        Arc<rah_tools::RepositoryRenameFilePreparer>,
    )>,
) {
    let tool_name = call.name.to_string();
    let result = authorized_tool_dispatch(
        &registry,
        &expected_definition,
        &allowed_permissions,
        call,
        ToolContext::default(),
    )
    .await;
    let state = app.state::<DesktopAppState>();
    let (activity_state, output) = match result {
        Ok(output) => {
            if kind == HostInvocationKind::RepoPatch {
                let classification = classify_repository_patch_output(&output);
                if host_repository_context_is_current(
                    state.inner(),
                    repository_identity.as_deref(),
                    generations,
                ) {
                    invalidate_repository_commit_review(state.inner()).await;
                    let _ = refresh_repository_workflow(state.inner()).await;
                    emit_repository_refresh(&app);
                }
                (
                    patch_host_terminal_state(classification),
                    (classification != RepositoryPatchResultClassification::Malformed)
                        .then_some(output),
                )
            } else if kind == HostInvocationKind::RepoEditFiles {
                let classification = classify_repository_multi_file_output(
                    &output,
                    multi_file_target_order.as_deref().unwrap_or(&[]),
                );
                invalidate_repository_commit_review(state.inner()).await;
                if host_repository_context_is_current(
                    state.inner(),
                    repository_identity.as_deref(),
                    generations,
                ) {
                    let _ = refresh_repository_workflow(state.inner()).await;
                    emit_repository_refresh(&app);
                }
                (
                    multi_file_host_terminal_state(classification),
                    safe_multi_file_activity_result(output, classification),
                )
            } else if kind == HostInvocationKind::RepoDeleteFile {
                let classification = classify_repository_delete_file_result(
                    &output,
                    deletion_proof
                        .as_ref()
                        .map(|(preparation, _)| preparation.review().path())
                        .unwrap_or_default(),
                    deletion_proof.as_ref(),
                )
                .await;
                if matches!(
                    classification,
                    DeleteFileResultClassification::DeletedVerified
                        | DeleteFileResultClassification::KnownNoEffect
                        | DeleteFileResultClassification::Uncertain
                        | DeleteFileResultClassification::Malformed
                ) && host_repository_context_is_current(
                    state.inner(),
                    repository_identity.as_deref(),
                    generations,
                ) {
                    let _ = refresh_repository_workflow(state.inner()).await;
                    emit_repository_refresh(&app);
                }
                (
                    delete_file_host_terminal_state(classification),
                    Some(safe_delete_file_activity_result(classification)),
                )
            } else if kind == HostInvocationKind::RepoRenameFile {
                let proof = match rename_proof.as_ref() {
                    Some((preparation, preparer)) => {
                        preparer.prove_result(preparation, &output).await
                    }
                    None => RepositoryRenameFileProof::Uncertain,
                };
                if host_repository_context_is_current(
                    state.inner(),
                    repository_identity.as_deref(),
                    generations,
                ) {
                    let _ = refresh_repository_workflow(state.inner()).await;
                    emit_repository_refresh(&app);
                }
                (
                    rename_file_host_terminal_state(proof),
                    Some(safe_rename_file_activity_result(proof)),
                )
            } else if kind == HostInvocationKind::RepoCreateFile {
                let classification = create_file_expected_output
                    .as_ref()
                    .map_or(CreateFileResultClassification::Malformed, |expected| {
                        classify_repository_create_file_output(&output, expected)
                    });
                invalidate_repository_commit_review(state.inner()).await;
                if host_repository_context_is_current(
                    state.inner(),
                    repository_identity.as_deref(),
                    generations,
                ) {
                    let _ = refresh_repository_workflow(state.inner()).await;
                    emit_repository_refresh(&app);
                }
                (
                    create_file_host_terminal_state(classification),
                    safe_create_file_activity_result(classification),
                )
            } else {
                if kind == HostInvocationKind::RepoCreateBranch
                    && matches!(
                        branch_result_classification(&output),
                        BranchActivityClassification::Uncertain
                    )
                {
                    invalidate_repository_commit_review(state.inner()).await;
                    if host_repository_context_is_current(
                        state.inner(),
                        repository_identity.as_deref(),
                        generations,
                    ) {
                        emit_repository_refresh(&app);
                    }
                }
                (
                    if output.is_error {
                        HostActivityState::ToolError
                    } else {
                        HostActivityState::ToolCompleted
                    },
                    Some(output),
                )
            }
        }
        Err(AuthorizedDispatchError::Rejected(_)) => {
            if kind == HostInvocationKind::RepoDeleteFile {
                if host_repository_context_is_current(
                    state.inner(),
                    repository_identity.as_deref(),
                    generations,
                ) {
                    let _ = refresh_repository_workflow(state.inner()).await;
                    emit_repository_refresh(&app);
                }
            } else if repository_bound_authoring_kind(kind) {
                invalidate_repository_commit_review(state.inner()).await;
                if host_repository_context_is_current(
                    state.inner(),
                    repository_identity.as_deref(),
                    generations,
                ) {
                    let _ = refresh_repository_workflow(state.inner()).await;
                    emit_repository_refresh(&app);
                }
            }
            if kind == HostInvocationKind::RepoDeleteFile {
                (
                    HostActivityState::PossibleEffectUnknown,
                    Some(safe_delete_file_activity_result(
                        DeleteFileResultClassification::Uncertain,
                    )),
                )
            } else if kind == HostInvocationKind::RepoRenameFile {
                if host_repository_context_is_current(
                    state.inner(),
                    repository_identity.as_deref(),
                    generations,
                ) {
                    let _ = refresh_repository_workflow(state.inner()).await;
                    emit_repository_refresh(&app);
                }
                (
                    HostActivityState::PossibleEffectUnknown,
                    Some(safe_rename_file_activity_result(
                        RepositoryRenameFileProof::Uncertain,
                    )),
                )
            } else {
                (HostActivityState::RejectedStale, None)
            }
        }
        Err(AuthorizedDispatchError::Tool(_)) => {
            if kind == HostInvocationKind::RepoDeleteFile {
                if host_repository_context_is_current(
                    state.inner(),
                    repository_identity.as_deref(),
                    generations,
                ) {
                    let _ = refresh_repository_workflow(state.inner()).await;
                    emit_repository_refresh(&app);
                }
            } else if kind == HostInvocationKind::RepoPatch {
                if host_repository_context_is_current(
                    state.inner(),
                    repository_identity.as_deref(),
                    generations,
                ) {
                    invalidate_repository_commit_review(state.inner()).await;
                    let _ = refresh_repository_workflow(state.inner()).await;
                    emit_repository_refresh(&app);
                }
            } else if kind == HostInvocationKind::RepoEditFiles {
                invalidate_repository_commit_review(state.inner()).await;
                if host_repository_context_is_current(
                    state.inner(),
                    repository_identity.as_deref(),
                    generations,
                ) {
                    let _ = refresh_repository_workflow(state.inner()).await;
                    emit_repository_refresh(&app);
                }
            } else if kind == HostInvocationKind::RepoCreateBranch {
                invalidate_repository_commit_review(state.inner()).await;
                if host_repository_context_is_current(
                    state.inner(),
                    repository_identity.as_deref(),
                    generations,
                ) {
                    emit_repository_refresh(&app);
                }
            } else if kind == HostInvocationKind::RepoCreateFile {
                invalidate_repository_commit_review(state.inner()).await;
                if host_repository_context_is_current(
                    state.inner(),
                    repository_identity.as_deref(),
                    generations,
                ) {
                    let _ = refresh_repository_workflow(state.inner()).await;
                    emit_repository_refresh(&app);
                }
            }
            if kind == HostInvocationKind::RepoDeleteFile {
                (
                    HostActivityState::PossibleEffectUnknown,
                    Some(safe_delete_file_activity_result(
                        DeleteFileResultClassification::Uncertain,
                    )),
                )
            } else if kind == HostInvocationKind::RepoRenameFile {
                if host_repository_context_is_current(
                    state.inner(),
                    repository_identity.as_deref(),
                    generations,
                ) {
                    let _ = refresh_repository_workflow(state.inner()).await;
                    emit_repository_refresh(&app);
                }
                (
                    HostActivityState::PossibleEffectUnknown,
                    Some(safe_rename_file_activity_result(
                        RepositoryRenameFileProof::Uncertain,
                    )),
                )
            } else {
                (HostActivityState::PossibleEffectUnknown, None)
            }
        }
    };
    state
        .host_invocation
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .finish_host();
    emit_host_activity(
        &app,
        HostActivityEvent {
            source: "host_explicit",
            invocation_id,
            tool: tool_name,
            state: activity_state,
            result: output,
            review: None,
        },
    );
}

#[cfg(target_os = "windows")]
async fn classify_repository_delete_file_result(
    output: &ToolOutput,
    expected_path: &str,
    deletion_proof: Option<&(
        Box<rah_tools::RepositoryDeleteFilePreparation>,
        Arc<rah_tools::RepositoryDeleteFilePreparer>,
    )>,
) -> DeleteFileResultClassification {
    let structural = classify_repository_delete_file_output(output, expected_path);
    match structural {
        DeleteFileResultClassification::DeletedVerified => {
            let proven = match deletion_proof {
                Some((preparation, preparer)) => preparer.prove_deleted_verified(preparation).await,
                None => false,
            };
            if proven {
                structural
            } else {
                DeleteFileResultClassification::Uncertain
            }
        }
        DeleteFileResultClassification::KnownNoEffect => {
            let proven = match deletion_proof {
                Some((preparation, preparer)) => preparer.prove_known_no_effect(preparation).await,
                None => false,
            };
            if proven {
                structural
            } else {
                DeleteFileResultClassification::Uncertain
            }
        }
        _ => structural,
    }
}

#[cfg(target_os = "windows")]
fn repository_bound_authoring_kind(kind: HostInvocationKind) -> bool {
    matches!(
        kind,
        HostInvocationKind::RepoPatch
            | HostInvocationKind::RepoEditFiles
            | HostInvocationKind::RepoCreateFile
            | HostInvocationKind::RepoDeleteFile
            | HostInvocationKind::RepoRenameFile
    )
}

#[cfg(target_os = "windows")]
fn patch_host_terminal_state(
    classification: RepositoryPatchResultClassification,
) -> HostActivityState {
    match classification {
        RepositoryPatchResultClassification::ChangedVerified => HostActivityState::ToolCompleted,
        RepositoryPatchResultClassification::PreconditionFailed
        | RepositoryPatchResultClassification::ReplacementFailedKnown => {
            HostActivityState::ToolError
        }
        RepositoryPatchResultClassification::Uncertain
        | RepositoryPatchResultClassification::Malformed => {
            HostActivityState::PossibleEffectUnknown
        }
    }
}

#[cfg(target_os = "windows")]
fn host_repository_context_is_current(
    state: &DesktopAppState,
    captured: Option<&str>,
    captured_generations: [u64; 4],
) -> bool {
    let current = state
        .repository
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .as_ref()
        .map(|value| repository_context_fingerprint(&value.root));
    current.as_deref() == captured && current_host_generation_tuple(state) == captured_generations
}

#[cfg(target_os = "windows")]
#[tauri::command]
async fn host_invoke_read(
    request: HostReadRequest,
    app: AppHandle,
    state: State<'_, DesktopAppState>,
) -> Result<HostInvocationResponse, FrontendError> {
    let (kind, name, input) =
        read_request(request).map_err(|_| FrontendError::HostInvocationInvalidInput)?;
    let mut coordinator = state
        .host_invocation
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    coordinator.reap_expired(std::time::Instant::now());
    if coordinator.state() != CoordinatorState::Idle {
        return Err(FrontendError::HostInvocationBusy);
    }
    let current = current_host_composition(state.inner())?;
    let expected_definition = host_tool_definition(&current, &name, CoordinatorState::Idle)?;
    let call = host_call(name, input);
    authorize_tool_dispatch(
        &current.registry,
        &expected_definition,
        &current.allowed_permissions,
        &call,
    )
    .map_err(|rejection| match rejection {
        AuthorizedDispatchRejection::PermissionDenied { .. } => {
            FrontendError::HostInvocationPermissionDenied
        }
        AuthorizedDispatchRejection::UnknownTool { .. }
        | AuthorizedDispatchRejection::NameMismatch { .. }
        | AuthorizedDispatchRejection::DefinitionMismatch { .. } => {
            FrontendError::HostInvocationStale
        }
    })?;
    let invocation_id = coordinator
        .begin_read()
        .map_err(|_| FrontendError::HostInvocationBusy)?;
    drop(coordinator);
    emit_host_activity(
        &app,
        HostActivityEvent {
            source: "host_explicit",
            invocation_id: invocation_id.clone(),
            tool: expected_definition.name.to_string(),
            state: HostActivityState::Started,
            result: None,
            review: None,
        },
    );
    tauri::async_runtime::spawn(run_host_tool(
        app,
        current.registry,
        expected_definition,
        current.allowed_permissions,
        call,
        kind,
        invocation_id.clone(),
        current.repository_identity,
        current.generations,
        None,
        None,
        None,
        None,
    ));
    Ok(HostInvocationResponse { invocation_id })
}

#[cfg(target_os = "windows")]
#[tauri::command]
fn host_prepare_repo_create_branch(
    request: HostPrepareBranchRequest,
    app: AppHandle,
    state: State<'_, DesktopAppState>,
) -> Result<PreparedBranchResponse, FrontendError> {
    if !validate_bounded_string(&request.name, DESKTOP_HOST_BRANCH_NAME_MAX_BYTES) {
        return Err(FrontendError::HostInvocationInvalidInput);
    }
    let mut coordinator = state
        .host_invocation
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    coordinator.reap_expired(std::time::Instant::now());
    if coordinator.state() != CoordinatorState::Idle {
        return Err(FrontendError::HostInvocationBusy);
    }
    let current = current_host_composition(state.inner())?;
    let name = ToolName::new(REPOSITORY_CREATE_BRANCH_TOOL_NAME);
    let expected_definition = host_tool_definition(&current, &name, CoordinatorState::Idle)?;
    let call = host_call(
        name.clone(),
        ToolInput(serde_json::json!({"name": request.name.clone()})),
    );
    authorize_tool_dispatch(
        &current.registry,
        &expected_definition,
        &current.allowed_permissions,
        &call,
    )
    .map_err(|rejection| match rejection {
        AuthorizedDispatchRejection::PermissionDenied { .. } => {
            FrontendError::HostInvocationPermissionDenied
        }
        _ => FrontendError::HostInvocationStale,
    })?;
    if current.repository.is_none()
        || current
            .repository
            .as_ref()
            .is_none_or(|value| value.branch_creation_authority.is_none())
    {
        return Err(FrontendError::HostInvocationNotEligible);
    }
    let ticket_id = coordinator.next_ticket_id();
    let activity_id = coordinator.next_invocation_id();
    let review = BranchReview {
        operation: "Create local branch",
        branch: request.name,
        target: "Current committed HEAD",
        effect: "Creates one new local branch reference.",
        non_effect: "Does not switch branches or modify HEAD/index/worktree.",
        permission_category: "execute",
        authority_category: "repository_local_branch_creation",
    };
    let ticket = PreparedHostInvocation::new(
        ticket_id.clone(),
        activity_id.clone(),
        HostInvocationKind::RepoCreateBranch,
        name,
        expected_definition,
        call,
        current.registry,
        current.allowed_permissions,
        current.generations,
        current.repository_identity,
        current.composition_identity,
        PreparedHostPayload::Branch {
            review: review.clone(),
        },
    );
    coordinator
        .prepare(ticket)
        .map_err(|_| FrontendError::HostInvocationBusy)?;
    emit_host_activity(
        &app,
        prepared_host_activity(
            activity_id,
            REPOSITORY_CREATE_BRANCH_TOOL_NAME.to_owned(),
            Some(HostInvocationReview::Branch(review.clone())),
        ),
    );
    Ok(PreparedBranchResponse { ticket_id, review })
}

#[cfg(target_os = "windows")]
fn patch_preparation_frontend_error(error: RepositoryPatchPreparationError) -> FrontendError {
    match error {
        RepositoryPatchPreparationError::Stale => FrontendError::HostInvocationStale,
        RepositoryPatchPreparationError::InvalidInput { .. }
        | RepositoryPatchPreparationError::Unsupported { .. }
        | RepositoryPatchPreparationError::PreconditionFailed { .. }
        | RepositoryPatchPreparationError::NoEffect
        | RepositoryPatchPreparationError::ReviewTooLarge => {
            FrontendError::HostInvocationInvalidInput
        }
    }
}

#[cfg(target_os = "windows")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MultiFileResultClassification {
    Ok,
    InvalidTarget,
    PreconditionFailed,
    FailedKnownNoEffect,
    PartialEffect,
    Uncertain,
    Malformed,
}

#[cfg(target_os = "windows")]
fn classify_repository_multi_file_output(
    output: &ToolOutput,
    expected_paths: &[String],
) -> MultiFileResultClassification {
    let [ToolContent::Json(value)] = output.content.as_slice() else {
        return MultiFileResultClassification::Malformed;
    };
    let Some(object) = value.as_object() else {
        return MultiFileResultClassification::Malformed;
    };
    let Some(status) = object.get("status").and_then(serde_json::Value::as_str) else {
        return MultiFileResultClassification::Malformed;
    };
    let exact_keys = |keys: &[&str]| {
        object.len() == keys.len() && keys.iter().all(|key| object.contains_key(*key))
    };
    let Some(effects) = object.get("effects").and_then(serde_json::Value::as_array) else {
        if output.is_error
            && matches!(status, "invalid_target" | "precondition_failed")
            && exact_keys(&["status"])
        {
            return if status == "invalid_target" {
                MultiFileResultClassification::InvalidTarget
            } else {
                MultiFileResultClassification::PreconditionFailed
            };
        }
        return MultiFileResultClassification::Malformed;
    };
    if !exact_keys(&["status", "effects"]) || effects.len() != expected_paths.len() {
        return MultiFileResultClassification::Malformed;
    }
    let mut states = Vec::with_capacity(effects.len());
    for (effect, expected_path) in effects.iter().zip(expected_paths) {
        let Some(effect) = effect.as_object() else {
            return MultiFileResultClassification::Malformed;
        };
        if effect.len() != 2
            || effect.get("path").and_then(serde_json::Value::as_str) != Some(expected_path)
        {
            return MultiFileResultClassification::Malformed;
        }
        let Some(state) = effect.get("state").and_then(serde_json::Value::as_str) else {
            return MultiFileResultClassification::Malformed;
        };
        states.push(state);
    }
    let prefix_len = states
        .iter()
        .take_while(|state| **state == "committed_verified")
        .count();
    let valid = match status {
        "ok" => !output.is_error && states.iter().all(|state| *state == "committed_verified"),
        "failed_known_no_effect" => {
            output.is_error
                && prefix_len == 0
                && states.first() == Some(&"unchanged_verified")
                && states[1..].iter().all(|state| *state == "not_attempted")
        }
        "partial_effect" => {
            output.is_error
                && prefix_len > 0
                && prefix_len < states.len()
                && states[prefix_len..]
                    .iter()
                    .all(|state| matches!(*state, "unchanged_verified" | "not_attempted"))
        }
        "uncertain" => {
            output.is_error
                && prefix_len < states.len()
                && states[prefix_len] == "uncertain"
                && states[prefix_len + 1..]
                    .iter()
                    .all(|state| *state == "not_attempted")
        }
        _ => false,
    };
    if !valid {
        return MultiFileResultClassification::Malformed;
    }
    match status {
        "ok" => MultiFileResultClassification::Ok,
        "failed_known_no_effect" => MultiFileResultClassification::FailedKnownNoEffect,
        "partial_effect" => MultiFileResultClassification::PartialEffect,
        "uncertain" => MultiFileResultClassification::Uncertain,
        _ => MultiFileResultClassification::Malformed,
    }
}

#[cfg(target_os = "windows")]
fn multi_file_host_terminal_state(
    classification: MultiFileResultClassification,
) -> HostActivityState {
    match classification {
        MultiFileResultClassification::Ok => HostActivityState::ToolCompleted,
        MultiFileResultClassification::InvalidTarget
        | MultiFileResultClassification::PreconditionFailed
        | MultiFileResultClassification::FailedKnownNoEffect => HostActivityState::ToolError,
        MultiFileResultClassification::PartialEffect => HostActivityState::PartialEffect,
        MultiFileResultClassification::Uncertain | MultiFileResultClassification::Malformed => {
            HostActivityState::PossibleEffectUnknown
        }
    }
}

#[cfg(target_os = "windows")]
fn safe_multi_file_activity_result(
    output: ToolOutput,
    classification: MultiFileResultClassification,
) -> Option<ToolOutput> {
    (!matches!(classification, MultiFileResultClassification::Malformed)).then_some(output)
}

#[cfg(target_os = "windows")]
fn multi_file_edit_preparation_frontend_error(
    error: RepositoryMultiFileEditPreparationError,
) -> FrontendError {
    match error {
        RepositoryMultiFileEditPreparationError::InvalidInput { .. } => {
            FrontendError::HostInvocationInvalidInput
        }
        RepositoryMultiFileEditPreparationError::InvalidTarget { .. } => {
            FrontendError::HostInvocationInvalidTarget
        }
        RepositoryMultiFileEditPreparationError::PreconditionChanged { .. } => {
            FrontendError::HostInvocationPreconditionChanged
        }
        RepositoryMultiFileEditPreparationError::Stale => FrontendError::HostInvocationStale,
        RepositoryMultiFileEditPreparationError::ReviewTooLarge => {
            FrontendError::HostInvocationReviewTooLarge
        }
    }
}

#[cfg(target_os = "windows")]
fn create_file_preparation_frontend_error(
    error: RepositoryCreateFilePreparationError,
) -> FrontendError {
    match error {
        RepositoryCreateFilePreparationError::InvalidInput { .. } => {
            FrontendError::HostInvocationInvalidInput
        }
        RepositoryCreateFilePreparationError::PreconditionFailed { .. } => {
            FrontendError::HostInvocationPreconditionChanged
        }
        RepositoryCreateFilePreparationError::ReviewTooLarge => {
            FrontendError::HostInvocationReviewTooLarge
        }
        RepositoryCreateFilePreparationError::Stale => FrontendError::HostInvocationStale,
    }
}

#[cfg(target_os = "windows")]
fn delete_file_preparation_frontend_error(
    error: RepositoryDeleteFilePreparationError,
) -> FrontendError {
    match error {
        RepositoryDeleteFilePreparationError::InvalidInput { .. } => {
            FrontendError::HostInvocationInvalidInput
        }
        RepositoryDeleteFilePreparationError::PreconditionFailed { .. } => {
            FrontendError::HostInvocationPreconditionChanged
        }
        RepositoryDeleteFilePreparationError::ReviewTooLarge => {
            FrontendError::HostInvocationReviewTooLarge
        }
        RepositoryDeleteFilePreparationError::Stale => FrontendError::HostInvocationStale,
    }
}

#[cfg(target_os = "windows")]
fn rename_file_preparation_frontend_error(
    error: RepositoryRenameFilePreparationError,
) -> FrontendError {
    match error {
        RepositoryRenameFilePreparationError::InvalidInput { .. } => {
            FrontendError::HostInvocationInvalidInput
        }
        RepositoryRenameFilePreparationError::Unsupported { .. }
        | RepositoryRenameFilePreparationError::PreconditionFailed { .. } => {
            FrontendError::HostInvocationPreconditionChanged
        }
        RepositoryRenameFilePreparationError::ReviewTooLarge => {
            FrontendError::HostInvocationReviewTooLarge
        }
        RepositoryRenameFilePreparationError::Stale => FrontendError::HostInvocationStale,
    }
}

#[cfg(target_os = "windows")]
fn abort_host_patch_prepare(state: &DesktopAppState) {
    state
        .host_invocation
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .abort_prepare();
}

#[cfg(target_os = "windows")]
#[tauri::command]
async fn host_prepare_repo_patch(
    request: HostPreparePatchRequest,
    app: AppHandle,
    state: State<'_, DesktopAppState>,
) -> Result<PreparedPatchResponse, FrontendError> {
    {
        let mut coordinator = state
            .host_invocation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        coordinator.reap_expired(std::time::Instant::now());
        coordinator
            .begin_prepare()
            .map_err(|_| FrontendError::HostInvocationBusy)?;
    }

    let current = match current_host_composition(state.inner()) {
        Ok(current) => current,
        Err(error) => {
            abort_host_patch_prepare(state.inner());
            return Err(error);
        }
    };
    let name = ToolName::new("repo.patch");
    let expected_definition = match host_tool_definition(&current, &name, CoordinatorState::Idle) {
        Ok(definition) => definition,
        Err(error) => {
            abort_host_patch_prepare(state.inner());
            return Err(error);
        }
    };
    let preparer = match current.repository_patch_preparer.clone() {
        Some(preparer) => preparer,
        None => {
            abort_host_patch_prepare(state.inner());
            return Err(FrontendError::HostInvocationNotEligible);
        }
    };

    let preflight_call = host_call(name.clone(), ToolInput(serde_json::json!({})));
    if let Err(rejection) = authorize_tool_dispatch(
        &current.registry,
        &expected_definition,
        &current.allowed_permissions,
        &preflight_call,
    ) {
        abort_host_patch_prepare(state.inner());
        return Err(match rejection {
            AuthorizedDispatchRejection::PermissionDenied { .. } => {
                FrontendError::HostInvocationPermissionDenied
            }
            _ => FrontendError::HostInvocationStale,
        });
    }

    let preparation = match preparer
        .prepare(RepositoryPatchPreparationRequest {
            path: request.path,
            expected_old_text: request.expected_old_text,
            replacement_text: request.replacement_text,
        })
        .await
    {
        Ok(preparation) => preparation,
        Err(error) => {
            abort_host_patch_prepare(state.inner());
            return Err(patch_preparation_frontend_error(error));
        }
    };
    let review = preparation.review().clone();
    let call = host_call(name.clone(), preparation.tool_input().clone());
    let (ticket_id, activity_id, ticket) = {
        let mut coordinator = state
            .host_invocation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let ticket_id = coordinator.next_ticket_id();
        let activity_id = coordinator.next_invocation_id();
        let ticket = PreparedHostInvocation::new(
            ticket_id.clone(),
            activity_id.clone(),
            HostInvocationKind::RepoPatch,
            name,
            expected_definition,
            call,
            current.registry,
            current.allowed_permissions,
            current.generations,
            current.repository_identity,
            current.composition_identity,
            PreparedHostPayload::Patch {
                preparation: Box::new(preparation),
                preparer,
            },
        );
        if coordinator.finalize_prepare(ticket).is_err() {
            coordinator.abort_prepare();
            return Err(FrontendError::HostInvocationBusy);
        }
        (ticket_id, activity_id, review)
    };
    emit_host_activity(
        &app,
        prepared_host_activity(activity_id, "repo.patch".to_owned(), None),
    );
    Ok(PreparedPatchResponse {
        ticket_id,
        review: ticket,
    })
}

#[cfg(target_os = "windows")]
#[tauri::command]
async fn host_prepare_repo_edit_files(
    request: HostPrepareMultiFileEditRequest,
    app: AppHandle,
    state: State<'_, DesktopAppState>,
) -> Result<PreparedMultiFileEditResponse, FrontendError> {
    {
        let mut coordinator = state
            .host_invocation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        coordinator.reap_expired(std::time::Instant::now());
        coordinator
            .begin_prepare()
            .map_err(|_| FrontendError::HostInvocationBusy)?;
    }

    let current = match current_host_composition(state.inner()) {
        Ok(current) => current,
        Err(error) => {
            abort_host_patch_prepare(state.inner());
            return Err(error);
        }
    };
    let name = ToolName::new("repo.edit-files");
    let expected_definition = match host_tool_definition(&current, &name, CoordinatorState::Idle) {
        Ok(definition) => definition,
        Err(error) => {
            abort_host_patch_prepare(state.inner());
            return Err(error);
        }
    };
    let preparer = match current.repository_multi_file_edit_preparer.clone() {
        Some(preparer) => preparer,
        None => {
            abort_host_patch_prepare(state.inner());
            return Err(FrontendError::HostInvocationNotEligible);
        }
    };
    let preflight_call = host_call(name.clone(), ToolInput(serde_json::json!({"targets": []})));
    if let Err(rejection) = authorize_tool_dispatch(
        &current.registry,
        &expected_definition,
        &current.allowed_permissions,
        &preflight_call,
    ) {
        abort_host_patch_prepare(state.inner());
        return Err(match rejection {
            AuthorizedDispatchRejection::PermissionDenied { .. } => {
                FrontendError::HostInvocationPermissionDenied
            }
            _ => FrontendError::HostInvocationStale,
        });
    }
    let request = RepositoryMultiFileEditPreparationRequest {
        targets: request
            .targets
            .into_iter()
            .map(|target| RepositoryMultiFileEditPreparationTarget {
                path: target.path,
                replacements: target
                    .replacements
                    .into_iter()
                    .map(|replacement| RepositoryMultiFileEditTextReplacement {
                        expected_old_text: replacement.expected_old_text,
                        replacement_text: replacement.replacement_text,
                    })
                    .collect(),
            })
            .collect(),
    };
    let preparation = match preparer.prepare(request).await {
        Ok(preparation) => preparation,
        Err(error) => {
            abort_host_patch_prepare(state.inner());
            return Err(multi_file_edit_preparation_frontend_error(error));
        }
    };
    let review = preparation.review().clone();
    let call = host_call(name.clone(), preparation.tool_input().clone());
    let (ticket_id, activity_id) = {
        let mut coordinator = state
            .host_invocation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let ticket_id = coordinator.next_ticket_id();
        let activity_id = coordinator.next_invocation_id();
        let ticket = PreparedHostInvocation::new(
            ticket_id.clone(),
            activity_id.clone(),
            HostInvocationKind::RepoEditFiles,
            name,
            expected_definition,
            call,
            current.registry,
            current.allowed_permissions,
            current.generations,
            current.repository_identity,
            current.composition_identity,
            PreparedHostPayload::MultiFileEdit {
                preparation: Box::new(preparation),
                preparer,
            },
        );
        if coordinator.finalize_prepare(ticket).is_err() {
            coordinator.abort_prepare();
            return Err(FrontendError::HostInvocationBusy);
        }
        (ticket_id, activity_id)
    };
    emit_host_activity(
        &app,
        prepared_host_activity(activity_id, "repo.edit-files".to_owned(), None),
    );
    Ok(PreparedMultiFileEditResponse { ticket_id, review })
}

#[cfg(target_os = "windows")]
#[tauri::command]
async fn host_prepare_repo_create_file(
    request: HostPrepareCreateFileRequest,
    app: AppHandle,
    state: State<'_, DesktopAppState>,
) -> Result<PreparedCreateFileResponse, FrontendError> {
    {
        let mut coordinator = state
            .host_invocation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        coordinator.reap_expired(std::time::Instant::now());
        coordinator
            .begin_prepare()
            .map_err(|_| FrontendError::HostInvocationBusy)?;
    }

    let current = match current_host_composition(state.inner()) {
        Ok(current) => current,
        Err(error) => {
            abort_host_patch_prepare(state.inner());
            return Err(error);
        }
    };
    let name = ToolName::new("repo.create-file");
    let expected_definition = match host_tool_definition(&current, &name, CoordinatorState::Idle) {
        Ok(definition) => definition,
        Err(error) => {
            abort_host_patch_prepare(state.inner());
            return Err(error);
        }
    };
    let preparer = match current.repository_create_file_preparer.clone() {
        Some(preparer) => preparer,
        None => {
            abort_host_patch_prepare(state.inner());
            return Err(FrontendError::HostInvocationNotEligible);
        }
    };
    let preflight_call = host_call(name.clone(), ToolInput(serde_json::json!({})));
    if let Err(rejection) = authorize_tool_dispatch(
        &current.registry,
        &expected_definition,
        &current.allowed_permissions,
        &preflight_call,
    ) {
        abort_host_patch_prepare(state.inner());
        return Err(match rejection {
            AuthorizedDispatchRejection::PermissionDenied { .. } => {
                FrontendError::HostInvocationPermissionDenied
            }
            _ => FrontendError::HostInvocationStale,
        });
    }
    let preparation = match preparer
        .prepare(RepositoryCreateFilePreparationRequest {
            path: request.path,
            content: request.content,
        })
        .await
    {
        Ok(preparation) => preparation,
        Err(error) => {
            abort_host_patch_prepare(state.inner());
            return Err(create_file_preparation_frontend_error(error));
        }
    };
    let review = preparation.review().clone();
    let call = host_call(name.clone(), preparation.tool_input().clone());
    let (ticket_id, activity_id) = {
        let mut coordinator = state
            .host_invocation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let ticket_id = coordinator.next_ticket_id();
        let activity_id = coordinator.next_invocation_id();
        let ticket = PreparedHostInvocation::new(
            ticket_id.clone(),
            activity_id.clone(),
            HostInvocationKind::RepoCreateFile,
            name,
            expected_definition,
            call,
            current.registry,
            current.allowed_permissions,
            current.generations,
            current.repository_identity,
            current.composition_identity,
            PreparedHostPayload::CreateFile {
                preparation: Box::new(preparation),
                preparer,
            },
        );
        if coordinator.finalize_prepare(ticket).is_err() {
            coordinator.abort_prepare();
            return Err(FrontendError::HostInvocationBusy);
        }
        (ticket_id, activity_id)
    };
    emit_host_activity(
        &app,
        prepared_host_activity(activity_id, "repo.create-file".to_owned(), None),
    );
    Ok(PreparedCreateFileResponse { ticket_id, review })
}

#[cfg(target_os = "windows")]
async fn prepare_repo_delete_file_with_current(
    request: HostPrepareDeleteFileRequest,
    app: &AppHandle,
    state: &DesktopAppState,
    current: CurrentHostComposition,
) -> Result<PreparedDeleteFileResponse, FrontendError> {
    let name = ToolName::new("repo.delete-file");
    let expected_definition = match host_tool_definition(&current, &name, CoordinatorState::Idle) {
        Ok(definition) => definition,
        Err(error) => {
            abort_host_patch_prepare(state);
            return Err(error);
        }
    };
    let preparer = match current.repository_delete_file_preparer.clone() {
        Some(preparer) => preparer,
        None => {
            abort_host_patch_prepare(state);
            return Err(FrontendError::HostInvocationNotEligible);
        }
    };
    let preflight_call = host_call(name.clone(), ToolInput(serde_json::json!({})));
    if let Err(rejection) = authorize_tool_dispatch(
        &current.registry,
        &expected_definition,
        &current.allowed_permissions,
        &preflight_call,
    ) {
        abort_host_patch_prepare(state);
        return Err(match rejection {
            AuthorizedDispatchRejection::PermissionDenied { .. } => {
                FrontendError::HostInvocationPermissionDenied
            }
            _ => FrontendError::HostInvocationStale,
        });
    }
    let preparation = match preparer
        .prepare(RepositoryDeleteFilePreparationRequest { path: request.path })
        .await
    {
        Ok(preparation) => preparation,
        Err(error) => {
            abort_host_patch_prepare(state);
            return Err(delete_file_preparation_frontend_error(error));
        }
    };
    let review = preparation.review().clone();
    let call = host_call(name.clone(), preparation.tool_input().clone());
    let (ticket_id, activity_id) = {
        let mut coordinator = state
            .host_invocation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let ticket_id = coordinator.next_ticket_id();
        let activity_id = coordinator.next_invocation_id();
        let ticket = PreparedHostInvocation::new(
            ticket_id.clone(),
            activity_id.clone(),
            HostInvocationKind::RepoDeleteFile,
            name,
            expected_definition,
            call,
            current.registry,
            current.allowed_permissions,
            current.generations,
            current.repository_identity,
            current.composition_identity,
            PreparedHostPayload::DeleteFile {
                preparation: Box::new(preparation),
                preparer,
            },
        );
        if coordinator.finalize_prepare(ticket).is_err() {
            coordinator.abort_prepare();
            return Err(FrontendError::HostInvocationBusy);
        }
        (ticket_id, activity_id)
    };
    emit_host_activity(
        app,
        prepared_host_activity(activity_id, "repo.delete-file".to_owned(), None),
    );
    Ok(PreparedDeleteFileResponse { ticket_id, review })
}

#[cfg(target_os = "windows")]
#[tauri::command]
async fn host_prepare_repo_delete_file(
    request: HostPrepareDeleteFileRequest,
    app: AppHandle,
    state: State<'_, DesktopAppState>,
) -> Result<PreparedDeleteFileResponse, FrontendError> {
    {
        let mut coordinator = state
            .host_invocation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        coordinator.reap_expired(std::time::Instant::now());
        coordinator
            .begin_prepare()
            .map_err(|_| FrontendError::HostInvocationBusy)?;
    }

    let current = match current_host_composition(state.inner()) {
        Ok(current) => current,
        Err(error) => {
            abort_host_patch_prepare(state.inner());
            return Err(error);
        }
    };
    prepare_repo_delete_file_with_current(request, &app, state.inner(), current).await
}

#[cfg(target_os = "windows")]
async fn prepare_repo_rename_file_with_current(
    request: HostPrepareRenameFileRequest,
    app: &AppHandle,
    state: &DesktopAppState,
    current: CurrentHostComposition,
) -> Result<PreparedRenameFileResponse, FrontendError> {
    let name = ToolName::new("repo.rename-file");
    let expected_definition = match host_tool_definition(&current, &name, CoordinatorState::Idle) {
        Ok(definition) => definition,
        Err(error) => {
            abort_host_patch_prepare(state);
            return Err(error);
        }
    };
    let preparer = match current.repository_rename_file_preparer.clone() {
        Some(preparer) => preparer,
        None => {
            abort_host_patch_prepare(state);
            return Err(FrontendError::HostInvocationNotEligible);
        }
    };
    let preflight_call = host_call(name.clone(), ToolInput(serde_json::json!({})));
    if let Err(rejection) = authorize_tool_dispatch(
        &current.registry,
        &expected_definition,
        &current.allowed_permissions,
        &preflight_call,
    ) {
        abort_host_patch_prepare(state);
        return Err(match rejection {
            AuthorizedDispatchRejection::PermissionDenied { .. } => {
                FrontendError::HostInvocationPermissionDenied
            }
            _ => FrontendError::HostInvocationStale,
        });
    }
    let preparation = match preparer
        .prepare(RepositoryRenameFilePreparationRequest {
            source_path: request.source_path,
            destination_path: request.destination_path,
        })
        .await
    {
        Ok(preparation) => preparation,
        Err(error) => {
            abort_host_patch_prepare(state);
            return Err(rename_file_preparation_frontend_error(error));
        }
    };
    let review = preparation.review().clone();
    let call = host_call(name.clone(), preparation.tool_input().clone());
    let (ticket_id, activity_id) = {
        let mut coordinator = state
            .host_invocation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let ticket_id = coordinator.next_ticket_id();
        let activity_id = coordinator.next_invocation_id();
        let ticket = PreparedHostInvocation::new(
            ticket_id.clone(),
            activity_id.clone(),
            HostInvocationKind::RepoRenameFile,
            name,
            expected_definition,
            call,
            current.registry,
            current.allowed_permissions,
            current.generations,
            current.repository_identity,
            current.composition_identity,
            PreparedHostPayload::RenameFile {
                preparation: Box::new(preparation),
                preparer,
            },
        );
        if coordinator.finalize_prepare(ticket).is_err() {
            coordinator.abort_prepare();
            return Err(FrontendError::HostInvocationBusy);
        }
        (ticket_id, activity_id)
    };
    emit_host_activity(
        app,
        prepared_host_activity(activity_id, "repo.rename-file".to_owned(), None),
    );
    Ok(PreparedRenameFileResponse { ticket_id, review })
}

#[cfg(target_os = "windows")]
#[tauri::command]
async fn host_prepare_repo_rename_file(
    request: HostPrepareRenameFileRequest,
    app: AppHandle,
    state: State<'_, DesktopAppState>,
) -> Result<PreparedRenameFileResponse, FrontendError> {
    {
        let mut coordinator = state
            .host_invocation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        coordinator.reap_expired(std::time::Instant::now());
        coordinator
            .begin_prepare()
            .map_err(|_| FrontendError::HostInvocationBusy)?;
    }
    let current = match current_host_composition(state.inner()) {
        Ok(current) => current,
        Err(error) => {
            abort_host_patch_prepare(state.inner());
            return Err(error);
        }
    };
    prepare_repo_rename_file_with_current(request, &app, state.inner(), current).await
}

#[cfg(target_os = "windows")]
async fn validate_host_confirmation_ticket(
    ticket: &PreparedHostInvocation,
    current: &CurrentHostComposition,
) -> Result<(), FrontendError> {
    if ticket.generations != current.generations
        || ticket.composition_identity != current.composition_identity
        || ticket.repository_identity != current.repository_identity
        || !Arc::ptr_eq(&ticket.registry, &current.registry)
    {
        return Err(FrontendError::HostInvocationStale);
    }
    let current_definition =
        host_tool_definition(current, &ticket.tool_name, CoordinatorState::Idle)?;
    if current_definition != ticket.expected_definition
        || current.allowed_permissions != ticket.allowed_permissions
    {
        return Err(FrontendError::HostInvocationStale);
    }
    match &ticket.payload {
        PreparedHostPayload::Patch { preparer, .. }
            if current
                .repository_patch_preparer
                .as_ref()
                .is_none_or(|current_preparer| !Arc::ptr_eq(current_preparer, preparer)) =>
        {
            return Err(FrontendError::HostInvocationStale);
        }
        PreparedHostPayload::MultiFileEdit { preparer, .. }
            if current
                .repository_multi_file_edit_preparer
                .as_ref()
                .is_none_or(|current_preparer| !Arc::ptr_eq(current_preparer, preparer)) =>
        {
            return Err(FrontendError::HostInvocationStale);
        }
        PreparedHostPayload::CreateFile { preparer, .. }
            if current
                .repository_create_file_preparer
                .as_ref()
                .is_none_or(|current_preparer| !Arc::ptr_eq(current_preparer, preparer)) =>
        {
            return Err(FrontendError::HostInvocationStale);
        }
        PreparedHostPayload::DeleteFile { preparer, .. }
            if current
                .repository_delete_file_preparer
                .as_ref()
                .is_none_or(|current_preparer| !Arc::ptr_eq(current_preparer, preparer)) =>
        {
            return Err(FrontendError::HostInvocationStale);
        }
        PreparedHostPayload::RenameFile { preparer, .. }
            if current
                .repository_rename_file_preparer
                .as_ref()
                .is_none_or(|current_preparer| !Arc::ptr_eq(current_preparer, preparer)) =>
        {
            return Err(FrontendError::HostInvocationStale);
        }
        _ => {}
    }
    match &ticket.payload {
        PreparedHostPayload::Patch {
            preparation,
            preparer,
        } => preparer
            .revalidate(preparation)
            .await
            .map_err(patch_preparation_frontend_error),
        PreparedHostPayload::MultiFileEdit {
            preparation,
            preparer,
        } => preparer
            .revalidate(preparation)
            .await
            .map_err(multi_file_edit_preparation_frontend_error),
        PreparedHostPayload::CreateFile {
            preparation,
            preparer,
        } => preparer
            .revalidate(preparation)
            .await
            .map_err(create_file_preparation_frontend_error),
        PreparedHostPayload::DeleteFile {
            preparation,
            preparer,
        } => preparer
            .revalidate(preparation)
            .await
            .map_err(delete_file_preparation_frontend_error),
        PreparedHostPayload::RenameFile {
            preparation,
            preparer,
        } => preparer
            .revalidate(preparation)
            .await
            .map_err(rename_file_preparation_frontend_error),
        PreparedHostPayload::Branch { .. } => Ok(()),
    }?;
    authorize_tool_dispatch(
        &current.registry,
        &ticket.expected_definition,
        &current.allowed_permissions,
        &ticket.call,
    )
    .map_err(|rejection| match rejection {
        AuthorizedDispatchRejection::PermissionDenied { .. } => {
            FrontendError::HostInvocationPermissionDenied
        }
        _ => FrontendError::HostInvocationStale,
    })
}

#[cfg(target_os = "windows")]
#[tauri::command]
async fn host_confirm_tool_invocation(
    request: HostConfirmRequest,
    app: AppHandle,
    state: State<'_, DesktopAppState>,
) -> Result<HostInvocationResponse, FrontendError> {
    let ticket = {
        let mut coordinator = state
            .host_invocation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        coordinator.reap_expired(std::time::Instant::now());
        coordinator
            .take_prepared(&request.ticket_id, std::time::Instant::now())
            .map_err(|_| FrontendError::HostInvocationTicketInvalid)?
    };
    let current = match current_host_composition(state.inner()) {
        Ok(current) => current,
        Err(error) => {
            state
                .host_invocation
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .finish_host();
            return Err(error);
        }
    };
    if let Err(error) = validate_host_confirmation_ticket(&ticket, &current).await {
        state
            .host_invocation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .finish_host();
        return Err(error);
    }
    if repository_bound_authoring_kind(ticket.kind) {
        invalidate_repository_commit_review(state.inner()).await;
    }
    let invocation_id = ticket.activity_id.clone();
    let multi_file_target_order = match &ticket.payload {
        PreparedHostPayload::MultiFileEdit { preparation, .. } => Some(
            preparation
                .review()
                .targets()
                .iter()
                .map(|target| target.path().to_owned())
                .collect(),
        ),
        _ => None,
    };
    let create_file_expected_output = match &ticket.payload {
        PreparedHostPayload::CreateFile { preparation, .. } => Some(CreateFileExpectedOutput {
            path: preparation.review().path().to_owned(),
            length: preparation.content_byte_length(),
            sha256: preparation.content_sha256().to_owned(),
        }),
        _ => None,
    };
    let review = match &ticket.payload {
        PreparedHostPayload::Branch { review } => {
            Some(HostInvocationReview::Branch(review.clone()))
        }
        PreparedHostPayload::Patch { .. }
        | PreparedHostPayload::MultiFileEdit { .. }
        | PreparedHostPayload::CreateFile { .. }
        | PreparedHostPayload::DeleteFile { .. }
        | PreparedHostPayload::RenameFile { .. } => None,
    };
    let (deletion_proof, rename_proof) = match ticket.payload {
        PreparedHostPayload::DeleteFile {
            preparation,
            preparer,
        } => (Some((preparation, preparer)), None),
        PreparedHostPayload::RenameFile {
            preparation,
            preparer,
        } => (None, Some((preparation, preparer))),
        _ => (None, None),
    };
    emit_host_activity(
        &app,
        HostActivityEvent {
            source: "host_explicit",
            invocation_id: invocation_id.clone(),
            tool: ticket.tool_name.to_string(),
            state: HostActivityState::Started,
            result: None,
            review,
        },
    );
    tauri::async_runtime::spawn(run_host_tool(
        app,
        ticket.registry,
        ticket.expected_definition,
        ticket.allowed_permissions,
        ticket.call,
        ticket.kind,
        invocation_id.clone(),
        ticket.repository_identity,
        ticket.generations,
        multi_file_target_order,
        create_file_expected_output,
        deletion_proof,
        rename_proof,
    ));
    Ok(HostInvocationResponse { invocation_id })
}

#[cfg(target_os = "windows")]
#[tauri::command]
fn host_cancel_tool_invocation(
    request: HostConfirmRequest,
    app: AppHandle,
    state: State<'_, DesktopAppState>,
) -> Result<(), FrontendError> {
    let mut coordinator = state
        .host_invocation
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    coordinator.reap_expired(std::time::Instant::now());
    let (kind, activity_id) = coordinator
        .cancel(&request.ticket_id)
        .map_err(|_| FrontendError::HostInvocationTicketInvalid)?;
    emit_host_activity(
        &app,
        HostActivityEvent {
            source: "host_explicit",
            invocation_id: activity_id,
            tool: match kind {
                HostInvocationKind::RepoPatch => "repo.patch".to_owned(),
                HostInvocationKind::RepoEditFiles => "repo.edit-files".to_owned(),
                HostInvocationKind::RepoCreateFile => "repo.create-file".to_owned(),
                HostInvocationKind::RepoDeleteFile => "repo.delete-file".to_owned(),
                HostInvocationKind::RepoRenameFile => "repo.rename-file".to_owned(),
                _ => REPOSITORY_CREATE_BRANCH_TOOL_NAME.to_owned(),
            },
            state: HostActivityState::CancelledBeforeStart,
            result: None,
            review: None,
        },
    );
    Ok(())
}

#[cfg(target_os = "windows")]
#[tauri::command]
fn app_status(state: State<'_, DesktopAppState>) -> AppStatus {
    state.status()
}

#[cfg(target_os = "windows")]
fn trusted_profile_selection_allowed(
    chat: ChatState,
    connection: &ConnectionState,
) -> Result<(), FrontendError> {
    if chat != ChatState::Idle
        || !matches!(
            connection,
            ConnectionState::NotConnected | ConnectionState::Error(_)
        )
    {
        Err(FrontendError::ProfileBusy)
    } else {
        Ok(())
    }
}

#[cfg(target_os = "windows")]
fn trusted_profile_forget_allowed(
    chat: ChatState,
    connection: &ConnectionState,
) -> Result<(), FrontendError> {
    if chat != ChatState::Idle
        || !matches!(
            connection,
            ConnectionState::NotConnected
                | ConnectionState::Error(_)
                | ConnectionState::Connected { .. }
        )
    {
        Err(FrontendError::ProfileBusy)
    } else {
        Ok(())
    }
}

#[cfg(target_os = "windows")]
fn ensure_trusted_profile_selection_allowed(state: &DesktopAppState) -> Result<(), FrontendError> {
    let chat = *state
        .chat
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let connection = state
        .connection
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    trusted_profile_selection_allowed(chat, &connection)
}

#[cfg(target_os = "windows")]
fn ensure_trusted_profile_forget_allowed(state: &DesktopAppState) -> Result<(), FrontendError> {
    let chat = *state
        .chat
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let connection = state
        .connection
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    trusted_profile_forget_allowed(chat, &connection)
}

#[cfg(target_os = "windows")]
fn profile_selection_error(error: ProfileSelectionError) -> FrontendError {
    match error {
        ProfileSelectionError::InvalidProfile => FrontendError::ProfileInvalid,
        ProfileSelectionError::FirstPartyCapabilities => {
            FrontendError::ProfileFirstPartyCapabilitiesUnsupported
        }
    }
}

#[cfg(target_os = "windows")]
fn publish_trusted_profile_selection(
    state: &DesktopAppState,
    selection: DesktopTrustedProfileSelection,
) {
    *state
        .trusted_profile
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(selection);
    let mut generation = state
        .trusted_profile_generation
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    *generation = generation.wrapping_add(1);
}

#[cfg(target_os = "windows")]
fn clear_trusted_profile_selection(state: &DesktopAppState) -> bool {
    let changed = state
        .trusted_profile
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .take()
        .is_some();
    if changed {
        let mut generation = state
            .trusted_profile_generation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *generation = generation.wrapping_add(1);
    }
    changed
}

#[cfg(target_os = "windows")]
fn save_trusted_profile_preference(
    state: &DesktopAppState,
    path: RememberedTrustedProfilePath,
) -> Result<(), PreferencesWarning> {
    let _ordering = state
        .preference_ordering
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let model_selection = state
        .model
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .selection
        .clone();
    state
        .preferences
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .save_trusted_profile_path(&model_selection, path)
}

#[cfg(target_os = "windows")]
fn restore_trusted_profile_selection(state: &DesktopAppState) -> Result<(), FrontendError> {
    ensure_trusted_profile_selection_allowed(state)?;
    let remembered_path = state
        .preferences
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .remembered_trusted_profile_path()
        .ok_or(FrontendError::NoRememberedTrustedProfile)?;
    let selection = load_provider_only_profile(remembered_path.path().to_path_buf()).map_err(|error| {
        tracing::warn!(reason = ?error, "Desktop remembered Trusted Profile static validation failed");
        profile_selection_error(error)
    })?;
    // Restore rereads the current source but never starts a provider or rewrites
    // the remembered preference.
    ensure_trusted_profile_selection_allowed(state)?;
    publish_trusted_profile_selection(state, selection);
    Ok(())
}

#[cfg(target_os = "windows")]
fn forget_trusted_profile_preference(state: &DesktopAppState) -> Result<(), FrontendError> {
    ensure_trusted_profile_forget_allowed(state)?;
    let _ordering = state
        .preference_ordering
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    ensure_trusted_profile_forget_allowed(state)?;
    let model_selection = state
        .model
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .selection
        .clone();
    state
        .preferences
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .forget_trusted_profile_path(&model_selection)
        .map_err(|_| FrontendError::TrustedProfilePreferenceSaveFailed)
}

#[cfg(target_os = "windows")]
#[tauri::command]
fn trusted_profile_selection(state: State<'_, DesktopAppState>) -> TrustedProfilePresentation {
    let remembered = state
        .preferences
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .remembered_trusted_profile_path()
        .is_some();
    state
        .trusted_profile
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .as_ref()
        .map_or_else(
            || TrustedProfilePresentation::none(remembered),
            |profile| profile.presentation_with_remembered(remembered),
        )
}

#[cfg(target_os = "windows")]
#[tauri::command]
fn choose_trusted_profile(
    app: AppHandle,
    state: State<'_, DesktopAppState>,
) -> Result<(), FrontendError> {
    ensure_trusted_profile_selection_allowed(state.inner())?;
    let selected = app.dialog().file().blocking_pick_file();
    let Some(selected) = selected else {
        return Ok(());
    };
    let path = selected
        .into_path()
        .map_err(|_| FrontendError::ProfileDialogFailed)?;
    // A connection may have started while the native picker was open.
    ensure_trusted_profile_selection_allowed(state.inner())?;
    let selection = load_provider_only_profile(path.clone()).map_err(|error| {
        tracing::warn!(reason = ?error, "Desktop Trusted Profile static validation failed");
        profile_selection_error(error)
    })?;
    let remembered_path =
        RememberedTrustedProfilePath::parse(path).map_err(|_| FrontendError::ProfileInvalid)?;
    // Static loading is intentionally not an activation lock. Revalidate host
    // lifecycle before the durable preference transaction.
    ensure_trusted_profile_selection_allowed(state.inner())?;
    save_trusted_profile_preference(state.inner(), remembered_path).map_err(|warning| {
        emit_preferences_warning(&app, warning);
        FrontendError::TrustedProfilePreferenceSaveFailed
    })?;
    // A lifecycle transition after durable save must win over publication;
    // the remembered preference is intentionally left durable in that case.
    ensure_trusted_profile_selection_allowed(state.inner())?;
    publish_trusted_profile_selection(state.inner(), selection);
    Ok(())
}

#[cfg(target_os = "windows")]
#[tauri::command]
fn restore_trusted_profile(state: State<'_, DesktopAppState>) -> Result<(), FrontendError> {
    restore_trusted_profile_selection(state.inner())
}

#[cfg(target_os = "windows")]
#[tauri::command]
fn forget_trusted_profile(
    app: AppHandle,
    state: State<'_, DesktopAppState>,
) -> Result<(), FrontendError> {
    forget_trusted_profile_preference(state.inner()).inspect_err(|error| {
        if *error == FrontendError::TrustedProfilePreferenceSaveFailed {
            emit_preferences_warning(&app, PreferencesWarning::SaveFailed);
        }
    })
}

#[cfg(target_os = "windows")]
#[tauri::command]
fn clear_trusted_profile(state: State<'_, DesktopAppState>) -> Result<(), FrontendError> {
    ensure_trusted_profile_selection_allowed(state.inner())?;
    clear_trusted_profile_selection(state.inner());
    Ok(())
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
        .preference_ordering
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
    let _ordering = state
        .preference_ordering
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
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
        .preference_ordering
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
        match control.review_current_staged_snapshot().await {
            Ok((presentation, review)) => (presentation, review),
            Err(_) => (
                repository
                    .staged_diff
                    .execute(input, ToolContext::default())
                    .await
                    .map_err(|_| RepositoryObservationStage::StagedDiffExecution)?,
                None,
            ),
        }
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
        let profile_generation = *state
            .trusted_profile_generation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let connection_generation = *state
            .next_connection_generation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let connected_context_current = matches!(
            &*state
                .connection
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
            ConnectionState::Connected {
                repository_generation,
                model_generation: connected_model_generation,
                profile_generation: connected_profile_generation,
                connection_generation: connected_connection_generation,
                ..
            } if connection_activation_publication_is_current(
                [
                    *repository_generation,
                    *connected_model_generation,
                    *connected_profile_generation,
                    *connected_connection_generation,
                ],
                [generation, model_generation, profile_generation, connection_generation],
            )
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
#[cfg(test)]
fn replace_selected_repository(state: &DesktopAppState, repository: DesktopRepository) {
    *state
        .commit_capability
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
    let repository_fingerprint = repository_context_fingerprint(&repository.root);
    *state
        .repository
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(Arc::new(repository));
    *state
        .repository_generation
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) += 1;
    let repository_generation = *state
        .repository_generation
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    append_live_evidence(serde_json::json!({
        "event": "repository_selected",
        "repository_generation": repository_generation,
        "repository_fingerprint": repository_fingerprint,
    }));
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
fn construct_repository_for_admission(
    git: &Path,
    root: &Path,
) -> Result<DesktopRepository, FrontendError> {
    let deletion_authority = RepositoryFileDeletionAuthority::new(git, root).map_err(|error| {
        let _ = error;
        tracing::warn!("admitted repository cannot receive deletion authority");
        FrontendError::RepositoryInvalid
    })?;
    let rename_authority = match RepositoryFileRenameAuthority::new(git, root) {
        Ok(authority) => Some(authority),
        Err(error) => {
            let _ = error;
            tracing::warn!("admitted repository cannot receive rename authority");
            None
        }
    };
    let directory_creation_authority =
        RepositoryDirectoryCreationAuthority::new(root).map_err(|error| {
            let _ = error;
            tracing::warn!("admitted repository cannot receive directory authority");
            FrontendError::RepositoryInvalid
        })?;
    let branch_creation_authority = match RepositoryBranchCreationAuthority::new(git, root) {
        Ok(authority) => Some(authority),
        Err(error) => {
            let _ = error;
            tracing::warn!("admitted repository cannot receive branch authority");
            None
        }
    };
    DesktopRepository::new_with_authorities(
        git,
        root,
        Some(directory_creation_authority),
        Some(deletion_authority),
        rename_authority,
        branch_creation_authority,
    )
    .map_err(|error| {
        let _ = error;
        tracing::warn!("admitted repository is invalid");
        FrontendError::RepositoryInvalid
    })
}

#[cfg(target_os = "windows")]
fn admit_repository(
    state: &DesktopAppState,
    git: &Path,
    selected_path: &Path,
) -> Result<RepositoryMemberId, FrontendError> {
    let identity = RepositoryAdmissionIdentity::capture(git, selected_path).map_err(|error| {
        let _ = error;
        tracing::warn!("repository admission identity capture failed");
        FrontendError::RepositoryInvalid
    })?;
    let root = identity.canonical_root().to_path_buf();
    // Admission may use current validators as a proof, but the constructed
    // repository and all temporary authorities are dropped before publication.
    let _ = construct_repository_for_admission(git, &root)?;
    identity.revalidate(git, &root).map_err(|error| {
        let _ = error;
        tracing::warn!("repository became stale before admission");
        FrontendError::RepositoryMemberStale
    })?;

    let _coordination = state
        .membership_coordination
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut membership = state
        .workspace_membership
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    match membership.relation_to_existing(&identity) {
        Some(RepositoryAdmissionRelation::Same) => {
            return Err(FrontendError::RepositoryAlreadyMember);
        }
        Some(RepositoryAdmissionRelation::Nested) => {
            return Err(FrontendError::RepositoryNestedMembershipConflict);
        }
        Some(RepositoryAdmissionRelation::Distinct) | None => {}
    }
    let member = membership.admit(root.display().to_string(), root, identity);
    Ok(member.id)
}

#[cfg(target_os = "windows")]
fn publish_active_repository(
    state: &DesktopAppState,
    member_id: RepositoryMemberId,
    repository: DesktopRepository,
) -> Result<(), FrontendError> {
    let repository_fingerprint = repository_context_fingerprint(&repository.root);
    let repository = Arc::new(repository);
    let repository_generation = {
        let mut membership = state
            .workspace_membership
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if !membership.publish_active(member_id) {
            return Err(FrontendError::RepositoryMemberNotFound);
        }
        *state
            .repository
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(repository);
        let mut generation = state
            .repository_generation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *generation = generation.wrapping_add(1);
        *generation
    };
    append_live_evidence(serde_json::json!({
        "event": "repository_selected",
        "repository_generation": repository_generation,
        "repository_fingerprint": repository_fingerprint,
    }));
    *state
        .repository_workflow
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = RepositoryWorkflowState::default();
    state.select_persistence_namespace();
    state
        .conversation
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .start_new();
    Ok(())
}

#[cfg(target_os = "windows")]
async fn activate_admitted_member(
    state: &DesktopAppState,
    member_id: RepositoryMemberId,
) -> Result<(), FrontendError> {
    let _coordination = state
        .membership_coordination
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    repository_selection_allowed(
        *state
            .chat
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
    )?;
    repository_selection_allowed_for_connection(
        &state
            .connection
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
    )?;
    {
        let mut coordinator = state
            .host_invocation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        coordinator.reap_expired(std::time::Instant::now());
        match coordinator.state() {
            CoordinatorState::Idle | CoordinatorState::HostPrepared => {}
            CoordinatorState::ModelTurn | CoordinatorState::HostRunning => {
                return Err(FrontendError::HostInvocationBusy);
            }
        }
    }

    let member = state
        .workspace_membership
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .member(member_id)
        .cloned()
        .ok_or(FrontendError::RepositoryMemberNotFound)?;
    let git = selected_git_executable().map_err(|_| FrontendError::RepositoryMemberStale)?;
    member
        .identity
        .revalidate(&git, &member.root)
        .map_err(|error| {
            let _ = error;
            tracing::warn!("admitted repository member is stale");
            FrontendError::RepositoryMemberStale
        })?;
    let repository = construct_repository_for_admission(&git, &member.root)?;
    member
        .identity
        .revalidate(&git, &member.root)
        .map_err(|_| FrontendError::RepositoryMemberStale)?;

    repository_selection_allowed(
        *state
            .chat
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
    )?;
    repository_selection_allowed_for_connection(
        &state
            .connection
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
    )?;
    drop(_coordination);
    revoke_repository_commit_context(state).await;
    state
        .host_invocation
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clear_prepared();
    let _publication_coordination = state
        .membership_coordination
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    publish_active_repository(state, member.id, repository)
}

#[cfg(target_os = "windows")]
#[tauri::command]
async fn choose_repository(
    app: AppHandle,
    state: State<'_, DesktopAppState>,
) -> Result<(), FrontendError> {
    repository_selection_allowed(
        *state
            .chat
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
    )?;
    repository_selection_allowed_for_connection(
        &state
            .connection
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
    let member_id = admit_repository(state.inner(), &git, &path)?;
    if let Err(error) = activate_admitted_member(state.inner(), member_id).await {
        state
            .workspace_membership
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(member_id);
        return Err(error);
    }
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
        let fs_read = FsReadTool::new_repository(&repository.root, DESKTOP_FS_READ_MAX_BYTES)
            .map_err(|error| ToolError::Execution {
                message: error.to_string(),
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
        if let Some(authority) = &repository.directory_creation_authority {
            registry.register(Arc::new(RepositoryDirectoryCreationTool::from_authority(
                authority.clone(),
            )))?;
        }
        if let Some(authority) = &repository.branch_creation_authority {
            registry.register(Arc::new(RepositoryBranchCreationTool::from_authority(
                authority.clone(),
            )))?;
        }
        registry.register(Arc::new(edit_files))?;
        if let Some(authority) = &repository.deletion_authority {
            registry.register(Arc::new(RepositoryFileDeletionTool::from_authority(
                authority.clone(),
            )))?;
        }
        if let Some(authority) = &repository.rename_authority {
            registry.register(Arc::new(RepositoryFileRenameTool::from_authority(
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
        "rename_authority_present": repository
            .is_some_and(|value| value.rename_authority.is_some()),
        "directory_creation_authority_present": repository
            .is_some_and(|value| value.directory_creation_authority.is_some()),
        "branch_creation_authority_present": repository
            .is_some_and(|value| value.branch_creation_authority.is_some()),
        "registry_contains_repo_delete_file": registry
            .definitions()
            .iter()
            .any(|definition| definition.name.as_str() == "repo.delete-file"),
        "registry_contains_repo_create_branch": registry
            .definitions()
            .iter()
            .any(|definition| definition.name.as_str() == REPOSITORY_CREATE_BRANCH_TOOL_NAME),
        "relevant_tool_names": registry
            .definitions()
            .into_iter()
            .filter(|definition| definition.name.as_str().starts_with("repo."))
            .map(|definition| definition.name.to_string())
            .collect::<Vec<_>>(),
    }));
    Ok(Arc::new(registry))
}

#[cfg(target_os = "windows")]
fn desktop_tool_composition_from_registry(
    registry: Arc<ToolRegistry>,
    repository: Option<&DesktopRepository>,
    commit_tool_present: bool,
    external_descriptors: &[ExternalToolDescriptor],
) -> Result<Arc<DesktopToolComposition>, effective_authority::CompositionError> {
    Ok(Arc::new(effective_authority::compose(
        registry,
        repository,
        commit_tool_present,
        external_descriptors,
    )?))
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
    if state
        .host_invocation
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .state()
        != CoordinatorState::Idle
    {
        return Err(FrontendError::HostInvocationBusy);
    }
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

    let connection_generation = {
        let mut generation = state
            .next_connection_generation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *generation = generation.wrapping_add(1);
        *generation
    };

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
    let (profile_selection, profile_generation) = {
        let selection = state
            .trusted_profile
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        let generation = *state
            .trusted_profile_generation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        (selection, generation)
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
    let first_party_registry =
        match desktop_tool_registry(repository.as_deref(), commit_tool.clone()) {
            Ok(registry) => registry,
            Err(error) => {
                tracing::error!(error = %error, "failed to construct Desktop first-party registry");
                let frontend_error = FrontendError::ToolRegistryFailed;
                *state
                    .connection
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) =
                    ConnectionState::Error(frontend_error);
                return Err(frontend_error);
            }
        };

    let mut provider_activation = match profile_selection.as_ref() {
        Some(selection) => match DesktopProviderActivation::activate(selection).await {
            Ok(activation) => Some(activation),
            Err(error) => {
                let frontend_error = match error {
                    ProviderActivationError::Profile(error) => profile_selection_error(error),
                    ProviderActivationError::ProviderUnavailable => {
                        FrontendError::ProfileActivationFailed
                    }
                };
                tracing::warn!(reason = ?error, "Desktop provider activation failed");
                *state
                    .connection
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) =
                    ConnectionState::Error(frontend_error);
                return Err(frontend_error);
            }
        },
        None => None,
    };
    let registry = match merge_tool_registries(
        first_party_registry.as_ref(),
        provider_activation
            .as_ref()
            .map(DesktopProviderActivation::registry),
    ) {
        Ok(registry) => registry,
        Err(error) => {
            tracing::warn!(error = %error, "Desktop final Tool registry merge failed");
            if let Some(activation) = provider_activation.take() {
                activation.shutdown().await;
            }
            let frontend_error = FrontendError::ToolRegistryFailed;
            *state
                .connection
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) =
                ConnectionState::Error(frontend_error);
            return Err(frontend_error);
        }
    };
    let allowed_permissions = desktop_allowed_permissions(
        repository.is_some(),
        provider_activation
            .as_ref()
            .map_or(&[], DesktopProviderActivation::permissions),
    );
    let retained_allowed_permissions = allowed_permissions.clone();
    let external_descriptors = provider_activation
        .as_ref()
        .map_or_else(Vec::new, |activation| activation.external_tools().to_vec());
    let composition = match desktop_tool_composition_from_registry(
        Arc::clone(&registry),
        repository.as_deref(),
        commit_tool.is_some(),
        &external_descriptors,
    ) {
        Ok(composition) => composition,
        Err(_) => {
            tracing::warn!("Desktop authority classification failed closed");
            if let Some(activation) = provider_activation.take() {
                activation.shutdown().await;
            }
            let frontend_error = FrontendError::ToolRegistryFailed;
            *state
                .connection
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) =
                ConnectionState::Error(frontend_error);
            return Err(frontend_error);
        }
    };
    let repository_fingerprint = repository
        .as_ref()
        .map(|value| repository_context_fingerprint(&value.root));
    let connection_repository_fingerprint = repository_fingerprint.clone();
    let selected_profile = profile_selection.is_some();
    match resolve_prepare_and_connect_codex(
        resolve_codex_executable,
        model_config,
        |prepared| async move {
            append_live_evidence(serde_json::json!({
                "event": "connection_started",
                "repository_generation": repository_generation,
                "repository_fingerprint": connection_repository_fingerprint,
                "model_generation": model_generation,
                "profile_generation": profile_generation,
                "connection_generation": connection_generation,
                "selected_repository": repository.is_some(),
                "selected_profile": selected_profile,
                "deletion_authority_present": repository
                    .as_ref()
                    .is_some_and(|value| value.deletion_authority.is_some()),
                "rename_authority_present": repository
                    .as_ref()
                    .is_some_and(|value| value.rename_authority.is_some()),
                "branch_creation_authority_present": repository
                    .as_ref()
                    .is_some_and(|value| value.branch_creation_authority.is_some()),
                "bridge_enabled": true,
            }));
            CodexRuntime::connect_tool_bridge_with_model_config_and_workspace(
                prepared.executable,
                registry,
                allowed_permissions,
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
            let runtime = Arc::new(runtime);
            let published_fingerprint = repository_fingerprint.clone();
            let current_repository_generation = *state
                .repository_generation
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let current_model_generation = state
                .model
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .generation;
            let current_profile_generation = *state
                .trusted_profile_generation
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let current_connection_generation = *state
                .next_connection_generation
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let stale = !connection_activation_publication_is_current(
                [
                    repository_generation,
                    model_generation,
                    profile_generation,
                    connection_generation,
                ],
                [
                    current_repository_generation,
                    current_model_generation,
                    current_profile_generation,
                    current_connection_generation,
                ],
            );
            if stale {
                append_live_evidence(serde_json::json!({
                    "event": "connection_publication_rejected_stale",
                    "captured_repository_generation": repository_generation,
                    "current_repository_generation": current_repository_generation,
                    "captured_model_generation": model_generation,
                    "current_model_generation": current_model_generation,
                    "captured_profile_generation": profile_generation,
                    "current_profile_generation": current_profile_generation,
                    "connection_generation": connection_generation,
                }));
                if let Err(error) = runtime.shutdown().await {
                    tracing::warn!(error = %error, "stale Codex desktop runtime shutdown failed");
                }
                if let Some(activation) = provider_activation.take() {
                    activation.shutdown().await;
                }
                let mut connection = state
                    .connection
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                if matches!(*connection, ConnectionState::Connecting) {
                    *connection = ConnectionState::NotConnected;
                }
                return Err(FrontendError::CodexReconnectRequired);
            }

            let pending = PendingConnectedPublication {
                runtime,
                activation: provider_activation.take(),
                source,
                repository_generation,
                model_generation,
                profile_generation,
                connection_generation,
                repository_fingerprint,
                composition: Arc::clone(&composition),
                allowed_permissions: retained_allowed_permissions,
            };
            if let Err(rejected) = publish_connected_provider_state(state.inner(), pending) {
                let RejectedProviderPublication {
                    runtime,
                    activation,
                    reason,
                } = *rejected;
                if let Err(error) = runtime.shutdown().await {
                    tracing::warn!(error = %error, "rejected Codex runtime shutdown failed");
                }
                if let Some(activation) = activation {
                    activation.shutdown().await;
                }
                return match reason {
                    ProviderPublicationRejectionReason::Superseded
                    | ProviderPublicationRejectionReason::Stale => {
                        let mut connection = state
                            .connection
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner);
                        if matches!(*connection, ConnectionState::Connecting) {
                            *connection = ConnectionState::NotConnected;
                        }
                        Err(FrontendError::CodexReconnectRequired)
                    }
                    ProviderPublicationRejectionReason::DuplicateOwner => {
                        state.shutdown_provider_activation().await;
                        let frontend_error = FrontendError::ProfileActivationFailed;
                        let mut connection = state
                            .connection
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner);
                        if matches!(*connection, ConnectionState::Connecting) {
                            *connection = ConnectionState::Error(frontend_error);
                        }
                        Err(frontend_error)
                    }
                };
            }
            append_live_evidence(serde_json::json!({
                "event": "connection_published",
                "repository_generation": repository_generation,
                "repository_fingerprint": published_fingerprint,
                "model_generation": model_generation,
                "profile_generation": profile_generation,
                "connection_generation": connection_generation,
                "profile_active": selected_profile,
            }));
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
            if let Some(activation) = provider_activation.take() {
                activation.shutdown().await;
            }
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
    if matches!(
        state
            .host_invocation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .state(),
        CoordinatorState::ModelTurn | CoordinatorState::HostPrepared
    ) {
        return Err(FrontendError::HostInvocationBusy);
    }
    let runtime = {
        revoke_repository_commit_context(state.inner()).await;
        let mut connection = state
            .connection
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match std::mem::replace(&mut *connection, ConnectionState::Disconnecting) {
            ConnectionState::Connected { runtime, .. } => Some(runtime),
            previous => {
                *connection = previous;
                None
            }
        }
    };

    let activation = state
        .provider_activation
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .take();
    let Some(runtime) = runtime else {
        if let Some(activation) = activation {
            activation.shutdown().await;
        }
        return Ok(ConnectionResult::not_connected());
    };

    let runtime_result = runtime.shutdown().await;
    if let Some(activation) = activation {
        activation.shutdown().await;
    }
    if let Err(error) = runtime_result {
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
fn repository_selection_allowed_for_connection(
    connection: &ConnectionState,
) -> Result<(), FrontendError> {
    if matches!(connection, ConnectionState::Connecting) {
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RepositoryRefreshReason {
    FirstPartyMutation,
    UncertainRepositoryEffect,
}

#[cfg(target_os = "windows")]
#[derive(Clone, Debug)]
struct ToolCallActivity {
    tool: String,
    external: bool,
    started: bool,
}

#[cfg(target_os = "windows")]
#[derive(Clone, Debug, PartialEq, Eq)]
struct ActivityOutcome {
    activity: ActivityEvent,
    invalidate_review: bool,
    refresh_reason: Option<RepositoryRefreshReason>,
}

#[cfg(target_os = "windows")]
#[cfg(test)]
fn activity_event(
    event: &AgentEvent,
    tool_calls: &mut HashMap<rah_protocol::ToolCallId, ToolCallActivity>,
) -> Option<(ActivityEvent, bool)> {
    activity_event_with_composition(event, tool_calls, &empty_composition_metadata(), false)
        .map(|outcome| (outcome.activity, outcome.refresh_reason.is_some()))
}

#[cfg(target_os = "windows")]
fn activity_event_with_composition(
    event: &AgentEvent,
    tool_calls: &mut HashMap<rah_protocol::ToolCallId, ToolCallActivity>,
    composition: &DesktopToolComposition,
    repository_selected: bool,
) -> Option<ActivityOutcome> {
    let is_external = |tool: &str| {
        composition.tools.iter().any(|entry| {
            entry.public_tool_name == tool
                && matches!(
                    entry.source_kind,
                    SourceKind::Mcp | SourceKind::ProcessPlugin
                )
        })
    };
    match event {
        AgentEvent::ToolRequested { tool_call, .. } => {
            if tool_calls.len() >= MAX_TURN_TOOL_CALLS {
                return None;
            }
            let tool = tool_call.name.as_str().to_owned();
            let external = is_external(&tool);
            tool_calls.insert(
                tool_call.id.clone(),
                ToolCallActivity {
                    tool: tool.clone(),
                    external,
                    started: false,
                },
            );
            Some(ActivityOutcome {
                activity: ActivityEvent::Requested { tool },
                invalidate_review: false,
                refresh_reason: None,
            })
        }
        AgentEvent::ToolStarted { tool_call_id, .. } => {
            let activity = tool_calls.get_mut(tool_call_id)?;
            activity.started = true;
            Some(ActivityOutcome {
                activity: ActivityEvent::Started {
                    tool: activity.tool.clone(),
                },
                invalidate_review: activity.external && repository_selected,
                refresh_reason: None,
            })
        }
        AgentEvent::ToolFinished {
            tool_call_id,
            output,
            ..
        } => tool_calls.remove(tool_call_id).map(|activity| {
            let branch_requires_refresh = activity.tool == REPOSITORY_CREATE_BRANCH_TOOL_NAME
                && repository_selected
                && matches!(
                    branch_result_classification(output),
                    BranchActivityClassification::Uncertain
                );
            let first_party_refresh = matches!(
                activity.tool.as_str(),
                "repo.patch"
                    | "repo.create-file"
                    | "repo.create-directory"
                    | "repo.edit-files"
                    | "repo.delete-file"
                    | "repo.rename-file"
                    | "repo.commit"
            );
            let refresh_reason = if activity.external && activity.started && repository_selected {
                Some(RepositoryRefreshReason::UncertainRepositoryEffect)
            } else if branch_requires_refresh || first_party_refresh {
                Some(RepositoryRefreshReason::FirstPartyMutation)
            } else {
                None
            };
            let commit = (activity.tool == "repo.commit")
                .then(|| commit_activity_presentation(output))
                .flatten();
            ActivityOutcome {
                activity: ActivityEvent::Finished {
                    tool: activity.tool,
                    result: if output.is_error {
                        ActivityResult::Failed
                    } else {
                        ActivityResult::Success
                    },
                    commit,
                },
                invalidate_review: refresh_reason.is_some(),
                refresh_reason,
            }
        }),
        _ => None,
    }
}

#[cfg(target_os = "windows")]
enum BranchActivityClassification {
    ProvenSafe,
    Uncertain,
}

#[cfg(target_os = "windows")]
fn branch_result_classification(output: &rah_protocol::ToolOutput) -> BranchActivityClassification {
    let [ToolContent::Json(value)] = output.content.as_slice() else {
        return BranchActivityClassification::Uncertain;
    };
    let Some(object) = value.as_object() else {
        return BranchActivityClassification::Uncertain;
    };
    let Some(status) = object.get("status").and_then(serde_json::Value::as_str) else {
        return BranchActivityClassification::Uncertain;
    };
    let has_exact_keys = |expected: &[&str]| {
        object.len() == expected.len() && expected.iter().all(|key| object.contains_key(*key))
    };
    let boolean_field_is = |key: &str, expected: bool| matches!(object.get(key), Some(serde_json::Value::Bool(value)) if *value == expected);
    let non_empty_string_field = |key: &str| {
        object
            .get(key)
            .and_then(serde_json::Value::as_str)
            .is_some_and(|value| !value.is_empty())
    };
    let full_oid_field = |key: &str| {
        object
            .get(key)
            .and_then(serde_json::Value::as_str)
            .is_some_and(|oid| {
                matches!(oid.len(), 40 | 64) && oid.bytes().all(|byte| byte.is_ascii_hexdigit())
            })
    };

    let proven_safe = match status {
        "invalid_input" | "precondition_failed" | "known_no_effect" => {
            has_exact_keys(&["status", "uncertain"])
                && boolean_field_is("uncertain", false)
                && output.is_error
        }
        "branch_created_verified" => {
            has_exact_keys(&["status", "uncertain", "name", "oid"])
                && boolean_field_is("uncertain", false)
                && non_empty_string_field("name")
                && full_oid_field("oid")
                && !output.is_error
        }
        "desired_state_observed_after_uncertain_attempt" => {
            has_exact_keys(&["status", "uncertain", "name", "oid"])
                && boolean_field_is("uncertain", true)
                && non_empty_string_field("name")
                && full_oid_field("oid")
                && output.is_error
        }
        "uncertain" => false,
        _ => false,
    };
    if proven_safe {
        BranchActivityClassification::ProvenSafe
    } else {
        BranchActivityClassification::Uncertain
    }
}

#[cfg(target_os = "windows")]
#[cfg(test)]
fn empty_composition_metadata() -> DesktopToolComposition {
    DesktopToolComposition {
        registry: Arc::new(ToolRegistry::new()),
        expected_definitions: Vec::new(),
        tools: Vec::new(),
        unavailable: Vec::new(),
        repository_patch_preparer: None,
        repository_multi_file_edit_preparer: None,
        repository_create_file_preparer: None,
        repository_delete_file_preparer: None,
        repository_rename_file_preparer: None,
    }
}

#[cfg(target_os = "windows")]
fn uncertain_repository_effect_pending(
    tool_calls: &HashMap<rah_protocol::ToolCallId, ToolCallActivity>,
) -> bool {
    tool_calls.values().any(|activity| {
        activity.started
            && (activity.external || activity.tool == REPOSITORY_CREATE_BRANCH_TOOL_NAME)
    })
}

#[cfg(target_os = "windows")]
fn uncertain_repository_effect_requires_refresh(
    repository_selected: bool,
    tool_calls: &HashMap<rah_protocol::ToolCallId, ToolCallActivity>,
) -> bool {
    repository_selected && uncertain_repository_effect_pending(tool_calls)
}

#[cfg(target_os = "windows")]
async fn handle_uncertain_repository_effects(
    app: &AppHandle,
    tool_calls: &mut HashMap<rah_protocol::ToolCallId, ToolCallActivity>,
    repository_selected: bool,
) {
    let refresh = uncertain_repository_effect_requires_refresh(repository_selected, tool_calls);
    tool_calls.clear();
    if refresh {
        invalidate_repository_commit_review(app.state::<DesktopAppState>().inner()).await;
        append_live_evidence(serde_json::json!({
            "event": "repository_refresh",
            "reason": "uncertain_repository_effect",
        }));
        emit_repository_refresh(app);
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
    rah_protocol::live_evidence::append(&record);
}

#[cfg(target_os = "windows")]
fn append_desktop_context_evidence(
    event: &str,
    repository_generation: u64,
    model_generation: u64,
    connection_generation: u64,
    session_generation: u64,
    repository_fingerprint: Option<&str>,
) {
    append_live_evidence(serde_json::json!({
        "event": event,
        "repository_generation": repository_generation,
        "repository_fingerprint": repository_fingerprint,
        "model_generation": model_generation,
        "runtime_generation": connection_generation,
        "connection_generation": connection_generation,
        "session_generation": session_generation,
    }));
}

#[cfg(target_os = "windows")]
fn desktop_failure_stage(code: AgentErrorCode) -> &'static str {
    match code {
        AgentErrorCode::Tool | AgentErrorCode::PermissionDenied | AgentErrorCode::Sandbox => {
            "tool_dispatch_failure"
        }
        AgentErrorCode::Model => "model_runtime_failure",
        AgentErrorCode::InvalidRequest | AgentErrorCode::Session | AgentErrorCode::Internal => {
            "terminal_disconnect_failure"
        }
    }
}

#[cfg(target_os = "windows")]
fn contains_live_completion_marker(text: &str) -> bool {
    const PREFIX: &str = "RAH_";
    const SUFFIX: &str = "_LIVE_OK";

    for (start, _) in text.match_indices(PREFIX) {
        if start > 0 && text.as_bytes()[start - 1].is_ascii_alphanumeric() {
            continue;
        }
        let candidate = &text[start..]
            .split(|character: char| {
                !character.is_ascii_uppercase() && !character.is_ascii_digit() && character != '_'
            })
            .next()
            .unwrap_or_default();
        let end = start + candidate.len();
        let has_token_suffix = text
            .as_bytes()
            .get(end)
            .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_');
        if !has_token_suffix
            && candidate.ends_with(SUFFIX)
            && candidate.len() > PREFIX.len() + SUFFIX.len()
        {
            return true;
        }
    }
    false
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
    captured_generations: [u64; 4],
) {
    let (
        repository_generation,
        model_generation,
        profile_generation,
        connection_generation,
        repository_fingerprint,
        repository_selected,
        composition,
    ) = {
        let state = app.state::<DesktopAppState>();
        let connection = state
            .connection
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match &*connection {
            ConnectionState::Connected {
                repository_generation,
                model_generation,
                profile_generation,
                connection_generation,
                repository_fingerprint,
                composition,
                ..
            } => (
                *repository_generation,
                *model_generation,
                *profile_generation,
                *connection_generation,
                repository_fingerprint.clone(),
                repository_fingerprint.is_some(),
                Arc::clone(composition),
            ),
            _ => return,
        }
    };
    let current_generations = current_host_generation_tuple(app.state::<DesktopAppState>().inner());
    if !connection_activation_publication_is_current(
        [
            repository_generation,
            model_generation,
            profile_generation,
            connection_generation,
        ],
        current_generations,
    ) || [
        repository_generation,
        model_generation,
        profile_generation,
        connection_generation,
    ] != captured_generations
    {
        append_live_evidence(serde_json::json!({
            "event": "desktop_failure",
            "failure_stage": "pre_turn_async_stale_generation_rejection",
            "repository_generation": current_generations[0],
            "model_generation": current_generations[1],
            "profile_generation": current_generations[2],
            "runtime_generation": captured_generations[3],
            "connection_generation": captured_generations[3],
            "session_generation": chat_generation,
        }));
        if app
            .state::<DesktopAppState>()
            .claim_start_failure(chat_generation)
        {
            emit_chat_event(
                &app,
                ChatEvent::Failed {
                    code: FrontendError::CodexReconnectRequired,
                },
            );
        }
        return;
    }
    let handle = match runtime.start(request).await {
        Ok(handle) => handle,
        Err(error) => {
            tracing::warn!(error = %error, "desktop chat turn failed to start");
            append_live_evidence(serde_json::json!({
                "event": "desktop_failure",
                "failure_stage": "thread_or_turn_start_failure",
                "repository_generation": repository_generation,
                "repository_fingerprint": repository_fingerprint,
                "model_generation": model_generation,
                "runtime_generation": connection_generation,
                "connection_generation": connection_generation,
                "session_generation": chat_generation,
            }));
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

    append_desktop_context_evidence(
        "thread_start",
        repository_generation,
        model_generation,
        connection_generation,
        chat_generation,
        repository_fingerprint.as_deref(),
    );

    if !app.state::<DesktopAppState>().register_chat_session(
        chat_generation,
        Arc::clone(&runtime),
        handle.session_id().clone(),
    ) {
        tracing::warn!("desktop chat session was no longer current before streaming began");
        app.state::<DesktopAppState>().finish_chat(chat_generation);
        return;
    }

    let session_id = handle.session_id().clone();
    emit_chat_event(&app, ChatEvent::Started);
    let mut terminal = false;
    let mut tool_calls = HashMap::new();
    let mut events = handle.into_events();
    while let Some(event) = events.next().await {
        if let Some(outcome) = activity_event_with_composition(
            &event,
            &mut tool_calls,
            &composition,
            repository_selected,
        ) {
            let activity_name = match &outcome.activity {
                ActivityEvent::Requested { .. } => "tool_requested",
                ActivityEvent::Started { .. } => "tool_started",
                ActivityEvent::Finished { .. } => "tool_finished",
            };
            append_desktop_context_evidence(
                activity_name,
                repository_generation,
                model_generation,
                connection_generation,
                chat_generation,
                repository_fingerprint.as_deref(),
            );
            if outcome.invalidate_review {
                invalidate_repository_commit_review(app.state::<DesktopAppState>().inner()).await;
            }
            emit_activity_event(&app, outcome.activity);
            if let Some(reason) = outcome.refresh_reason {
                append_live_evidence(serde_json::json!({
                    "event": "repository_refresh",
                    "reason": match reason {
                        RepositoryRefreshReason::FirstPartyMutation => "repository_mutation",
                        RepositoryRefreshReason::UncertainRepositoryEffect => {
                            "uncertain_repository_effect"
                        }
                    },
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
                handle_uncertain_repository_effects(&app, &mut tool_calls, repository_selected)
                    .await;
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
                    "marker_observed": contains_live_completion_marker(&assistant_text),
                    "repository_generation": repository_generation,
                    "repository_fingerprint": repository_fingerprint,
                    "model_generation": model_generation,
                    "runtime_generation": connection_generation,
                    "connection_generation": connection_generation,
                    "session_generation": chat_generation,
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
            AgentEvent::Failed { code, message, .. } => {
                handle_uncertain_repository_effects(&app, &mut tool_calls, repository_selected)
                    .await;
                // The frontend receives only a closed error code. Retain the
                // adapter-provided stage privately for live diagnosis.
                tracing::warn!(stage = "post-start runtime/event failure", error = %message, "desktop chat turn failed after start");
                append_live_evidence(serde_json::json!({
                    "event": "desktop_failure",
                    "failure_stage": desktop_failure_stage(code),
                    "repository_generation": repository_generation,
                    "repository_fingerprint": repository_fingerprint,
                    "model_generation": model_generation,
                    "runtime_generation": connection_generation,
                    "connection_generation": connection_generation,
                    "session_generation": chat_generation,
                }));
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
                handle_uncertain_repository_effects(&app, &mut tool_calls, repository_selected)
                    .await;
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
    handle_uncertain_repository_effects(&app, &mut tool_calls, repository_selected).await;
    if !terminal
        && app
            .state::<DesktopAppState>()
            .claim_terminal(chat_generation, &runtime, &session_id)
    {
        append_live_evidence(serde_json::json!({
            "event": "desktop_failure",
            "failure_stage": "terminal_disconnect_failure",
            "repository_generation": repository_generation,
            "repository_fingerprint": repository_fingerprint,
            "model_generation": model_generation,
            "runtime_generation": connection_generation,
            "connection_generation": connection_generation,
            "session_generation": chat_generation,
        }));
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
    let (runtime, identity, profile_generation, connection_generation) = {
        let connection = state
            .connection
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match &*connection {
            ConnectionState::Connected {
                runtime,
                repository_generation,
                model_generation,
                profile_generation,
                connection_generation,
                ..
            } => (
                Arc::clone(runtime),
                ConversationContextIdentity {
                    repository_generation: *repository_generation,
                    model_generation: *model_generation,
                },
                *profile_generation,
                *connection_generation,
            ),
            _ => {
                state.finish_chat(chat_generation);
                return Err(FrontendError::CodexNotConnected);
            }
        }
    };
    let current_generations = current_host_generation_tuple(state.inner());
    if !connection_activation_publication_is_current(
        [
            identity.repository_generation,
            identity.model_generation,
            profile_generation,
            connection_generation,
        ],
        current_generations,
    ) {
        append_live_evidence(serde_json::json!({
            "event": "desktop_failure",
            "failure_stage": "pre_turn_stale_generation_rejection",
            "repository_generation": current_generations[0],
            "model_generation": current_generations[1],
            "profile_generation": current_generations[2],
            "runtime_generation": identity.repository_generation,
            "session_generation": chat_generation,
        }));
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
        [
            identity.repository_generation,
            identity.model_generation,
            profile_generation,
            connection_generation,
        ],
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
            state.shutdown_provider_activation().await;
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
    let profile_generation = *state
        .trusted_profile_generation
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let connection_generation = *state
        .next_connection_generation
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let identity = {
        let connection = state
            .connection
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match &*connection {
            ConnectionState::Connected {
                repository_generation: connected_repository_generation,
                model_generation: connected_model_generation,
                profile_generation: connected_profile_generation,
                connection_generation: connected_connection_generation,
                ..
            } if connection_activation_publication_is_current(
                [
                    *connected_repository_generation,
                    *connected_model_generation,
                    *connected_profile_generation,
                    *connected_connection_generation,
                ],
                [
                    repository_generation,
                    model_generation,
                    profile_generation,
                    connection_generation,
                ],
            ) =>
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
            trusted_profile_selection,
            choose_trusted_profile,
            restore_trusted_profile,
            forget_trusted_profile,
            clear_trusted_profile,
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
            conversation_transcript,
            get_effective_authority_snapshot,
            host_invoke_read,
            host_prepare_repo_create_branch,
            host_prepare_repo_patch,
            host_prepare_repo_edit_files,
            host_prepare_repo_create_file,
            host_prepare_repo_delete_file,
            host_prepare_repo_rename_file,
            host_confirm_tool_invocation,
            host_cancel_tool_invocation
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
    use super::effective_authority::{
        AuthorityCategory, EffectClass, EffectiveToolEntry, SnapshotStatus,
    };
    use super::host_invocation::{
        BranchReview, CoordinatorState, EmptyHostRequest, HostConfirmRequest,
        HostInvocationDescriptor, HostInvocationKind, HostInvocationReview,
        HostPrepareBranchRequest, HostPrepareCreateFileRequest, HostPrepareDeleteFileRequest,
        HostPrepareMultiFileEditReplacement, HostPrepareMultiFileEditRequest,
        HostPrepareMultiFileEditTarget, HostPreparePatchRequest, HostPrepareRenameFileRequest,
        HostReadRequest, host_descriptor, host_descriptor_with_rename,
    };
    use super::trusted_profile_selection::load_provider_only_profile;
    use super::{
        ActivityEvent, ActivityResult, BranchActivityClassification, CancelRecoveryOutcome,
        ChatEvent, ChatState, CodexExecutableSourcePresentation, CommitAuthorizationPresentation,
        ConnectRequest, ConnectionState, ConversationContextChange, ConversationContextIdentity,
        CreateFileResultClassification, DESKTOP_TOOL_NAME, DeleteFileResultClassification,
        DesktopAppState, DesktopCommitCapability, DesktopCommitIdentity, DesktopConversationState,
        DesktopModelProvider, DesktopModelSelection, DesktopModelState, DesktopRepository,
        DesktopToolComposition, FrontendError, GracefulCancelOutcome, HardShutdownOutcome,
        HostActivityEvent, HostActivityState, HostInvocationCoordinator,
        HostInvocationUnavailableReason, LlamaCppReadinessProbe, MAX_CONVERSATION_REPLAY_BYTES,
        MAX_CONVERSATION_REPLAY_MESSAGES, MAX_PROMPT_BYTES, ModelConfigurationPresentation,
        MultiFileResultClassification, NEUTRAL_WORKSPACE_DIRECTORY, PendingConnectedPublication,
        Preferences, PreferencesWarning, PreparedDeleteFileResponse, PreparedHostInvocation,
        PreparedHostPayload, ProviderEndpoint, ProviderEndpointInput, ProviderEndpointPresentation,
        ProviderScheme, READINESS_BODY_LIMIT, READINESS_TOTAL_TIMEOUT,
        REPOSITORY_CREATE_BRANCH_TOOL_NAME, ReadinessState, RepositoryIndexActionKind,
        RepositoryObservationStage, RepositoryRefreshReason, ResumePair, SendChatResult,
        SourceKind, StagedReviewPresentation, StartupActivationCounters, TerminalOwnership,
        activate_admitted_member, activity_event, activity_event_with_composition,
        admit_repository, apply_model_selection, authorize_repository_commit_review,
        await_cancel_recovery, await_graceful_cancel, await_hard_shutdown, begin_chat,
        branch_result_classification, classify_repository_delete_file_result,
        classify_repository_multi_file_output, clear_conversation_allowed,
        clear_trusted_profile_selection, commit_activity_presentation, connect_codex,
        connect_prepared_codex, connection_activation_publication_is_current, current_app_status,
        current_host_generation_tuple, delete_file_host_terminal_state,
        desktop_repository_snapshot, desktop_repository_snapshot_with_review,
        desktop_tool_composition_from_registry, desktop_tool_registry, emit_host_activity,
        empty_composition_metadata, forget_trusted_profile_preference, frontend_error,
        get_effective_authority_snapshot, host_call, host_cancel_tool_invocation,
        host_confirm_tool_invocation, host_invoke_read, host_kind, host_prepare_repo_create_branch,
        host_prepare_repo_create_file, host_prepare_repo_delete_file, host_prepare_repo_edit_files,
        host_prepare_repo_patch, host_prepare_repo_rename_file, install_repository_workflow,
        invalidate_repository_commit_review, model_configuration_status, patch_host_terminal_state,
        prepare_codex_connection, prepare_repo_delete_file_with_current,
        prepare_repo_rename_file_with_current, prepared_host_activity,
        publish_connected_provider_state, publish_readiness_result,
        publish_trusted_profile_selection, refresh_repository_workflow,
        replace_selected_repository, repository_authorize_commit_review,
        repository_context_fingerprint, repository_index_action, repository_selection_allowed,
        repository_selection_allowed_for_connection, repository_snapshot,
        repository_tool_authority, request_connect, reset_startup_activation_counters,
        resolve_codex_executable, resolve_prepare_and_connect_codex,
        restore_trusted_profile_selection, revoke_repository_commit_context, run_host_tool,
        safe_delete_file_activity_result, same_arc, save_trusted_profile_preference,
        selected_git_executable, set_commit_identity, startup_activation_snapshot,
        uncertain_repository_effect_pending, uncertain_repository_effect_requires_refresh,
        validate_host_confirmation_ticket, validate_prompt,
    };
    use super::{SUPPORTED_CODEX_VERSION, current_host_composition};
    use async_trait::async_trait;
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
    use rah_tools::{
        RepositoryBranchCreationAuthority, RepositoryCommitControl, RepositoryCommitTool,
        RepositoryDirectoryCreationAuthority, RepositoryFileCreationTool,
        RepositoryFileDeletionAuthority, RepositoryFileDeletionTool, RepositoryFileRenameAuthority,
        RepositoryFileRenameTool, RepositoryMultiFileEditPreparationRequest,
        RepositoryMultiFileEditPreparationTarget, RepositoryMultiFileEditPreparer,
        RepositoryMultiFileEditTextReplacement, RepositoryPatchResultClassification, Tool,
        ToolContext, ToolRegistry, classify_repository_patch_output,
        classify_repository_rename_file_output, clear_live_test_create_file_native_attempts,
        clear_live_test_create_file_tool_executions, clear_live_test_delete_file_native_attempts,
        clear_live_test_delete_file_tool_executions, clear_live_test_multi_file_native_attempts,
        clear_live_test_multi_file_tool_executions, clear_live_test_rename_file_native_attempts,
        clear_live_test_rename_file_tool_executions, live_test_create_file_native_attempts,
        live_test_create_file_tool_executions, live_test_delete_file_native_attempts,
        live_test_delete_file_tool_executions, live_test_multi_file_native_attempts,
        live_test_multi_file_tool_executions, live_test_rename_file_native_attempts,
        live_test_rename_file_tool_executions,
    };
    use serde_json::Value;
    use sha2::{Digest, Sha256};
    use std::os::windows::io::AsRawHandle;
    use std::{
        collections::{BTreeSet, HashMap},
        ffi::OsString,
        fs,
        io::{Read, Write},
        net::{Ipv4Addr, SocketAddrV4, TcpListener},
        path::{Path, PathBuf},
        process::Command,
        sync::{
            Arc, Mutex,
            atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
        },
        thread,
        time::{Duration, SystemTime, UNIX_EPOCH},
    };
    use tauri::{Listener, Manager};
    use windows_sys::Win32::Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle,
    };

    #[derive(Debug)]
    struct RuntimeMarker;

    struct CountingCreateFileTool {
        inner: RepositoryFileCreationTool,
        executions: Arc<AtomicUsize>,
    }

    struct CountingDeleteFileTool {
        inner: RepositoryFileDeletionTool,
        executions: Arc<AtomicUsize>,
    }

    struct CountingRenameFileTool {
        inner: RepositoryFileRenameTool,
        executions: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl rah_tools::Tool for CountingDeleteFileTool {
        fn definition(&self) -> rah_protocol::ToolDefinition {
            self.inner.definition()
        }

        async fn execute(
            &self,
            input: ToolInput,
            context: ToolContext,
        ) -> Result<ToolOutput, rah_tools::ToolError> {
            self.executions.fetch_add(1, Ordering::SeqCst);
            self.inner.execute(input, context).await
        }
    }

    #[async_trait]
    impl rah_tools::Tool for CountingRenameFileTool {
        fn definition(&self) -> rah_protocol::ToolDefinition {
            self.inner.definition()
        }

        async fn execute(
            &self,
            input: ToolInput,
            context: ToolContext,
        ) -> Result<ToolOutput, rah_tools::ToolError> {
            self.executions.fetch_add(1, Ordering::SeqCst);
            self.inner.execute(input, context).await
        }
    }

    struct FailingDeleteFileTool {
        inner: RepositoryFileDeletionTool,
        executions: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl rah_tools::Tool for FailingDeleteFileTool {
        fn definition(&self) -> rah_protocol::ToolDefinition {
            self.inner.definition()
        }

        async fn execute(
            &self,
            _input: ToolInput,
            _context: ToolContext,
        ) -> Result<ToolOutput, rah_tools::ToolError> {
            self.executions.fetch_add(1, Ordering::SeqCst);
            Err(rah_tools::ToolError::Execution {
                message: "RAH_RAW_DELETE_RUNTIME_FAILURE_SENTINEL".to_owned(),
            })
        }
    }

    #[async_trait]
    impl rah_tools::Tool for CountingCreateFileTool {
        fn definition(&self) -> rah_protocol::ToolDefinition {
            self.inner.definition()
        }

        async fn execute(
            &self,
            input: ToolInput,
            context: ToolContext,
        ) -> Result<ToolOutput, rah_tools::ToolError> {
            self.executions.fetch_add(1, Ordering::SeqCst);
            self.inner.execute(input, context).await
        }
    }

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
            DesktopRepository::new_with_authorities(
                &git,
                &self.0,
                None,
                Some(authority),
                None,
                None,
            )
            .expect("selected deletion repository should construct")
        }

        fn directory_repository(&self) -> DesktopRepository {
            let git = Self::native_git();
            let authority = RepositoryDirectoryCreationAuthority::new(&self.0)
                .expect("host directory creation authority should construct");
            DesktopRepository::new_with_authorities(
                &git,
                &self.0,
                Some(authority),
                None,
                None,
                None,
            )
            .expect("selected directory repository should construct")
        }

        fn rename_repository(&self) -> DesktopRepository {
            let git = Self::native_git();
            let authority = RepositoryFileRenameAuthority::new(&git, &self.0)
                .expect("host rename authority should construct");
            DesktopRepository::new_with_authorities(
                &git,
                &self.0,
                None,
                None,
                Some(authority),
                None,
            )
            .expect("selected rename repository should construct")
        }

        fn branch_repository(&self) -> DesktopRepository {
            let git = Self::native_git();
            let authority = RepositoryBranchCreationAuthority::new(&git, &self.0)
                .expect("host branch creation authority should construct");
            DesktopRepository::new_with_authorities(
                &git,
                &self.0,
                None,
                None,
                None,
                Some(authority),
            )
            .expect("selected branch repository should construct")
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

    fn counting_delete_registry(
        repository: &DesktopRepository,
        executions: Arc<AtomicUsize>,
    ) -> Arc<ToolRegistry> {
        let authority = repository
            .deletion_authority
            .clone()
            .expect("deletion authority should be present");
        let mut registry = ToolRegistry::new();
        registry
            .register(Arc::new(CountingDeleteFileTool {
                inner: RepositoryFileDeletionTool::from_authority(authority),
                executions,
            }))
            .expect("counting deletion tool should register");
        Arc::new(registry)
    }

    fn counting_rename_registry(
        repository: &DesktopRepository,
        executions: Arc<AtomicUsize>,
    ) -> Arc<ToolRegistry> {
        let authority = repository
            .rename_authority
            .clone()
            .expect("rename authority should be present");
        let mut registry = ToolRegistry::new();
        registry
            .register(Arc::new(CountingRenameFileTool {
                inner: RepositoryFileRenameTool::from_authority(authority),
                executions,
            }))
            .expect("counting rename tool should register");
        Arc::new(registry)
    }

    fn current_delete_composition(
        state: &DesktopAppState,
        registry: Arc<ToolRegistry>,
    ) -> super::CurrentHostComposition {
        let repository = state
            .repository
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
            .expect("repository should be selected");
        let composition = desktop_tool_composition_from_registry(
            Arc::clone(&registry),
            Some(&repository),
            false,
            &[],
        )
        .expect("deletion composition should be classified");
        super::CurrentHostComposition {
            registry,
            expected_definitions: composition.expected_definitions.clone(),
            composition_identity: composition.registry.as_ref() as *const ToolRegistry as usize,
            allowed_permissions: vec![
                PermissionLevel::None,
                PermissionLevel::Read,
                PermissionLevel::Execute,
            ],
            generations: current_host_generation_tuple(state),
            repository_identity: Some(repository_context_fingerprint(&repository.root)),
            repository: Some(repository),
            tools: composition.tools.clone(),
            repository_patch_preparer: None,
            repository_multi_file_edit_preparer: None,
            repository_create_file_preparer: None,
            repository_delete_file_preparer: composition.repository_delete_file_preparer.clone(),
            repository_rename_file_preparer: composition.repository_rename_file_preparer.clone(),
        }
    }

    fn current_rename_composition(
        state: &DesktopAppState,
        registry: Arc<ToolRegistry>,
    ) -> super::CurrentHostComposition {
        let repository = state
            .repository
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
            .expect("repository should be selected");
        let composition = desktop_tool_composition_from_registry(
            Arc::clone(&registry),
            Some(&repository),
            false,
            &[],
        )
        .expect("rename composition should be classified");
        super::CurrentHostComposition {
            registry,
            expected_definitions: composition.expected_definitions.clone(),
            composition_identity: composition.registry.as_ref() as *const ToolRegistry as usize,
            allowed_permissions: vec![
                PermissionLevel::None,
                PermissionLevel::Read,
                PermissionLevel::Execute,
            ],
            generations: current_host_generation_tuple(state),
            repository_identity: Some(repository_context_fingerprint(&repository.root)),
            repository: Some(repository),
            tools: composition.tools.clone(),
            repository_patch_preparer: None,
            repository_multi_file_edit_preparer: None,
            repository_create_file_preparer: None,
            repository_delete_file_preparer: None,
            repository_rename_file_preparer: composition.repository_rename_file_preparer.clone(),
        }
    }

    async fn prepare_real_delete_ticket(
        app: &tauri::AppHandle,
        state: &DesktopAppState,
        registry: Arc<ToolRegistry>,
    ) -> PreparedDeleteFileResponse {
        state
            .host_invocation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .begin_prepare()
            .expect("deletion preparation should reserve the coordinator");
        prepare_repo_delete_file_with_current(
            HostPrepareDeleteFileRequest {
                path: "tracked.txt".to_owned(),
            },
            app,
            state,
            current_delete_composition(state, registry),
        )
        .await
        .expect("deletion preparation should succeed")
    }

    async fn prepare_real_delete_ticket_with_activity(
        app: &tauri::AppHandle,
        state: &DesktopAppState,
        registry: Arc<ToolRegistry>,
        events: &Arc<Mutex<Vec<String>>>,
        event_number: usize,
    ) -> (PreparedDeleteFileResponse, String) {
        let prepared = prepare_real_delete_ticket(app, state, registry).await;
        let events = wait_for_test_events(events, event_number)
            .await
            .expect("real deletion Prepare should emit activity");
        let activity = events[event_number - 1]
            .get("invocationId")
            .and_then(Value::as_str)
            .expect("activity ID should be present")
            .to_owned();
        (prepared, activity)
    }

    async fn authorize_test_commit(
        state: &DesktopAppState,
        repository: Arc<DesktopRepository>,
    ) -> Arc<RepositoryCommitControl> {
        *state
            .commit_identity
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(DesktopCommitIdentity {
            name: "RAH Delete Test".to_owned(),
            email: "rah-delete@example.invalid".to_owned(),
        });
        let (tool, control) = RepositoryCommitTool::compose(
            &repository.git_executable,
            &repository.root,
            "RAH Delete Test".to_owned(),
            "rah-delete@example.invalid".to_owned(),
        )
        .expect("commit capability should compose");
        let control = Arc::new(control);
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
        *state
            .commit_capability
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(DesktopCommitCapability {
            repository_generation,
            model_generation,
            identity_generation,
            _tool: Arc::new(tool),
            control: Arc::clone(&control),
        });
        let (snapshot, review) =
            desktop_repository_snapshot_with_review(&repository, Some(Arc::clone(&control)))
                .await
                .expect("commit review should be observed");
        let snapshot = install_repository_workflow(
            state,
            &repository,
            repository_generation,
            snapshot,
            review,
            identity_generation,
        );
        let StagedReviewPresentation::ReviewAvailable {
            review_id: Some(review_id),
            ..
        } = snapshot.review
        else {
            panic!("commit review should be available");
        };
        authorize_repository_commit_review(state, &review_id)
            .await
            .expect("commit review should authorize");
        assert!(control.has_pending_authorization().await);
        control
    }

    #[test]
    fn rename_file_result_classification_is_strict_and_status_only() {
        let success = ToolOutput {
            content: vec![ToolContent::Json(
                serde_json::json!({"status":"renamed_verified","uncertain":false,"path":"new.rs"}),
            )],
            is_error: false,
        };
        assert_eq!(
            classify_repository_rename_file_output(&success, "new.rs"),
            rah_tools::RepositoryRenameFileProof::ReviewedSuccess
        );
        for malformed in [
            ToolOutput {
                content: vec![ToolContent::Json(
                    serde_json::json!({"status":"renamed_verified","uncertain":false,"path":"wrong.rs"}),
                )],
                is_error: false,
            },
            ToolOutput {
                content: vec![ToolContent::Json(
                    serde_json::json!({"status":"known_no_effect","uncertain":false,"extra":true}),
                )],
                is_error: true,
            },
            ToolOutput {
                content: vec![ToolContent::Text("RAH_RAW_RENAME_OUTPUT".to_owned())],
                is_error: true,
            },
        ] {
            assert_eq!(
                classify_repository_rename_file_output(&malformed, "new.rs"),
                rah_tools::RepositoryRenameFileProof::Uncertain
            );
        }
        assert_eq!(
            super::safe_rename_file_activity_result(
                rah_tools::RepositoryRenameFileProof::ReviewedSuccess,
            )
            .content,
            vec![ToolContent::Json(serde_json::json!({
                "status": "renamed_verified"
            }))]
        );
    }

    #[tokio::test(flavor = "current_thread")]
    #[ignore = "requires the certified Windows Codex live gate"]
    async fn windows_live_desktop_hostexplicit_create_file() -> Result<(), String> {
        let fixture = TestRepository::git_repository(GitRepositoryState::Staged);
        let storage = TestRepository::new();
        let git = selected_git_executable()
            .map_err(|error| format!("Git discovery failed: {error:?}"))?;
        let target = "src/rah-hostexplicit-live-created.txt";
        let content =
            "RAH_SECRET_CREATE_FILE_LIVE_CONTENT_SENTINEL\nline-two: λ\nfinal-line-no-newline";
        let expected_bytes = content.as_bytes();
        let expected_sha256 = live_sha256(expected_bytes);
        let target_path = fixture.0.join(target);
        let parent_path = fixture.0.join("src");
        fs::create_dir(&parent_path)
            .map_err(|error| format!("fixture parent creation failed: {error}"))?;

        let require_regular_non_reparse = |path: &Path, description: &str| {
            use std::os::windows::fs::MetadataExt;
            let metadata = fs::symlink_metadata(path)
                .map_err(|error| format!("{description} metadata failed: {error}"))?;
            if !metadata.is_file()
                || metadata.file_type().is_symlink()
                || metadata.file_attributes() & 0x400 != 0
            {
                return Err(format!(
                    "{description} was not an ordinary non-reparse file"
                ));
            }
            Ok::<(), String>(())
        };
        let require_ordinary_directory = |path: &Path, description: &str| {
            use std::os::windows::fs::MetadataExt;
            let metadata = fs::symlink_metadata(path)
                .map_err(|error| format!("{description} metadata failed: {error}"))?;
            if !metadata.is_dir()
                || metadata.file_type().is_symlink()
                || metadata.file_attributes() & 0x400 != 0
            {
                return Err(format!(
                    "{description} was not an ordinary non-reparse directory"
                ));
            }
            Ok::<(), String>(())
        };
        require_ordinary_directory(&parent_path, "target parent")?;
        let sparse_checkout = {
            let output = Command::new(&git)
                .args(["config", "--get", "core.sparseCheckout"])
                .current_dir(&fixture.0)
                .output()
                .map_err(|error| format!("sparse-checkout observation failed: {error}"))?;
            if !output.status.success() && output.status.code() != Some(1) {
                return Err(format!(
                    "sparse-checkout observation returned status {}",
                    output.status
                ));
            }
            String::from_utf8(output.stdout)
                .map_err(|error| format!("sparse-checkout observation was not UTF-8: {error}"))?
        };
        if target_path.exists()
            || live_git_text(&git, &fixture.0, &["ls-tree", "-r", "--name-only", "HEAD"])?
                .lines()
                .any(|line| line == target)
            || !live_git_text(&git, &fixture.0, &["ls-files", "-s", "--", target])?.is_empty()
            || live_git_exit_success(&git, &fixture.0, &["check-ignore", "-q", "--", target])?
            || sparse_checkout.trim().eq_ignore_ascii_case("true")
        {
            return Err("live target was not a fresh supported untracked path".to_owned());
        }

        let fixture_before_git = live_git_state(&git, &fixture.0, "__rah_no_excluded_branch__")?;
        if fixture_before_git.status.lines().count() != 1
            || !fixture_before_git
                .status
                .lines()
                .any(|line| line.starts_with("M  tracked.txt"))
            || fixture_before_git.index_semantics.lines().count() != 2
            || !live_git_exit_success(&git, &fixture.0, &["diff", "--quiet"])?
            || live_git_exit_success(&git, &fixture.0, &["diff", "--cached", "--quiet"])?
        {
            return Err("protected staged fixture baseline was not established".to_owned());
        }

        clear_live_test_create_file_tool_executions(&fixture.0);
        clear_live_test_create_file_native_attempts(&fixture.0);
        let branch_authority = RepositoryBranchCreationAuthority::new(&git, &fixture.0)
            .map_err(|error| format!("branch authority construction failed: {error}"))?;
        let repository = DesktopRepository::new_with_authorities(
            &git,
            &fixture.0,
            None,
            None,
            None,
            Some(branch_authority),
        )
        .map_err(|error| format!("Desktop repository construction failed: {error:?}"))?;
        let app = tauri::Builder::default()
            .any_thread()
            .manage(DesktopAppState::new(storage.0.clone()))
            .build(tauri::generate_context!())
            .map_err(|error| format!("Desktop test app construction failed: {error}"))?;
        let host_activity = listen_for_test_event(app.handle(), "host_activity_event");
        let refresh_events = listen_for_test_event(app.handle(), "repository_snapshot_refresh");
        let chat_events = listen_for_test_event(app.handle(), "chat_event");
        let model_activity = listen_for_test_event(app.handle(), "activity_event");
        replace_selected_repository(app.state::<DesktopAppState>().inner(), repository);
        set_commit_identity(
            app.handle().clone(),
            app.state(),
            "RAH Create-File HostExplicit Live Test".to_owned(),
            "rah-create-file@example.invalid".to_owned(),
        )
        .map_err(|error| format!("Desktop commit identity setup failed: {error:?}"))?;
        connect_codex(app.state())
            .await
            .map_err(|error| format!("production Desktop connection failed: {error:?}"))?;

        let connected_snapshot = get_effective_authority_snapshot(app.state());
        let eligible = connected_snapshot
            .effective_tools
            .iter()
            .filter(|tool| tool.host_invocation.eligible)
            .map(|tool| tool.public_tool_name.as_str())
            .collect::<BTreeSet<_>>();
        let expected_eligible = [
            "fs.read",
            "repo.file-info",
            "repo.status",
            "repo.diff",
            "repo.diff-staged",
            "repo.create-branch",
            "repo.patch",
            "repo.edit-files",
            "repo.create-file",
        ]
        .into_iter()
        .collect::<BTreeSet<_>>();
        let create_file_tool = connected_snapshot
            .effective_tools
            .iter()
            .find(|tool| tool.public_tool_name == "repo.create-file")
            .ok_or_else(|| "repo.create-file was not advertised".to_owned())?;
        let external_effective = connected_snapshot
            .effective_tools
            .iter()
            .filter(|tool| {
                matches!(
                    tool.source_kind,
                    SourceKind::Mcp | SourceKind::ProcessPlugin
                )
            })
            .count();
        let provider_activation_present = app
            .state::<DesktopAppState>()
            .provider_activation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_some();
        let trusted_profile_present = app
            .state::<DesktopAppState>()
            .trusted_profile
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_some();
        if connected_snapshot.status != SnapshotStatus::ConnectedCurrent
            || eligible != expected_eligible
            || !create_file_tool.host_invocation.eligible
            || create_file_tool.host_invocation.kind != Some(HostInvocationKind::RepoCreateFile)
            || create_file_tool.effect_class != EffectClass::RepositoryMutation
            || create_file_tool.authority_category != AuthorityCategory::RepositoryFileCreation
            || create_file_tool.permission != PermissionLevel::Execute
            || !create_file_tool.repository_bound
            || external_effective != 0
            || connected_snapshot.configured.configured_provider_count != 0
            || provider_activation_present
            || trusted_profile_present
        {
            shutdown_live_state(app.state::<DesktopAppState>().inner()).await;
            return Err("connected-current create-file composition was not exact".to_owned());
        }
        let commit_control = app
            .state::<DesktopAppState>()
            .commit_capability
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .map(|capability| Arc::clone(&capability.control));
        reset_startup_activation_counters();
        if startup_activation_snapshot() != StartupActivationCounters::default() {
            shutdown_live_state(app.state::<DesktopAppState>().inner()).await;
            return Err("live operation baseline counters were not zero".to_owned());
        }

        let reviewed = refresh_repository_workflow(app.state::<DesktopAppState>().inner())
            .await
            .map_err(|error| format!("review snapshot failed: {error:?}"))?;
        let review_id = match reviewed.review {
            StagedReviewPresentation::ReviewAvailable {
                review_id: Some(review_id),
                can_authorize: true,
                authorization_state: CommitAuthorizationPresentation::ReadyToAuthorize,
            } => review_id,
            other => {
                shutdown_live_state(app.state::<DesktopAppState>().inner()).await;
                return Err(format!(
                    "fixture did not expose a reviewed Commit authorization: {other:?}"
                ));
            }
        };
        let authorization =
            authorize_repository_commit_review(app.state::<DesktopAppState>().inner(), &review_id)
                .await
                .map_err(|error| format!("review authorization failed: {error:?}"))?;
        let commit_pending = match &commit_control {
            Some(control) => control.has_pending_authorization().await,
            None => false,
        };
        if authorization.authorization_state != CommitAuthorizationPresentation::AuthorizedPending
            || !commit_pending
        {
            shutdown_live_state(app.state::<DesktopAppState>().inner()).await;
            return Err("reviewed Commit authorization was not pending".to_owned());
        }
        let before_directory_entries = live_directory_entries(&fixture.0)?;
        let before_parent_entries = live_directory_entries(&parent_path)?;
        let before_index = fs::read(fixture.0.join(".git").join("index"))
            .map_err(|error| format!("Git index baseline read failed: {error}"))?;
        let before_git = live_git_state(&git, &fixture.0, "__rah_no_excluded_branch__")?;
        let before_head = before_git.head_oid.clone();
        let before_branch = before_git.current_branch.clone();
        let before_refs = before_git.all_refs.clone();
        let before_generations =
            current_host_generation_tuple(app.state::<DesktopAppState>().inner());
        let before_namespace = app.state::<DesktopAppState>().persistence_namespace();
        let before_conversation = serde_json::to_value(
            app.state::<DesktopAppState>()
                .persistence
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .presentation(),
        )
        .map_err(|error| format!("conversation baseline serialization failed: {error}"))?;

        let request = HostPrepareCreateFileRequest {
            path: target.to_owned(),
            content: content.to_owned(),
        };
        let prepared = host_prepare_repo_create_file(request, app.handle().clone(), app.state())
            .await
            .map_err(|error| format!("production repo.create-file Prepare failed: {error:?}"))?;
        let prepare_events = wait_for_test_events(&host_activity.0, 1).await?;
        require_host_event(&prepare_events[0], "prepared", "repo.create-file")?;
        let activity_id = prepare_events[0]
            .get("invocationId")
            .and_then(Value::as_str)
            .ok_or_else(|| "Prepared activity omitted invocationId".to_owned())?;
        let prepare_activity = serde_json::to_string(&prepare_events[0])
            .map_err(|error| format!("Prepared activity serialization failed: {error}"))?;
        let native_target = target_path.to_string_lossy().into_owned();
        let native_parent = parent_path.to_string_lossy().into_owned();
        let forbidden_prepare_values = [
            prepared.ticket_id.as_str(),
            "RAH_SECRET_CREATE_FILE_LIVE_CONTENT_SENTINEL",
            content,
            expected_sha256.as_str(),
            target,
            native_target.as_str(),
            native_parent.as_str(),
        ];
        if prepare_events[0].get("review").is_some()
            || prepare_events[0].get("result").is_some()
            || activity_id == prepared.ticket_id
            || forbidden_prepare_values
                .iter()
                .any(|value| !value.is_empty() && prepare_activity.contains(value))
        {
            return Err("generic Prepared activity was not value-level private".to_owned());
        }
        let expected_escaped =
            "RAH_SECRET_CREATE_FILE_LIVE_CONTENT_SENTINEL\\nline-two: λ\\nfinal-line-no-newline";
        let facts = prepared.review.content_facts();
        if prepared.ticket_id.is_empty()
            || prepared.ticket_id.len() > 256
            || prepared.ticket_id == activity_id
            || prepared.review.operation() != "repo.create-file"
            || prepared.review.target_count() != 1
            || prepared.review.path() != target
            || prepared.review.parent_path() != "src"
            || prepared.review.existing_parent()
                != "safe existing ordinary directory; no parent creation"
            || prepared.review.target_worktree() != "absent"
            || prepared.review.target_head() != "absent"
            || prepared.review.target_index()
                != "absent from every index stage, including intent-to-add"
            || prepared.review.expected_effect() != "one new untracked regular non-executable file"
            || prepared.review.content_escaped() != expected_escaped
            || prepared.review.content_byte_length() != expected_bytes.len()
            || prepared.review.content_sha256() != expected_sha256
            || prepared.review.bom() != rah_tools::RepositoryCreateFileBomState::Absent
            || facts.carriage_returns != 0
            || facts.line_feeds != 2
            || facts.crlf_pairs != 0
            || facts.final_eof != "no_final_newline"
            || facts.control_characters != 2
            || facts.format_characters != 0
            || prepared.review.file_intent() != "regular non-executable file"
            || !prepared
                .review
                .creation_semantics()
                .contains(&"exclusive create-new")
            || !prepared.review.creation_semantics().contains(&"no clobber")
            || !prepared
                .review
                .creation_semantics()
                .contains(&"no overwrite")
            || !prepared
                .review
                .non_effects()
                .contains(&"no parent creation")
            || !prepared.review.non_effects().contains(&"no Stage")
            || !prepared.review.non_effects().contains(&"no Commit")
            || !prepared
                .review
                .warnings()
                .contains(&"full content creation is not crash-atomic")
            || !prepared
                .review
                .warnings()
                .contains(&"write failure may retain an empty or partial file")
            || !prepared.review.warnings().contains(&"no automatic cleanup")
            || !prepared.review.warnings().contains(&"no retry or replay")
        {
            return Err("direct Prepared review was incomplete or altered".to_owned());
        }
        let prepare_index_unchanged = fs::read(fixture.0.join(".git").join("index"))
            .map_err(|error| format!("Prepare index read failed: {error}"))?
            == before_index;
        let prepare_git_unchanged =
            live_git_state(&git, &fixture.0, "__rah_no_excluded_branch__")? == before_git;
        let prepare_root_unchanged =
            live_directory_entries(&fixture.0)? == before_directory_entries;
        let prepare_parent_unchanged =
            live_directory_entries(&parent_path)? == before_parent_entries;
        let prepare_conversation_unchanged = serde_json::to_value(
            app.state::<DesktopAppState>()
                .persistence
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .presentation(),
        )
        .map_err(|error| format!("Prepare conversation serialization failed: {error}"))?
            == before_conversation;
        let prepare_coordinator = app
            .state::<DesktopAppState>()
            .host_invocation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .state();
        if !prepare_index_unchanged
            || !prepare_git_unchanged
            || !prepare_root_unchanged
            || !prepare_parent_unchanged
            || target_path.exists()
            || live_test_create_file_tool_executions(&fixture.0) != 0
            || live_test_create_file_native_attempts(&fixture.0) != 0
            || current_host_generation_tuple(app.state::<DesktopAppState>().inner())
                != before_generations
            || app.state::<DesktopAppState>().persistence_namespace() != before_namespace
            || !prepare_conversation_unchanged
            || prepare_coordinator != CoordinatorState::HostPrepared
        {
            return Err(format!(
                "Prepare was not zero effect: index={prepare_index_unchanged} git={prepare_git_unchanged} root={prepare_root_unchanged} parent={prepare_parent_unchanged} target_absent={} tool={} native={} generations={} namespace={} conversation={} coordinator={prepare_coordinator:?}",
                !target_path.exists(),
                live_test_create_file_tool_executions(&fixture.0),
                live_test_create_file_native_attempts(&fixture.0),
                current_host_generation_tuple(app.state::<DesktopAppState>().inner())
                    == before_generations,
                app.state::<DesktopAppState>().persistence_namespace() == before_namespace,
                prepare_conversation_unchanged,
            ));
        }
        let commit_pending_after_prepare = match &commit_control {
            Some(control) => control.has_pending_authorization().await,
            None => false,
        };
        if !commit_pending_after_prepare {
            return Err("Prepare invalidated reviewed Commit authorization".to_owned());
        }
        println!("RAH_CREATE_FILE_HOSTEXPLICIT_PREPARE_TOOL_EXECUTIONS=0");
        println!("RAH_CREATE_FILE_HOSTEXPLICIT_PREPARE_NATIVE_ATTEMPTS=0");
        println!("RAH_CREATE_FILE_HOSTEXPLICIT_SOURCE_SHA256={expected_sha256}");
        println!(
            "RAH_CREATE_FILE_HOSTEXPLICIT_SOURCE_LENGTH={}",
            expected_bytes.len()
        );

        let activity_id = activity_id.to_owned();
        let confirmed = host_confirm_tool_invocation(
            HostConfirmRequest {
                ticket_id: prepared.ticket_id.clone(),
            },
            app.handle().clone(),
            app.state(),
        )
        .await
        .map_err(|error| format!("production repo.create-file Confirm failed: {error:?}"))?;
        if confirmed.invocation_id != activity_id {
            return Err("Confirm changed the independent activity ID".to_owned());
        }
        let events = wait_for_test_events(&host_activity.0, 3).await?;
        if events.len() != 3
            || require_host_event(&events[1], "started", "repo.create-file").is_err()
            || require_host_event(&events[2], "tool_completed", "repo.create-file").is_err()
        {
            return Err("HostExplicit lifecycle was not prepared/started/completed".to_owned());
        }
        let terminal_output = event_tool_output(&events[2])?;
        let [ToolContent::Json(terminal_result)] = terminal_output.content.as_slice() else {
            return Err("terminal result was not one JSON status object".to_owned());
        };
        if terminal_output.is_error || terminal_result != &serde_json::json!({"status": "ok"}) {
            return Err("terminal public result was not sanitized status-only ok".to_owned());
        }
        for event in &events {
            if event.get("invocationId").and_then(Value::as_str) != Some(activity_id.as_str()) {
                return Err("HostExplicit activity correlation ID changed".to_owned());
            }
            let serialized = serde_json::to_string(event)
                .map_err(|error| format!("HostActivity serialization failed: {error}"))?;
            if event.get("review").is_some()
                || forbidden_prepare_values
                    .iter()
                    .any(|value| !value.is_empty() && serialized.contains(value))
            {
                return Err("generic HostActivity privacy boundary failed".to_owned());
            }
        }
        if live_test_create_file_tool_executions(&fixture.0) != 1
            || live_test_create_file_native_attempts(&fixture.0) != 1
        {
            return Err(
                "Confirm was not exactly one Tool and one native CREATE_NEW attempt".to_owned(),
            );
        }
        require_regular_non_reparse(&target_path, "created target")?;
        let actual_bytes = fs::read(&target_path)
            .map_err(|error| format!("created target read failed: {error}"))?;
        let target_identity = live_file_identity(&target_path)?;
        if actual_bytes != expected_bytes
            || actual_bytes.len() != expected_bytes.len()
            || live_sha256(&actual_bytes) != expected_sha256
        {
            return Err("created target bytes, length, or SHA-256 did not match".to_owned());
        }

        let after_git = live_git_state(&git, &fixture.0, "__rah_no_excluded_branch__")?;
        let mut expected_status = before_git
            .status
            .lines()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        expected_status.push(format!("?? {target}"));
        expected_status.sort();
        let mut actual_status = live_git_text(
            &git,
            &fixture.0,
            &["status", "--porcelain=v1", "--untracked-files=all"],
        )?
        .lines()
        .map(str::to_owned)
        .collect::<Vec<_>>();
        actual_status.sort();
        let post_index_unchanged = fs::read(fixture.0.join(".git").join("index"))
            .map_err(|error| format!("post-effect index read failed: {error}"))?
            == before_index;
        let post_root_unchanged = live_directory_entries(&fixture.0)? == before_directory_entries;
        let mut expected_parent_entries = before_parent_entries.clone();
        expected_parent_entries.insert("rah-hostexplicit-live-created.txt".to_owned());
        let post_parent_expected = live_directory_entries(&parent_path)? == expected_parent_entries;
        let post_git_protected = after_git.index_semantics == before_git.index_semantics
            && after_git.head_oid == before_head
            && after_git.symbolic_head == before_git.symbolic_head
            && after_git.current_branch == before_branch
            && after_git.local_heads == before_git.local_heads
            && after_git.tags_and_remotes == before_git.tags_and_remotes
            && after_git.all_refs == before_refs
            && after_git.raw_worktree_diff == before_git.raw_worktree_diff
            && after_git.raw_staged_diff == before_git.raw_staged_diff;
        if actual_status != expected_status
            || !post_git_protected
            || !post_index_unchanged
            || !post_root_unchanged
            || !post_parent_expected
        {
            return Err(format!(
                "create-file protected-state proof failed: status={} git={} index={} root={} parent={} expected_status={expected_status:?} actual_status={actual_status:?}",
                actual_status == expected_status,
                post_git_protected,
                post_index_unchanged,
                post_root_unchanged,
                post_parent_expected,
            ));
        }
        let refresh = wait_for_test_events(&refresh_events.0, 1).await?;
        if refresh.len() != 1 {
            return Err("descriptive repository refresh was not emitted exactly once".to_owned());
        }
        let refreshed = repository_snapshot(app.state())
            .await
            .map_err(|error| format!("refreshed repository snapshot failed: {error:?}"))?;
        if refreshed.status_entries.is_empty() {
            return Err("descriptive refresh did not expose repository status".to_owned());
        }
        let commit_authorization_pending = match &commit_control {
            Some(control) => control.has_pending_authorization().await,
            None => false,
        };
        if commit_authorization_pending {
            return Err("Started recreated reviewed Commit authorization".to_owned());
        }
        println!("RAH_CREATE_FILE_HOSTEXPLICIT_COMMIT_AUTHORIZATION_PENDING=0");
        if current_host_generation_tuple(app.state::<DesktopAppState>().inner())
            != before_generations
            || app.state::<DesktopAppState>().persistence_namespace() != before_namespace
            || serde_json::to_value(
                app.state::<DesktopAppState>()
                    .persistence
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .presentation(),
            )
            .map_err(|error| format!("terminal conversation serialization failed: {error}"))?
                != before_conversation
        {
            return Err(
                "terminal refresh changed currentness or conversation persistence".to_owned(),
            );
        }
        let duplicate = host_confirm_tool_invocation(
            HostConfirmRequest {
                ticket_id: prepared.ticket_id.clone(),
            },
            app.handle().clone(),
            app.state(),
        )
        .await;
        let activity_as_confirm = host_confirm_tool_invocation(
            HostConfirmRequest {
                ticket_id: activity_id.clone(),
            },
            app.handle().clone(),
            app.state(),
        )
        .await;
        let activity_as_cancel = host_cancel_tool_invocation(
            HostConfirmRequest {
                ticket_id: activity_id.clone(),
            },
            app.handle().clone(),
            app.state(),
        );
        if duplicate != Err(FrontendError::HostInvocationTicketInvalid)
            || activity_as_confirm != Err(FrontendError::HostInvocationTicketInvalid)
            || activity_as_cancel != Err(FrontendError::HostInvocationTicketInvalid)
            || live_test_create_file_tool_executions(&fixture.0) != 1
            || live_test_create_file_native_attempts(&fixture.0) != 1
            || fs::read(&target_path)
                .map_err(|error| format!("duplicate target read failed: {error}"))?
                != expected_bytes
        {
            return Err("duplicate or activity-ID authorization caused a second effect".to_owned());
        }
        let final_coordinator = app
            .state::<DesktopAppState>()
            .host_invocation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .state();
        let final_chat = *app
            .state::<DesktopAppState>()
            .chat
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if final_coordinator != CoordinatorState::Idle
            || final_chat != ChatState::Idle
            || !app
                .state::<DesktopAppState>()
                .active_chat
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .is_none()
            || !chat_events.0.lock().unwrap().is_empty()
            || !model_activity.0.lock().unwrap().is_empty()
        {
            return Err("HostExplicit creation left chat/coordinator/model activity".to_owned());
        }
        let operation_counters = startup_activation_snapshot();
        if operation_counters != StartupActivationCounters::default() {
            return Err(format!(
                "unexpected model/runtime operation counters: {operation_counters:?}"
            ));
        }
        println!("RAH_CREATE_FILE_HOSTEXPLICIT_CONFIRM_TOOL_EXECUTIONS=1");
        println!("RAH_CREATE_FILE_HOSTEXPLICIT_CONFIRM_NATIVE_ATTEMPTS=1");
        println!("RAH_CREATE_FILE_HOSTEXPLICIT_TARGET_IDENTITY={target_identity:?}");
        println!("RAH_CREATE_FILE_HOSTEXPLICIT_TARGET_SHA256={expected_sha256}");
        println!(
            "RAH_CREATE_FILE_HOSTEXPLICIT_TARGET_LENGTH={}",
            expected_bytes.len()
        );
        println!("RAH_CREATE_FILE_HOSTEXPLICIT_MODEL_LIFECYCLE=0");
        println!("RAH_CREATE_FILE_HOSTEXPLICIT_MCP=0");
        println!("RAH_CREATE_FILE_HOSTEXPLICIT_PROCESS_PLUGINS=0");
        println!("RAH_CREATE_FILE_HOSTEXPLICIT_GENERATIONS={before_generations:?}");
        println!("RAH_CREATE_FILE_HOSTEXPLICIT_COMMIT_INVALIDATED=1");
        app.unlisten(host_activity.1);
        app.unlisten(refresh_events.1);
        app.unlisten(chat_events.1);
        app.unlisten(model_activity.1);
        shutdown_live_state(app.state::<DesktopAppState>().inner()).await;
        clear_live_test_create_file_tool_executions(&fixture.0);
        clear_live_test_create_file_native_attempts(&fixture.0);
        println!("RAH_CREATE_FILE_HOSTEXPLICIT_LIVE_OK");
        Ok(())
    }

    #[tokio::test(flavor = "current_thread")]
    #[ignore = "requires the certified Windows Codex live gate"]
    async fn windows_live_desktop_hostexplicit_delete_file() -> Result<(), String> {
        let fixture = TestRepository::new();
        let storage = TestRepository::new();
        let git = selected_git_executable()
            .map_err(|error| format!("Git discovery failed: {error:?}"))?;
        let target = "src/rah-hostexplicit-live-delete.txt";
        let unrelated = "unrelated-staged.txt";
        let content =
            "RAH_DELETE_FILE_HOSTEXPLICIT_LIVE_SOURCE_SENTINEL\nline-two\nfinal-line-no-newline";
        let expected_bytes = content.as_bytes();
        let expected_sha256 = live_sha256(expected_bytes);
        let target_path = fixture.0.join(target);
        let parent_path = fixture.0.join("src");
        let unrelated_path = fixture.0.join(unrelated);

        fs::remove_dir_all(fixture.0.join(".git"))
            .map_err(|error| format!("placeholder metadata cleanup failed: {error}"))?;
        fs::remove_file(fixture.0.join("inside.txt"))
            .map_err(|error| format!("placeholder file cleanup failed: {error}"))?;
        let run_git = |arguments: &[&str]| -> Result<(), String> {
            let output = Command::new(&git)
                .args(arguments)
                .current_dir(&fixture.0)
                .output()
                .map_err(|error| format!("Git fixture command failed to start: {error}"))?;
            if !output.status.success() {
                return Err(format!(
                    "Git fixture command failed with status {}",
                    output.status
                ));
            }
            Ok(())
        };
        run_git(&["init", "--quiet"])?;
        run_git(&["config", "user.email", "rah-delete-live@example.invalid"])?;
        run_git(&["config", "user.name", "RAH Delete Live Test"])?;
        run_git(&["config", "core.autocrlf", "false"])?;
        fs::create_dir_all(&parent_path)
            .map_err(|error| format!("target parent creation failed: {error}"))?;
        fs::write(&target_path, expected_bytes)
            .map_err(|error| format!("target fixture write failed: {error}"))?;
        fs::write(&unrelated_path, b"unrelated committed\n")
            .map_err(|error| format!("unrelated fixture write failed: {error}"))?;
        run_git(&["add", target, unrelated])?;
        run_git(&["commit", "--quiet", "-m", "live deletion fixture"])?;
        fs::write(&unrelated_path, b"unrelated staged change\n")
            .map_err(|error| format!("unrelated staged change failed: {error}"))?;
        run_git(&["add", unrelated])?;

        let require_regular_non_reparse = |path: &Path, description: &str| {
            use std::os::windows::fs::MetadataExt;
            let metadata = fs::symlink_metadata(path)
                .map_err(|error| format!("{description} metadata failed: {error}"))?;
            if !metadata.is_file()
                || metadata.file_type().is_symlink()
                || metadata.file_attributes() & 0x400 != 0
            {
                return Err(format!(
                    "{description} was not an ordinary non-reparse file"
                ));
            }
            Ok::<(), String>(())
        };
        let require_link_count_one = |path: &Path| -> Result<(), String> {
            let file = fs::File::open(path)
                .map_err(|error| format!("target link-count handle failed: {error}"))?;
            let mut information = std::mem::MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::zeroed();
            let result = unsafe {
                GetFileInformationByHandle(file.as_raw_handle(), information.as_mut_ptr())
            };
            if result == 0 {
                return Err("target link-count observation failed".to_owned());
            }
            let information = unsafe { information.assume_init() };
            if information.nNumberOfLinks != 1 {
                return Err("target link count was not one".to_owned());
            }
            Ok(())
        };
        require_regular_non_reparse(&target_path, "target")?;
        require_link_count_one(&target_path)?;
        if expected_bytes.len() > 65536
            || std::str::from_utf8(expected_bytes).is_err()
            || expected_bytes.contains(&0)
        {
            return Err("live target source did not meet reviewed bounds".to_owned());
        }
        let target_stage = live_git_text(&git, &fixture.0, &["ls-files", "--stage", "--", target])?;
        if !target_stage.starts_with("100644 ") || !target_stage.contains(target) {
            return Err("live target was not mode 100644 in the stage-0 index".to_owned());
        }
        let before_target_bytes = fs::read(&target_path)
            .map_err(|error| format!("target baseline read failed: {error}"))?;
        let before_target_identity = live_file_identity(&target_path)?;
        let sparse_checkout = live_git_text(
            &git,
            &fixture.0,
            &["config", "--get", "core.sparseCheckout"],
        )
        .or_else(|error| {
            if error.contains("status") {
                Ok(String::new())
            } else {
                Err(error)
            }
        })?;
        if sparse_checkout.trim().eq_ignore_ascii_case("true") {
            return Err("live fixture unexpectedly enabled sparse checkout".to_owned());
        }

        clear_live_test_delete_file_tool_executions(&fixture.0);
        clear_live_test_delete_file_native_attempts(&fixture.0);
        let branch_authority = RepositoryBranchCreationAuthority::new(&git, &fixture.0)
            .map_err(|error| format!("branch authority construction failed: {error}"))?;
        let deletion_authority = RepositoryFileDeletionAuthority::new(&git, &fixture.0)
            .map_err(|error| format!("deletion authority construction failed: {error}"))?;
        let repository = DesktopRepository::new_with_authorities(
            &git,
            &fixture.0,
            None,
            Some(deletion_authority),
            None,
            Some(branch_authority),
        )
        .map_err(|error| format!("Desktop repository construction failed: {error:?}"))?;
        let app = tauri::Builder::default()
            .any_thread()
            .manage(DesktopAppState::new(storage.0.clone()))
            .build(tauri::generate_context!())
            .map_err(|error| format!("Desktop test app construction failed: {error}"))?;
        let host_activity = listen_for_test_event(app.handle(), "host_activity_event");
        let refresh_events = listen_for_test_event(app.handle(), "repository_snapshot_refresh");
        let chat_events = listen_for_test_event(app.handle(), "chat_event");
        let model_activity = listen_for_test_event(app.handle(), "activity_event");
        replace_selected_repository(app.state::<DesktopAppState>().inner(), repository);
        set_commit_identity(
            app.handle().clone(),
            app.state(),
            "RAH Delete-File HostExplicit Live Test".to_owned(),
            "rah-delete-file@example.invalid".to_owned(),
        )
        .map_err(|error| format!("Desktop commit identity setup failed: {error:?}"))?;
        connect_codex(app.state())
            .await
            .map_err(|error| format!("production Desktop connection failed: {error:?}"))?;

        let connected_snapshot = get_effective_authority_snapshot(app.state());
        let eligible = connected_snapshot
            .effective_tools
            .iter()
            .filter(|tool| tool.host_invocation.eligible)
            .map(|tool| tool.public_tool_name.as_str())
            .collect::<BTreeSet<_>>();
        let expected_eligible = [
            "fs.read",
            "repo.file-info",
            "repo.status",
            "repo.diff",
            "repo.diff-staged",
            "repo.create-branch",
            "repo.patch",
            "repo.edit-files",
            "repo.create-file",
            "repo.delete-file",
        ]
        .into_iter()
        .collect::<BTreeSet<_>>();
        let delete_tool = connected_snapshot
            .effective_tools
            .iter()
            .find(|tool| tool.public_tool_name == "repo.delete-file")
            .ok_or_else(|| "repo.delete-file was not advertised".to_owned())?;
        let ineligible_names = ["repo.rename-file", "repo.create-directory", "repo.commit"];
        let ineligible_was_eligible = connected_snapshot.effective_tools.iter().any(|tool| {
            ineligible_names.contains(&tool.public_tool_name.as_str())
                && tool.host_invocation.eligible
        });
        let external_effective = connected_snapshot
            .effective_tools
            .iter()
            .filter(|tool| {
                matches!(
                    tool.source_kind,
                    SourceKind::Mcp | SourceKind::ProcessPlugin
                )
            })
            .count();
        let provider_activation_present = app
            .state::<DesktopAppState>()
            .provider_activation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_some();
        let trusted_profile_present = app
            .state::<DesktopAppState>()
            .trusted_profile
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_some();
        if connected_snapshot.status != SnapshotStatus::ConnectedCurrent
            || eligible != expected_eligible
            || ineligible_was_eligible
            || !delete_tool.host_invocation.eligible
            || delete_tool.host_invocation.kind != Some(HostInvocationKind::RepoDeleteFile)
            || delete_tool.source_kind != SourceKind::RepositoryHost
            || delete_tool.source_label != "desktop_repository"
            || delete_tool.effect_class != EffectClass::RepositoryMutation
            || delete_tool.authority_category != AuthorityCategory::RepositoryFileDeletion
            || delete_tool.permission != PermissionLevel::Execute
            || !delete_tool.repository_bound
            || external_effective != 0
            || connected_snapshot.configured.configured_provider_count != 0
            || provider_activation_present
            || trusted_profile_present
        {
            shutdown_live_state(app.state::<DesktopAppState>().inner()).await;
            return Err(format!(
                "connected-current deletion composition was not exact: status={:?} eligible={eligible:?} ineligible_was_eligible={ineligible_was_eligible} delete_eligible={} delete_kind={:?} delete_source={:?} delete_label={} delete_effect={:?} delete_category={:?} delete_permission={:?} delete_repository_bound={} external={} providers={} provider_activation={} trusted_profile={}",
                connected_snapshot.status,
                delete_tool.host_invocation.eligible,
                delete_tool.host_invocation.kind,
                delete_tool.source_kind,
                delete_tool.source_label,
                delete_tool.effect_class,
                delete_tool.authority_category,
                delete_tool.permission,
                delete_tool.repository_bound,
                external_effective,
                connected_snapshot.configured.configured_provider_count,
                provider_activation_present,
                trusted_profile_present,
            ));
        }
        let commit_control = app
            .state::<DesktopAppState>()
            .commit_capability
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .map(|capability| Arc::clone(&capability.control))
            .ok_or_else(|| "reviewed Commit control was not composed".to_owned())?;
        reset_startup_activation_counters();
        if startup_activation_snapshot() != StartupActivationCounters::default()
            || !chat_events.0.lock().unwrap().is_empty()
            || !model_activity.0.lock().unwrap().is_empty()
        {
            shutdown_live_state(app.state::<DesktopAppState>().inner()).await;
            return Err("live operation baseline included model lifecycle activity".to_owned());
        }

        let reviewed = refresh_repository_workflow(app.state::<DesktopAppState>().inner())
            .await
            .map_err(|error| format!("review snapshot failed: {error:?}"))?;
        let review_id = match reviewed.review {
            StagedReviewPresentation::ReviewAvailable {
                review_id: Some(review_id),
                can_authorize: true,
                authorization_state: CommitAuthorizationPresentation::ReadyToAuthorize,
            } => review_id,
            other => {
                shutdown_live_state(app.state::<DesktopAppState>().inner()).await;
                return Err(format!(
                    "fixture did not expose a reviewed Commit authorization: {other:?}"
                ));
            }
        };
        let authorization =
            authorize_repository_commit_review(app.state::<DesktopAppState>().inner(), &review_id)
                .await
                .map_err(|error| format!("review authorization failed: {error:?}"))?;
        if authorization.authorization_state != CommitAuthorizationPresentation::AuthorizedPending
            || !commit_control.has_pending_authorization().await
        {
            shutdown_live_state(app.state::<DesktopAppState>().inner()).await;
            return Err("reviewed Commit authorization was not pending".to_owned());
        }

        let before_directory_entries = live_directory_entries(&fixture.0)?;
        let before_parent_entries = live_directory_entries(&parent_path)?;
        let before_index = fs::read(fixture.0.join(".git").join("index"))
            .map_err(|error| format!("Git index baseline read failed: {error}"))?;
        let before_cached_binary =
            live_git_text(&git, &fixture.0, &["diff", "--cached", "--binary"])?;
        let before_git = live_git_state(&git, &fixture.0, "__rah_no_excluded_branch__")?;
        if !before_git
            .status
            .lines()
            .any(|line| line == format!("M  {unrelated}").as_str())
            || before_git.status.lines().any(|line| line.contains(target))
            || !before_git.index_semantics.contains(target)
            || !before_cached_binary.contains(unrelated)
        {
            return Err("protected staged fixture baseline was not established".to_owned());
        }
        let before_head = before_git.head_oid.clone();
        let before_branch = before_git.current_branch.clone();
        let before_refs = before_git.all_refs.clone();
        let before_generations =
            current_host_generation_tuple(app.state::<DesktopAppState>().inner());
        let before_namespace = app.state::<DesktopAppState>().persistence_namespace();
        let before_conversation = serde_json::to_value(
            app.state::<DesktopAppState>()
                .persistence
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .presentation(),
        )
        .map_err(|error| format!("conversation baseline serialization failed: {error}"))?;
        if before_git.raw_staged_diff
            != live_git_text(&git, &fixture.0, &["diff", "--cached", "--raw"])?
        {
            return Err("raw staged and binary staged baselines disagreed".to_owned());
        }

        let prepared = host_prepare_repo_delete_file(
            HostPrepareDeleteFileRequest {
                path: target.to_owned(),
            },
            app.handle().clone(),
            app.state(),
        )
        .await
        .map_err(|error| format!("production repo.delete-file Prepare failed: {error:?}"))?;
        let prepare_events = wait_for_test_events(&host_activity.0, 1).await?;
        require_host_event(&prepare_events[0], "prepared", "repo.delete-file")?;
        let activity_id = prepare_events[0]
            .get("invocationId")
            .and_then(Value::as_str)
            .ok_or_else(|| "Prepared activity omitted invocationId".to_owned())?
            .to_owned();
        let prepare_activity = serde_json::to_string(&prepare_events[0])
            .map_err(|error| format!("Prepared activity serialization failed: {error}"))?;
        let native_target = target_path.to_string_lossy().into_owned();
        let native_repository = fixture.0.to_string_lossy().into_owned();
        let escaped_source =
            "RAH_DELETE_FILE_HOSTEXPLICIT_LIVE_SOURCE_SENTINEL\\nline-two\\nfinal-line-no-newline";
        let raw_tool_input = serde_json::to_string(&serde_json::json!({
            "path": target,
            "expected_file_sha256": expected_sha256,
            "expected_file_byte_length": expected_bytes.len(),
        }))
        .map_err(|error| format!("Tool input serialization failed: {error}"))?;
        let direct_review = serde_json::to_string(&prepared.review)
            .map_err(|error| format!("direct review serialization failed: {error}"))?;
        let target_identity_text = format!("{before_target_identity:?}");
        let source_length_marker = format!("\"content_byte_length\":{}", expected_bytes.len());
        let forbidden_prepare_values = [
            prepared.ticket_id.as_str(),
            "RAH_DELETE_FILE_HOSTEXPLICIT_LIVE_SOURCE_SENTINEL",
            escaped_source,
            expected_sha256.as_str(),
            target,
            native_repository.as_str(),
            native_target.as_str(),
            target_identity_text.as_str(),
            raw_tool_input.as_str(),
            direct_review.as_str(),
            source_length_marker.as_str(),
        ];
        if prepare_events[0].get("review").is_some()
            || prepare_events[0].get("result").is_some()
            || activity_id == prepared.ticket_id
            || forbidden_prepare_values
                .iter()
                .any(|value| !value.is_empty() && prepare_activity.contains(value))
        {
            return Err("generic Prepared activity was not value-level private".to_owned());
        }
        let facts = prepared.review.content_facts();
        if prepared.ticket_id.is_empty()
            || prepared.ticket_id.len() > 256
            || prepared.review.operation() != "repo.delete-file"
            || prepared.review.target_count() != 1
            || prepared.review.path() != target
            || prepared.review.tracked_state() != "clean HEAD-tracked stage-0 regular file"
            || prepared.review.file_mode() != "100644"
            || prepared.review.file_intent() != "permanently remove this existing regular file"
            || prepared.review.preimage_encoding() != "complete_utf8_escaped"
            || prepared.review.content_escaped() != escaped_source
            || prepared.review.content_byte_length() != expected_bytes.len()
            || prepared.review.content_sha256() != expected_sha256
            || prepared.review.bom() != rah_tools::RepositoryDeleteFileBomState::Absent
            || facts.contains_cr
            || !facts.contains_lf
            || facts.contains_crlf
            || facts.carriage_returns != 0
            || facts.line_feeds != 2
            || facts.crlf_pairs != 0
            || facts.ends_with_newline
            || facts.final_eof != "no_final_newline"
            || facts.contains_tab
            || facts.contains_trailing_space
            || !facts.contains_control_or_format_escape
            || facts.control_characters != 2
            || facts.format_characters != 0
            || facts.empty
            || prepared.review.head_blob_relationship() != "worktree bytes equal current HEAD blob"
            || prepared.review.index_relationship()
                != "exact stage-0 index entry equals HEAD tree entry and worktree"
            || prepared.review.expected_effect()
                != "one worktree file becomes absent; one unstaged deletion"
            || prepared.review.post_delete_git_meaning()
                != "index, HEAD, branch, refs, and history remain unchanged"
            || !prepared.review.non_effects().contains(&"not Stage")
            || !prepared.review.non_effects().contains(&"not Unstage")
            || !prepared.review.non_effects().contains(&"not Commit")
            || !prepared
                .review
                .non_effects()
                .contains(&"does not modify the index")
            || !prepared
                .review
                .warnings()
                .contains(&"no Trash or Recycle Bin guarantee")
            || !prepared.review.warnings().contains(&"no retry or replay")
        {
            return Err("direct Prepared deletion review was incomplete or altered".to_owned());
        }
        let prepare_index_unchanged = fs::read(fixture.0.join(".git").join("index"))
            .map_err(|error| format!("Prepare index read failed: {error}"))?
            == before_index;
        let prepare_git_unchanged =
            live_git_state(&git, &fixture.0, "__rah_no_excluded_branch__")? == before_git;
        let prepare_cached_unchanged =
            live_git_text(&git, &fixture.0, &["diff", "--cached", "--binary"])?
                == before_cached_binary;
        let prepare_root_unchanged =
            live_directory_entries(&fixture.0)? == before_directory_entries;
        let prepare_parent_unchanged =
            live_directory_entries(&parent_path)? == before_parent_entries;
        let prepare_conversation_unchanged = serde_json::to_value(
            app.state::<DesktopAppState>()
                .persistence
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .presentation(),
        )
        .map_err(|error| format!("Prepare conversation serialization failed: {error}"))?
            == before_conversation;
        let prepare_coordinator = app
            .state::<DesktopAppState>()
            .host_invocation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .state();
        if !prepare_index_unchanged
            || !prepare_git_unchanged
            || !prepare_cached_unchanged
            || !prepare_root_unchanged
            || !prepare_parent_unchanged
            || fs::read(&target_path)
                .map_err(|error| format!("Prepare target read failed: {error}"))?
                != before_target_bytes
            || live_file_identity(&target_path)? != before_target_identity
            || live_test_delete_file_tool_executions(&fixture.0) != 0
            || live_test_delete_file_native_attempts(&fixture.0) != 0
            || current_host_generation_tuple(app.state::<DesktopAppState>().inner())
                != before_generations
            || app.state::<DesktopAppState>().persistence_namespace() != before_namespace
            || !prepare_conversation_unchanged
            || prepare_coordinator != CoordinatorState::HostPrepared
            || !commit_control.has_pending_authorization().await
        {
            return Err("Prepare was not zero effect or invalidated Commit review".to_owned());
        }
        println!("RAH_DELETE_FILE_HOSTEXPLICIT_TARGET={target}");
        println!(
            "RAH_DELETE_FILE_HOSTEXPLICIT_SOURCE_LENGTH={}",
            expected_bytes.len()
        );
        println!("RAH_DELETE_FILE_HOSTEXPLICIT_SOURCE_SHA256={expected_sha256}");
        println!("RAH_DELETE_FILE_HOSTEXPLICIT_PREPARE_TOOL_EXECUTIONS=0");
        println!("RAH_DELETE_FILE_HOSTEXPLICIT_PREPARE_NATIVE_ATTEMPTS=0");

        let confirmed = host_confirm_tool_invocation(
            HostConfirmRequest {
                ticket_id: prepared.ticket_id.clone(),
            },
            app.handle().clone(),
            app.state(),
        )
        .await
        .map_err(|error| format!("production repo.delete-file Confirm failed: {error:?}"))?;
        if confirmed.invocation_id != activity_id {
            return Err("Confirm changed the independent activity ID".to_owned());
        }
        let started_events = wait_for_test_events(&host_activity.0, 2).await?;
        require_host_event(&started_events[1], "started", "repo.delete-file")?;
        if commit_control.has_pending_authorization().await {
            return Err("reviewed Commit authorization remained pending at Started".to_owned());
        }
        if !target_path.exists() {
            return Err("target disappeared before Started observation".to_owned());
        }
        let events = wait_for_test_events(&host_activity.0, 3).await?;
        require_host_event(&events[2], "tool_completed", "repo.delete-file")?;
        let terminal_output = event_tool_output(&events[2])?;
        let [ToolContent::Json(terminal_result)] = terminal_output.content.as_slice() else {
            return Err("terminal result was not one JSON status object".to_owned());
        };
        if terminal_output.is_error
            || terminal_result != &serde_json::json!({"status": "deleted_verified"})
        {
            return Err("terminal public result was not status-only deleted_verified".to_owned());
        }
        for event in &events {
            if event.get("invocationId").and_then(Value::as_str) != Some(activity_id.as_str())
                || event.get("review").is_some()
            {
                return Err("HostExplicit activity correlation or review privacy failed".to_owned());
            }
            let serialized = serde_json::to_string(event)
                .map_err(|error| format!("HostActivity serialization failed: {error}"))?;
            if forbidden_prepare_values
                .iter()
                .any(|value| !value.is_empty() && serialized.contains(value))
            {
                return Err("terminal HostActivity privacy boundary failed".to_owned());
            }
        }
        if live_test_delete_file_tool_executions(&fixture.0) != 1
            || live_test_delete_file_native_attempts(&fixture.0) != 1
        {
            return Err(
                "Confirm was not exactly one Tool and one native delete attempt".to_owned(),
            );
        }
        let final_target_exists = target_path.exists();
        let final_parent_entries = live_directory_entries(&parent_path)?;
        if final_target_exists
            || final_parent_entries
                .iter()
                .any(|entry| entry.eq_ignore_ascii_case("rah-hostexplicit-live-delete.txt"))
        {
            return Err("independent final absence proof failed".to_owned());
        }
        let after_git = live_git_state(&git, &fixture.0, "__rah_no_excluded_branch__")?;
        let after_cached_binary =
            live_git_text(&git, &fixture.0, &["diff", "--cached", "--binary"])?;
        let after_status = after_git
            .status
            .lines()
            .map(str::to_owned)
            .collect::<BTreeSet<_>>();
        let expected_status = [format!(" D {target}"), format!("M  {unrelated}")]
            .into_iter()
            .collect::<BTreeSet<_>>();
        if after_status != expected_status
            || after_git.index_semantics != before_git.index_semantics
            || after_git.head_oid != before_head
            || after_git.symbolic_head != before_git.symbolic_head
            || after_git.current_branch != before_branch
            || after_git.local_heads != before_git.local_heads
            || after_git.tags_and_remotes != before_git.tags_and_remotes
            || after_git.all_refs != before_refs
            || fs::read(fixture.0.join(".git").join("index"))
                .map_err(|error| format!("post-effect index read failed: {error}"))?
                != before_index
            || after_cached_binary != before_cached_binary
        {
            return Err("verified deletion changed protected Git state or staged state".to_owned());
        }
        let refresh = wait_for_test_events(&refresh_events.0, 1).await?;
        if refresh.len() != 1 {
            return Err("descriptive repository refresh was not emitted exactly once".to_owned());
        }
        if commit_control.has_pending_authorization().await {
            return Err("Started recreated reviewed Commit authorization".to_owned());
        }
        let duplicate = host_confirm_tool_invocation(
            HostConfirmRequest {
                ticket_id: prepared.ticket_id.clone(),
            },
            app.handle().clone(),
            app.state(),
        )
        .await;
        let activity_as_confirm = host_confirm_tool_invocation(
            HostConfirmRequest {
                ticket_id: activity_id.clone(),
            },
            app.handle().clone(),
            app.state(),
        )
        .await;
        let activity_as_cancel = host_cancel_tool_invocation(
            HostConfirmRequest {
                ticket_id: activity_id.clone(),
            },
            app.handle().clone(),
            app.state(),
        );
        if duplicate != Err(FrontendError::HostInvocationTicketInvalid)
            || activity_as_confirm != Err(FrontendError::HostInvocationTicketInvalid)
            || activity_as_cancel != Err(FrontendError::HostInvocationTicketInvalid)
            || live_test_delete_file_tool_executions(&fixture.0) != 1
            || live_test_delete_file_native_attempts(&fixture.0) != 1
        {
            return Err("duplicate or activity-ID authority rejection was not exact".to_owned());
        }
        let final_coordinator = app
            .state::<DesktopAppState>()
            .host_invocation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .state();
        let final_chat = *app
            .state::<DesktopAppState>()
            .chat
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if final_coordinator != CoordinatorState::Idle
            || final_chat != ChatState::Idle
            || !app
                .state::<DesktopAppState>()
                .active_chat
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .is_none()
            || !chat_events.0.lock().unwrap().is_empty()
            || !model_activity.0.lock().unwrap().is_empty()
            || startup_activation_snapshot() != StartupActivationCounters::default()
        {
            return Err("HostExplicit deletion left coordinator/chat/model activity".to_owned());
        }
        println!("RAH_DELETE_FILE_HOSTEXPLICIT_CONFIRM_TOOL_EXECUTIONS=1");
        println!("RAH_DELETE_FILE_HOSTEXPLICIT_CONFIRM_NATIVE_ATTEMPTS=1");
        println!("RAH_DELETE_FILE_HOSTEXPLICIT_TERMINAL_STATUS=deleted_verified");
        println!("RAH_DELETE_FILE_HOSTEXPLICIT_TERMINAL_STATE=tool_completed");
        println!("RAH_DELETE_FILE_HOSTEXPLICIT_GIT_UNSTAGED_DELETION=1");
        println!("RAH_DELETE_FILE_HOSTEXPLICIT_INDEX_UNCHANGED=1");
        println!("RAH_DELETE_FILE_HOSTEXPLICIT_HEAD_UNCHANGED=1");
        println!("RAH_DELETE_FILE_HOSTEXPLICIT_REFS_UNCHANGED=1");
        println!("RAH_DELETE_FILE_HOSTEXPLICIT_COMMIT_AUTHORIZATION_INVALIDATED=1");
        println!("RAH_DELETE_FILE_HOSTEXPLICIT_TICKET_ACTIVITY_SEPARATE=1");
        println!("RAH_DELETE_FILE_HOSTEXPLICIT_DUPLICATE_CONFIRM_REJECTED=1");
        println!("RAH_DELETE_FILE_HOSTEXPLICIT_ACTIVITY_CONFIRM_REJECTED=1");
        println!("RAH_DELETE_FILE_HOSTEXPLICIT_ACTIVITY_CANCEL_REJECTED=1");
        println!("RAH_DELETE_FILE_HOSTEXPLICIT_COORDINATOR_IDLE=1");
        println!("RAH_DELETE_FILE_HOSTEXPLICIT_CHAT_IDLE=1");
        println!("RAH_DELETE_FILE_HOSTEXPLICIT_GENERATIONS={before_generations:?}");
        println!("RAH_DELETE_FILE_HOSTEXPLICIT_MODEL_LIFECYCLE=0");
        println!("RAH_DELETE_FILE_HOSTEXPLICIT_MCP=0");
        println!("RAH_DELETE_FILE_HOSTEXPLICIT_PROCESS_PLUGINS=0");
        app.unlisten(host_activity.1);
        app.unlisten(refresh_events.1);
        app.unlisten(chat_events.1);
        app.unlisten(model_activity.1);
        shutdown_live_state(app.state::<DesktopAppState>().inner()).await;
        clear_live_test_delete_file_tool_executions(&fixture.0);
        clear_live_test_delete_file_native_attempts(&fixture.0);
        println!("RAH_DELETE_FILE_HOSTEXPLICIT_CODEX_CLEANUP=1");
        println!("RAH_DELETE_FILE_HOSTEXPLICIT_LIVE_OK");
        Ok(())
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

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct LiveGitState {
        symbolic_head: String,
        head_oid: String,
        current_branch: String,
        status: String,
        index_semantics: String,
        raw_worktree_diff: String,
        raw_staged_diff: String,
        tracking: String,
        local_heads: String,
        tags_and_remotes: String,
        all_refs: String,
    }

    fn live_git_text(git: &Path, root: &Path, arguments: &[&str]) -> Result<String, String> {
        let output = Command::new(git)
            .args(arguments)
            .current_dir(root)
            .output()
            .map_err(|error| format!("Git command failed to start: {error}"))?;
        if !output.status.success() {
            return Err(format!("Git command failed with status {}", output.status));
        }
        String::from_utf8(output.stdout)
            .map_err(|error| format!("Git output was not UTF-8: {error}"))
    }

    fn live_git_state(
        git: &Path,
        root: &Path,
        excluded_branch: &str,
    ) -> Result<LiveGitState, String> {
        let local_heads = live_git_text(
            git,
            root,
            &[
                "for-each-ref",
                "--format=%(refname) %(objectname)",
                "refs/heads",
            ],
        )?
        .lines()
        .filter(|line| !line.starts_with(&format!("refs/heads/{excluded_branch} ")))
        .collect::<Vec<_>>()
        .join("\n");
        Ok(LiveGitState {
            symbolic_head: live_git_text(git, root, &["symbolic-ref", "HEAD"])?
                .trim()
                .to_owned(),
            head_oid: live_git_text(git, root, &["rev-parse", "HEAD"])?
                .trim()
                .to_owned(),
            current_branch: live_git_text(git, root, &["branch", "--show-current"])?
                .trim()
                .to_owned(),
            status: live_git_text(
                git,
                root,
                &["status", "--porcelain=v1", "--untracked-files=all"],
            )?,
            index_semantics: live_git_text(git, root, &["ls-files", "--stage"])?,
            raw_worktree_diff: live_git_text(git, root, &["diff", "--raw"])?,
            raw_staged_diff: live_git_text(git, root, &["diff", "--cached", "--raw"])?,
            tracking: live_git_text(
                git,
                root,
                &[
                    "for-each-ref",
                    "--format=%(refname:short) %(upstream:short)",
                    "refs/heads",
                ],
            )?
            .lines()
            .filter(|line| !line.starts_with(&format!("{excluded_branch} ")))
            .collect::<Vec<_>>()
            .join("\n"),
            local_heads,
            tags_and_remotes: live_git_text(
                git,
                root,
                &[
                    "for-each-ref",
                    "--format=%(refname) %(objectname)",
                    "refs/tags",
                    "refs/remotes",
                ],
            )?,
            all_refs: live_git_text(
                git,
                root,
                &["for-each-ref", "--format=%(refname) %(objectname)", "refs"],
            )?,
        })
    }

    fn live_git_exit_success(git: &Path, root: &Path, arguments: &[&str]) -> Result<bool, String> {
        let output = Command::new(git)
            .args(arguments)
            .current_dir(root)
            .output()
            .map_err(|error| format!("Git status command failed to start: {error}"))?;
        match output.status.code() {
            Some(0) => Ok(true),
            Some(1) => Ok(false),
            code => Err(format!("Git status command returned {code:?}")),
        }
    }

    fn live_directory_entries(root: &Path) -> Result<BTreeSet<String>, String> {
        fs::read_dir(root)
            .map_err(|error| format!("repository directory observation failed: {error}"))?
            .map(|entry| {
                entry
                    .map_err(|error| format!("repository directory entry failed: {error}"))
                    .map(|entry| entry.file_name().to_string_lossy().into_owned())
            })
            .collect()
    }

    fn live_multi_file_temporary_count(root: &Path) -> Result<usize, String> {
        let mut count = 0;
        for directory in [root.to_owned(), root.join("nested")] {
            if !directory.is_dir() {
                continue;
            }
            for entry in fs::read_dir(directory)
                .map_err(|error| format!("temporary-artifact observation failed: {error}"))?
            {
                let name = entry
                    .map_err(|error| format!("temporary directory entry failed: {error}"))?
                    .file_name()
                    .to_string_lossy()
                    .into_owned();
                if name.starts_with(".rah-repo-edit-files-") && name.ends_with(".tmp") {
                    count += 1;
                }
            }
        }
        Ok(count)
    }

    fn live_target_exists(git: &Path, root: &Path, branch: &str) -> Result<bool, String> {
        let reference = format!("refs/heads/{branch}");
        let output = Command::new(git)
            .args(["show-ref", "--verify", "--quiet", &reference])
            .current_dir(root)
            .output()
            .map_err(|error| format!("target-ref observation failed to start: {error}"))?;
        match output.status.code() {
            Some(0) => Ok(true),
            Some(1) => Ok(false),
            code => Err(format!("target-ref observation returned status {code:?}")),
        }
    }

    async fn shutdown_live_state(state: &DesktopAppState) {
        state.shutdown_for_exit().await;
        *state
            .connection
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = ConnectionState::NotConnected;
    }

    fn listen_for_test_event(
        app: &tauri::AppHandle,
        event_name: &'static str,
    ) -> (Arc<Mutex<Vec<String>>>, tauri::EventId) {
        let events = Arc::new(Mutex::new(Vec::new()));
        let captured = Arc::clone(&events);
        let listener = app.listen(event_name, move |event| {
            captured
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(event.payload().to_owned());
        });
        (events, listener)
    }

    async fn wait_for_test_events(
        events: &Arc<Mutex<Vec<String>>>,
        count: usize,
    ) -> Result<Vec<Value>, String> {
        let deadline = std::time::Instant::now() + Duration::from_secs(30);
        loop {
            let captured = events
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone();
            if captured.len() >= count {
                return captured
                    .into_iter()
                    .map(|payload| {
                        serde_json::from_str(&payload)
                            .map_err(|error| format!("Desktop event payload was invalid: {error}"))
                    })
                    .collect();
            }
            if std::time::Instant::now() >= deadline {
                return Err(format!(
                    "timed out waiting for {count} Desktop events; observed {}",
                    captured.len()
                ));
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    fn event_tool_output(event: &Value) -> Result<ToolOutput, String> {
        serde_json::from_value(
            event
                .get("result")
                .cloned()
                .ok_or_else(|| "HostExplicit completion did not contain a result".to_owned())?,
        )
        .map_err(|error| format!("HostExplicit result was not a ToolOutput: {error}"))
    }

    fn require_host_event(event: &Value, state: &str, tool: &str) -> Result<(), String> {
        if event.get("source") != Some(&Value::String("host_explicit".to_owned()))
            || event.get("tool") != Some(&Value::String(tool.to_owned()))
            || event.get("state") != Some(&Value::String(state.to_owned()))
        {
            return Err(format!("unexpected HostExplicit event: {event}"));
        }
        Ok(())
    }

    fn status_entry_path(entry: &Value) -> Option<&str> {
        entry
            .get("path")
            .and_then(|path| path.get("value"))
            .and_then(Value::as_str)
    }

    fn live_sha256(bytes: &[u8]) -> String {
        let digest = Sha256::digest(bytes);
        digest.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    fn live_file_identity(path: &Path) -> Result<(u32, u64), String> {
        let file = fs::File::open(path).map_err(|_| "target metadata failed".to_owned())?;
        live_handle_identity(&file)
    }

    fn live_directory_identity(path: &Path) -> Result<(u32, u64), String> {
        use std::os::windows::fs::OpenOptionsExt;

        let file = fs::OpenOptions::new()
            .read(true)
            .custom_flags(0x02000000)
            .open(path)
            .map_err(|_| "directory metadata failed".to_owned())?;
        live_handle_identity(&file)
    }

    fn live_handle_identity(file: &fs::File) -> Result<(u32, u64), String> {
        let mut information = std::mem::MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::zeroed();
        let result =
            unsafe { GetFileInformationByHandle(file.as_raw_handle(), information.as_mut_ptr()) };
        if result == 0 {
            return Err("target identity observation failed".to_owned());
        }
        let information = unsafe { information.assume_init() };
        let file_index =
            (u64::from(information.nFileIndexHigh) << 32) | u64::from(information.nFileIndexLow);
        Ok((information.dwVolumeSerialNumber, file_index))
    }

    #[tokio::test(flavor = "current_thread")]
    #[ignore = "requires the certified Windows Codex live gate"]
    async fn windows_live_desktop_hostexplicit_rename_file() -> Result<(), String> {
        let codex_selection = resolve_codex_executable()
            .map_err(|error| format!("Codex discovery failed: {error:?}"))?;
        if !matches!(
            codex_selection.source,
            CodexExecutableSource::CertifiedBaseline | CodexExecutableSource::Override
        ) {
            return Err("certified Codex 0.149.0 baseline was not selected".to_owned());
        }
        let codex_executable = fs::canonicalize(&codex_selection.executable)
            .map_err(|error| format!("Codex executable canonicalization failed: {error}"))?;
        let codex_version = String::from_utf8(
            Command::new(&codex_executable)
                .arg("--version")
                .output()
                .map_err(|error| format!("Codex version probe failed: {error}"))?
                .stdout,
        )
        .map_err(|error| format!("Codex version output was not UTF-8: {error}"))?
        .trim()
        .to_owned();
        let codex_sha256 = live_sha256(
            &fs::read(&codex_executable)
                .map_err(|error| format!("Codex executable read failed: {error}"))?,
        );
        if codex_version != SUPPORTED_CODEX_VERSION {
            return Err(format!("certified Codex version mismatch: {codex_version}"));
        }

        let fixture = TestRepository::new();
        let storage = TestRepository::new();
        let git = selected_git_executable()
            .map_err(|error| format!("Git discovery failed: {error:?}"))?;
        let source = "src/rah-hostexplicit-live-rename.txt";
        let destination = "safe-destination/rah-hostexplicit-live-renamed.txt";
        let unrelated = "unrelated-staged.txt";
        let content =
            "RAH_RENAME_FILE_HOSTEXPLICIT_LIVE_SOURCE_SENTINEL\nline-two: λ\nfinal-line-no-newline";
        let expected_bytes = content.as_bytes();
        let expected_sha256 = live_sha256(expected_bytes);
        let source_path = fixture.0.join(source);
        let destination_path = fixture.0.join(destination);
        let source_parent = fixture.0.join("src");
        let destination_parent = fixture.0.join("safe-destination");
        let unrelated_path = fixture.0.join(unrelated);

        fs::remove_dir_all(fixture.0.join(".git"))
            .map_err(|error| format!("placeholder metadata cleanup failed: {error}"))?;
        fs::remove_file(fixture.0.join("inside.txt"))
            .map_err(|error| format!("placeholder file cleanup failed: {error}"))?;
        let run_git = |arguments: &[&str]| -> Result<(), String> {
            let output = Command::new(&git)
                .args(arguments)
                .current_dir(&fixture.0)
                .output()
                .map_err(|error| format!("Git fixture command failed to start: {error}"))?;
            if !output.status.success() {
                return Err(format!(
                    "Git fixture command failed with status {}",
                    output.status
                ));
            }
            Ok(())
        };
        run_git(&["init", "--quiet", "--initial-branch=master"])?;
        run_git(&["config", "user.email", "rah-rename-live@example.invalid"])?;
        run_git(&["config", "user.name", "RAH Rename Live Test"])?;
        run_git(&["config", "core.autocrlf", "false"])?;
        fs::create_dir_all(&source_parent)
            .map_err(|error| format!("source parent creation failed: {error}"))?;
        fs::create_dir_all(&destination_parent)
            .map_err(|error| format!("destination parent creation failed: {error}"))?;
        fs::write(&source_path, expected_bytes)
            .map_err(|error| format!("source fixture write failed: {error}"))?;
        fs::write(&unrelated_path, b"unrelated committed\n")
            .map_err(|error| format!("unrelated fixture write failed: {error}"))?;
        run_git(&["add", source, unrelated])?;
        run_git(&["commit", "--quiet", "-m", "live rename fixture"])?;
        fs::write(&unrelated_path, b"unrelated staged change\n")
            .map_err(|error| format!("unrelated staged change failed: {error}"))?;
        run_git(&["add", unrelated])?;

        let require_regular_non_reparse = |path: &Path, description: &str| {
            use std::os::windows::fs::MetadataExt;
            let metadata = fs::symlink_metadata(path)
                .map_err(|error| format!("{description} metadata failed: {error}"))?;
            if !metadata.is_file()
                || metadata.file_type().is_symlink()
                || metadata.file_attributes() & 0x400 != 0
            {
                return Err(format!(
                    "{description} was not an ordinary non-reparse file"
                ));
            }
            Ok::<(), String>(())
        };
        let require_ordinary_directory = |path: &Path, description: &str| {
            use std::os::windows::fs::MetadataExt;
            let metadata = fs::symlink_metadata(path)
                .map_err(|error| format!("{description} metadata failed: {error}"))?;
            if !metadata.is_dir()
                || metadata.file_type().is_symlink()
                || metadata.file_attributes() & 0x400 != 0
            {
                return Err(format!(
                    "{description} was not an ordinary non-reparse directory"
                ));
            }
            Ok::<(), String>(())
        };
        require_ordinary_directory(&fixture.0, "repository root")?;
        require_ordinary_directory(&fixture.0.join(".git"), ".git metadata")?;
        require_ordinary_directory(&source_parent, "source parent")?;
        require_ordinary_directory(&destination_parent, "destination parent")?;
        require_regular_non_reparse(&source_path, "source")?;
        if expected_bytes.len() > 65_536
            || std::str::from_utf8(expected_bytes).is_err()
            || expected_bytes.contains(&0)
            || destination_path.exists()
        {
            return Err(
                "rename fixture did not meet the reviewed source/destination bounds".to_owned(),
            );
        }
        let source_stage = live_git_text(&git, &fixture.0, &["ls-files", "--stage", "--", source])?;
        if !source_stage.starts_with("100644 ") || !source_stage.contains(source) {
            return Err("source was not mode 100644 in the stage-0 index".to_owned());
        }
        if !live_git_text(&git, &fixture.0, &["ls-tree", "-r", "--name-only", "HEAD"])?
            .lines()
            .any(|line| line == source)
            || !live_git_text(&git, &fixture.0, &["ls-tree", "-r", "--name-only", "HEAD"])?
                .lines()
                .all(|line| line != destination)
            || !live_git_text(&git, &fixture.0, &["ls-files", "-s", "--", destination])?.is_empty()
            || live_git_exit_success(&git, &fixture.0, &["check-ignore", "-q", "--", destination])?
        {
            return Err("destination was not absent and visible to the reviewed route".to_owned());
        }
        let sparse_checkout = live_git_text(
            &git,
            &fixture.0,
            &["config", "--get", "core.sparseCheckout"],
        )
        .or_else(|error| {
            if error.contains("status") {
                Ok(String::new())
            } else {
                Err(error)
            }
        })?;
        if sparse_checkout.trim().eq_ignore_ascii_case("true")
            || live_git_text(&git, &fixture.0, &["rev-parse", "--is-bare-repository"])?.trim()
                != "false"
            || live_git_text(&git, &fixture.0, &["worktree", "list", "--porcelain"])?
                .lines()
                .filter(|line| line.starts_with("worktree "))
                .count()
                != 1
        {
            return Err("live repository used sparse or linked-worktree state".to_owned());
        }
        for state_path in [
            "MERGE_HEAD",
            "CHERRY_PICK_HEAD",
            "REVERT_HEAD",
            "BISECT_LOG",
            "sequencer",
            "rebase-merge",
            "rebase-apply",
        ] {
            if fixture.0.join(".git").join(state_path).exists() {
                return Err(format!("active Git state was present: {state_path}"));
            }
        }

        let branch_authority = RepositoryBranchCreationAuthority::new(&git, &fixture.0)
            .map_err(|error| format!("branch authority construction failed: {error}"))?;
        let deletion_authority = RepositoryFileDeletionAuthority::new(&git, &fixture.0)
            .map_err(|error| format!("deletion authority construction failed: {error}"))?;
        let rename_authority = RepositoryFileRenameAuthority::new(&git, &fixture.0)
            .map_err(|error| format!("rename authority construction failed: {error}"))?;
        let repository = DesktopRepository::new_with_authorities(
            &git,
            &fixture.0,
            None,
            Some(deletion_authority),
            Some(rename_authority),
            Some(branch_authority),
        )
        .map_err(|error| format!("Desktop repository construction failed: {error:?}"))?;
        let app = tauri::Builder::default()
            .any_thread()
            .manage(DesktopAppState::new(storage.0.clone()))
            .build(tauri::generate_context!())
            .map_err(|error| format!("Desktop test app construction failed: {error}"))?;
        let host_activity = listen_for_test_event(app.handle(), "host_activity_event");
        let refresh_events = listen_for_test_event(app.handle(), "repository_snapshot_refresh");
        let chat_events = listen_for_test_event(app.handle(), "chat_event");
        let model_activity = listen_for_test_event(app.handle(), "activity_event");
        replace_selected_repository(app.state::<DesktopAppState>().inner(), repository);
        set_commit_identity(
            app.handle().clone(),
            app.state(),
            "RAH Rename HostExplicit Live Test".to_owned(),
            "rah-rename-hostexplicit@example.invalid".to_owned(),
        )
        .map_err(|error| format!("Desktop commit identity setup failed: {error:?}"))?;
        connect_codex(app.state())
            .await
            .map_err(|error| format!("production Desktop connection failed: {error:?}"))?;

        let connected_snapshot = get_effective_authority_snapshot(app.state());
        let eligible = connected_snapshot
            .effective_tools
            .iter()
            .filter(|tool| tool.host_invocation.eligible)
            .map(|tool| tool.public_tool_name.as_str())
            .collect::<BTreeSet<_>>();
        let expected_eligible = [
            "fs.read",
            "repo.file-info",
            "repo.status",
            "repo.diff",
            "repo.diff-staged",
            "repo.create-branch",
            "repo.patch",
            "repo.edit-files",
            "repo.create-file",
            "repo.delete-file",
            "repo.rename-file",
        ]
        .into_iter()
        .collect::<BTreeSet<_>>();
        let rename_tools = connected_snapshot
            .effective_tools
            .iter()
            .filter(|tool| tool.public_tool_name == "repo.rename-file")
            .collect::<Vec<_>>();
        let rename_tool = rename_tools
            .first()
            .ok_or_else(|| "repo.rename-file was not advertised".to_owned())?;
        let external_effective = connected_snapshot
            .effective_tools
            .iter()
            .filter(|tool| {
                matches!(
                    tool.source_kind,
                    SourceKind::Mcp | SourceKind::ProcessPlugin
                )
            })
            .count();
        let provider_activation_present = app
            .state::<DesktopAppState>()
            .provider_activation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_some();
        let trusted_profile_present = app
            .state::<DesktopAppState>()
            .trusted_profile
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_some();
        let connection_source = match &*app
            .state::<DesktopAppState>()
            .connection
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
        {
            ConnectionState::Connected { source, .. } => *source,
            _ => return Err("Desktop connection was not Connected".to_owned()),
        };
        let current = current_host_composition(app.state::<DesktopAppState>().inner())
            .map_err(|error| format!("current HostExplicit composition failed: {error:?}"))?;
        if connected_snapshot.status != SnapshotStatus::ConnectedCurrent
            || eligible != expected_eligible
            || rename_tools.len() != 1
            || !rename_tool.host_invocation.eligible
            || rename_tool.host_invocation.kind != Some(HostInvocationKind::RepoRenameFile)
            || rename_tool.source_kind != SourceKind::RepositoryHost
            || rename_tool.effect_class != EffectClass::RepositoryMutation
            || rename_tool.authority_category != AuthorityCategory::RepositoryFileRename
            || rename_tool.permission != PermissionLevel::Execute
            || !rename_tool.repository_bound
            || current.repository_rename_file_preparer.is_none()
            || external_effective != 0
            || connected_snapshot.configured.configured_provider_count != 0
            || provider_activation_present
            || trusted_profile_present
            || !matches!(
                connection_source,
                CodexExecutableSource::CertifiedBaseline | CodexExecutableSource::Override
            )
        {
            return Err(format!(
                "connected-current rename composition was not exact: status={:?} eligible={eligible:?} rename_count={} rename_eligible={} rename_kind={:?} rename_source={:?} rename_effect={:?} rename_category={:?} rename_permission={:?} rename_repository_bound={} preparer={} external={} providers={} provider_activation={} trusted_profile={} source={:?}",
                connected_snapshot.status,
                rename_tools.len(),
                rename_tool.host_invocation.eligible,
                rename_tool.host_invocation.kind,
                rename_tool.source_kind,
                rename_tool.effect_class,
                rename_tool.authority_category,
                rename_tool.permission,
                rename_tool.repository_bound,
                current.repository_rename_file_preparer.is_some(),
                external_effective,
                connected_snapshot.configured.configured_provider_count,
                provider_activation_present,
                trusted_profile_present,
                connection_source,
            ));
        }

        reset_startup_activation_counters();
        let commit_control = app
            .state::<DesktopAppState>()
            .commit_capability
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .map(|capability| Arc::clone(&capability.control))
            .ok_or_else(|| "reviewed Commit control was not composed".to_owned())?;
        let reviewed = refresh_repository_workflow(app.state::<DesktopAppState>().inner())
            .await
            .map_err(|error| format!("review snapshot failed: {error:?}"))?;
        let review_id = match reviewed.review {
            StagedReviewPresentation::ReviewAvailable {
                review_id: Some(review_id),
                can_authorize: true,
                authorization_state: CommitAuthorizationPresentation::ReadyToAuthorize,
            } => review_id,
            other => {
                return Err(format!(
                    "fixture did not expose a reviewed Commit authorization: {other:?}"
                ));
            }
        };
        let authorization =
            authorize_repository_commit_review(app.state::<DesktopAppState>().inner(), &review_id)
                .await
                .map_err(|error| format!("review authorization failed: {error:?}"))?;
        if authorization.authorization_state != CommitAuthorizationPresentation::AuthorizedPending
            || !commit_control.has_pending_authorization().await
        {
            return Err("reviewed Commit authorization was not pending before Prepare".to_owned());
        }

        host_activity.0.lock().unwrap().clear();
        refresh_events.0.lock().unwrap().clear();
        chat_events.0.lock().unwrap().clear();
        model_activity.0.lock().unwrap().clear();
        let before_root_identity = live_directory_identity(&fixture.0)?;
        let before_dot_git_identity = live_directory_identity(&fixture.0.join(".git"))?;
        let before_git_identity = live_file_identity(&git)?;
        let before_source_bytes = fs::read(&source_path)
            .map_err(|error| format!("source baseline read failed: {error}"))?;
        let before_source_identity = live_file_identity(&source_path)?;
        let before_source_parent_identity = live_directory_identity(&source_parent)?;
        let before_destination_parent_identity = live_directory_identity(&destination_parent)?;
        let before_root_entries = live_directory_entries(&fixture.0)?;
        let before_source_parent_entries = live_directory_entries(&source_parent)?;
        let before_destination_parent_entries = live_directory_entries(&destination_parent)?;
        let before_index = fs::read(fixture.0.join(".git").join("index"))
            .map_err(|error| format!("Git index baseline read failed: {error}"))?;
        let before_cached_binary =
            live_git_text(&git, &fixture.0, &["diff", "--cached", "--binary"])?;
        let before_git = live_git_state(&git, &fixture.0, "__rah_no_excluded_branch__")?;
        if before_source_bytes != expected_bytes
            || live_sha256(&before_source_bytes) != expected_sha256
            || before_git
                .status
                .lines()
                .map(str::to_owned)
                .collect::<BTreeSet<_>>()
                != [format!("M  {unrelated}")]
                    .into_iter()
                    .collect::<BTreeSet<_>>()
            || !before_git.index_semantics.contains(source)
            || before_git.symbolic_head.is_empty()
            || before_git.current_branch.is_empty()
            || before_git.head_oid.is_empty()
            || before_source_parent_entries
                != ["rah-hostexplicit-live-rename.txt".to_owned()]
                    .into_iter()
                    .collect()
            || !before_destination_parent_entries.is_empty()
        {
            return Err("protected staged fixture baseline was not exact".to_owned());
        }
        let before_head = before_git.head_oid.clone();
        let before_branch = before_git.current_branch.clone();
        let before_refs = before_git.all_refs.clone();
        let before_generations =
            current_host_generation_tuple(app.state::<DesktopAppState>().inner());
        let before_namespace = app.state::<DesktopAppState>().persistence_namespace();
        let before_conversation = serde_json::to_value(
            app.state::<DesktopAppState>()
                .persistence
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .presentation(),
        )
        .map_err(|error| format!("conversation baseline serialization failed: {error}"))?;
        clear_live_test_rename_file_tool_executions(&fixture.0);
        clear_live_test_rename_file_native_attempts(&fixture.0);

        let prepared = host_prepare_repo_rename_file(
            HostPrepareRenameFileRequest {
                source_path: source.to_owned(),
                destination_path: destination.to_owned(),
            },
            app.handle().clone(),
            app.state(),
        )
        .await
        .map_err(|error| format!("production repo.rename-file Prepare failed: {error:?}"))?;
        let prepare_events = wait_for_test_events(&host_activity.0, 1).await?;
        require_host_event(&prepare_events[0], "prepared", "repo.rename-file")?;
        let activity_id = prepare_events[0]
            .get("invocationId")
            .and_then(Value::as_str)
            .ok_or_else(|| "Prepared activity omitted invocationId".to_owned())?
            .to_owned();
        let prepare_activity = serde_json::to_string(&prepare_events[0])
            .map_err(|error| format!("Prepared activity serialization failed: {error}"))?;
        let native_source = source_path.to_string_lossy().into_owned();
        let native_destination = destination_path.to_string_lossy().into_owned();
        let raw_tool_input = serde_json::to_string(&serde_json::json!({
            "source_path": source,
            "destination_path": destination,
            "expected_source_file_sha256": expected_sha256,
            "expected_source_file_byte_length": expected_bytes.len(),
        }))
        .map_err(|error| format!("Tool input serialization failed: {error}"))?;
        let direct_review = serde_json::to_string(&prepared.review)
            .map_err(|error| format!("direct review serialization failed: {error}"))?;
        let source_identity_text = format!("{before_source_identity:?}");
        let native_repository = fixture.0.to_string_lossy().into_owned();
        let forbidden_prepare_values = [
            prepared.ticket_id.as_str(),
            "RAH_RENAME_FILE_HOSTEXPLICIT_LIVE_SOURCE_SENTINEL",
            expected_sha256.as_str(),
            source,
            destination,
            native_source.as_str(),
            native_destination.as_str(),
            native_repository.as_str(),
            raw_tool_input.as_str(),
            direct_review.as_str(),
            source_identity_text.as_str(),
        ];
        if prepare_events[0].get("review").is_some()
            || prepare_events[0].get("result").is_some()
            || activity_id == prepared.ticket_id
            || forbidden_prepare_values
                .iter()
                .any(|value| !value.is_empty() && prepare_activity.contains(value))
        {
            return Err("generic Prepared activity was not value-level private".to_owned());
        }
        let expected_escaped = "RAH_RENAME_FILE_HOSTEXPLICIT_LIVE_SOURCE_SENTINEL\\nline-two:\\u{20}\\u{3bb}\\nfinal-line-no-newline";
        let expected_non_effects = [
            "no content rewrite",
            "no staging",
            "no unstaging",
            "no commit",
            "no branch/ref/history mutation",
            "no directory creation",
            "no overwrite",
            "no import/reference rewrite",
            "no automatic retry",
            "no replay",
            "no rollback",
            "no compensation",
        ];
        if prepared.ticket_id.is_empty()
            || prepared.ticket_id.len() > 256
            || prepared.review.operation() != "repo.rename-file"
            || prepared.review.source_path() != source
            || prepared.review.destination_path() != destination
            || prepared.review.source_byte_length() != expected_bytes.len()
            || prepared.review.source_sha256() != expected_sha256
            || prepared.review.source_content_escaped() != expected_escaped
            || prepared.review.source_format() != "strict UTF-8, NUL-free, complete escaped source"
            || prepared.review.source_mode() != "100644"
            || prepared.review.expected_effect()
                != "one reviewed source file moves to the absent destination"
            || prepared.review.expected_git_consequence()
                != "one unstaged worktree rename-like change; HEAD, index, refs, and history remain unchanged"
            || prepared.review.non_effects() != expected_non_effects
        {
            return Err("direct Prepared rename review was incomplete or altered".to_owned());
        }
        let prepare_conversation_unchanged = serde_json::to_value(
            app.state::<DesktopAppState>()
                .persistence
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .presentation(),
        )
        .map_err(|error| format!("Prepare conversation serialization failed: {error}"))?
            == before_conversation;
        let prepare_git = live_git_state(&git, &fixture.0, "__rah_no_excluded_branch__")?;
        if fs::read(&source_path).map_err(|error| format!("Prepare source read failed: {error}"))?
            != before_source_bytes
            || live_file_identity(&source_path)? != before_source_identity
            || live_directory_identity(&source_parent)? != before_source_parent_identity
            || live_directory_identity(&destination_parent)? != before_destination_parent_identity
            || destination_path.exists()
            || live_directory_entries(&fixture.0)? != before_root_entries
            || live_directory_entries(&source_parent)? != before_source_parent_entries
            || live_directory_entries(&destination_parent)? != before_destination_parent_entries
            || fs::read(fixture.0.join(".git").join("index"))
                .map_err(|error| format!("Prepare index read failed: {error}"))?
                != before_index
            || prepare_git != before_git
            || live_git_text(&git, &fixture.0, &["diff", "--cached", "--binary"])?
                != before_cached_binary
            || live_test_rename_file_tool_executions(&fixture.0) != 0
            || live_test_rename_file_native_attempts(&fixture.0) != 0
            || current_host_generation_tuple(app.state::<DesktopAppState>().inner())
                != before_generations
            || app.state::<DesktopAppState>().persistence_namespace() != before_namespace
            || !prepare_conversation_unchanged
            || app
                .state::<DesktopAppState>()
                .host_invocation
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .state()
                != CoordinatorState::HostPrepared
            || !commit_control.has_pending_authorization().await
            || !chat_events.0.lock().unwrap().is_empty()
            || !model_activity.0.lock().unwrap().is_empty()
            || !refresh_events.0.lock().unwrap().is_empty()
        {
            return Err("Prepare was not zero effect or invalidated protected state".to_owned());
        }

        let confirmed = host_confirm_tool_invocation(
            HostConfirmRequest {
                ticket_id: prepared.ticket_id.clone(),
            },
            app.handle().clone(),
            app.state(),
        )
        .await
        .map_err(|error| format!("production repo.rename-file Confirm failed: {error:?}"))?;
        if confirmed.invocation_id != activity_id
            || commit_control.has_pending_authorization().await
        {
            return Err(
                "Confirm did not preserve activity identity or invalidate Commit before Started"
                    .to_owned(),
            );
        }
        let started_events = wait_for_test_events(&host_activity.0, 2).await?;
        require_host_event(&started_events[1], "started", "repo.rename-file")?;
        let events = wait_for_test_events(&host_activity.0, 3).await?;
        require_host_event(&events[2], "tool_completed", "repo.rename-file")?;
        let terminal_output = event_tool_output(&events[2])?;
        let [ToolContent::Json(terminal_result)] = terminal_output.content.as_slice() else {
            return Err("terminal result was not one JSON status object".to_owned());
        };
        if terminal_output.is_error
            || terminal_result != &serde_json::json!({"status": "renamed_verified"})
            || live_test_rename_file_tool_executions(&fixture.0) != 1
            || live_test_rename_file_native_attempts(&fixture.0) != 1
        {
            return Err(
                "Confirm was not exactly one reviewed Tool and native rename attempt".to_owned(),
            );
        }
        for event in &events {
            if event.get("invocationId").and_then(Value::as_str) != Some(activity_id.as_str())
                || event.get("review").is_some()
            {
                return Err("HostExplicit activity correlation or review privacy failed".to_owned());
            }
            let serialized = serde_json::to_string(event)
                .map_err(|error| format!("HostActivity serialization failed: {error}"))?;
            if forbidden_prepare_values
                .iter()
                .any(|value| !value.is_empty() && serialized.contains(value))
            {
                return Err("terminal HostActivity privacy boundary failed".to_owned());
            }
        }

        require_regular_non_reparse(&destination_path, "destination")?;
        let mut expected_destination_parent_entries = before_destination_parent_entries.clone();
        expected_destination_parent_entries.insert("rah-hostexplicit-live-renamed.txt".to_owned());
        if source_path.exists()
            || fs::read(&destination_path)
                .map_err(|error| format!("destination read failed: {error}"))?
                != before_source_bytes
            || live_file_identity(&destination_path)? != before_source_identity
            || live_directory_identity(&fixture.0)? != before_root_identity
            || live_directory_identity(&fixture.0.join(".git"))? != before_dot_git_identity
            || live_file_identity(&git)? != before_git_identity
            || live_directory_identity(&source_parent)? != before_source_parent_identity
            || live_directory_identity(&destination_parent)? != before_destination_parent_identity
            || live_directory_entries(&fixture.0)? != before_root_entries
            || !live_directory_entries(&source_parent)?.is_empty()
            || live_directory_entries(&destination_parent)? != expected_destination_parent_entries
        {
            return Err("independent post-effect filesystem proof failed".to_owned());
        }
        let after_git = live_git_state(&git, &fixture.0, "__rah_no_excluded_branch__")?;
        let expected_status = [
            format!("M  {unrelated}"),
            format!(" D {source}"),
            format!("?? {destination}"),
        ]
        .into_iter()
        .collect::<BTreeSet<_>>();
        if after_git
            .status
            .lines()
            .map(str::to_owned)
            .collect::<BTreeSet<_>>()
            != expected_status
            || after_git.index_semantics != before_git.index_semantics
            || after_git.head_oid != before_head
            || after_git.symbolic_head != before_git.symbolic_head
            || after_git.current_branch != before_branch
            || after_git.local_heads != before_git.local_heads
            || after_git.tags_and_remotes != before_git.tags_and_remotes
            || after_git.all_refs != before_refs
            || after_git.raw_staged_diff != before_git.raw_staged_diff
            || after_git.raw_worktree_diff.is_empty()
            || !after_git.raw_worktree_diff.contains(source)
            || fs::read(fixture.0.join(".git").join("index"))
                .map_err(|error| format!("post-effect index read failed: {error}"))?
                != before_index
            || live_git_text(&git, &fixture.0, &["diff", "--cached", "--binary"])?
                != before_cached_binary
            || !live_git_text(&git, &fixture.0, &["ls-tree", "-r", "--name-only", "HEAD"])?
                .lines()
                .any(|line| line == source)
            || !live_git_text(&git, &fixture.0, &["ls-files", "-s", "--", source])?.contains(source)
            || !live_git_text(&git, &fixture.0, &["ls-tree", "-r", "--name-only", "HEAD"])?
                .lines()
                .all(|line| line != destination)
            || !live_git_text(&git, &fixture.0, &["ls-files", "-s", "--", destination])?.is_empty()
            || live_git_exit_success(&git, &fixture.0, &["check-ignore", "-q", "--", destination])?
        {
            return Err("protected Git state or semantic worktree move proof failed".to_owned());
        }
        let refresh = wait_for_test_events(&refresh_events.0, 1).await?;
        if refresh.len() != 1
            || commit_control.has_pending_authorization().await
            || !chat_events.0.lock().unwrap().is_empty()
            || !model_activity.0.lock().unwrap().is_empty()
        {
            return Err("refresh or post-effect activity proof was not exact".to_owned());
        }
        let duplicate = host_confirm_tool_invocation(
            HostConfirmRequest {
                ticket_id: prepared.ticket_id.clone(),
            },
            app.handle().clone(),
            app.state(),
        )
        .await;
        let activity_as_confirm = host_confirm_tool_invocation(
            HostConfirmRequest {
                ticket_id: activity_id.clone(),
            },
            app.handle().clone(),
            app.state(),
        )
        .await;
        let activity_as_cancel = host_cancel_tool_invocation(
            HostConfirmRequest {
                ticket_id: activity_id,
            },
            app.handle().clone(),
            app.state(),
        );
        if duplicate != Err(FrontendError::HostInvocationTicketInvalid)
            || activity_as_confirm != Err(FrontendError::HostInvocationTicketInvalid)
            || activity_as_cancel != Err(FrontendError::HostInvocationTicketInvalid)
            || live_test_rename_file_tool_executions(&fixture.0) != 1
            || live_test_rename_file_native_attempts(&fixture.0) != 1
        {
            return Err("duplicate or activity-ID authority rejection was not exact".to_owned());
        }
        let final_coordinator = app
            .state::<DesktopAppState>()
            .host_invocation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .state();
        let final_chat = *app
            .state::<DesktopAppState>()
            .chat
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if final_coordinator != CoordinatorState::Idle
            || final_chat != ChatState::Idle
            || app
                .state::<DesktopAppState>()
                .active_chat
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .is_some()
            || live_test_rename_file_tool_executions(&fixture.0) != 1
            || live_test_rename_file_native_attempts(&fixture.0) != 1
        {
            return Err(
                "HostExplicit rename left coordinator activity or replay evidence".to_owned(),
            );
        }

        let codex_source = match codex_selection.source {
            CodexExecutableSource::CertifiedBaseline => "certified_baseline",
            CodexExecutableSource::Override => "certified_baseline_override",
            CodexExecutableSource::Path => "path",
        };
        println!("RAH_RENAME_FILE_HOSTEXPLICIT_CODEX_SOURCE={codex_source}");
        println!("RAH_RENAME_FILE_HOSTEXPLICIT_CODEX_VERSION={codex_version}");
        println!("RAH_RENAME_FILE_HOSTEXPLICIT_CODEX_SHA256={codex_sha256}");
        println!("RAH_RENAME_FILE_HOSTEXPLICIT_PREPARE_TOOL_EXECUTIONS=0");
        println!("RAH_RENAME_FILE_HOSTEXPLICIT_PREPARE_NATIVE_ATTEMPTS=0");
        println!("RAH_RENAME_FILE_HOSTEXPLICIT_CONFIRM_TOOL_EXECUTIONS=1");
        println!("RAH_RENAME_FILE_HOSTEXPLICIT_CONFIRM_NATIVE_ATTEMPTS=1");
        println!("RAH_RENAME_FILE_HOSTEXPLICIT_TERMINAL_STATUS=renamed_verified");
        println!("RAH_RENAME_FILE_HOSTEXPLICIT_FINAL_PROOF=ReviewedSuccess");
        println!("RAH_RENAME_FILE_HOSTEXPLICIT_INDEX_UNCHANGED=1");
        println!("RAH_RENAME_FILE_HOSTEXPLICIT_HEAD_UNCHANGED=1");
        println!("RAH_RENAME_FILE_HOSTEXPLICIT_REFS_UNCHANGED=1");
        println!("RAH_RENAME_FILE_HOSTEXPLICIT_COMMIT_AUTHORIZATION_INVALIDATED=1");
        println!("RAH_RENAME_FILE_HOSTEXPLICIT_MODEL_LIFECYCLE=0");
        println!("RAH_RENAME_FILE_HOSTEXPLICIT_MCP=0");
        println!("RAH_RENAME_FILE_HOSTEXPLICIT_PROCESS_PLUGINS=0");

        app.unlisten(host_activity.1);
        app.unlisten(refresh_events.1);
        app.unlisten(chat_events.1);
        app.unlisten(model_activity.1);
        shutdown_live_state(app.state::<DesktopAppState>().inner()).await;
        clear_live_test_rename_file_tool_executions(&fixture.0);
        clear_live_test_rename_file_native_attempts(&fixture.0);
        let shutdown_connection = matches!(
            &*app
                .state::<DesktopAppState>()
                .connection
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
            ConnectionState::NotConnected
        );
        let shutdown_provider = app
            .state::<DesktopAppState>()
            .provider_activation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_none();
        if !shutdown_connection || !shutdown_provider {
            return Err(
                "Desktop shutdown did not complete the existing Codex reaping path".to_owned(),
            );
        }
        if startup_activation_snapshot() != StartupActivationCounters::default() {
            return Err(
                "live certification startup counters were not zero after shutdown".to_owned(),
            );
        }
        println!("RAH_RENAME_FILE_HOSTEXPLICIT_CLEANUP_REAPED=1");
        println!("RAH_REVIEWED_RENAME_HOSTEXPLICIT_LIVE_OK");
        Ok(())
    }

    fn live_patch_temporary_count(root: &Path) -> Result<usize, String> {
        fs::read_dir(root)
            .map_err(|_| "patch temporary-artifact observation failed".to_owned())?
            .try_fold(0, |count, entry| {
                let entry = entry.map_err(|_| "patch directory observation failed".to_owned())?;
                let name = entry.file_name();
                let is_patch_temporary = name.to_str().is_some_and(|name| {
                    name.starts_with(".rah-repo-patch-") && name.ends_with(".tmp")
                });
                Ok::<_, String>(count + usize::from(is_patch_temporary))
            })
    }

    #[tokio::test(flavor = "current_thread")]
    #[ignore = "requires the certified Windows Codex live gate"]
    async fn windows_live_desktop_explicit_host_tool_invocation() -> Result<(), String> {
        let fixture = TestRepository::git_repository(GitRepositoryState::Clean);
        let storage = TestRepository::new();
        let git = selected_git_executable()
            .map_err(|error| format!("Git discovery failed: {error:?}"))?;

        fs::write(fixture.0.join("nested/ordinary.txt"), "live unstaged\n")
            .map_err(|error| format!("unstaged fixture modification failed: {error}"))?;
        fs::write(fixture.0.join("tracked.txt"), "live staged\n")
            .map_err(|error| format!("staged fixture modification failed: {error}"))?;
        let stage = Command::new(&git)
            .args(["add", "tracked.txt"])
            .current_dir(&fixture.0)
            .status()
            .map_err(|error| format!("staged fixture command failed to start: {error}"))?;
        if !stage.success() {
            return Err("staged fixture command failed".to_owned());
        }

        let branch_name = format!(
            "rah-host-explicit-live-{:x}-{:x}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|error| format!("clock failed: {error}"))?
                .as_nanos(),
            NEXT_TEST_DIRECTORY.fetch_add(1, Ordering::Relaxed)
        );
        let before = live_git_state(&git, &fixture.0, &branch_name)?;
        if live_target_exists(&git, &fixture.0, &branch_name)? {
            return Err("generated live target unexpectedly exists".to_owned());
        }

        let authority = RepositoryBranchCreationAuthority::new(&git, &fixture.0)
            .map_err(|error| format!("branch authority construction failed: {error}"))?;
        let repository = DesktopRepository::new_with_authorities(
            &git,
            &fixture.0,
            None,
            None,
            None,
            Some(authority),
        )
        .map_err(|error| format!("Desktop repository construction failed: {error}"))?;

        let app = tauri::Builder::default()
            .any_thread()
            .manage(DesktopAppState::new(storage.0.clone()))
            .build(tauri::generate_context!())
            .map_err(|error| format!("Desktop test app construction failed: {error}"))?;
        let host_activity = listen_for_test_event(app.handle(), "host_activity_event");
        let refresh_events = listen_for_test_event(app.handle(), "repository_snapshot_refresh");
        replace_selected_repository(app.state::<DesktopAppState>().inner(), repository);
        set_commit_identity(
            app.handle().clone(),
            app.state(),
            "RAH Host Live Test".to_owned(),
            "rah-host@example.invalid".to_owned(),
        )
        .map_err(|error| format!("Desktop commit identity setup failed: {error:?}"))?;

        connect_codex(app.state())
            .await
            .map_err(|error| format!("production Desktop connection failed: {error:?}"))?;
        let status = app.state::<DesktopAppState>().status();
        if status.runtime_status != "connected"
            || status.codex_status != "connected"
            || status.codex_version != Some("codex-cli 0.149.0")
        {
            return Err("connected Desktop did not report the certified Codex baseline".to_owned());
        }

        let connected_snapshot = get_effective_authority_snapshot(app.state());
        if connected_snapshot.status != SnapshotStatus::ConnectedCurrent
            || connected_snapshot.connection.state
                != super::effective_authority::ConnectionBindingState::Connected
            || !connected_snapshot.connection.advertised
            || !connected_snapshot.repository.selected
            || connected_snapshot.repository.identity
                != super::effective_authority::RepositoryIdentity::Current
        {
            return Err("Desktop connection was not connected-current".to_owned());
        }
        if app
            .state::<DesktopAppState>()
            .provider_activation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_some()
        {
            return Err("unexpected external provider activation was present".to_owned());
        }
        println!("RAH_HOST_EXPLICIT_LIVE_CONNECTION_CURRENT=1");

        let find_tool = |name: &str| {
            connected_snapshot
                .effective_tools
                .iter()
                .find(|tool| tool.public_tool_name == name)
        };
        let status_tool = find_tool("repo.status")
            .ok_or_else(|| "repo.status was not in the current Effective Authority".to_owned())?;
        if !status_tool.host_invocation.eligible
            || status_tool.host_invocation.kind != Some(HostInvocationKind::RepoStatus {})
        {
            return Err("repo.status was not host eligible".to_owned());
        }
        println!("RAH_HOST_EXPLICIT_LIVE_REPO_STATUS_ELIGIBLE=1");
        let branch_tool = find_tool(REPOSITORY_CREATE_BRANCH_TOOL_NAME).ok_or_else(|| {
            "repo.create-branch was not in the current Effective Authority".to_owned()
        })?;
        if !branch_tool.host_invocation.eligible
            || branch_tool.host_invocation.kind != Some(HostInvocationKind::RepoCreateBranch)
            || branch_tool.effect_class != EffectClass::RepositoryMutation
            || branch_tool.authority_category != AuthorityCategory::RepositoryLocalBranchCreation
            || branch_tool.permission != PermissionLevel::Execute
            || !branch_tool.repository_bound
        {
            return Err(
                "repo.create-branch Effective Authority classification was wrong".to_owned(),
            );
        }
        let deferred = find_tool("repo.commit")
            .or_else(|| find_tool("repo.patch"))
            .ok_or_else(|| "no known deferred Tool was registered".to_owned())?;
        if deferred.host_invocation.eligible {
            return Err("a deferred Tool was incorrectly host eligible".to_owned());
        }
        println!("RAH_HOST_EXPLICIT_LIVE_BRANCH_ELIGIBLE=1");
        println!("RAH_HOST_EXPLICIT_LIVE_DEFERRED_UNAVAILABLE=1");

        let generations_before_read =
            current_host_generation_tuple(app.state::<DesktopAppState>().inner());
        let conversation_namespace_before_read =
            app.state::<DesktopAppState>().persistence_namespace();
        let conversation_presentation_before_read = serde_json::to_value(
            app.state::<DesktopAppState>()
                .persistence
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .presentation(),
        )
        .map_err(|error| format!("conversation presentation serialization failed: {error}"))?;
        if app
            .state::<DesktopAppState>()
            .host_invocation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .state()
            != CoordinatorState::Idle
        {
            return Err("coordinator was not Idle before repo.status".to_owned());
        }
        let read_response = host_invoke_read(
            HostReadRequest::RepoStatus {
                fields: EmptyHostRequest::default(),
            },
            app.handle().clone(),
            app.state(),
        )
        .await
        .map_err(|error| format!("production repo.status host command failed: {error:?}"))?;
        if read_response.invocation_id.is_empty() {
            return Err("repo.status HostExplicit invocation ID was empty".to_owned());
        }
        let read_events = wait_for_test_events(&host_activity.0, 2).await?;
        require_host_event(&read_events[0], "started", "repo.status")?;
        require_host_event(&read_events[1], "tool_completed", "repo.status")?;
        let read_output = event_tool_output(&read_events[1])?;
        if read_output.is_error || read_output.content.len() != 1 {
            return Err(
                "repo.status did not return one successful ToolOutput content item".to_owned(),
            );
        }
        let ToolContent::Json(read_value) = &read_output.content[0] else {
            return Err("repo.status did not return structured JSON".to_owned());
        };
        if read_value["status"] != "ok"
            || read_value["consistency"] != "best_effort"
            || read_value["sparse_index_flags"] != "not_enumerated"
        {
            return Err("repo.status returned the wrong normalized result shape".to_owned());
        }
        let entries = read_value["entries"]
            .as_array()
            .ok_or_else(|| "repo.status entries were not an array".to_owned())?;
        let staged = entries.iter().find(|entry| {
            status_entry_path(entry) == Some("tracked.txt")
                && entry["index_state"] == "modified"
                && entry["worktree_state"] == "unmodified"
        });
        let unstaged = entries.iter().find(|entry| {
            status_entry_path(entry) == Some("nested/ordinary.txt")
                && entry["index_state"] == "unmodified"
                && entry["worktree_state"] == "modified"
        });
        if staged.is_none() || unstaged.is_none() {
            return Err(
                "repo.status did not reflect the fresh staged and unstaged fixture".to_owned(),
            );
        }
        if live_git_state(&git, &fixture.0, &branch_name)? != before
            || current_host_generation_tuple(app.state::<DesktopAppState>().inner())
                != generations_before_read
            || app.state::<DesktopAppState>().persistence_namespace()
                != conversation_namespace_before_read
            || serde_json::to_value(
                app.state::<DesktopAppState>()
                    .persistence
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .presentation(),
            )
            .map_err(|error| format!("conversation presentation serialization failed: {error}"))?
                != conversation_presentation_before_read
        {
            return Err("repo.status changed protected repository or Desktop state".to_owned());
        }
        if app
            .state::<DesktopAppState>()
            .host_invocation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .state()
            != CoordinatorState::Idle
        {
            return Err("coordinator did not return to Idle after repo.status".to_owned());
        }
        println!("RAH_HOST_EXPLICIT_LIVE_REPO_STATUS_STARTED=1");
        println!("RAH_HOST_EXPLICIT_LIVE_REPO_STATUS_COMPLETED=1");

        let review_snapshot = repository_snapshot(app.state())
            .await
            .map_err(|error| format!("production repository review snapshot failed: {error:?}"))?;
        let review_id = match review_snapshot.review {
            StagedReviewPresentation::ReviewAvailable {
                review_id: Some(review_id),
                can_authorize: true,
                authorization_state: CommitAuthorizationPresentation::ReadyToAuthorize,
            } => review_id,
            other => return Err(format!("fixture did not expose a valid review: {other:?}")),
        };
        let authorization = repository_authorize_commit_review(app.state(), review_id)
            .await
            .map_err(|error| format!("production review authorization failed: {error:?}"))?;
        if authorization.authorization_state != CommitAuthorizationPresentation::AuthorizedPending
            || get_effective_authority_snapshot(app.state()).reviewed_commit
                != super::effective_authority::ReviewedCommitState::AuthorizedPending
        {
            return Err(
                "reviewed authorization was not pending before branch preparation".to_owned(),
            );
        }
        let reviewed_commit_control = app
            .state::<DesktopAppState>()
            .commit_capability
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .map(|capability| Arc::clone(&capability.control))
            .ok_or_else(|| "Desktop reviewed commit control was not composed".to_owned())?;

        let branch_tool_generations =
            current_host_generation_tuple(app.state::<DesktopAppState>().inner());
        let branch_conversation_namespace = app.state::<DesktopAppState>().persistence_namespace();
        let branch_snapshot_before_prepare = get_effective_authority_snapshot(app.state());
        let prepared = host_prepare_repo_create_branch(
            HostPrepareBranchRequest {
                name: branch_name.clone(),
            },
            app.handle().clone(),
            app.state(),
        )
        .map_err(|error| format!("production branch prepare failed: {error:?}"))?;
        let prepare_events = wait_for_test_events(&host_activity.0, 3).await?;
        require_host_event(
            &prepare_events[2],
            "prepared",
            REPOSITORY_CREATE_BRANCH_TOOL_NAME,
        )?;
        if prepared.ticket_id.is_empty()
            || prepared.ticket_id == branch_name
            || prepared.ticket_id.len() > 256
            || prepared.review.branch != branch_name
            || prepared.review.operation != "Create local branch"
            || !prepared
                .review
                .non_effect
                .contains("Does not switch branches")
            || !prepared
                .review
                .effect
                .contains("one new local branch reference")
            || live_target_exists(&git, &fixture.0, &branch_name)?
            || live_git_state(&git, &fixture.0, &branch_name)? != before
        {
            return Err(
                "branch prepare did not remain zero effect with the exact review".to_owned(),
            );
        }
        if app
            .state::<DesktopAppState>()
            .host_invocation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .state()
            != CoordinatorState::HostPrepared
        {
            return Err("coordinator was not HostPrepared after branch prepare".to_owned());
        }
        if !matches!(
            app.state::<DesktopAppState>().start_chat(),
            Err(FrontendError::HostInvocationBusy)
        ) {
            return Err("production model-start admission did not reject HostPrepared".to_owned());
        }
        println!("RAH_HOST_EXPLICIT_LIVE_BRANCH_PREPARED=1");

        if prepared.review.branch != branch_name
            || get_effective_authority_snapshot(app.state()).status
                != SnapshotStatus::ConnectedCurrent
            || current_host_generation_tuple(app.state::<DesktopAppState>().inner())
                != branch_tool_generations
            || app.state::<DesktopAppState>().persistence_namespace()
                != branch_conversation_namespace
            || get_effective_authority_snapshot(app.state())
                .effective_tools
                .iter()
                .find(|tool| tool.public_tool_name == REPOSITORY_CREATE_BRANCH_TOOL_NAME)
                .is_none_or(|tool| tool.host_invocation.eligible)
        {
            return Err(
                "branch preparation changed currentness or advertised eligibility".to_owned(),
            );
        }
        if branch_snapshot_before_prepare
            .effective_tools
            .iter()
            .find(|tool| tool.public_tool_name == REPOSITORY_CREATE_BRANCH_TOOL_NAME)
            .is_none_or(|tool| !tool.host_invocation.eligible)
        {
            return Err("branch was not eligible immediately before prepare".to_owned());
        }

        let pre_confirm_head_oid = live_git_text(&git, &fixture.0, &["rev-parse", "HEAD"])?
            .trim()
            .to_owned();
        if pre_confirm_head_oid != before.head_oid.trim() {
            return Err("pre-confirm HEAD changed before the possible-effect boundary".to_owned());
        }
        println!("RAH_HOST_EXPLICIT_LIVE_BRANCH_EFFECT_BOUNDARY=1");

        let confirmed = host_confirm_tool_invocation(
            HostConfirmRequest {
                ticket_id: prepared.ticket_id,
            },
            app.handle().clone(),
            app.state(),
        )
        .await
        .map_err(|error| format!("production branch confirm failed: {error:?}"))?;
        if confirmed.invocation_id.is_empty() {
            return Err(
                "branch HostExplicit confirmation returned an empty invocation ID".to_owned(),
            );
        }

        let mut branch_output_marker = "unavailable".to_owned();
        let mut observed_ref = "unavailable".to_owned();
        let mut observed_reflog = "unavailable".to_owned();
        macro_rules! fail_after_branch_start {
            ($message:expr) => {{
                let message = $message.to_owned();
                shutdown_live_state(app.state::<DesktopAppState>().inner()).await;
                eprintln!("RAH_HOST_EXPLICIT_LIVE_POST_START_FAILURE={message}");
                eprintln!("RAH_HOST_EXPLICIT_LIVE_BRANCH={branch_name}");
                eprintln!("RAH_HOST_EXPLICIT_LIVE_OID={pre_confirm_head_oid}");
                eprintln!("RAH_HOST_EXPLICIT_LIVE_TOOL_OUTPUT={branch_output_marker}");
                eprintln!("RAH_HOST_EXPLICIT_LIVE_REF={observed_ref}");
                eprintln!("RAH_HOST_EXPLICIT_LIVE_REFLOG={observed_reflog}");
                std::mem::forget(fixture);
                return Err(message);
            }};
        }
        let branch_events = match wait_for_test_events(&host_activity.0, 5).await {
            Ok(events) => events,
            Err(error) => fail_after_branch_start!(error),
        };
        if let Err(error) = require_host_event(
            &branch_events[3],
            "started",
            REPOSITORY_CREATE_BRANCH_TOOL_NAME,
        ) {
            fail_after_branch_start!(error);
        }
        if let Err(error) = require_host_event(
            &branch_events[4],
            "tool_completed",
            REPOSITORY_CREATE_BRANCH_TOOL_NAME,
        ) {
            fail_after_branch_start!(error);
        }
        let branch_output = match event_tool_output(&branch_events[4]) {
            Ok(output) => output,
            Err(error) => fail_after_branch_start!(error),
        };
        branch_output_marker = serde_json::to_string(&branch_output)
            .unwrap_or_else(|_| "serialization-failed".to_owned());
        if !matches!(
            branch_result_classification(&branch_output),
            BranchActivityClassification::ProvenSafe
        ) || branch_output.is_error
        {
            fail_after_branch_start!("branch result was not strictly ProvenSafe");
        }
        let [ToolContent::Json(branch_value)] = branch_output.content.as_slice() else {
            fail_after_branch_start!("branch result was not one structured JSON value");
        };
        if branch_value["status"] != "branch_created_verified"
            || branch_value["uncertain"] != false
            || branch_value["name"] != branch_name
            || branch_value["oid"] != pre_confirm_head_oid
        {
            fail_after_branch_start!("branch result did not match the prepared name and HEAD OID");
        }

        let after = match live_git_state(&git, &fixture.0, &branch_name) {
            Ok(state) => state,
            Err(error) => fail_after_branch_start!(error),
        };
        let target_ref = format!("refs/heads/{branch_name}");
        let target_heads = match live_git_text(
            &git,
            &fixture.0,
            &["for-each-ref", "--format=%(refname)", &target_ref],
        ) {
            Ok(value) => value,
            Err(error) => fail_after_branch_start!(error),
        };
        observed_ref = match live_git_text(&git, &fixture.0, &["rev-parse", &target_ref]) {
            Ok(value) => value.trim().to_owned(),
            Err(error) => fail_after_branch_start!(error),
        };
        observed_reflog = match live_git_text(
            &git,
            &fixture.0,
            &[
                "reflog",
                "show",
                "--format=%gn%x09%ge%x09%gs",
                "-n",
                "1",
                &target_ref,
            ],
        ) {
            Ok(value) => value.trim().to_owned(),
            Err(error) => fail_after_branch_start!(error),
        };
        let target_tracking = match live_git_text(
            &git,
            &fixture.0,
            &[
                "for-each-ref",
                "--format=%(refname:short) %(upstream:short)",
                &target_ref,
            ],
        ) {
            Ok(value) => value.trim().to_owned(),
            Err(error) => fail_after_branch_start!(error),
        };
        if target_heads.lines().count() != 1
            || target_heads.trim() != target_ref
            || observed_ref != pre_confirm_head_oid
            || observed_reflog != "RAH Host\trah-host@example.invalid\tRAH create local branch"
            || target_tracking != branch_name
            || after != before
        {
            fail_after_branch_start!("verified branch effect changed protected Git state");
        }
        let after_snapshot = get_effective_authority_snapshot(app.state());
        let review_preserved = reviewed_commit_control.has_pending_authorization().await
            && after_snapshot.reviewed_commit
                == super::effective_authority::ReviewedCommitState::AuthorizedPending;
        if after_snapshot.status != SnapshotStatus::ConnectedCurrent
            || !review_preserved
            || current_host_generation_tuple(app.state::<DesktopAppState>().inner())
                != branch_tool_generations
            || app.state::<DesktopAppState>().persistence_namespace()
                != branch_conversation_namespace
            || !app
                .state::<DesktopAppState>()
                .provider_activation
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .is_none()
            || !refresh_events
                .0
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .is_empty()
        {
            fail_after_branch_start!(
                "verified branch effect changed review, currentness, or providers"
            );
        }
        let after_branch_tool = match after_snapshot
            .effective_tools
            .iter()
            .find(|tool| tool.public_tool_name == REPOSITORY_CREATE_BRANCH_TOOL_NAME)
        {
            Some(tool) => tool,
            None => {
                fail_after_branch_start!("repo.create-branch disappeared after verified success")
            }
        };
        if !after_branch_tool.host_invocation.eligible
            || after_branch_tool.effect_class != EffectClass::RepositoryMutation
            || after_branch_tool.authority_category
                != AuthorityCategory::RepositoryLocalBranchCreation
            || after_branch_tool.permission != PermissionLevel::Execute
            || !after_branch_tool.repository_bound
        {
            fail_after_branch_start!("repo.create-branch was not still advertised and eligible");
        }
        if app
            .state::<DesktopAppState>()
            .host_invocation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .state()
            != CoordinatorState::Idle
        {
            fail_after_branch_start!("coordinator did not return to Idle after branch completion");
        }
        println!("RAH_HOST_EXPLICIT_LIVE_BRANCH_STARTED=1");
        println!("RAH_HOST_EXPLICIT_LIVE_BRANCH_COMPLETED=1");
        println!("RAH_HOST_EXPLICIT_LIVE_MODEL_TURN_STARTED=0");
        println!("RAH_HOST_EXPLICIT_LIVE_MODEL_TOOL_REQUESTED=0");
        println!("RAH_HOST_EXPLICIT_LIVE_MODEL_TOOL_STARTED=0");
        println!("RAH_HOST_EXPLICIT_LIVE_MODEL_TOOL_FINISHED=0");
        println!("RAH_HOST_EXPLICIT_LIVE_BRANCH={branch_name}");
        println!("RAH_HOST_EXPLICIT_LIVE_OID={pre_confirm_head_oid}");
        app.unlisten(host_activity.1);
        app.unlisten(refresh_events.1);
        shutdown_live_state(app.state::<DesktopAppState>().inner()).await;
        println!("RAH_HOST_EXPLICIT_LIVE_OK");
        Ok(())
    }

    #[cfg(windows)]
    #[tokio::test(flavor = "current_thread")]
    #[ignore = "requires the certified Windows Codex live gate"]
    async fn windows_live_desktop_hostexplicit_repo_patch() -> Result<(), String> {
        let fixture = TestRepository::git_repository(GitRepositoryState::Clean);
        let storage = TestRepository::new();
        let git = selected_git_executable()
            .map_err(|error| format!("Git discovery failed: {error:?}"))?;
        let target = fixture.0.join("tracked.txt");
        let sentinel = fixture.0.join("nested/ordinary.txt");
        let preimage = b"alpha\nRAH_PATCH_OLD\nomega\n".to_vec();
        let postimage = b"alpha\nRAH_PATCH_NEW\nomega\n".to_vec();

        fs::write(&target, &preimage).map_err(|_| "patch target setup failed".to_owned())?;
        let run_git = |arguments: &[&str]| -> Result<(), String> {
            let status = Command::new(&git)
                .args(arguments)
                .current_dir(&fixture.0)
                .status()
                .map_err(|_| "Git fixture command failed to start".to_owned())?;
            if status.success() {
                Ok(())
            } else {
                Err("Git fixture command failed".to_owned())
            }
        };
        run_git(&["add", "tracked.txt"])?;
        run_git(&["commit", "--quiet", "-m", "patch preimage"])?;
        fs::write(&sentinel, b"RAH_PATCH_SENTINEL\n")
            .map_err(|_| "review sentinel setup failed".to_owned())?;
        run_git(&["add", "nested/ordinary.txt"])?;

        let before_target = fs::read(&target).map_err(|_| "target read failed".to_owned())?;
        let before_target_identity = live_file_identity(&target)?;
        let before_target_sha256 = live_sha256(&before_target);
        let before_sentinel = fs::read(&sentinel).map_err(|_| "sentinel read failed".to_owned())?;
        let before_git = live_git_state(&git, &fixture.0, "__rah_no_excluded_branch__")?;
        if before_target != preimage
            || before_target_sha256 != live_sha256(&preimage)
            || live_patch_temporary_count(&fixture.0)? != 0
        {
            return Err("fixture did not start in the expected zero-effect state".to_owned());
        }

        let repository = DesktopRepository::new(&git, &fixture.0)
            .map_err(|_| "Desktop repository construction failed".to_owned())?;
        let app = tauri::Builder::default()
            .any_thread()
            .manage(DesktopAppState::new(storage.0.clone()))
            .build(tauri::generate_context!())
            .map_err(|error| format!("Desktop test app construction failed: {error}"))?;
        let host_activity = listen_for_test_event(app.handle(), "host_activity_event");
        let refresh_events = listen_for_test_event(app.handle(), "repository_snapshot_refresh");
        replace_selected_repository(app.state::<DesktopAppState>().inner(), repository);
        set_commit_identity(
            app.handle().clone(),
            app.state(),
            "RAH Host Patch Live Test".to_owned(),
            "rah-host-patch@example.invalid".to_owned(),
        )
        .map_err(|error| format!("Desktop commit identity setup failed: {error:?}"))?;
        connect_codex(app.state())
            .await
            .map_err(|error| format!("production Desktop connection failed: {error:?}"))?;

        let connected_snapshot = get_effective_authority_snapshot(app.state());
        let find_tool = |name: &str| {
            connected_snapshot
                .effective_tools
                .iter()
                .find(|tool| tool.public_tool_name == name)
        };
        let patch_tool = find_tool("repo.patch")
            .ok_or_else(|| "repo.patch was not in the current Effective Authority".to_owned())?;
        if connected_snapshot.status != SnapshotStatus::ConnectedCurrent
            || !patch_tool.host_invocation.eligible
            || patch_tool.host_invocation.kind != Some(HostInvocationKind::RepoPatch)
            || patch_tool.effect_class != EffectClass::RepositoryMutation
            || patch_tool.authority_category != AuthorityCategory::RepositoryContentMutation
            || patch_tool.permission != PermissionLevel::Execute
            || !patch_tool.repository_bound
        {
            return Err("repo.patch was not connected-current HostExplicit eligible".to_owned());
        }
        let multi_file_tool = find_tool("repo.edit-files").ok_or_else(|| {
            "repo.edit-files was not in the current Effective Authority".to_owned()
        })?;
        if !multi_file_tool.host_invocation.eligible
            || multi_file_tool.host_invocation.kind != Some(HostInvocationKind::RepoEditFiles)
        {
            return Err("repo.edit-files was not HostExplicit eligible".to_owned());
        }
        let external_effective = connected_snapshot
            .effective_tools
            .iter()
            .filter(|tool| {
                matches!(
                    tool.source_kind,
                    SourceKind::Mcp | SourceKind::ProcessPlugin
                )
            })
            .count();
        if external_effective != 0
            || connected_snapshot.configured.configured_provider_count != 0
            || app
                .state::<DesktopAppState>()
                .provider_activation
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .is_some()
            || app
                .state::<DesktopAppState>()
                .trusted_profile
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .is_some()
        {
            return Err("unexpected external provider or Trusted Profile state".to_owned());
        }

        let review_snapshot = repository_snapshot(app.state())
            .await
            .map_err(|error| format!("review snapshot failed: {error:?}"))?;
        let review_id = match review_snapshot.review {
            StagedReviewPresentation::ReviewAvailable {
                review_id: Some(review_id),
                can_authorize: true,
                authorization_state: CommitAuthorizationPresentation::ReadyToAuthorize,
            } => review_id,
            other => return Err(format!("fixture did not expose a review: {other:?}")),
        };
        let original_review_id = review_id.clone();
        let authorization = repository_authorize_commit_review(app.state(), review_id)
            .await
            .map_err(|error| format!("review authorization failed: {error:?}"))?;
        if authorization.authorization_state != CommitAuthorizationPresentation::AuthorizedPending
            || get_effective_authority_snapshot(app.state()).reviewed_commit
                != super::effective_authority::ReviewedCommitState::AuthorizedPending
        {
            return Err("reviewed authorization was not pending before preparation".to_owned());
        }
        let reviewed_commit_control = app
            .state::<DesktopAppState>()
            .commit_capability
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .map(|capability| Arc::clone(&capability.control))
            .ok_or_else(|| "reviewed commit control was not composed".to_owned())?;

        let before_prepare_git = live_git_state(&git, &fixture.0, "__rah_no_excluded_branch__")?;
        let before_prepare_generations =
            current_host_generation_tuple(app.state::<DesktopAppState>().inner());
        let before_prepare_namespace = app.state::<DesktopAppState>().persistence_namespace();
        if before_prepare_git != before_git
            || before_prepare_generations[0] == 0
            || before_prepare_namespace.is_empty()
        {
            return Err("preparation baseline was not stable".to_owned());
        }

        let prepared = host_prepare_repo_patch(
            HostPreparePatchRequest {
                path: "tracked.txt".to_owned(),
                expected_old_text: "RAH_PATCH_OLD".to_owned(),
                replacement_text: "RAH_PATCH_NEW".to_owned(),
            },
            app.handle().clone(),
            app.state(),
        )
        .await
        .map_err(|error| format!("production repo.patch Prepare failed: {error:?}"))?;
        let prepare_events = wait_for_test_events(&host_activity.0, 1).await?;
        require_host_event(&prepare_events[0], "prepared", "repo.patch")?;
        let prepare_event_json = serde_json::to_string(&prepare_events[0])
            .map_err(|_| "Prepare activity serialization failed".to_owned())?;
        if prepare_events[0].get("review").is_some()
            || prepare_events[0].get("result").is_some()
            || prepare_event_json.contains("RAH_PATCH_OLD")
            || prepare_event_json.contains("RAH_PATCH_NEW")
        {
            return Err("Prepare activity exposed patch review or source text".to_owned());
        }
        if prepared.ticket_id.is_empty()
            || prepared.ticket_id == "tracked.txt"
            || prepared.ticket_id.len() > 256
        {
            return Err("Prepare did not return one opaque bounded ticket".to_owned());
        }
        let expected_review = serde_json::json!({
            "operation": "repo.patch",
            "path": "tracked.txt",
            "replacement_count": 1,
            "changed_range": {"start": 6, "end": 19, "length": 13},
            "old_text_escaped": "RAH_PATCH_OLD",
            "replacement_text_escaped": "RAH_PATCH_NEW",
            "bom": "absent",
            "preimage_eof": "final_newline",
            "postimage_eof": "final_newline",
            "preimage_eof_marker": "EOF after final newline",
            "postimage_eof_marker": "EOF after final newline",
            "intended_effect": "replace exactly one literal match in the selected tracked worktree file",
            "non_effects": [
                "preparation performs no target replacement",
                "preparation performs no temporary-file write",
                "the Git index, HEAD, refs, and history remain unchanged",
                "other paths remain unchanged",
                "no shell, process, network, or provider action occurs"
            ],
            "unchanged_context": "unchanged surrounding context omitted",
            "preimage_sha256": before_target_sha256,
            "preimage_byte_length": preimage.len(),
            "postimage_sha256": live_sha256(&postimage),
            "postimage_byte_length": postimage.len()
        });
        if serde_json::to_value(&prepared.review)
            .map_err(|_| "Prepare review serialization failed".to_owned())?
            != expected_review
            || prepared.review.preimage_sha256() != before_target_sha256
            || prepared.review.preimage_byte_length() != preimage.len()
            || prepared.review.postimage_sha256() != live_sha256(&postimage)
            || prepared.review.postimage_byte_length() != postimage.len()
        {
            return Err("Prepare returned an unexpected exact RepositoryPatchReview".to_owned());
        }
        if app
            .state::<DesktopAppState>()
            .host_invocation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .state()
            != CoordinatorState::HostPrepared
        {
            return Err("coordinator was not HostPrepared after Prepare".to_owned());
        }
        if !matches!(
            app.state::<DesktopAppState>().start_chat(),
            Err(FrontendError::HostInvocationBusy)
        ) {
            return Err("model-start admission did not reject HostPrepared".to_owned());
        }
        let after_prepare_target =
            fs::read(&target).map_err(|_| "target read failed".to_owned())?;
        let after_prepare_git = live_git_state(&git, &fixture.0, "__rah_no_excluded_branch__")?;
        if after_prepare_target != before_target
            || live_file_identity(&target)? != before_target_identity
            || after_prepare_git != before_prepare_git
            || fs::read(&sentinel).map_err(|_| "sentinel read failed".to_owned())?
                != before_sentinel
            || current_host_generation_tuple(app.state::<DesktopAppState>().inner())
                != before_prepare_generations
            || app.state::<DesktopAppState>().persistence_namespace() != before_prepare_namespace
            || !reviewed_commit_control.has_pending_authorization().await
            || get_effective_authority_snapshot(app.state()).reviewed_commit
                != super::effective_authority::ReviewedCommitState::AuthorizedPending
            || live_patch_temporary_count(&fixture.0)? != 0
        {
            return Err("Prepare was not zero effect".to_owned());
        }
        println!("RAH_DESKTOP_PATCH_PREPARE_TOOL_EXECUTIONS=0");
        println!("RAH_DESKTOP_PATCH_PREPARE_REPLACEMENTS=0");
        println!("RAH_DESKTOP_PATCH_PREPARED=1");

        let confirmed = host_confirm_tool_invocation(
            HostConfirmRequest {
                ticket_id: prepared.ticket_id,
            },
            app.handle().clone(),
            app.state(),
        )
        .await
        .map_err(|error| format!("production repo.patch Confirm failed: {error:?}"))?;
        if confirmed.invocation_id.is_empty() {
            return Err("Confirm returned an empty invocation ID".to_owned());
        }

        macro_rules! fail_after_patch_start {
            ($message:expr) => {{
                let message = $message.to_owned();
                shutdown_live_state(app.state::<DesktopAppState>().inner()).await;
                eprintln!("RAH_DESKTOP_PATCH_POST_START_FAILURE={message}");
                std::mem::forget(fixture);
                return Err(message);
            }};
        }

        let events = match wait_for_test_events(&host_activity.0, 3).await {
            Ok(events) => events,
            Err(error) => fail_after_patch_start!(error),
        };
        if events.len() != 3 {
            fail_after_patch_start!("unexpected HostExplicit event count");
        }
        for event in [&events[1], &events[2]] {
            if event.get("review").is_some() {
                fail_after_patch_start!("patch activity exposed a review");
            }
            let serialized = match serde_json::to_string(event) {
                Ok(serialized) => serialized,
                Err(_) => fail_after_patch_start!("patch activity serialization failed"),
            };
            if serialized.contains("RAH_PATCH_OLD") || serialized.contains("RAH_PATCH_NEW") {
                fail_after_patch_start!("patch activity exposed source text");
            }
        }
        if let Err(error) = require_host_event(&events[1], "started", "repo.patch") {
            fail_after_patch_start!(error);
        }
        if let Err(error) = require_host_event(&events[2], "tool_completed", "repo.patch") {
            fail_after_patch_start!(error);
        }
        let output = match event_tool_output(&events[2]) {
            Ok(output) => output,
            Err(error) => fail_after_patch_start!(error),
        };
        if output.is_error
            || !matches!(
                classify_repository_patch_output(&output),
                RepositoryPatchResultClassification::ChangedVerified
            )
        {
            fail_after_patch_start!("repo.patch did not classify as ChangedVerified");
        }
        let [ToolContent::Json(result)] = output.content.as_slice() else {
            fail_after_patch_start!("repo.patch result was not one structured JSON value");
        };
        if result
            != &serde_json::json!({
                "status": "ok",
                "changed": true,
                "uncertain": false,
                "reason": "none"
            })
        {
            fail_after_patch_start!("repo.patch result contract was not exact");
        }

        let after_target = match fs::read(&target) {
            Ok(bytes) => bytes,
            Err(_) => fail_after_patch_start!("target read failed after Confirm"),
        };
        let after_git = match live_git_state(&git, &fixture.0, "__rah_no_excluded_branch__") {
            Ok(state) => state,
            Err(error) => fail_after_patch_start!(error),
        };
        let worktree_names = match live_git_text(&git, &fixture.0, &["diff", "--name-only"]) {
            Ok(names) => names,
            Err(error) => fail_after_patch_start!(error),
        };
        let staged_names =
            match live_git_text(&git, &fixture.0, &["diff", "--cached", "--name-only"]) {
                Ok(names) => names,
                Err(error) => fail_after_patch_start!(error),
            };
        if after_target != postimage
            || live_sha256(&after_target) != prepared.review.postimage_sha256()
            || after_target.len() != prepared.review.postimage_byte_length()
            || worktree_names.trim() != "tracked.txt"
            || staged_names.trim() != "nested/ordinary.txt"
            || fs::read(&sentinel).unwrap_or_default() != before_sentinel
            || after_git.symbolic_head != before_git.symbolic_head
            || after_git.head_oid != before_git.head_oid
            || after_git.current_branch != before_git.current_branch
            || after_git.index_semantics != before_git.index_semantics
            || after_git.raw_staged_diff != before_git.raw_staged_diff
            || after_git.tracking != before_git.tracking
            || after_git.local_heads != before_git.local_heads
            || after_git.tags_and_remotes != before_git.tags_and_remotes
            || live_patch_temporary_count(&fixture.0).unwrap_or(usize::MAX) != 0
        {
            fail_after_patch_start!("repo.patch changed protected repository state");
        }
        let refresh = match wait_for_test_events(&refresh_events.0, 1).await {
            Ok(events) => events,
            Err(error) => fail_after_patch_start!(error),
        };
        if refresh.len() != 1 {
            fail_after_patch_start!("repository refresh count was not exactly one");
        }
        let after_snapshot = get_effective_authority_snapshot(app.state());
        let (
            fresh_authorization_is_ready,
            fresh_review_is_current,
            fresh_review_selector_is_distinct,
            fresh_commit_review_present,
        ) = {
            let state = app.state::<DesktopAppState>();
            let workflow = state
                .repository_workflow
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            (
                workflow.authorization == CommitAuthorizationPresentation::ReadyToAuthorize,
                workflow.review.as_ref().is_some_and(|review| {
                    review.repository_generation == before_prepare_generations[0]
                        && review.observation_generation == workflow.observation_generation
                        && review.complete
                        && review.binary_supported
                }),
                workflow
                    .review_selector
                    .as_deref()
                    .is_some_and(|selector| selector != original_review_id),
                workflow.commit_review.is_some(),
            )
        };
        if after_snapshot.status != SnapshotStatus::ConnectedCurrent
            || after_snapshot.reviewed_commit
                != super::effective_authority::ReviewedCommitState::ReadyToAuthorize
            || reviewed_commit_control.has_pending_authorization().await
            || !fresh_authorization_is_ready
            || !fresh_review_is_current
            || !fresh_review_selector_is_distinct
            || !fresh_commit_review_present
            || current_host_generation_tuple(app.state::<DesktopAppState>().inner())
                != before_prepare_generations
            || app.state::<DesktopAppState>().persistence_namespace() != before_prepare_namespace
            || app
                .state::<DesktopAppState>()
                .provider_activation
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .is_some()
            || after_snapshot.configured.configured_provider_count != 0
            || after_snapshot.effective_tools.iter().any(|tool| {
                matches!(
                    tool.source_kind,
                    SourceKind::Mcp | SourceKind::ProcessPlugin
                )
            })
            || app
                .state::<DesktopAppState>()
                .trusted_profile
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .is_some()
            || app
                .state::<DesktopAppState>()
                .host_invocation
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .state()
                != CoordinatorState::Idle
            || *app
                .state::<DesktopAppState>()
                .chat
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                != ChatState::Idle
            || app
                .state::<DesktopAppState>()
                .active_chat
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .is_some()
        {
            fail_after_patch_start!("patch success changed currentness or authorization state");
        }

        // ChangedVerified plus distinct exact preimage and postimage proves the one native replacement point was reached.
        println!("RAH_DESKTOP_PATCH_CONFIRM_TOOL_EXECUTIONS=1");
        println!("RAH_DESKTOP_PATCH_CONFIRM_REPLACEMENTS=1");
        println!("RAH_DESKTOP_PATCH_RESULT=changed_verified");
        println!("RAH_DESKTOP_PATCH_REVIEW_INVALIDATED=1");
        println!("RAH_DESKTOP_PATCH_MODEL_RUNTIME_STARTS=0");
        println!("RAH_DESKTOP_PATCH_MODEL_AGENT_REQUESTS=0");
        println!("RAH_DESKTOP_PATCH_MODEL_PROMPTS=0");
        println!("RAH_DESKTOP_PATCH_MODEL_TOOL_REQUESTED=0");
        println!("RAH_DESKTOP_PATCH_MODEL_TOOL_STARTED=0");
        println!("RAH_DESKTOP_PATCH_MODEL_TOOL_FINISHED=0");
        println!("RAH_DESKTOP_PATCH_MODEL_TOOL_LIFECYCLE=0/0/0");
        println!("RAH_DESKTOP_PATCH_MCP_PROVIDERS=0");
        println!("RAH_DESKTOP_PATCH_PROCESS_PLUGINS=0");
        println!("RAH_DESKTOP_PATCH_PRE_SHA256={before_target_sha256}");
        println!(
            "RAH_DESKTOP_PATCH_POST_SHA256={}",
            live_sha256(&after_target)
        );
        app.unlisten(host_activity.1);
        app.unlisten(refresh_events.1);
        shutdown_live_state(app.state::<DesktopAppState>().inner()).await;
        println!("RAH_DESKTOP_PATCH_LIVE_OK");
        Ok(())
    }

    #[tokio::test(flavor = "current_thread")]
    #[ignore = "requires the certified Windows Codex live gate"]
    async fn windows_live_desktop_hostexplicit_multi_file_edit() -> Result<(), String> {
        let fixture = TestRepository::new();
        let storage = TestRepository::new();
        let git = selected_git_executable()
            .map_err(|error| format!("Git discovery failed: {error:?}"))?;
        fs::remove_dir_all(fixture.0.join(".git"))
            .map_err(|error| format!("placeholder Git metadata removal failed: {error}"))?;
        fs::remove_file(fixture.0.join("inside.txt"))
            .map_err(|error| format!("placeholder file removal failed: {error}"))?;

        type Replacement = (&'static str, &'static str);
        type Specification = (
            &'static str,
            &'static str,
            &'static str,
            &'static [Replacement],
        );
        let specifications: [Specification; 4] = [
            (
                "a.txt",
                "A_PREIMAGE\n",
                "A_POSTIMAGE\n",
                &[("A_PREIMAGE", "A_POSTIMAGE")],
            ),
            (
                "b.txt",
                "B_PREIMAGE_1\nB_PREIMAGE_2\n",
                "B_POSTIMAGE_1\nB_POSTIMAGE_2\n",
                &[
                    ("B_PREIMAGE_1", "B_POSTIMAGE_1"),
                    ("B_PREIMAGE_2", "B_POSTIMAGE_2"),
                ],
            ),
            (
                "c.txt",
                "C_PREIMAGE\n",
                "C_POSTIMAGE\n",
                &[("C_PREIMAGE", "C_POSTIMAGE")],
            ),
            (
                "d.txt",
                "D_PREIMAGE\n",
                "D_POSTIMAGE\n",
                &[("D_PREIMAGE", "D_POSTIMAGE")],
            ),
        ];
        let run_git = |arguments: &[&str]| -> Result<(), String> {
            let status = Command::new(&git)
                .args(arguments)
                .current_dir(&fixture.0)
                .status()
                .map_err(|error| format!("Git fixture command failed to start: {error}"))?;
            if status.success() {
                Ok(())
            } else {
                Err(format!("Git fixture command failed: {arguments:?}"))
            }
        };
        run_git(&["init", "--quiet"])?;
        run_git(&["config", "user.email", "rah-multi-file@example.invalid"])?;
        run_git(&["config", "user.name", "RAH Multi-File Live Test"])?;
        for (path, preimage, _, _) in specifications {
            fs::write(fixture.0.join(path), preimage.as_bytes())
                .map_err(|error| format!("fixture target setup failed for {path}: {error}"))?;
        }
        run_git(&["add", "a.txt", "b.txt", "c.txt", "d.txt"])?;
        run_git(&[
            "commit",
            "--quiet",
            "-m",
            "multi-file HostExplicit preimage",
        ])?;

        let target_paths = ["a.txt", "b.txt", "c.txt", "d.txt"];
        let caller_order = ["d.txt", "b.txt", "a.txt", "c.txt"];
        let before_directory_entries = live_directory_entries(&fixture.0)?;
        let before_index = fs::read(fixture.0.join(".git").join("index"))
            .map_err(|error| format!("Git index baseline read failed: {error}"))?;
        let before_git = live_git_state(&git, &fixture.0, "__rah_no_excluded_branch__")?;
        let tracked = live_git_text(
            &git,
            &fixture.0,
            &[
                "ls-files",
                "--error-unmatch",
                "a.txt",
                "b.txt",
                "c.txt",
                "d.txt",
            ],
        )?;
        if tracked.lines().collect::<BTreeSet<_>>()
            != target_paths.iter().copied().collect::<BTreeSet<_>>()
            || live_git_text(&git, &fixture.0, &["ls-tree", "-r", "--name-only", "HEAD"])?
                .lines()
                .collect::<BTreeSet<_>>()
                != target_paths.iter().copied().collect::<BTreeSet<_>>()
            || !live_git_exit_success(&git, &fixture.0, &["diff", "--quiet"])?
            || !live_git_exit_success(&git, &fixture.0, &["diff", "--cached", "--quiet"])?
            || !before_git.status.is_empty()
            || before_git.index_semantics.lines().count() != 4
            || live_multi_file_temporary_count(&fixture.0)? != 0
        {
            return Err("multi-file fixture was not clean and fully tracked".to_owned());
        }
        let before_targets = specifications
            .iter()
            .map(|(path, preimage, postimage, _)| {
                let bytes = fs::read(fixture.0.join(path))
                    .map_err(|error| format!("target baseline read failed for {path}: {error}"))?;
                if bytes != preimage.as_bytes() {
                    return Err(format!("target preimage mismatch for {path}"));
                }
                Ok((
                    (*path).to_owned(),
                    bytes.clone(),
                    live_sha256(&bytes),
                    bytes.len(),
                    live_file_identity(&fixture.0.join(path))?,
                    postimage.as_bytes().to_vec(),
                ))
            })
            .collect::<Result<Vec<_>, String>>()?;
        let before_target_identities = before_targets
            .iter()
            .map(|(_, _, _, _, identity, _)| *identity)
            .collect::<Vec<_>>();

        clear_live_test_multi_file_tool_executions(&fixture.0);
        clear_live_test_multi_file_native_attempts(&fixture.0);
        let branch_authority = RepositoryBranchCreationAuthority::new(&git, &fixture.0)
            .map_err(|error| format!("branch authority construction failed: {error}"))?;
        let repository = DesktopRepository::new_with_authorities(
            &git,
            &fixture.0,
            None,
            None,
            None,
            Some(branch_authority),
        )
        .map_err(|error| format!("Desktop repository construction failed: {error:?}"))?;
        let app = tauri::Builder::default()
            .any_thread()
            .manage(DesktopAppState::new(storage.0.clone()))
            .build(tauri::generate_context!())
            .map_err(|error| format!("Desktop test app construction failed: {error}"))?;
        let host_activity = listen_for_test_event(app.handle(), "host_activity_event");
        let refresh_events = listen_for_test_event(app.handle(), "repository_snapshot_refresh");
        let chat_events = listen_for_test_event(app.handle(), "chat_event");
        let model_activity = listen_for_test_event(app.handle(), "activity_event");
        replace_selected_repository(app.state::<DesktopAppState>().inner(), repository);
        set_commit_identity(
            app.handle().clone(),
            app.state(),
            "RAH Multi-File HostExplicit Live Test".to_owned(),
            "rah-multi-file@example.invalid".to_owned(),
        )
        .map_err(|error| format!("Desktop commit identity setup failed: {error:?}"))?;
        connect_codex(app.state())
            .await
            .map_err(|error| format!("production Desktop connection failed: {error:?}"))?;

        let connected_snapshot = get_effective_authority_snapshot(app.state());
        let eligible = connected_snapshot
            .effective_tools
            .iter()
            .filter(|tool| tool.host_invocation.eligible)
            .map(|tool| tool.public_tool_name.as_str())
            .collect::<BTreeSet<_>>();
        let expected_eligible = [
            "fs.read",
            "repo.file-info",
            "repo.status",
            "repo.diff",
            "repo.diff-staged",
            "repo.create-branch",
            "repo.patch",
            "repo.edit-files",
        ]
        .into_iter()
        .collect::<BTreeSet<_>>();
        let multi_file_tool = connected_snapshot
            .effective_tools
            .iter()
            .find(|tool| tool.public_tool_name == "repo.edit-files");
        let external_effective = connected_snapshot
            .effective_tools
            .iter()
            .filter(|tool| {
                matches!(
                    tool.source_kind,
                    SourceKind::Mcp | SourceKind::ProcessPlugin
                )
            })
            .count();
        let multi_file_eligible = multi_file_tool.is_some_and(|tool| {
            tool.host_invocation.eligible
                && tool.host_invocation.kind == Some(HostInvocationKind::RepoEditFiles)
                && tool.effect_class == EffectClass::RepositoryMutation
                && tool.authority_category == AuthorityCategory::RepositoryContentMutation
                && tool.permission == PermissionLevel::Execute
                && tool.repository_bound
        });
        let provider_activation_present = app
            .state::<DesktopAppState>()
            .provider_activation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_some();
        let trusted_profile_present = app
            .state::<DesktopAppState>()
            .trusted_profile
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_some();
        let coordinator_state = app
            .state::<DesktopAppState>()
            .host_invocation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .state();
        let chat_state = *app
            .state::<DesktopAppState>()
            .chat
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let active_chat_present = app
            .state::<DesktopAppState>()
            .active_chat
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_some();
        if connected_snapshot.status != SnapshotStatus::ConnectedCurrent
            || eligible != expected_eligible
            || !multi_file_eligible
            || external_effective != 0
            || connected_snapshot.configured.configured_provider_count != 0
            || provider_activation_present
            || trusted_profile_present
            || coordinator_state != CoordinatorState::Idle
            || chat_state != ChatState::Idle
            || active_chat_present
        {
            shutdown_live_state(app.state::<DesktopAppState>().inner()).await;
            return Err(format!(
                "connected-current baseline mismatch: status={:?} eligible={eligible:?} expected={expected_eligible:?} multi_file_eligible={multi_file_eligible} external={external_effective} configured_providers={} provider_activation={provider_activation_present} trusted_profile={trusted_profile_present} coordinator={coordinator_state:?} chat={chat_state:?} active_chat={active_chat_present}",
                connected_snapshot.status, connected_snapshot.configured.configured_provider_count
            ));
        }
        let before_generations =
            current_host_generation_tuple(app.state::<DesktopAppState>().inner());
        let before_namespace = app.state::<DesktopAppState>().persistence_namespace();
        if before_generations[0] == 0 || before_namespace.is_empty() {
            shutdown_live_state(app.state::<DesktopAppState>().inner()).await;
            return Err(
                "connected-current generation or persistence baseline was empty".to_owned(),
            );
        }
        reset_startup_activation_counters();
        let operation_baseline = startup_activation_snapshot();
        if operation_baseline != StartupActivationCounters::default() {
            shutdown_live_state(app.state::<DesktopAppState>().inner()).await;
            return Err(format!(
                "operation baseline was not zero: {operation_baseline:?}"
            ));
        }

        let request = HostPrepareMultiFileEditRequest {
            targets: caller_order
                .iter()
                .map(|path| {
                    let (_, _, _, replacements) = specifications
                        .iter()
                        .find(|(candidate, _, _, _)| candidate == path)
                        .expect("caller target has a specification");
                    HostPrepareMultiFileEditTarget {
                        path: (*path).to_owned(),
                        replacements: replacements
                            .iter()
                            .map(|(old, new)| HostPrepareMultiFileEditReplacement {
                                expected_old_text: (*old).to_owned(),
                                replacement_text: (*new).to_owned(),
                            })
                            .collect(),
                    }
                })
                .collect(),
        };
        let prepared = host_prepare_repo_edit_files(request, app.handle().clone(), app.state())
            .await
            .map_err(|error| format!("production repo.edit-files Prepare failed: {error:?}"))?;
        let prepare_events = wait_for_test_events(&host_activity.0, 1).await?;
        require_host_event(&prepare_events[0], "prepared", "repo.edit-files")?;
        let prepare_activity = serde_json::to_string(&prepare_events[0])
            .map_err(|error| format!("Prepare activity serialization failed: {error}"))?;
        let activity_id = prepare_events[0]
            .get("invocationId")
            .and_then(Value::as_str)
            .ok_or_else(|| "Prepare activity omitted invocationId".to_owned())?;
        if prepare_events[0].get("review").is_some()
            || prepare_events[0].get("result").is_some()
            || prepare_activity.contains(&prepared.ticket_id)
            || prepare_activity.contains("A_PREIMAGE")
            || prepare_activity.contains("A_POSTIMAGE")
            || prepare_activity.contains("expectedOldText")
            || prepare_activity.contains("replacementText")
            || prepare_activity.contains(&fixture.0.display().to_string())
            || activity_id == prepared.ticket_id
        {
            shutdown_live_state(app.state::<DesktopAppState>().inner()).await;
            return Err(format!(
                "Prepare activity redaction mismatch: ticket_id_present={} source_material_present={} raw_fields_present={} absolute_fixture_path_present={}",
                prepare_activity.contains(&prepared.ticket_id),
                prepare_activity.contains("A_PREIMAGE") || prepare_activity.contains("A_POSTIMAGE"),
                prepare_activity.contains("expectedOldText")
                    || prepare_activity.contains("replacementText"),
                prepare_activity.contains(&fixture.0.display().to_string())
            ));
        }
        if prepared.ticket_id.is_empty()
            || prepared.ticket_id.len() > 256
            || prepared.ticket_id == "repo.edit-files"
            || prepared.review.operation() != "repo.edit-files"
            || prepared.review.target_count() != 4
            || prepared.review.replacement_count() != 5
            || prepared
                .review
                .targets()
                .iter()
                .map(|target| target.path())
                .collect::<Vec<_>>()
                != target_paths
        {
            shutdown_live_state(app.state::<DesktopAppState>().inner()).await;
            return Err("Prepare review or ticket contract was incomplete".to_owned());
        }
        if prepared.review.matching()
            != "all literal matches resolve exactly once against the same original snapshot; duplicate, overlap, and no-op replacements are rejected"
            || prepared.review.unchanged_context()
                != "unchanged surrounding context omitted; complete changed material is retained"
            || prepared.review.intended_effect()
                != "replace complete postimages of the reviewed clean tracked files in host order"
            || prepared.review.non_atomic_warning()
                != "repo.edit-files is non-atomic; targets have independent native commit points"
            || prepared.review.non_effects().len() != 4
            || !prepared
                .review
                .non_effects()
                .iter()
                .any(|effect| effect.contains("no Tool execution or native replacement"))
            || !prepared
                .review
                .non_effects()
                .iter()
                .any(|effect| effect.contains("Stage, Unstage, and Commit"))
        {
            shutdown_live_state(app.state::<DesktopAppState>().inner()).await;
            return Err(
                "Prepare review semantics or protected non-effects were incomplete".to_owned(),
            );
        }
        for (ordinal, (path, preimage, pre_sha256, pre_len, _, postimage)) in
            before_targets.iter().enumerate()
        {
            let review_target = &prepared.review.targets()[ordinal];
            let expected = specifications
                .iter()
                .find(|(candidate, _, _, _)| candidate == path)
                .expect("review target has a specification");
            if review_target.ordinal() != ordinal
                || review_target.path() != path
                || review_target.target_identity().len() != 64
                || review_target.replacement_count() != expected.3.len()
                || review_target.preimage_sha256() != pre_sha256
                || review_target.preimage_byte_length() != *pre_len
                || review_target.postimage_sha256() != live_sha256(postimage)
                || review_target.postimage_byte_length() != postimage.len()
                || review_target.changed_ranges().len() != expected.3.len()
            {
                shutdown_live_state(app.state::<DesktopAppState>().inner()).await;
                return Err(format!("Prepare review evidence was incomplete for {path}"));
            }
            for (range, (old, new)) in review_target.changed_ranges().iter().zip(expected.3) {
                let start = String::from_utf8(preimage.clone())
                    .expect("fixture preimage is UTF-8")
                    .find(old)
                    .expect("replacement old text is present");
                if range.start() != start
                    || range.end() != start + old.len()
                    || range.length() != old.len()
                    || range.expected_old_text_escaped() != *old
                    || range.replacement_text_escaped() != *new
                {
                    shutdown_live_state(app.state::<DesktopAppState>().inner()).await;
                    return Err(format!(
                        "Prepare changed range evidence was incomplete for {path}"
                    ));
                }
            }
        }
        if live_directory_entries(&fixture.0)? != before_directory_entries
            || fs::read(fixture.0.join(".git").join("index"))
                .map_err(|error| format!("Git index Prepare read failed: {error}"))?
                != before_index
            || live_git_state(&git, &fixture.0, "__rah_no_excluded_branch__")? != before_git
            || live_multi_file_temporary_count(&fixture.0)? != 0
            || live_test_multi_file_tool_executions(&fixture.0) != 0
            || (0..4).any(|index| live_test_multi_file_native_attempts(&fixture.0, index) != 0)
            || before_targets
                .iter()
                .enumerate()
                .any(|(index, (path, bytes, _, _, _, _))| {
                    fs::read(fixture.0.join(path)).ok().as_deref() != Some(bytes.as_slice())
                        || live_file_identity(&fixture.0.join(path)).ok()
                            != Some(before_target_identities[index])
                })
            || current_host_generation_tuple(app.state::<DesktopAppState>().inner())
                != before_generations
            || app.state::<DesktopAppState>().persistence_namespace() != before_namespace
            || app
                .state::<DesktopAppState>()
                .host_invocation
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .state()
                != CoordinatorState::HostPrepared
        {
            shutdown_live_state(app.state::<DesktopAppState>().inner()).await;
            return Err("Prepare was not zero effect".to_owned());
        }
        println!("RAH_MULTI_FILE_HOSTEXPLICIT_PREPARE_TOOL_EXECUTIONS=0");
        println!("RAH_MULTI_FILE_HOSTEXPLICIT_PREPARE_NATIVE_ATTEMPTS=0");
        println!("RAH_MULTI_FILE_HOSTEXPLICIT_PREPARED=1");

        let ticket_id = prepared.ticket_id.clone();
        let confirmed = host_confirm_tool_invocation(
            HostConfirmRequest { ticket_id },
            app.handle().clone(),
            app.state(),
        )
        .await
        .map_err(|error| format!("production repo.edit-files Confirm failed: {error:?}"))?;
        if confirmed.invocation_id.is_empty() || confirmed.invocation_id != activity_id {
            shutdown_live_state(app.state::<DesktopAppState>().inner()).await;
            std::mem::forget(fixture);
            return Err("Confirm did not preserve the separate activity correlation ID".to_owned());
        }
        let events = wait_for_test_events(&host_activity.0, 3).await?;
        if events.len() != 3
            || require_host_event(&events[1], "started", "repo.edit-files").is_err()
            || require_host_event(&events[2], "tool_completed", "repo.edit-files").is_err()
        {
            shutdown_live_state(app.state::<DesktopAppState>().inner()).await;
            std::mem::forget(fixture);
            return Err(
                "HostExplicit lifecycle was not exactly prepared/started/completed".to_owned(),
            );
        }
        if events
            .iter()
            .any(|event| event.get("invocationId").and_then(Value::as_str) != Some(activity_id))
        {
            shutdown_live_state(app.state::<DesktopAppState>().inner()).await;
            std::mem::forget(fixture);
            return Err("HostExplicit activity correlation ID changed unexpectedly".to_owned());
        }
        for event in [&events[1], &events[2]] {
            let serialized = serde_json::to_string(event)
                .map_err(|error| format!("HostExplicit activity serialization failed: {error}"))?;
            if event.get("review").is_some()
                || serialized.contains("A_PREIMAGE")
                || serialized.contains("A_POSTIMAGE")
                || serialized.contains("expectedOldText")
                || serialized.contains("replacementText")
                || serialized.contains(&prepared.ticket_id)
                || serialized.contains(&fixture.0.display().to_string())
            {
                shutdown_live_state(app.state::<DesktopAppState>().inner()).await;
                std::mem::forget(fixture);
                return Err("Confirm activity was not redacted".to_owned());
            }
        }
        let output = event_tool_output(&events[2])?;
        let [ToolContent::Json(result)] = output.content.as_slice() else {
            shutdown_live_state(app.state::<DesktopAppState>().inner()).await;
            std::mem::forget(fixture);
            return Err("Confirm result was not one structured JSON value".to_owned());
        };
        let expected_effects = target_paths
            .iter()
            .map(|path| serde_json::json!({"path": path, "state": "committed_verified"}))
            .collect::<Vec<_>>();
        if output.is_error
            || classify_repository_multi_file_output(
                &output,
                &target_paths
                    .iter()
                    .map(|path| (*path).to_owned())
                    .collect::<Vec<_>>(),
            ) != MultiFileResultClassification::Ok
            || result != &serde_json::json!({"status": "ok", "effects": expected_effects})
            || live_test_multi_file_tool_executions(&fixture.0) != 1
            || (0..4).any(|index| live_test_multi_file_native_attempts(&fixture.0, index) != 1)
        {
            shutdown_live_state(app.state::<DesktopAppState>().inner()).await;
            std::mem::forget(fixture);
            return Err("Confirm was not one exact-once four-target effect".to_owned());
        }
        let after_targets = target_paths
            .iter()
            .map(|path| {
                let bytes = fs::read(fixture.0.join(path)).map_err(|error| {
                    format!("post-effect target read failed for {path}: {error}")
                })?;
                let expected = specifications
                    .iter()
                    .find(|(candidate, _, _, _)| candidate == path)
                    .expect("post-effect target has a specification");
                if bytes != expected.2.as_bytes() {
                    return Err(format!("postimage mismatch for {path}"));
                }
                Ok((
                    (*path).to_owned(),
                    bytes.clone(),
                    live_sha256(&bytes),
                    bytes.len(),
                ))
            })
            .collect::<Result<Vec<_>, String>>()?;
        let after_git = live_git_state(&git, &fixture.0, "__rah_no_excluded_branch__")?;
        let expected_worktree_names = target_paths.join("\n") + "\n";
        if after_git.index_semantics != before_git.index_semantics
            || after_git.head_oid != before_git.head_oid
            || after_git.symbolic_head != before_git.symbolic_head
            || after_git.current_branch != before_git.current_branch
            || after_git.local_heads != before_git.local_heads
            || after_git.tags_and_remotes != before_git.tags_and_remotes
            || after_git.all_refs != before_git.all_refs
            || fs::read(fixture.0.join(".git").join("index"))
                .map_err(|error| format!("Git index post-effect read failed: {error}"))?
                != before_index
            || !live_git_text(&git, &fixture.0, &["diff", "--cached", "--name-only"])?.is_empty()
            || live_git_text(&git, &fixture.0, &["diff", "--name-only"])? != expected_worktree_names
            || live_directory_entries(&fixture.0)? != before_directory_entries
            || live_multi_file_temporary_count(&fixture.0)? != 0
        {
            shutdown_live_state(app.state::<DesktopAppState>().inner()).await;
            std::mem::forget(fixture);
            return Err("protected Git state or exact worktree scope changed".to_owned());
        }
        let refresh = wait_for_test_events(&refresh_events.0, 1).await?;
        if refresh.len() != 1 {
            shutdown_live_state(app.state::<DesktopAppState>().inner()).await;
            std::mem::forget(fixture);
            return Err("repository refresh/reconciliation was not exactly once".to_owned());
        }
        let refreshed = repository_snapshot(app.state())
            .await
            .map_err(|error| format!("refreshed repository snapshot failed: {error:?}"))?;
        if refreshed.status_entries.len() != 4
            || refreshed.worktree_diff.len() != 4
            || !refreshed.staged_diff.is_empty()
        {
            shutdown_live_state(app.state::<DesktopAppState>().inner()).await;
            std::mem::forget(fixture);
            return Err(format!(
                "refresh did not observe exactly four unstaged worktree changes: path={} status_entries={} worktree_diff={} staged_diff={}",
                refreshed.path,
                refreshed.status_entries.len(),
                refreshed.worktree_diff.len(),
                refreshed.staged_diff.len()
            ));
        }
        let (
            refreshed_workflow_is_descriptive,
            coordinator_is_idle,
            chat_is_idle,
            active_chat_is_idle,
        ) = {
            let app_state = app.state::<DesktopAppState>();
            let refreshed_workflow = app_state
                .repository_workflow
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let coordinator_is_idle = app_state
                .host_invocation
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .state()
                == CoordinatorState::Idle;
            let chat_is_idle = *app_state
                .chat
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                == ChatState::Idle;
            let active_chat_is_idle = app_state
                .active_chat
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .is_none();
            (
                refreshed_workflow.authorization
                    == CommitAuthorizationPresentation::AuthorizationRevoked
                    && refreshed_workflow.commit_review.is_none(),
                coordinator_is_idle,
                chat_is_idle,
                active_chat_is_idle,
            )
        };
        if !refreshed_workflow_is_descriptive
            || !coordinator_is_idle
            || !chat_is_idle
            || !active_chat_is_idle
            || current_host_generation_tuple(app.state::<DesktopAppState>().inner())
                != before_generations
            || app.state::<DesktopAppState>().persistence_namespace() != before_namespace
            || get_effective_authority_snapshot(app.state()).status
                != SnapshotStatus::ConnectedCurrent
        {
            shutdown_live_state(app.state::<DesktopAppState>().inner()).await;
            std::mem::forget(fixture);
            return Err(
                "refresh or terminal Desktop state was not current and descriptive".to_owned(),
            );
        }
        let duplicate = host_confirm_tool_invocation(
            HostConfirmRequest {
                ticket_id: prepared.ticket_id.clone(),
            },
            app.handle().clone(),
            app.state(),
        )
        .await;
        if duplicate != Err(FrontendError::HostInvocationTicketInvalid)
            || live_test_multi_file_tool_executions(&fixture.0) != 1
            || (0..4).any(|index| live_test_multi_file_native_attempts(&fixture.0, index) != 1)
        {
            shutdown_live_state(app.state::<DesktopAppState>().inner()).await;
            std::mem::forget(fixture);
            return Err("consumed ticket was reusable".to_owned());
        }
        if !chat_events.0.lock().unwrap().is_empty() || !model_activity.0.lock().unwrap().is_empty()
        {
            shutdown_live_state(app.state::<DesktopAppState>().inner()).await;
            std::mem::forget(fixture);
            return Err("HostExplicit operation initiated model/chat activity".to_owned());
        }
        let activity_as_ticket = host_confirm_tool_invocation(
            HostConfirmRequest {
                ticket_id: activity_id.to_owned(),
            },
            app.handle().clone(),
            app.state(),
        )
        .await;
        let activity_as_cancel = host_cancel_tool_invocation(
            HostConfirmRequest {
                ticket_id: activity_id.to_owned(),
            },
            app.handle().clone(),
            app.state(),
        );
        if activity_as_ticket != Err(FrontendError::HostInvocationTicketInvalid)
            || activity_as_cancel != Err(FrontendError::HostInvocationTicketInvalid)
            || live_test_multi_file_tool_executions(&fixture.0) != 1
            || (0..4).any(|index| live_test_multi_file_native_attempts(&fixture.0, index) != 1)
        {
            shutdown_live_state(app.state::<DesktopAppState>().inner()).await;
            std::mem::forget(fixture);
            return Err("activity correlation ID was accepted as authority".to_owned());
        }
        println!("RAH_MULTI_FILE_HOSTEXPLICIT_CONFIRM_TOOL_EXECUTIONS=1");
        println!("RAH_MULTI_FILE_HOSTEXPLICIT_CONFIRM_NATIVE_ATTEMPTS=1/1/1/1");
        for (path, _, sha256, length) in &after_targets {
            println!("RAH_MULTI_FILE_HOSTEXPLICIT_POSTIMAGE_{path}={sha256}:{length}");
        }
        println!("RAH_MULTI_FILE_HOSTEXPLICIT_MODEL_RUNTIME_STARTS=0");
        println!("RAH_MULTI_FILE_HOSTEXPLICIT_MODEL_AGENT_REQUESTS=0");
        println!("RAH_MULTI_FILE_HOSTEXPLICIT_MODEL_PROMPTS=0");
        println!("RAH_MULTI_FILE_HOSTEXPLICIT_MODEL_TOOL_REQUESTED=0");
        println!("RAH_MULTI_FILE_HOSTEXPLICIT_MODEL_TOOL_STARTED=0");
        println!("RAH_MULTI_FILE_HOSTEXPLICIT_MODEL_TOOL_FINISHED=0");
        println!("RAH_MULTI_FILE_HOSTEXPLICIT_MODEL_TOOL_LIFECYCLE=0/0/0");
        println!("RAH_MULTI_FILE_HOSTEXPLICIT_MCP_PROVIDERS=0");
        println!("RAH_MULTI_FILE_HOSTEXPLICIT_PROCESS_PLUGINS=0");
        println!("RAH_MULTI_FILE_HOSTEXPLICIT_GENERATIONS={before_generations:?}");
        app.unlisten(host_activity.1);
        app.unlisten(refresh_events.1);
        app.unlisten(chat_events.1);
        app.unlisten(model_activity.1);
        shutdown_live_state(app.state::<DesktopAppState>().inner()).await;
        clear_live_test_multi_file_tool_executions(&fixture.0);
        clear_live_test_multi_file_native_attempts(&fixture.0);
        println!("RAH_MULTI_FILE_HOSTEXPLICIT_LIVE_OK");
        Ok(())
    }

    #[tokio::test(flavor = "current_thread")]
    #[ignore = "requires certified Codex live gate and authentication"]
    async fn windows_live_desktop_repo_create_branch() -> Result<(), String> {
        let fixture = TestRepository::git_repository(GitRepositoryState::Clean);
        let storage = TestRepository::new();
        let git = selected_git_executable()
            .map_err(|error| format!("Git discovery failed: {error:?}"))?;

        fs::write(fixture.0.join("nested/ordinary.txt"), "live unstaged\n")
            .map_err(|error| format!("unstaged fixture modification failed: {error}"))?;
        fs::write(fixture.0.join("tracked.txt"), "live staged\n")
            .map_err(|error| format!("staged fixture modification failed: {error}"))?;
        let stage = Command::new(&git)
            .args(["add", "tracked.txt"])
            .current_dir(&fixture.0)
            .status()
            .map_err(|error| format!("staged fixture command failed to start: {error}"))?;
        if !stage.success() {
            return Err("staged fixture command failed".to_owned());
        }

        let authority = RepositoryBranchCreationAuthority::new(&git, &fixture.0)
            .map_err(|error| format!("branch authority construction failed: {error}"))?;
        let repository = DesktopRepository::new_with_authorities(
            &git,
            &fixture.0,
            None,
            None,
            None,
            Some(authority),
        )
        .map_err(|error| format!("Desktop repository construction failed: {error}"))?;
        let state = DesktopAppState::new(storage.0.clone());
        replace_selected_repository(&state, repository);
        let branch_name = format!(
            "rah-live-{:x}-{:x}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|error| format!("clock failed: {error}"))?
                .as_nanos(),
            NEXT_TEST_DIRECTORY.fetch_add(1, Ordering::Relaxed)
        );
        let before = live_git_state(&git, &fixture.0, &branch_name)?;
        if live_target_exists(&git, &fixture.0, &branch_name)? {
            return Err("generated live target unexpectedly exists".to_owned());
        }
        let conversation_namespace_before = state.persistence_namespace();
        let conversation_presentation_before = serde_json::to_value(
            state
                .persistence
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .presentation(),
        )
        .map_err(|error| format!("conversation presentation serialization failed: {error}"))?;
        let generations_before = current_host_generation_tuple(&state);

        let selected = state
            .repository
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
            .ok_or_else(|| "Desktop repository was not selected".to_owned())?;
        if selected.branch_creation_authority.is_none() {
            return Err("Desktop branch authority was not stored".to_owned());
        }
        let (commit_tool, commit_control) = RepositoryCommitTool::compose(
            &selected.git_executable,
            &selected.root,
            "RAH Live Test".to_owned(),
            "rah-live-test@example.invalid".to_owned(),
        )
        .map_err(|error| format!("Desktop commit composition failed: {error}"))
        .map(|(tool, control)| (Arc::new(tool), Arc::new(control)))?;
        let composed_registry =
            desktop_tool_registry(Some(&selected), Some(Arc::clone(&commit_tool)))
                .map_err(|error| format!("Desktop registry composition failed: {error}"))?;
        if !composed_registry
            .definitions()
            .iter()
            .any(|definition| definition.name.as_str() == REPOSITORY_CREATE_BRANCH_TOOL_NAME)
        {
            return Err("Desktop registry did not contain repo.create-branch".to_owned());
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
        let profile_generation = *state
            .trusted_profile_generation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let connection_generation = {
            let mut generation = state
                .next_connection_generation
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            *generation += 1;
            *generation
        };
        {
            let mut connection = state
                .connection
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            *connection = ConnectionState::Connecting;
        }
        let prepared =
            prepare_codex_connection(resolve_codex_executable, CodexModelConfig::Inherit)
                .map_err(|error| format!("certified Codex preparation failed: {error:?}"))?;
        let source = prepared.source;
        let runtime = CodexRuntime::connect_tool_bridge_with_model_config_and_workspace(
            prepared.executable,
            Arc::clone(&composed_registry),
            vec![
                PermissionLevel::None,
                PermissionLevel::Read,
                PermissionLevel::Execute,
            ],
            prepared.model_config,
            &fixture.0,
        )
        .await
        .map_err(|error| format!("Desktop Codex connection failed: {error}"))?;
        let composition = desktop_tool_composition_from_registry(
            Arc::clone(&composed_registry),
            Some(&selected),
            true,
            &[],
        )
        .map_err(|error| format!("Desktop authority classification failed: {error:?}"))?;
        let repository_fingerprint = Some(repository_context_fingerprint(&selected.root));
        publish_connected_provider_state(
            &state,
            PendingConnectedPublication {
                runtime: Arc::new(runtime),
                activation: None,
                source,
                repository_generation,
                model_generation,
                profile_generation,
                connection_generation,
                repository_fingerprint,
                composition: Arc::clone(&composition),
                allowed_permissions: vec![
                    PermissionLevel::None,
                    PermissionLevel::Read,
                    PermissionLevel::Execute,
                ],
            },
        )
        .map_err(|_| "Desktop connection publication was rejected".to_owned())?;
        *state
            .commit_capability
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(DesktopCommitCapability {
            repository_generation,
            model_generation,
            identity_generation: *state
                .commit_identity_generation
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
            _tool: commit_tool,
            control: Arc::clone(&commit_control),
        });
        let connected_current = match &*state
            .connection
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
        {
            ConnectionState::Connected {
                repository_generation: captured_repository_generation,
                model_generation: captured_model_generation,
                profile_generation: captured_profile_generation,
                connection_generation: captured_connection_generation,
                composition,
                ..
            } => {
                connection_activation_publication_is_current(
                    [
                        *captured_repository_generation,
                        *captured_model_generation,
                        *captured_profile_generation,
                        *captured_connection_generation,
                    ],
                    current_host_generation_tuple(&state),
                ) && composition.registry.definitions().len() == composition.tools.len()
            }
            _ => false,
        };
        if !connected_current {
            return Err("Desktop Effective Authority was not connected_current".to_owned());
        }
        let effective = composition
            .tools
            .iter()
            .find(|tool| tool.public_tool_name == REPOSITORY_CREATE_BRANCH_TOOL_NAME)
            .ok_or_else(|| "repo.create-branch was not effectively composed".to_owned())?;
        if effective.source_kind != SourceKind::RepositoryHost
            || effective.source_label != "desktop_repository"
            || effective.effect_class != EffectClass::RepositoryMutation
            || effective.authority_category
                != super::effective_authority::AuthorityCategory::RepositoryLocalBranchCreation
            || effective.permission != PermissionLevel::Execute
            || !effective.repository_bound
        {
            return Err(
                "repo.create-branch Effective Authority classification was wrong".to_owned(),
            );
        }
        println!("RAH_DESKTOP_BRANCH_AUTHORITY_PRESENT=1");
        println!("RAH_DESKTOP_BRANCH_TOOL_REGISTERED=1");
        println!("RAH_DESKTOP_BRANCH_TOOL_ADVERTISED=1");

        let reviewed = refresh_repository_workflow(&state)
            .await
            .map_err(|error| format!("review snapshot failed: {error:?}"))?;
        let review_id = match reviewed.review {
            StagedReviewPresentation::ReviewAvailable {
                review_id: Some(review_id),
                can_authorize: true,
                authorization_state: CommitAuthorizationPresentation::ReadyToAuthorize,
            } => review_id,
            other => {
                return Err(format!(
                    "fixture did not expose an authorizable review: {other:?}"
                ));
            }
        };
        let authorization = authorize_repository_commit_review(&state, &review_id)
            .await
            .map_err(|error| format!("review authorization failed: {error:?}"))?;
        if authorization.authorization_state != CommitAuthorizationPresentation::AuthorizedPending {
            return Err("review authorization was not pending".to_owned());
        }
        let commit_control = state
            .commit_capability
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .map(|capability| Arc::clone(&capability.control))
            .ok_or_else(|| "Desktop commit control was not composed".to_owned())?;
        if !commit_control.has_pending_authorization().await {
            return Err("underlying reviewed commit authorization was not pending".to_owned());
        }

        let (runtime, composition) = match &*state
            .connection
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
        {
            ConnectionState::Connected {
                runtime,
                composition,
                ..
            } => (Arc::clone(runtime), Arc::clone(composition)),
            _ => return Err("Desktop connection was not current".to_owned()),
        };
        let handle = runtime
            .start(AgentRequest {
                request_id: RequestId::new(),
                input: AgentInput {
                    messages: vec![Message {
                        role: MessageRole::User,
                        content: format!(
                            "Use the repo.create-branch tool exactly once to create the local branch named \"{branch_name}\". Do not switch branches and do not use another mutating tool. After the tool result, reply with RAH_BRANCH_LIVE_DONE."
                        ),
                    }],
                },
                options: AgentOptions::default(),
            })
            .await
            .map_err(|error| format!("live model turn failed to start: {error}"))?;

        let mut tool_calls = HashMap::new();
        let mut call_names = HashMap::new();
        let mut requested = HashMap::<String, usize>::new();
        let mut started = HashMap::<String, usize>::new();
        let mut finished = HashMap::<String, usize>::new();
        let mut finished_output = None;
        let mut other_effectful_started = 0_usize;
        let mut final_text = None;
        let mut terminal_failure = None;
        let mut events = handle.into_events();
        let event_result = tokio::time::timeout(Duration::from_secs(180), async {
            while let Some(event) = events.next().await {
                if let Some(activity) =
                    activity_event_with_composition(&event, &mut tool_calls, &composition, true)
                    && (activity.invalidate_review || activity.refresh_reason.is_some())
                {
                    return Err(
                        "branch activity unexpectedly invalidated review or requested refresh"
                            .to_owned(),
                    );
                }
                match &event {
                    AgentEvent::ToolRequested { tool_call, .. } => {
                        let name = tool_call.name.as_str().to_owned();
                        call_names.insert(tool_call.id.clone(), name.clone());
                        *requested.entry(name).or_default() += 1;
                    }
                    AgentEvent::ToolStarted { tool_call_id, .. } => {
                        let name = call_names
                            .get(tool_call_id)
                            .cloned()
                            .ok_or_else(|| "ToolStarted lacked a requested call".to_owned())?;
                        *started.entry(name.clone()).or_default() += 1;
                        if name != REPOSITORY_CREATE_BRANCH_TOOL_NAME
                            && !matches!(
                                name.as_str(),
                                "repo.status" | "repo.diff" | "repo.diff-staged" | "repo.file-info"
                            )
                        {
                            other_effectful_started += 1;
                        }
                    }
                    AgentEvent::ToolFinished {
                        tool_call_id,
                        output,
                        ..
                    } => {
                        let name = call_names
                            .get(tool_call_id)
                            .cloned()
                            .ok_or_else(|| "ToolFinished lacked a requested call".to_owned())?;
                        *finished.entry(name.clone()).or_default() += 1;
                        if name == REPOSITORY_CREATE_BRANCH_TOOL_NAME {
                            finished_output = Some(output.clone());
                        }
                    }
                    AgentEvent::Completed { output, .. } => {
                        final_text = Some(output.message.content.clone());
                        break;
                    }
                    AgentEvent::Failed { message, .. } => {
                        terminal_failure = Some(message.clone());
                        break;
                    }
                    AgentEvent::Cancelled { .. } => {
                        terminal_failure = Some("Codex turn was cancelled".to_owned());
                        break;
                    }
                    AgentEvent::Started { .. }
                    | AgentEvent::ModelRequestStarted { .. }
                    | AgentEvent::ModelDelta { .. }
                    | AgentEvent::ApprovalRequired { .. } => {}
                }
            }
            Ok::<(), String>(())
        })
        .await;
        if !matches!(event_result, Ok(Ok(()))) {
            terminal_failure = Some("live turn did not complete normally".to_owned());
        }
        if let Some(error) = terminal_failure {
            shutdown_live_state(&state).await;
            eprintln!("RAH_DESKTOP_BRANCH_LIVE_FIXTURE={}", fixture.0.display());
            std::mem::forget(fixture);
            return Err(format!("possible-effect live turn failure: {error}"));
        }

        let branch_requested = requested
            .get(REPOSITORY_CREATE_BRANCH_TOOL_NAME)
            .copied()
            .unwrap_or_default();
        let branch_started = started
            .get(REPOSITORY_CREATE_BRANCH_TOOL_NAME)
            .copied()
            .unwrap_or_default();
        let branch_finished = finished
            .get(REPOSITORY_CREATE_BRANCH_TOOL_NAME)
            .copied()
            .unwrap_or_default();
        let after = live_git_state(&git, &fixture.0, &branch_name)?;
        let target_exists = live_target_exists(&git, &fixture.0, &branch_name)?;
        if branch_requested == 0 {
            if branch_started != 0
                || branch_finished != 0
                || other_effectful_started != 0
                || target_exists
                || after != before
            {
                shutdown_live_state(&state).await;
                eprintln!("RAH_DESKTOP_BRANCH_LIVE_FIXTURE={}", fixture.0.display());
                std::mem::forget(fixture);
                return Err("zero-request model result was not safely inconclusive".to_owned());
            }
            println!("RAH_DESKTOP_BRANCH_TOOL_REQUESTED=0");
            println!("RAH_DESKTOP_BRANCH_TOOL_STARTED=0");
            println!("RAH_DESKTOP_BRANCH_TOOL_FINISHED=0");
            println!("RAH_DESKTOP_BRANCH_LIVE_RESULT=INCONCLUSIVE_MODEL_DISPATCH");
            shutdown_live_state(&state).await;
            return Ok(());
        }
        if branch_requested != 1 || branch_started != 1 || branch_finished != 1 {
            shutdown_live_state(&state).await;
            eprintln!("RAH_DESKTOP_BRANCH_LIVE_FIXTURE={}", fixture.0.display());
            std::mem::forget(fixture);
            return Err("branch lifecycle was incomplete after a possible effect".to_owned());
        }
        if other_effectful_started != 0 {
            return Err(format!(
                "unexpected effectful ToolStarted count: {other_effectful_started}"
            ));
        }
        let output =
            finished_output.ok_or_else(|| "branch ToolFinished output was missing".to_owned())?;
        if !matches!(
            branch_result_classification(&output),
            BranchActivityClassification::ProvenSafe
        ) || output.is_error
        {
            return Err("branch ToolFinished output was not proven safe".to_owned());
        }
        let [ToolContent::Json(value)] = output.content.as_slice() else {
            return Err("branch ToolFinished output was not JSON".to_owned());
        };
        if value["status"] != "branch_created_verified"
            || value["uncertain"] != false
            || value["name"] != branch_name
            || value["oid"] != before.head_oid.trim()
            || final_text
                .as_deref()
                .is_none_or(|text| !text.contains("RAH_BRANCH_LIVE_DONE"))
        {
            return Err("branch result or completion marker was wrong".to_owned());
        }
        let branch_oid = live_git_text(
            &git,
            &fixture.0,
            &["rev-parse", &format!("refs/heads/{branch_name}")],
        )?;
        let reflog = live_git_text(
            &git,
            &fixture.0,
            &[
                "reflog",
                "show",
                "--format=%gs",
                "-n",
                "1",
                &format!("refs/heads/{branch_name}"),
            ],
        )?;
        if branch_oid.trim() != before.head_oid.trim()
            || reflog.trim() != "RAH create local branch"
            || after != before
        {
            return Err("verified branch effect changed protected Git state".to_owned());
        }
        if !commit_control.has_pending_authorization().await
            || state
                .repository_workflow
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .authorization
                != CommitAuthorizationPresentation::AuthorizedPending
        {
            return Err("verified branch effect consumed reviewed commit authorization".to_owned());
        }
        if current_host_generation_tuple(&state) != generations_before
            || state.persistence_namespace() != conversation_namespace_before
            || serde_json::to_value(
                state
                    .persistence
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .presentation(),
            )
            .map_err(|error| format!("conversation presentation serialization failed: {error}"))?
                != conversation_presentation_before
        {
            return Err(
                "branch effect changed Desktop currentness or conversation state".to_owned(),
            );
        }
        println!("RAH_DESKTOP_BRANCH_TOOL_REQUESTED=1");
        println!("RAH_DESKTOP_BRANCH_TOOL_STARTED=1");
        println!("RAH_DESKTOP_BRANCH_TOOL_FINISHED=1");
        println!("RAH_DESKTOP_BRANCH_CREATED_VERIFIED=1");
        println!("RAH_DESKTOP_BRANCH_HEAD_UNCHANGED=1");
        println!("RAH_DESKTOP_BRANCH_INDEX_UNCHANGED=1");
        println!("RAH_DESKTOP_BRANCH_WORKTREE_STATUS_UNCHANGED=1");
        println!("RAH_DESKTOP_BRANCH_REVIEW_PRESERVED=1");
        println!("RAH_DESKTOP_BRANCH_GENERATIONS_UNCHANGED=1");
        shutdown_live_state(&state).await;
        println!("RAH_DESKTOP_BRANCH_LIVE_OK");
        Ok(())
    }

    #[tokio::test(flavor = "current_thread")]
    #[ignore = "requires Windows host-driven live certification environment"]
    async fn windows_live_desktop_repo_create_branch_host_driven() -> Result<(), String> {
        let fixture = TestRepository::git_repository(GitRepositoryState::Clean);
        let storage = TestRepository::new();
        let git = selected_git_executable()
            .map_err(|error| format!("Git discovery failed: {error:?}"))?;

        fs::write(fixture.0.join("nested/ordinary.txt"), "live unstaged\n")
            .map_err(|error| format!("unstaged fixture modification failed: {error}"))?;
        fs::write(fixture.0.join("tracked.txt"), "live staged\n")
            .map_err(|error| format!("staged fixture modification failed: {error}"))?;
        let stage = Command::new(&git)
            .args(["add", "tracked.txt"])
            .current_dir(&fixture.0)
            .status()
            .map_err(|error| format!("staged fixture command failed to start: {error}"))?;
        if !stage.success() {
            return Err("staged fixture command failed".to_owned());
        }

        let authority = RepositoryBranchCreationAuthority::new(&git, &fixture.0)
            .map_err(|error| format!("branch authority construction failed: {error}"))?;
        let repository = DesktopRepository::new_with_authorities(
            &git,
            &fixture.0,
            None,
            None,
            None,
            Some(authority),
        )
        .map_err(|error| format!("Desktop repository construction failed: {error}"))?;
        let state = DesktopAppState::new(storage.0.clone());
        replace_selected_repository(&state, repository);
        let branch_name = format!(
            "rah-host-live-{:x}-{:x}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|error| format!("clock failed: {error}"))?
                .as_nanos(),
            NEXT_TEST_DIRECTORY.fetch_add(1, Ordering::Relaxed)
        );
        let before = live_git_state(&git, &fixture.0, &branch_name)?;
        if live_target_exists(&git, &fixture.0, &branch_name)? {
            return Err("generated live target unexpectedly exists".to_owned());
        }

        let selected = state
            .repository
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
            .ok_or_else(|| "Desktop repository was not selected".to_owned())?;
        if selected.branch_creation_authority.is_none() {
            return Err("Desktop branch authority was not stored".to_owned());
        }
        println!("RAH_DESKTOP_BRANCH_HOST_AUTHORITY_PRESENT=1");

        let (commit_tool, commit_control) = RepositoryCommitTool::compose(
            &selected.git_executable,
            &selected.root,
            "RAH Host Live Test".to_owned(),
            "rah-host@example.invalid".to_owned(),
        )
        .map_err(|error| format!("Desktop commit composition failed: {error}"))
        .map(|(tool, control)| (Arc::new(tool), Arc::new(control)))?;
        let composed_registry =
            desktop_tool_registry(Some(&selected), Some(Arc::clone(&commit_tool)))
                .map_err(|error| format!("Desktop registry composition failed: {error}"))?;
        if !composed_registry
            .definitions()
            .iter()
            .any(|definition| definition.name.as_str() == REPOSITORY_CREATE_BRANCH_TOOL_NAME)
        {
            return Err("Desktop registry did not contain repo.create-branch".to_owned());
        }
        println!("RAH_DESKTOP_BRANCH_HOST_TOOL_REGISTERED=1");

        let repository_generation = *state
            .repository_generation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let model_generation = state
            .model
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .generation;
        let profile_generation = *state
            .trusted_profile_generation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let connection_generation = {
            let mut generation = state
                .next_connection_generation
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            *generation += 1;
            *generation
        };
        {
            let mut connection = state
                .connection
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            *connection = ConnectionState::Connecting;
        }
        let prepared =
            prepare_codex_connection(resolve_codex_executable, CodexModelConfig::Inherit)
                .map_err(|error| format!("certified Codex preparation failed: {error:?}"))?;
        let source = prepared.source;
        let runtime = CodexRuntime::connect_tool_bridge_with_model_config_and_workspace(
            prepared.executable,
            Arc::clone(&composed_registry),
            vec![
                PermissionLevel::None,
                PermissionLevel::Read,
                PermissionLevel::Execute,
            ],
            prepared.model_config,
            &fixture.0,
        )
        .await
        .map_err(|error| format!("Desktop Codex connection failed: {error}"))?;
        let composition = desktop_tool_composition_from_registry(
            Arc::clone(&composed_registry),
            Some(&selected),
            true,
            &[],
        )
        .map_err(|error| format!("Desktop authority classification failed: {error:?}"))?;
        publish_connected_provider_state(
            &state,
            PendingConnectedPublication {
                runtime: Arc::new(runtime),
                activation: None,
                source,
                repository_generation,
                model_generation,
                profile_generation,
                connection_generation,
                repository_fingerprint: Some(repository_context_fingerprint(&selected.root)),
                composition: Arc::clone(&composition),
                allowed_permissions: vec![
                    PermissionLevel::None,
                    PermissionLevel::Read,
                    PermissionLevel::Execute,
                ],
            },
        )
        .map_err(|_| "Desktop connection publication was rejected".to_owned())?;
        *state
            .commit_capability
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(DesktopCommitCapability {
            repository_generation,
            model_generation,
            identity_generation: *state
                .commit_identity_generation
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
            _tool: commit_tool,
            control: Arc::clone(&commit_control),
        });

        let connected_current = match &*state
            .connection
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
        {
            ConnectionState::Connected {
                repository_generation: captured_repository_generation,
                model_generation: captured_model_generation,
                profile_generation: captured_profile_generation,
                connection_generation: captured_connection_generation,
                composition,
                ..
            } => {
                connection_activation_publication_is_current(
                    [
                        *captured_repository_generation,
                        *captured_model_generation,
                        *captured_profile_generation,
                        *captured_connection_generation,
                    ],
                    current_host_generation_tuple(&state),
                ) && composition.registry.definitions().len() == composition.tools.len()
            }
            _ => false,
        };
        if !connected_current {
            return Err("Desktop Effective Authority was not connected_current".to_owned());
        }
        let effective = composition
            .tools
            .iter()
            .find(|tool| tool.public_tool_name == REPOSITORY_CREATE_BRANCH_TOOL_NAME)
            .ok_or_else(|| "repo.create-branch was not effectively composed".to_owned())?;
        if effective.source_kind != SourceKind::RepositoryHost
            || effective.source_label != "desktop_repository"
            || effective.effect_class != EffectClass::RepositoryMutation
            || effective.authority_category
                != super::effective_authority::AuthorityCategory::RepositoryLocalBranchCreation
            || effective.permission != PermissionLevel::Execute
            || !effective.repository_bound
        {
            return Err(
                "repo.create-branch Effective Authority classification was wrong".to_owned(),
            );
        }
        println!("RAH_DESKTOP_BRANCH_HOST_TOOL_ADVERTISED=1");

        let reviewed = refresh_repository_workflow(&state)
            .await
            .map_err(|error| format!("review snapshot failed: {error:?}"))?;
        let review_id = match reviewed.review {
            StagedReviewPresentation::ReviewAvailable {
                review_id: Some(review_id),
                can_authorize: true,
                authorization_state: CommitAuthorizationPresentation::ReadyToAuthorize,
            } => review_id,
            other => {
                return Err(format!(
                    "fixture did not expose an authorizable review: {other:?}"
                ));
            }
        };
        let authorization = authorize_repository_commit_review(&state, &review_id)
            .await
            .map_err(|error| format!("review authorization failed: {error:?}"))?;
        if authorization.authorization_state != CommitAuthorizationPresentation::AuthorizedPending
            || !commit_control.has_pending_authorization().await
        {
            return Err("review authorization was not valid and pending".to_owned());
        }
        println!("RAH_DESKTOP_BRANCH_HOST_REVIEW_PENDING=1");

        let conversation_namespace_before = state.persistence_namespace();
        let conversation_presentation_before = serde_json::to_value(
            state
                .persistence
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .presentation(),
        )
        .map_err(|error| format!("conversation presentation serialization failed: {error}"))?;
        let generations_before = current_host_generation_tuple(&state);
        if state
            .provider_activation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_some()
        {
            return Err("unexpected provider activation was present".to_owned());
        }

        let output = composed_registry
            .execute(
                ToolCall {
                    id: ToolCallId::new(),
                    name: ToolName::new(REPOSITORY_CREATE_BRANCH_TOOL_NAME),
                    input: ToolInput(serde_json::json!({"name": branch_name})),
                },
                ToolContext::default(),
            )
            .await
            .map_err(|error| format!("host-driven registry dispatch failed: {error}"))?;
        println!("RAH_DESKTOP_BRANCH_HOST_DISPATCH=1");

        let after = live_git_state(&git, &fixture.0, &branch_name)?;
        let target_ref = format!("refs/heads/{branch_name}");
        let target_heads = live_git_text(
            &git,
            &fixture.0,
            &["for-each-ref", "--format=%(refname)", &target_ref],
        )?;
        let target_oid = live_git_text(&git, &fixture.0, &["rev-parse", &target_ref])?;
        let target_tracking = live_git_text(
            &git,
            &fixture.0,
            &[
                "for-each-ref",
                "--format=%(refname:short) %(upstream:short)",
                &target_ref,
            ],
        )?;
        let reflog = live_git_text(
            &git,
            &fixture.0,
            &[
                "reflog",
                "show",
                "--format=%gn%x09%ge%x09%gs",
                "-n",
                "1",
                &target_ref,
            ],
        )?;
        let strict_output = matches!(
            branch_result_classification(&output),
            BranchActivityClassification::ProvenSafe
        );
        let output_fields_correct = match output.content.as_slice() {
            [ToolContent::Json(value)] => {
                value.as_object().is_some_and(|object| object.len() == 4)
                    && value["status"] == "branch_created_verified"
                    && value["uncertain"] == false
                    && value["name"] == branch_name
                    && value["oid"] == before.head_oid.trim()
                    && !output.is_error
            }
            _ => false,
        };
        if !strict_output || !output_fields_correct {
            shutdown_live_state(&state).await;
            eprintln!(
                "RAH_DESKTOP_BRANCH_HOST_LIVE_FIXTURE={}",
                fixture.0.display()
            );
            std::mem::forget(fixture);
            return Err("host-driven branch result was not safely verified".to_owned());
        }
        if target_heads.lines().count() != 1
            || target_heads.trim() != target_ref
            || target_oid.trim() != before.head_oid.trim()
            || target_tracking.trim() != branch_name
            || reflog.trim() != "RAH Host\trah-host@example.invalid\tRAH create local branch"
        {
            shutdown_live_state(&state).await;
            eprintln!(
                "RAH_DESKTOP_BRANCH_HOST_LIVE_FIXTURE={}",
                fixture.0.display()
            );
            std::mem::forget(fixture);
            return Err("host-driven branch ref or reflog was not verified".to_owned());
        }

        let dispatch_id = ToolCallId::new();
        let requested = AgentEvent::ToolRequested {
            session_id: SessionId::new(),
            tool_call: ToolCall {
                id: dispatch_id.clone(),
                name: ToolName::new(REPOSITORY_CREATE_BRANCH_TOOL_NAME),
                input: ToolInput(serde_json::json!({"name": branch_name})),
            },
        };
        let finished = AgentEvent::ToolFinished {
            session_id: SessionId::new(),
            tool_call_id: dispatch_id,
            output: output.clone(),
        };
        let mut activity_calls = HashMap::new();
        if activity_event_with_composition(&requested, &mut activity_calls, &composition, true)
            .is_none()
        {
            shutdown_live_state(&state).await;
            eprintln!(
                "RAH_DESKTOP_BRANCH_HOST_LIVE_FIXTURE={}",
                fixture.0.display()
            );
            std::mem::forget(fixture);
            return Err("host activity request classification failed".to_owned());
        }
        let activity =
            activity_event_with_composition(&finished, &mut activity_calls, &composition, true)
                .ok_or_else(|| "host activity finish classification failed".to_owned())?;
        if activity.invalidate_review || activity.refresh_reason.is_some() {
            shutdown_live_state(&state).await;
            eprintln!(
                "RAH_DESKTOP_BRANCH_HOST_LIVE_FIXTURE={}",
                fixture.0.display()
            );
            std::mem::forget(fixture);
            return Err(
                "safe host branch activity invalidated review or requested refresh".to_owned(),
            );
        }

        let review_preserved = commit_control.has_pending_authorization().await
            && state
                .repository_workflow
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .authorization
                == CommitAuthorizationPresentation::AuthorizedPending;
        let current_after = current_host_generation_tuple(&state);
        let conversation_unchanged = state.persistence_namespace() == conversation_namespace_before
            && serde_json::to_value(
                state
                    .persistence
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .presentation(),
            )
            .map_err(|error| format!("conversation presentation serialization failed: {error}"))?
                == conversation_presentation_before;
        let connection_current_after = match &*state
            .connection
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
        {
            ConnectionState::Connected { composition, .. } => {
                composition.registry.definitions().len() == composition.tools.len()
                    && current_after == generations_before
                    && composition.tools.iter().any(|tool| {
                        tool.public_tool_name == REPOSITORY_CREATE_BRANCH_TOOL_NAME
                            && tool.source_kind == SourceKind::RepositoryHost
                            && tool.effect_class == EffectClass::RepositoryMutation
                            && tool.authority_category
                                == super::effective_authority::AuthorityCategory::RepositoryLocalBranchCreation
                            && tool.permission == PermissionLevel::Execute
                            && tool.repository_bound
                    })
            }
            _ => false,
        };
        let providers_absent = state
            .provider_activation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_none();
        if after != before
            || !review_preserved
            || !conversation_unchanged
            || !connection_current_after
            || !providers_absent
        {
            shutdown_live_state(&state).await;
            eprintln!(
                "RAH_DESKTOP_BRANCH_HOST_LIVE_FIXTURE={}",
                fixture.0.display()
            );
            std::mem::forget(fixture);
            return Err(
                "host-driven branch effect changed protected Desktop or Git state".to_owned(),
            );
        }

        println!("RAH_DESKTOP_BRANCH_HOST_CREATED_VERIFIED=1");
        println!("RAH_DESKTOP_BRANCH_HOST_REF_VERIFIED=1");
        println!("RAH_DESKTOP_BRANCH_HOST_REFLOG_VERIFIED=1");
        println!("RAH_DESKTOP_BRANCH_HOST_HEAD_UNCHANGED=1");
        println!("RAH_DESKTOP_BRANCH_HOST_INDEX_UNCHANGED=1");
        println!("RAH_DESKTOP_BRANCH_HOST_WORKTREE_UNCHANGED=1");
        println!("RAH_DESKTOP_BRANCH_HOST_REVIEW_PRESERVED=1");
        println!("RAH_DESKTOP_BRANCH_HOST_GENERATIONS_UNCHANGED=1");
        println!("RAH_DESKTOP_BRANCH_HOST_CONNECTION_CURRENT=1");
        println!("RAH_DESKTOP_BRANCH_MODEL_DISPATCH_ESTABLISHED=0");
        shutdown_live_state(&state).await;
        println!("RAH_DESKTOP_BRANCH_HOST_LIVE_OK");
        Ok(())
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
    fn connecting_repository_selection_is_rejected_deterministically() {
        assert_eq!(
            repository_selection_allowed_for_connection(&ConnectionState::Connecting),
            Err(FrontendError::RepositoryBusy)
        );
        assert_eq!(
            repository_selection_allowed_for_connection(&ConnectionState::NotConnected),
            Ok(())
        );
    }

    #[test]
    fn activation_publication_requires_repository_model_profile_and_connection_currentness() {
        assert!(super::connection_activation_publication_is_current(
            [4, 5, 6, 9],
            [4, 5, 6, 9],
        ));
        assert!(!super::connection_activation_publication_is_current(
            [4, 5, 6, 9],
            [5, 5, 6, 9],
        ));
        assert!(!super::connection_activation_publication_is_current(
            [4, 5, 6, 9],
            [4, 7, 6, 9],
        ));
        assert!(!super::connection_activation_publication_is_current(
            [4, 5, 6, 9],
            [4, 5, 8, 9],
        ));
        assert!(!super::connection_activation_publication_is_current(
            [4, 5, 6, 9],
            [4, 5, 6, 10],
        ));
    }

    #[test]
    fn host_generation_tuple_retains_profile_generation_as_currentness_identity() {
        let storage = TestRepository::new();
        let state = DesktopAppState::new(storage.0.clone());
        assert_eq!(super::current_host_generation_tuple(&state), [0, 0, 0, 0]);
        *state.trusted_profile_generation.lock().unwrap() = 7;
        assert_eq!(super::current_host_generation_tuple(&state), [0, 0, 7, 0]);
    }

    #[test]
    fn repository_context_fingerprint_is_opaque_and_process_stable() {
        let first = repository_context_fingerprint(Path::new(r"C:\\fixtures\\a"));
        let same = repository_context_fingerprint(Path::new(r"C:\\fixtures\\a"));
        let other = repository_context_fingerprint(Path::new(r"C:\\fixtures\\b"));
        assert_eq!(first, same);
        assert_ne!(first, other);
        assert!(!first.contains("fixtures"));
        assert!(first.starts_with("repo-context:"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn task_316_membership_admission_and_activation_matrix_is_inert_and_current() {
        let storage = TestRepository::new();
        let state = DesktopAppState::new(storage.0.clone());
        let repository_a = TestRepository::git_repository(GitRepositoryState::Clean);
        let repository_b = TestRepository::git_repository(GitRepositoryState::Untracked);
        let git = TestRepository::native_git();

        {
            let membership = state.workspace_membership.lock().unwrap();
            assert_eq!(membership.member_count(), 0);
            assert_eq!(membership.membership_generation(), 0);
            assert!(membership.active_member().is_none());
            assert!(state.repository.lock().unwrap().is_none());
            assert!(state.provider_activation.lock().unwrap().is_none());
        }

        let member_a = admit_repository(&state, &git, &repository_a.0).expect("admit A");
        {
            let membership = state.workspace_membership.lock().unwrap();
            assert_eq!(membership.member_count(), 1);
            assert_eq!(membership.membership_generation(), 1);
            assert!(membership.active_member().is_none());
            let member = membership.member(member_a).expect("A member");
            assert_eq!(member.id, member_a);
            assert_eq!(member.admission_generation, 1);
            assert!(member.root.is_absolute());
            assert!(member.display_path.contains("rah-desktop-tool-registry"));
        }
        assert!(state.repository.lock().unwrap().is_none());
        assert_eq!(*state.repository_generation.lock().unwrap(), 0);

        activate_admitted_member(&state, member_a)
            .await
            .expect("activate A");
        let active_a = state.repository.lock().unwrap().clone().expect("active A");
        let conversation_after_a = state.conversation.lock().unwrap().epoch;
        assert_eq!(
            state.workspace_membership.lock().unwrap().active_member(),
            Some(member_a)
        );
        assert_eq!(*state.repository_generation.lock().unwrap(), 1);

        let member_b = admit_repository(&state, &git, &repository_b.0).expect("admit B");
        assert_ne!(member_a.as_debug_tuple(), member_b.as_debug_tuple());
        assert_eq!(*state.repository_generation.lock().unwrap(), 1);
        assert_eq!(
            state.conversation.lock().unwrap().epoch,
            conversation_after_a
        );
        assert!(state.commit_capability.lock().unwrap().is_none());
        assert_eq!(
            state
                .repository_workflow
                .lock()
                .unwrap()
                .observation_generation,
            0
        );
        assert!(Arc::ptr_eq(
            &active_a,
            &state
                .repository
                .lock()
                .unwrap()
                .clone()
                .expect("A remains active")
        ));
        assert!(state.provider_activation.lock().unwrap().is_none());

        let before_duplicate = {
            let membership = state.workspace_membership.lock().unwrap();
            (
                membership.member_count(),
                membership.membership_generation(),
            )
        };
        assert_eq!(
            admit_repository(&state, &git, &repository_a.0),
            Err(FrontendError::RepositoryAlreadyMember)
        );
        let case_alias = PathBuf::from(format!(
            r"{}\.",
            repository_a.0.to_string_lossy().to_ascii_uppercase()
        ));
        assert_eq!(
            admit_repository(&state, &git, &case_alias),
            Err(FrontendError::RepositoryAlreadyMember)
        );
        let after_duplicate = {
            let membership = state.workspace_membership.lock().unwrap();
            (
                membership.member_count(),
                membership.membership_generation(),
            )
        };
        assert_eq!(after_duplicate, before_duplicate);
        assert_eq!(
            state.workspace_membership.lock().unwrap().active_member(),
            Some(member_a)
        );
        assert_eq!(*state.repository_generation.lock().unwrap(), 1);

        activate_admitted_member(&state, member_b)
            .await
            .expect("activate B");
        let active_b = state.repository.lock().unwrap().clone().expect("active B");
        assert_eq!(active_b.root, fs::canonicalize(&repository_b.0).unwrap());
        assert_eq!(
            state.workspace_membership.lock().unwrap().active_member(),
            Some(member_b)
        );
        assert_eq!(*state.repository_generation.lock().unwrap(), 2);
        assert_ne!(
            state.conversation.lock().unwrap().epoch,
            conversation_after_a
        );
        assert_eq!(
            state.repository_workflow.lock().unwrap().authorization,
            CommitAuthorizationPresentation::ReviewRequired
        );

        let prepared = PreparedHostInvocation::for_test(std::time::Instant::now());
        state
            .host_invocation
            .lock()
            .unwrap()
            .prepare(prepared)
            .unwrap();
        activate_admitted_member(&state, member_a)
            .await
            .expect("return to A clears prepared HostExplicit");
        assert_eq!(
            state.host_invocation.lock().unwrap().state(),
            CoordinatorState::Idle
        );
        assert_eq!(
            state.workspace_membership.lock().unwrap().active_member(),
            Some(member_a)
        );
        assert_eq!(*state.repository_generation.lock().unwrap(), 3);

        state.host_invocation.lock().unwrap().begin_model().unwrap();
        assert_eq!(
            activate_admitted_member(&state, member_b).await,
            Err(FrontendError::HostInvocationBusy)
        );
        assert_eq!(
            state.workspace_membership.lock().unwrap().active_member(),
            Some(member_a)
        );
        assert_eq!(*state.repository_generation.lock().unwrap(), 3);
        state.host_invocation.lock().unwrap().release_model();

        let stale = TestRepository::git_repository(GitRepositoryState::Clean);
        let stale_id = admit_repository(&state, &git, &stale.0).expect("admit stale candidate");
        fs::remove_dir_all(stale.0.join(".git")).expect("replace stale metadata");
        fs::create_dir(stale.0.join(".git")).expect("replacement metadata");
        assert_eq!(
            activate_admitted_member(&state, stale_id).await,
            Err(FrontendError::RepositoryMemberStale)
        );
        assert_eq!(
            state.workspace_membership.lock().unwrap().active_member(),
            Some(member_a)
        );
        assert_eq!(*state.repository_generation.lock().unwrap(), 3);

        let nested_parent = TestRepository::git_repository(GitRepositoryState::Clean);
        let nested_child = nested_parent.0.join("nested-member");
        fs::create_dir_all(&nested_child).expect("nested root");
        let output = Command::new(&git)
            .args(["init", "--quiet"])
            .current_dir(&nested_child)
            .output()
            .expect("nested Git should start");
        assert!(output.status.success());
        let nested_storage = TestRepository::new();
        let nested_state = DesktopAppState::new(nested_storage.0.clone());
        let child_id = admit_repository(&nested_state, &git, &nested_child).expect("admit child");
        assert_eq!(
            admit_repository(&nested_state, &git, &nested_parent.0),
            Err(FrontendError::RepositoryNestedMembershipConflict)
        );
        assert_eq!(
            nested_state
                .workspace_membership
                .lock()
                .unwrap()
                .member_count(),
            1
        );
        assert_eq!(
            nested_state
                .workspace_membership
                .lock()
                .unwrap()
                .active_member(),
            None
        );
        assert_ne!(child_id.as_debug_tuple(), (0, 0));

        let unsupported = TestRepository::new();
        fs::remove_dir_all(unsupported.0.join(".git")).unwrap();
        fs::write(unsupported.0.join(".git"), "gitdir: elsewhere").unwrap();
        assert_eq!(
            admit_repository(&state, &git, &unsupported.0),
            Err(FrontendError::RepositoryInvalid)
        );

        let restart_storage = TestRepository::new();
        let restart = DesktopAppState::new(restart_storage.0.clone());
        assert_eq!(
            restart.workspace_membership.lock().unwrap().member_count(),
            0
        );
        assert!(
            restart
                .workspace_membership
                .lock()
                .unwrap()
                .active_member()
                .is_none()
        );
        assert!(restart.repository.lock().unwrap().is_none());

        let host_names = [
            "fs.read",
            "repo.file-info",
            "repo.status",
            "repo.diff",
            "repo.diff-staged",
            "repo.create-branch",
            "repo.patch",
            "repo.edit-files",
            "repo.create-file",
            "repo.delete-file",
            "repo.rename-file",
        ];
        assert_eq!(
            host_names
                .iter()
                .filter(|name| host_kind(name).is_some())
                .count(),
            11
        );
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
    fn branch_authority_is_stored_only_when_resources_match() {
        let fixture = TestRepository::git_repository(GitRepositoryState::Clean);
        let git = TestRepository::native_git();
        let authority = RepositoryBranchCreationAuthority::new(&git, &fixture.0)
            .expect("branch authority should construct for selected repository");
        let repository = DesktopRepository::new_with_authorities(
            &git,
            &fixture.0,
            None,
            None,
            None,
            Some(authority.clone()),
        )
        .expect("matching authority should be stored");
        assert!(
            repository.branch_creation_authority.as_ref().is_some_and(
                |value| value.matches_resources(&repository.git_executable, &repository.root)
            )
        );

        let other = TestRepository::git_repository(GitRepositoryState::Clean);
        assert!(
            DesktopRepository::new_with_authorities(
                &git,
                &other.0,
                None,
                None,
                None,
                Some(authority),
            )
            .is_err()
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn branch_registry_requires_host_authority_and_dispatches_without_switching() {
        let fixture = TestRepository::git_repository(GitRepositoryState::Modified);
        let without_authority = fixture.desktop_repository();
        let registry = desktop_tool_registry(Some(&without_authority), None)
            .expect("registry without branch authority should build");
        assert!(
            !registry.definitions().iter().any(|definition| {
                definition.name.as_str() == REPOSITORY_CREATE_BRANCH_TOOL_NAME
            })
        );

        let repository = fixture.branch_repository();
        let registry = desktop_tool_registry(Some(&repository), None)
            .expect("registry with branch authority should build");
        let definition = registry
            .definitions()
            .into_iter()
            .find(|definition| definition.name.as_str() == REPOSITORY_CREATE_BRANCH_TOOL_NAME)
            .expect("branch Tool should be registered");
        assert_eq!(definition.permission, PermissionLevel::Execute);

        let before_head = String::from_utf8(
            Command::new(TestRepository::native_git())
                .args(["rev-parse", "HEAD"])
                .current_dir(&fixture.0)
                .output()
                .expect("HEAD should be readable")
                .stdout,
        )
        .expect("HEAD is UTF-8")
        .trim()
        .to_owned();
        let before_branch = String::from_utf8(
            Command::new(TestRepository::native_git())
                .args(["symbolic-ref", "--short", "HEAD"])
                .current_dir(&fixture.0)
                .output()
                .expect("symbolic HEAD should be readable")
                .stdout,
        )
        .expect("branch is UTF-8")
        .trim()
        .to_owned();

        let output = registry
            .execute(
                ToolCall {
                    id: ToolCallId::new(),
                    name: ToolName::new(REPOSITORY_CREATE_BRANCH_TOOL_NAME),
                    input: ToolInput(serde_json::json!({"name": "task-228-local"})),
                },
                ToolContext::default(),
            )
            .await
            .expect("branch dispatch should return a bounded result");
        assert!(!output.is_error);
        assert!(matches!(
            &output.content[0],
            ToolContent::Json(value) if value["status"] == "branch_created_verified"
        ));

        let after_head = String::from_utf8(
            Command::new(TestRepository::native_git())
                .args(["rev-parse", "HEAD"])
                .current_dir(&fixture.0)
                .output()
                .expect("HEAD should remain readable")
                .stdout,
        )
        .expect("HEAD is UTF-8")
        .trim()
        .to_owned();
        let after_branch = String::from_utf8(
            Command::new(TestRepository::native_git())
                .args(["symbolic-ref", "--short", "HEAD"])
                .current_dir(&fixture.0)
                .output()
                .expect("symbolic HEAD should remain readable")
                .stdout,
        )
        .expect("branch is UTF-8")
        .trim()
        .to_owned();
        let branch_head = String::from_utf8(
            Command::new(TestRepository::native_git())
                .args(["rev-parse", "refs/heads/task-228-local"])
                .current_dir(&fixture.0)
                .output()
                .expect("created branch should be readable")
                .stdout,
        )
        .expect("branch HEAD is UTF-8")
        .trim()
        .to_owned();
        assert_eq!(after_head, before_head);
        assert_eq!(after_branch, before_branch);
        assert_eq!(branch_head, before_head);
    }

    #[test]
    fn branch_effective_authority_is_host_classified_and_unavailable_paths_are_closed() {
        let no_repository = desktop_tool_composition_from_registry(
            desktop_tool_registry(None, None).expect("neutral registry should build"),
            None,
            false,
            &[],
        )
        .expect("neutral authority composition should build");
        let unavailable = no_repository
            .unavailable
            .iter()
            .find(|entry| {
                entry.public_tool_name.as_deref() == Some(REPOSITORY_CREATE_BRANCH_TOOL_NAME)
            })
            .expect("branch should be known unavailable without repository");
        assert_eq!(
            unavailable.reason,
            super::effective_authority::UnavailableReason::RepositoryRequired
        );

        let fixture = TestRepository::git_repository(GitRepositoryState::Clean);
        let without_authority = fixture.desktop_repository();
        let unavailable_repository = desktop_tool_composition_from_registry(
            desktop_tool_registry(Some(&without_authority), None).expect("registry should build"),
            Some(&without_authority),
            false,
            &[],
        )
        .expect("authority composition should build");
        let unavailable = unavailable_repository
            .unavailable
            .iter()
            .find(|entry| {
                entry.public_tool_name.as_deref() == Some(REPOSITORY_CREATE_BRANCH_TOOL_NAME)
            })
            .expect("branch should be unavailable without authority");
        assert_eq!(
            unavailable.reason,
            super::effective_authority::UnavailableReason::AuthorityNotGranted
        );

        let selected = fixture.branch_repository();
        let composition = desktop_tool_composition_from_registry(
            desktop_tool_registry(Some(&selected), None).expect("branch registry should build"),
            Some(&selected),
            false,
            &[],
        )
        .expect("branch authority composition should build");
        let tool = composition
            .tools
            .iter()
            .find(|entry| entry.public_tool_name == REPOSITORY_CREATE_BRANCH_TOOL_NAME)
            .expect("branch should be effective");
        assert_eq!(tool.source_kind, SourceKind::RepositoryHost);
        assert_eq!(tool.source_label, "desktop_repository");
        assert_eq!(tool.effect_class, EffectClass::RepositoryMutation);
        assert_eq!(
            tool.authority_category,
            super::effective_authority::AuthorityCategory::RepositoryLocalBranchCreation
        );
        assert_eq!(tool.permission, PermissionLevel::Execute);
        assert!(tool.repository_bound);
        assert!(!tool.advertised);
    }

    #[test]
    fn patch_is_the_seventh_host_tool_and_retains_its_shared_preparer() {
        let fixture = TestRepository::git_repository(GitRepositoryState::Clean);
        let repository = fixture.desktop_repository();
        let composition = desktop_tool_composition_from_registry(
            desktop_tool_registry(Some(&repository), None).expect("registry should build"),
            Some(&repository),
            false,
            &[],
        )
        .expect("patch composition should build");
        let patch = composition
            .tools
            .iter()
            .find(|entry| entry.public_tool_name == "repo.patch")
            .expect("repo.patch should be composed");
        assert_eq!(patch.source_kind, SourceKind::RepositoryHost);
        assert_eq!(patch.effect_class, EffectClass::RepositoryMutation);
        assert_eq!(
            patch.authority_category,
            super::effective_authority::AuthorityCategory::RepositoryContentMutation
        );
        assert_eq!(patch.permission, PermissionLevel::Execute);
        assert!(patch.repository_bound);
        assert_eq!(
            patch.host_invocation.kind,
            Some(HostInvocationKind::RepoPatch)
        );
        assert!(composition.repository_patch_preparer.is_some());
        assert!(
            host_descriptor(
                patch,
                true,
                true,
                true,
                false,
                true,
                false,
                false,
                false,
                CoordinatorState::Idle,
            )
            .eligible
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn production_patch_prepare_finalizes_a_real_ticket_without_effect() {
        let fixture = TestRepository::git_repository(GitRepositoryState::Clean);
        let preparer = Arc::new(
            rah_tools::RepositoryPatchPreparer::new(TestRepository::native_git(), &fixture.0)
                .expect("shared patch preparer should construct"),
        );
        let target = fixture.0.join("tracked.txt");
        let before_target = fs::read(&target).expect("patch target should read");
        let before_temporary_count = live_patch_temporary_count(&fixture.0)
            .expect("temporary artifacts should be observable");
        let preparation = preparer
            .prepare(super::RepositoryPatchPreparationRequest {
                path: "tracked.txt".to_owned(),
                expected_old_text: "base".to_owned(),
                replacement_text: "changed".to_owned(),
            })
            .await
            .expect("shared production preparation should succeed");

        let definition = desktop_tool_registry(Some(&fixture.desktop_repository()), None)
            .expect("host registry should compose")
            .definitions()
            .into_iter()
            .find(|definition| definition.name.as_str() == "repo.patch")
            .expect("repo.patch definition should exist");
        let mut coordinator = HostInvocationCoordinator::default();
        coordinator
            .begin_prepare()
            .expect("preparation reservation should begin");
        let ticket_id = coordinator.next_ticket_id();
        coordinator
            .finalize_prepare(PreparedHostInvocation::new(
                ticket_id.clone(),
                "test-activity".to_owned(),
                HostInvocationKind::RepoPatch,
                ToolName::new("repo.patch"),
                definition.clone(),
                ToolCall {
                    id: ToolCallId::new(),
                    name: ToolName::new("repo.patch"),
                    input: preparation.tool_input().clone(),
                },
                Arc::new(super::ToolRegistry::new()),
                vec![PermissionLevel::Execute],
                [0; 4],
                None,
                0,
                PreparedHostPayload::Patch {
                    preparation: Box::new(preparation),
                    preparer,
                },
            ))
            .expect("reserved production preparation should finalize");
        let ticket = coordinator
            .take_prepared(&ticket_id, std::time::Instant::now())
            .expect("production preparation should expose a real ticket");
        assert_eq!(ticket.kind, HostInvocationKind::RepoPatch);
        assert_eq!(
            fs::read(&target).expect("patch target should still read"),
            before_target
        );
        assert_eq!(
            live_patch_temporary_count(&fixture.0)
                .expect("temporary artifacts should be observable"),
            before_temporary_count
        );
        coordinator.finish_host();
    }

    #[tokio::test(flavor = "current_thread")]
    async fn production_multi_file_prepare_retains_shared_review_without_effect() {
        let fixture = TestRepository::git_repository(GitRepositoryState::Clean);
        let preparer = Arc::new(
            RepositoryMultiFileEditPreparer::new(TestRepository::native_git(), &fixture.0)
                .expect("shared multi-file preparer should construct"),
        );
        let before_tracked = fs::read(fixture.0.join("tracked.txt")).unwrap();
        let before_nested = fs::read(fixture.0.join("nested/ordinary.txt")).unwrap();
        let preparation = preparer
            .prepare(RepositoryMultiFileEditPreparationRequest {
                targets: vec![
                    RepositoryMultiFileEditPreparationTarget {
                        path: "nested/ordinary.txt".to_owned(),
                        replacements: vec![RepositoryMultiFileEditTextReplacement {
                            expected_old_text: "ordinary".to_owned(),
                            replacement_text: "nested changed".to_owned(),
                        }],
                    },
                    RepositoryMultiFileEditPreparationTarget {
                        path: "tracked.txt".to_owned(),
                        replacements: vec![RepositoryMultiFileEditTextReplacement {
                            expected_old_text: "base".to_owned(),
                            replacement_text: "tracked changed".to_owned(),
                        }],
                    },
                ],
            })
            .await
            .expect("shared multi-file preparation should succeed");
        assert_eq!(preparation.review().target_count(), 2);
        assert_eq!(
            preparation
                .review()
                .targets()
                .iter()
                .map(|target| target.path())
                .collect::<Vec<_>>(),
            vec!["nested/ordinary.txt", "tracked.txt"]
        );
        assert_eq!(
            fs::read(fixture.0.join("tracked.txt")).unwrap(),
            before_tracked
        );
        assert_eq!(
            fs::read(fixture.0.join("nested/ordinary.txt")).unwrap(),
            before_nested
        );

        let repository = fixture.desktop_repository();
        let composition = desktop_tool_composition_from_registry(
            desktop_tool_registry(Some(&repository), None).expect("registry should build"),
            Some(&repository),
            false,
            &[],
        )
        .expect("multi-file composition should build");
        assert!(composition.repository_multi_file_edit_preparer.is_some());
        let edit = composition
            .tools
            .iter()
            .find(|entry| entry.public_tool_name == "repo.edit-files")
            .expect("repo.edit-files should be composed");
        assert_eq!(
            edit.host_invocation.kind,
            Some(HostInvocationKind::RepoEditFiles)
        );
        assert!(
            host_descriptor(
                edit,
                true,
                true,
                true,
                false,
                true,
                true,
                false,
                false,
                CoordinatorState::Idle
            )
            .eligible
        );
    }

    #[test]
    fn create_file_remains_host_tool_and_retains_its_shared_preparer() {
        let fixture = TestRepository::git_repository(GitRepositoryState::Clean);
        let repository = fixture.desktop_repository();
        let composition = desktop_tool_composition_from_registry(
            desktop_tool_registry(Some(&repository), None).expect("registry should build"),
            Some(&repository),
            false,
            &[],
        )
        .expect("create-file composition should build");
        let create_file = composition
            .tools
            .iter()
            .find(|entry| entry.public_tool_name == "repo.create-file")
            .expect("repo.create-file should be composed");
        assert_eq!(create_file.source_kind, SourceKind::RepositoryHost);
        assert_eq!(create_file.effect_class, EffectClass::RepositoryMutation);
        assert_eq!(
            create_file.authority_category,
            super::effective_authority::AuthorityCategory::RepositoryFileCreation
        );
        assert!(create_file.repository_bound);
        assert_eq!(
            create_file.host_invocation.kind,
            Some(HostInvocationKind::RepoCreateFile)
        );
        assert!(composition.repository_create_file_preparer.is_some());
        assert!(
            host_descriptor(
                create_file,
                true,
                true,
                true,
                false,
                false,
                false,
                true,
                false,
                CoordinatorState::Idle,
            )
            .eligible
        );
    }

    #[test]
    fn delete_file_is_the_tenth_host_tool_and_requires_deletion_authority() {
        let fixture = TestRepository::git_repository(GitRepositoryState::Clean);
        let repository = fixture.deletion_repository();
        let composition = desktop_tool_composition_from_registry(
            desktop_tool_registry(Some(&repository), None).expect("registry should build"),
            Some(&repository),
            false,
            &[],
        )
        .expect("deletion composition should build");
        let deletion = composition
            .tools
            .iter()
            .find(|entry| entry.public_tool_name == "repo.delete-file")
            .expect("repo.delete-file should be composed");
        assert_eq!(
            deletion.host_invocation.kind,
            Some(HostInvocationKind::RepoDeleteFile)
        );
        assert!(composition.repository_delete_file_preparer.is_some());
        assert!(
            host_descriptor(
                deletion,
                true,
                true,
                true,
                false,
                false,
                false,
                false,
                true,
                CoordinatorState::Idle,
            )
            .eligible
        );

        let without_authority = fixture.desktop_repository();
        let composition = desktop_tool_composition_from_registry(
            desktop_tool_registry(Some(&without_authority), None).expect("registry should build"),
            Some(&without_authority),
            false,
            &[],
        )
        .expect("composition without deletion authority should build");
        assert!(composition.repository_delete_file_preparer.is_none());
        assert!(
            composition
                .tools
                .iter()
                .all(|entry| entry.public_tool_name != "repo.delete-file")
        );
    }

    #[test]
    fn rename_file_is_the_eleventh_host_tool_and_requires_rename_authority() {
        let fixture = TestRepository::git_repository(GitRepositoryState::Clean);
        let git = TestRepository::native_git();
        let repository = DesktopRepository::new_with_authorities(
            &git,
            &fixture.0,
            Some(
                RepositoryDirectoryCreationAuthority::new(&fixture.0)
                    .expect("directory authority should construct"),
            ),
            Some(
                RepositoryFileDeletionAuthority::new(&git, &fixture.0)
                    .expect("deletion authority should construct"),
            ),
            Some(
                RepositoryFileRenameAuthority::new(&git, &fixture.0)
                    .expect("rename authority should construct"),
            ),
            Some(
                RepositoryBranchCreationAuthority::new(&git, &fixture.0)
                    .expect("branch authority should construct"),
            ),
        )
        .expect("fully authorized repository should construct");
        let composition = desktop_tool_composition_from_registry(
            desktop_tool_registry(Some(&repository), None).expect("registry should build"),
            Some(&repository),
            false,
            &[],
        )
        .expect("rename composition should build");
        let rename = composition
            .tools
            .iter()
            .find(|entry| entry.public_tool_name == "repo.rename-file")
            .expect("repo.rename-file should be composed");
        assert_eq!(rename.source_kind, SourceKind::RepositoryHost);
        assert_eq!(rename.source_label, "desktop_repository");
        assert_eq!(rename.effect_class, EffectClass::RepositoryMutation);
        assert_eq!(
            rename.authority_category,
            super::effective_authority::AuthorityCategory::RepositoryFileRename
        );
        assert_eq!(rename.permission, PermissionLevel::Execute);
        assert!(rename.repository_bound);
        assert_eq!(
            rename.host_invocation.kind,
            Some(HostInvocationKind::RepoRenameFile)
        );
        assert!(composition.repository_rename_file_preparer.is_some());
        assert!(
            host_descriptor_with_rename(
                rename,
                true,
                true,
                true,
                false,
                false,
                false,
                false,
                false,
                true,
                CoordinatorState::Idle,
            )
            .eligible
        );
        let eligible_count = composition
            .tools
            .iter()
            .filter(|entry| {
                host_descriptor_with_rename(
                    entry,
                    true,
                    true,
                    true,
                    true,
                    true,
                    true,
                    true,
                    true,
                    true,
                    CoordinatorState::Idle,
                )
                .eligible
            })
            .count();
        assert_eq!(eligible_count, 11);

        let without_authority = fixture.desktop_repository();
        let composition = desktop_tool_composition_from_registry(
            desktop_tool_registry(Some(&without_authority), None)
                .expect("registry without rename authority should build"),
            Some(&without_authority),
            false,
            &[],
        )
        .expect("composition without rename authority should build");
        assert!(composition.repository_rename_file_preparer.is_none());
        assert!(
            composition
                .tools
                .iter()
                .all(|entry| entry.public_tool_name != "repo.rename-file")
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn reviewed_rename_prepare_is_zero_effect_and_dispatches_once_with_retained_input() {
        let fixture = TestRepository::git_repository(GitRepositoryState::Clean);
        let repository = fixture.rename_repository();
        let source = fixture.0.join("tracked.txt");
        let destination = fixture.0.join("renamed.txt");
        let before = fs::read(&source).expect("rename source should read");
        let executions = Arc::new(AtomicUsize::new(0));
        let registry = counting_rename_registry(&repository, Arc::clone(&executions));
        let storage = TestRepository::new();
        let app = tauri::Builder::default()
            .any_thread()
            .manage(DesktopAppState::new(storage.0.clone()))
            .build(tauri::generate_context!())
            .expect("rename Desktop app should build");
        replace_selected_repository(app.state::<DesktopAppState>().inner(), repository);
        let state = app.state::<DesktopAppState>().inner();
        state
            .host_invocation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .begin_prepare()
            .expect("rename preparation should reserve the coordinator");
        let app_handle = app.handle().clone();
        let prepared = prepare_repo_rename_file_with_current(
            HostPrepareRenameFileRequest {
                source_path: "tracked.txt".to_owned(),
                destination_path: "renamed.txt".to_owned(),
            },
            &app_handle,
            state,
            current_rename_composition(state, Arc::clone(&registry)),
        )
        .await
        .expect("reviewed rename preparation should succeed");
        assert!(!prepared.ticket_id.is_empty());
        assert_eq!(prepared.review.source_path(), "tracked.txt");
        assert_eq!(prepared.review.destination_path(), "renamed.txt");
        assert_eq!(executions.load(Ordering::SeqCst), 0);
        assert_eq!(
            fs::read(&source).expect("source should remain after Prepare"),
            before
        );
        assert!(!destination.exists());

        let ticket = state
            .host_invocation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take_prepared(&prepared.ticket_id, std::time::Instant::now())
            .expect("prepared rename ticket should be single-use");
        let PreparedHostInvocation {
            registry,
            expected_definition,
            allowed_permissions,
            call,
            kind,
            activity_id,
            repository_identity,
            generations,
            payload,
            ..
        } = ticket;
        let rename_proof = match payload {
            PreparedHostPayload::RenameFile {
                preparation,
                preparer,
            } => Some((preparation, preparer)),
            _ => panic!("ticket should retain rename proof"),
        };
        emit_host_activity(
            app.handle(),
            HostActivityEvent {
                source: "host_explicit",
                invocation_id: activity_id.clone(),
                tool: "repo.rename-file".to_owned(),
                state: HostActivityState::Started,
                result: None,
                review: None,
            },
        );
        run_host_tool(
            app.handle().clone(),
            registry,
            expected_definition,
            allowed_permissions,
            call,
            kind,
            activity_id,
            repository_identity,
            generations,
            None,
            None,
            None,
            rename_proof,
        )
        .await;
        assert_eq!(executions.load(Ordering::SeqCst), 1);
        assert!(!source.exists());
        assert_eq!(
            fs::read(&destination).expect("destination should be written"),
            before
        );
        assert_eq!(
            state
                .host_invocation
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .state(),
            CoordinatorState::Idle
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn deterministic_create_file_dispatch_runs_the_real_tool_once() {
        let fixture = TestRepository::git_repository(GitRepositoryState::Clean);
        let target = fixture.0.join("host-created.rs");
        let content = "RAH_SECRET_CREATE_FILE_CONTENT_SENTINEL\n";
        let index_before = fs::read(fixture.0.join(".git/index")).expect("index reads");
        let git_text = |args: &[&str]| {
            String::from_utf8(
                Command::new(TestRepository::native_git())
                    .args(args)
                    .current_dir(&fixture.0)
                    .output()
                    .expect("Git observation starts")
                    .stdout,
            )
            .expect("Git observation is UTF-8")
        };
        let head_before = git_text(&["rev-parse", "HEAD"]);
        let refs_before = git_text(&["for-each-ref", "--format=%(refname)%00%(objectname)%00"]);
        let executions = Arc::new(AtomicUsize::new(0));
        let tool = RepositoryFileCreationTool::new(TestRepository::native_git(), &fixture.0)
            .expect("real create-file Tool constructs");
        let mut registry = super::ToolRegistry::new();
        registry
            .register(Arc::new(CountingCreateFileTool {
                inner: tool,
                executions: Arc::clone(&executions),
            }))
            .expect("counting wrapper registers the real Tool");
        let registry = Arc::new(registry);
        let preparer = Arc::new(
            rah_tools::RepositoryCreateFilePreparer::new(TestRepository::native_git(), &fixture.0)
                .expect("shared create-file preparer constructs"),
        );
        let preparation = preparer
            .prepare(super::RepositoryCreateFilePreparationRequest {
                path: "host-created.rs".to_owned(),
                content: content.to_owned(),
            })
            .await
            .expect("shared preparation succeeds");
        assert_eq!(executions.load(Ordering::SeqCst), 0);
        assert!(!target.exists());
        assert_eq!(
            fs::read(fixture.0.join(".git/index")).unwrap(),
            index_before
        );
        assert_eq!(git_text(&["rev-parse", "HEAD"]), head_before);
        assert_eq!(
            git_text(&["for-each-ref", "--format=%(refname)%00%(objectname)%00"]),
            refs_before
        );

        let definition = registry
            .definitions()
            .into_iter()
            .find(|definition| definition.name.as_str() == "repo.create-file")
            .expect("current registry contains create-file");
        let mut coordinator = HostInvocationCoordinator::default();
        coordinator
            .begin_prepare()
            .expect("preparation reserves slot");
        let ticket_id = coordinator.next_ticket_id();
        let activity_id = coordinator.next_invocation_id();
        coordinator
            .finalize_prepare(PreparedHostInvocation::new(
                ticket_id.clone(),
                activity_id.clone(),
                HostInvocationKind::RepoCreateFile,
                ToolName::new("repo.create-file"),
                definition.clone(),
                ToolCall {
                    id: ToolCallId::new(),
                    name: ToolName::new("repo.create-file"),
                    input: preparation.tool_input().clone(),
                },
                Arc::clone(&registry),
                vec![PermissionLevel::Execute],
                [0; 4],
                None,
                0,
                PreparedHostPayload::CreateFile {
                    preparation: Box::new(preparation),
                    preparer: Arc::clone(&preparer),
                },
            ))
            .expect("prepared create-file ticket finalizes");
        let ticket = coordinator
            .take_prepared(&ticket_id, std::time::Instant::now())
            .expect("exact ticket confirms once");
        let expected = match &ticket.payload {
            PreparedHostPayload::CreateFile {
                preparation,
                preparer: retained,
            } => {
                assert!(Arc::ptr_eq(retained, &preparer));
                preparer
                    .revalidate(preparation)
                    .await
                    .expect("preparer revalidation succeeds");
                super::CreateFileExpectedOutput {
                    path: preparation.review().path().to_owned(),
                    length: preparation.content_byte_length(),
                    sha256: preparation.content_sha256().to_owned(),
                }
            }
            _ => panic!("wrong prepared payload kind"),
        };
        super::authorize_tool_dispatch(
            &ticket.registry,
            &ticket.expected_definition,
            &ticket.allowed_permissions,
            &ticket.call,
        )
        .expect("D2 preflight admits exact current registry call");
        let output = super::authorized_tool_dispatch(
            &ticket.registry,
            &ticket.expected_definition,
            &ticket.allowed_permissions,
            ticket.call,
            ToolContext::default(),
        )
        .await
        .expect("current ToolRegistry dispatch succeeds");
        assert_eq!(executions.load(Ordering::SeqCst), 1);
        assert_eq!(
            super::classify_repository_create_file_output(&output, &expected),
            CreateFileResultClassification::Ok
        );
        assert_eq!(
            fs::read(&target).expect("created target reads"),
            content.as_bytes()
        );
        assert_eq!(
            fs::read(fixture.0.join(".git/index")).unwrap(),
            index_before
        );
        assert_eq!(git_text(&["rev-parse", "HEAD"]), head_before);
        assert_eq!(
            git_text(&["for-each-ref", "--format=%(refname)%00%(objectname)%00"]),
            refs_before
        );
        let status = git_text(&["status", "--porcelain=v1", "--", "host-created.rs"]);
        assert_eq!(status, "?? host-created.rs\n");
        coordinator.finish_host();
        assert_eq!(coordinator.state(), CoordinatorState::Idle);
    }

    #[test]
    fn multi_file_result_mapping_is_strict_and_preserves_all_six_classes() {
        let paths = vec!["a.rs".to_owned(), "b.rs".to_owned()];
        let output = |value: Value, is_error| ToolOutput {
            content: vec![ToolContent::Json(value)],
            is_error,
        };
        let effects = |states: &[&str]| {
            serde_json::json!({
                "status": "placeholder",
                "effects": [
                    {"path": "a.rs", "state": states[0]},
                    {"path": "b.rs", "state": states[1]}
                ]
            })
        };
        assert_eq!(
            classify_repository_multi_file_output(
                &output(serde_json::json!({"status": "invalid_target"}), true),
                &paths,
            ),
            MultiFileResultClassification::InvalidTarget
        );
        assert_eq!(
            classify_repository_multi_file_output(
                &output(serde_json::json!({"status": "precondition_failed"}), true),
                &paths,
            ),
            MultiFileResultClassification::PreconditionFailed
        );
        let mut value = effects(&["committed_verified", "committed_verified"]);
        value["status"] = serde_json::json!("ok");
        assert_eq!(
            classify_repository_multi_file_output(&output(value, false), &paths),
            MultiFileResultClassification::Ok
        );
        let mut value = effects(&["unchanged_verified", "not_attempted"]);
        value["status"] = serde_json::json!("failed_known_no_effect");
        assert_eq!(
            classify_repository_multi_file_output(&output(value, true), &paths),
            MultiFileResultClassification::FailedKnownNoEffect
        );
        let mut value = effects(&["committed_verified", "not_attempted"]);
        value["status"] = serde_json::json!("partial_effect");
        assert_eq!(
            classify_repository_multi_file_output(&output(value, true), &paths),
            MultiFileResultClassification::PartialEffect
        );
        let mut value = effects(&["committed_verified", "uncertain"]);
        value["status"] = serde_json::json!("uncertain");
        assert_eq!(
            classify_repository_multi_file_output(&output(value, true), &paths),
            MultiFileResultClassification::Uncertain
        );
        let malformed = output(
            serde_json::json!({
                "status": "ok",
                "effects": [{"path": "a.rs", "state": "committed_verified"},
                             {"path": "wrong.rs", "state": "committed_verified"}]
            }),
            false,
        );
        assert_eq!(
            classify_repository_multi_file_output(&malformed, &paths),
            MultiFileResultClassification::Malformed
        );
        let assert_malformed = |output: ToolOutput| {
            assert_eq!(
                classify_repository_multi_file_output(&output, &paths),
                MultiFileResultClassification::Malformed
            );
            assert_eq!(
                super::multi_file_host_terminal_state(MultiFileResultClassification::Malformed),
                HostActivityState::PossibleEffectUnknown
            );
        };
        assert_malformed(ToolOutput {
            content: vec![ToolContent::Text("not-json".to_owned())],
            is_error: true,
        });
        assert_malformed(ToolOutput {
            content: vec![
                ToolContent::Json(serde_json::json!({"status": "ok"})),
                ToolContent::Json(serde_json::json!({"status": "ok"})),
            ],
            is_error: false,
        });
        assert_malformed(output(serde_json::json!({}), true));
        assert_malformed(output(serde_json::json!({"status": "unknown"}), true));
        assert_malformed(output(
            serde_json::json!({"status": "ok", "effects": []}),
            true,
        ));
        assert_malformed(output(
            serde_json::json!({"status": "invalid_target", "effects": []}),
            true,
        ));
        let mut extra_effect_field = effects(&["committed_verified", "committed_verified"]);
        extra_effect_field["status"] = serde_json::json!("ok");
        extra_effect_field["effects"][0]["detail"] = serde_json::json!("source");
        assert_malformed(output(extra_effect_field, false));
        let mut wrong_order = effects(&["committed_verified", "committed_verified"]);
        wrong_order["status"] = serde_json::json!("ok");
        wrong_order["effects"][0]["path"] = serde_json::json!("b.rs");
        wrong_order["effects"][1]["path"] = serde_json::json!("a.rs");
        assert_malformed(output(wrong_order, false));
        let mut impossible_state = effects(&["not_attempted", "committed_verified"]);
        impossible_state["status"] = serde_json::json!("ok");
        assert_malformed(output(impossible_state, false));
        let mut non_prefix = effects(&["committed_verified", "unchanged_verified"]);
        non_prefix["status"] = serde_json::json!("partial_effect");
        non_prefix["effects"][1]["state"] = serde_json::json!("committed_verified");
        assert_malformed(output(non_prefix, true));
        let mut contradictory_uncertain = effects(&["uncertain", "committed_verified"]);
        contradictory_uncertain["status"] = serde_json::json!("uncertain");
        assert_malformed(output(contradictory_uncertain, true));
        let mut contradictory_known_no_effect = effects(&["committed_verified", "not_attempted"]);
        contradictory_known_no_effect["status"] = serde_json::json!("failed_known_no_effect");
        assert_malformed(output(contradictory_known_no_effect, true));
        let redacted = output(
            serde_json::json!({
                "status": "partial_effect",
                "effects": [
                    {"path": "a.rs", "state": "committed_verified"},
                    {"path": "b.rs", "state": "not_attempted"}
                ]
            }),
            true,
        );
        let activity = HostActivityEvent {
            source: "host_explicit",
            invocation_id: "host-explicit-test".to_owned(),
            tool: "repo.edit-files".to_owned(),
            state: HostActivityState::PartialEffect,
            result: Some(redacted),
            review: None,
        };
        let serialized = serde_json::to_string(&activity).unwrap();
        assert!(!serialized.contains("OLD_SOURCE_SENTINEL"));
        assert!(!serialized.contains("NEW_REPLACEMENT_SENTINEL"));
        for forbidden in [
            "ticketId",
            "ToolInput",
            "ToolOutput",
            "expectedOldText",
            "replacementText",
            "C:\\\\Users\\\\secret",
        ] {
            assert!(
                !serialized.contains(forbidden),
                "activity leaked {forbidden}"
            );
        }
    }

    #[test]
    fn multi_file_malformed_source_is_never_retained_as_generic_activity() {
        let output = ToolOutput {
            content: vec![ToolContent::Json(serde_json::json!({
                "status": "ok",
                "source": "<script>alert(1)</script> SECRET_EXPECTED_OLD_SOURCE",
                "replacement": "SECRET_REPLACEMENT <img src=x>",
            }))],
            is_error: false,
        };
        let classification =
            classify_repository_multi_file_output(&output, &["safe.txt".to_owned()]);
        assert_eq!(classification, MultiFileResultClassification::Malformed);
        assert_eq!(
            super::multi_file_host_terminal_state(classification),
            HostActivityState::PossibleEffectUnknown
        );
        assert!(super::safe_multi_file_activity_result(output, classification).is_none());
    }

    #[test]
    fn repository_bound_authoring_invalidates_commit_at_started_boundary() {
        assert!(super::repository_bound_authoring_kind(
            HostInvocationKind::RepoPatch
        ));
        assert!(super::repository_bound_authoring_kind(
            HostInvocationKind::RepoEditFiles
        ));
        assert!(super::repository_bound_authoring_kind(
            HostInvocationKind::RepoCreateFile
        ));
        assert!(super::repository_bound_authoring_kind(
            HostInvocationKind::RepoDeleteFile
        ));
        assert!(!super::repository_bound_authoring_kind(
            HostInvocationKind::RepoCreateBranch
        ));
        assert!(!super::repository_bound_authoring_kind(
            HostInvocationKind::FsRead
        ));
    }

    #[test]
    fn create_file_result_classification_is_strict_and_redacted() {
        let content = "RAH_SECRET_CREATE_FILE_CONTENT_SENTINEL";
        let expected = super::CreateFileExpectedOutput {
            path: "src/new.rs".to_owned(),
            length: content.len(),
            sha256: format!("{:x}", Sha256::digest(content.as_bytes())),
        };
        let output = |value: Value, is_error| ToolOutput {
            content: vec![ToolContent::Json(value)],
            is_error,
        };
        let success = output(
            serde_json::json!({
                "status": "ok",
                "path": expected.path.clone(),
                "length": expected.length,
                "sha256": expected.sha256.clone(),
            }),
            false,
        );
        assert_eq!(
            super::classify_repository_create_file_output(&success, &expected),
            CreateFileResultClassification::Ok
        );
        for (status, expected_classification) in [
            (
                "invalid_target",
                CreateFileResultClassification::InvalidTarget,
            ),
            (
                "precondition_failed",
                CreateFileResultClassification::PreconditionFailed,
            ),
            (
                "create_failed_known",
                CreateFileResultClassification::CreateFailedKnown,
            ),
            (
                "write_failed_known",
                CreateFileResultClassification::WriteFailedKnown,
            ),
            ("uncertain", CreateFileResultClassification::Uncertain),
        ] {
            assert_eq!(
                super::classify_repository_create_file_output(
                    &output(serde_json::json!({"status": status}), true),
                    &expected,
                ),
                expected_classification
            );
        }
        assert_eq!(
            super::create_file_host_terminal_state(
                CreateFileResultClassification::WriteFailedKnown
            ),
            HostActivityState::PartialEffect
        );
        for malformed in [
            output(serde_json::json!({"status": "ok"}), false),
            output(serde_json::json!({"status": "unknown"}), true),
            output(
                serde_json::json!({
                    "status": "ok",
                    "path": expected.path.clone(),
                    "length": expected.length,
                    "sha256": expected.sha256.clone(),
                }),
                true,
            ),
            output(
                serde_json::json!({"status": "invalid_target", "path": "src/new.rs"}),
                true,
            ),
            output(
                serde_json::json!({
                    "status": "ok",
                    "path": "wrong.rs",
                    "length": expected.length,
                    "sha256": expected.sha256.clone(),
                }),
                false,
            ),
            output(
                serde_json::json!({
                    "status": "ok",
                    "path": expected.path.clone(),
                    "length": expected.length + 1,
                    "sha256": expected.sha256.clone(),
                }),
                false,
            ),
            output(
                serde_json::json!({
                    "status": "ok",
                    "path": expected.path.clone(),
                    "length": expected.length,
                    "sha256": "RAH_SECRET_CREATE_FILE_HASH_SENTINEL",
                }),
                false,
            ),
            output(
                serde_json::json!({
                    "status": "write_failed_known",
                    "path": expected.path.clone(),
                    "length": expected.length,
                    "sha256": expected.sha256.clone(),
                }),
                true,
            ),
            ToolOutput {
                content: vec![ToolContent::Text("not-json".to_owned())],
                is_error: true,
            },
            ToolOutput {
                content: vec![
                    ToolContent::Json(serde_json::json!({"status": "ok"})),
                    ToolContent::Json(serde_json::json!({"status": "ok"})),
                ],
                is_error: false,
            },
        ] {
            assert_eq!(
                super::classify_repository_create_file_output(&malformed, &expected),
                CreateFileResultClassification::Malformed
            );
            assert_eq!(
                super::create_file_host_terminal_state(CreateFileResultClassification::Malformed),
                HostActivityState::PossibleEffectUnknown
            );
        }
        let activity = HostActivityEvent {
            source: "host_explicit",
            invocation_id: "host-explicit-create-file-activity".to_owned(),
            tool: "repo.create-file".to_owned(),
            state: HostActivityState::ToolCompleted,
            result: super::safe_create_file_activity_result(CreateFileResultClassification::Ok),
            review: None,
        };
        let prepared = super::prepared_host_activity(
            "host-explicit-create-file-activity".to_owned(),
            "repo.create-file".to_owned(),
            None,
        );
        let serialized = format!(
            "{}{}",
            serde_json::to_string(&prepared).unwrap(),
            serde_json::to_string(&activity).unwrap()
        );
        for secret in [
            "RAH_SECRET_CREATE_FILE_TICKET_SENTINEL",
            content,
            expected.sha256.as_str(),
            "native-path-sentinel",
            "parent-identity-sentinel",
            "raw-tool-input-sentinel",
        ] {
            assert!(
                !serialized.contains(secret),
                "privacy sentinel leaked: {secret}"
            );
        }
        assert!(serialized.contains("host-explicit-create-file-activity"));
    }

    #[test]
    fn delete_file_result_classification_is_strict_and_status_only() {
        let output = |value: Value, is_error| ToolOutput {
            content: vec![ToolContent::Json(value)],
            is_error,
        };
        let path = "src/delete.rs";
        for (status, is_error, uncertain, expected) in [
            (
                "deleted_verified",
                false,
                false,
                DeleteFileResultClassification::DeletedVerified,
            ),
            (
                "known_no_effect",
                true,
                false,
                DeleteFileResultClassification::KnownNoEffect,
            ),
            (
                "invalid_input",
                true,
                false,
                DeleteFileResultClassification::InvalidInput,
            ),
            (
                "precondition_failed",
                true,
                false,
                DeleteFileResultClassification::PreconditionFailed,
            ),
            (
                "uncertain",
                true,
                true,
                DeleteFileResultClassification::Uncertain,
            ),
        ] {
            let value = if status == "invalid_input" || status == "uncertain" {
                serde_json::json!({"status": status, "uncertain": uncertain})
            } else {
                serde_json::json!({"status": status, "uncertain": uncertain, "path": path})
            };
            assert_eq!(
                super::classify_repository_delete_file_output(&output(value, is_error), path,),
                expected
            );
        }
        let uncertain_with_path = output(
            serde_json::json!({"status":"uncertain","uncertain":true,"path":path}),
            true,
        );
        assert_eq!(
            super::classify_repository_delete_file_output(&uncertain_with_path, path),
            DeleteFileResultClassification::Uncertain
        );
        for malformed in [
            output(
                serde_json::json!({"status":"unknown","uncertain":false}),
                true,
            ),
            output(
                serde_json::json!({"status":"deleted_verified","uncertain":true,"path":path}),
                false,
            ),
            output(
                serde_json::json!({"status":"deleted_verified","uncertain":false,"path":"wrong.rs"}),
                false,
            ),
            output(
                serde_json::json!({"status":"invalid_input","uncertain":false,"path":path}),
                true,
            ),
            output(
                serde_json::json!({"status":"precondition_failed","uncertain":false}),
                true,
            ),
            output(
                serde_json::json!({"status":"uncertain","uncertain":false}),
                true,
            ),
            output(
                serde_json::json!({"status":"uncertain","uncertain":true,"path":"wrong.rs"}),
                true,
            ),
            output(
                serde_json::json!({"status":"known_no_effect","uncertain":false,"path":path,"extra":true}),
                true,
            ),
            ToolOutput {
                content: vec![ToolContent::Text("not-json".to_owned())],
                is_error: true,
            },
            ToolOutput {
                content: vec![
                    ToolContent::Json(serde_json::json!({"status":"uncertain","uncertain":true})),
                    ToolContent::Json(serde_json::json!({"status":"uncertain","uncertain":true})),
                ],
                is_error: true,
            },
        ] {
            assert_eq!(
                super::classify_repository_delete_file_output(&malformed, path),
                DeleteFileResultClassification::Malformed
            );
        }
        for classification in [
            DeleteFileResultClassification::Uncertain,
            DeleteFileResultClassification::Malformed,
        ] {
            assert_eq!(
                super::delete_file_host_terminal_state(classification),
                HostActivityState::PossibleEffectUnknown
            );
            let safe = super::safe_delete_file_activity_result(classification);
            assert_eq!(
                safe.content,
                vec![ToolContent::Json(serde_json::json!({"status":"uncertain"}))]
            );
            assert!(safe.is_error);
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn failed_shared_patch_preparation_aborts_its_reservation() {
        let fixture = TestRepository::git_repository(GitRepositoryState::Clean);
        let preparer =
            rah_tools::RepositoryPatchPreparer::new(TestRepository::native_git(), &fixture.0)
                .expect("shared patch preparer should construct");
        let mut coordinator = HostInvocationCoordinator::default();
        coordinator
            .begin_prepare()
            .expect("preparation reservation should begin");
        let result = preparer
            .prepare(super::RepositoryPatchPreparationRequest {
                path: "tracked.txt".to_owned(),
                expected_old_text: "missing".to_owned(),
                replacement_text: "changed".to_owned(),
            })
            .await;
        assert!(result.is_err(), "shared preparation should fail closed");
        coordinator.abort_prepare();
        assert_eq!(coordinator.state(), CoordinatorState::Idle);
        assert_eq!(
            live_patch_temporary_count(&fixture.0)
                .expect("temporary artifacts should be observable"),
            0
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
    async fn deletion_host_prepare_path_is_zero_effect_complete_and_private() {
        let fixture = TestRepository::git_repository(GitRepositoryState::Clean);
        let git = TestRepository::native_git();
        let source = b"RAH_DELETE_SOURCE_SENTINEL\nline-two\n";
        fs::write(fixture.0.join("tracked.txt"), source).expect("source should be replaced");
        assert!(
            Command::new(&git)
                .args(["add", "tracked.txt"])
                .current_dir(&fixture.0)
                .status()
                .expect("Git add should start")
                .success()
        );
        assert!(
            Command::new(&git)
                .args(["commit", "--quiet", "-m", "deletion sentinel"])
                .current_dir(&fixture.0)
                .status()
                .expect("Git commit should start")
                .success()
        );
        fs::write(fixture.0.join("nested/ordinary.txt"), b"reviewed commit\n")
            .expect("reviewed Commit fixture should be written");
        assert!(
            Command::new(&git)
                .args(["add", "nested/ordinary.txt"])
                .current_dir(&fixture.0)
                .status()
                .expect("reviewed Commit fixture should stage")
                .success()
        );
        let repository = fixture.deletion_repository();
        let executions = Arc::new(AtomicUsize::new(0));
        let registry = counting_delete_registry(&repository, Arc::clone(&executions));
        let storage = TestRepository::new();
        let app = tauri::Builder::default()
            .any_thread()
            .manage(DesktopAppState::new(storage.0.clone()))
            .build(tauri::generate_context!())
            .expect("deterministic Desktop app should build");
        replace_selected_repository(app.state::<DesktopAppState>().inner(), repository);
        let selected = app
            .state::<DesktopAppState>()
            .repository
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
            .expect("selected repository should be retained");
        let commit_control = authorize_test_commit(
            app.state::<DesktopAppState>().inner(),
            Arc::clone(&selected),
        )
        .await;
        let before_bytes = fs::read(selected.root.join("tracked.txt")).expect("target should read");
        let before_identity = live_file_identity(&selected.root.join("tracked.txt"))
            .expect("target identity should read");
        let before_index = fs::read(selected.root.join(".git/index")).expect("index should read");
        let before_git = live_git_state(&git, &selected.root, "__rah_no_excluded_branch__")
            .expect("Git baseline should read");
        let host_activity = listen_for_test_event(app.handle(), "host_activity_event");
        let prepared = prepare_real_delete_ticket(
            app.handle(),
            app.state::<DesktopAppState>().inner(),
            Arc::clone(&registry),
        )
        .await;
        let events = wait_for_test_events(&host_activity.0, 1)
            .await
            .expect("Prepare activity should be emitted");
        let activity_id = events[0]
            .get("invocationId")
            .and_then(Value::as_str)
            .expect("Prepare activity should have an activity ID");
        let serialized_activity = serde_json::to_string(&events[0]).expect("activity serializes");
        let source_text = String::from_utf8(source.to_vec()).expect("source sentinel is UTF-8");
        let source_escaped = source_text.replace('\n', "\\n");
        let source_hash = live_sha256(source);
        let source_length = source.len().to_string();
        let native_path = selected.root.join("tracked.txt").display().to_string();
        let raw_input = serde_json::to_string(&serde_json::json!({
            "path": "tracked.txt",
            "expected_file_sha256": source_hash,
            "expected_file_byte_length": source.len()
        }))
        .expect("raw deletion input serializes");
        let identity_marker = format!("{before_identity:?}");
        for forbidden in [
            prepared.ticket_id.as_str(),
            source_text.as_str(),
            source_escaped.as_str(),
            source_hash.as_str(),
            source_length.as_str(),
            native_path.as_str(),
            raw_input.as_str(),
            identity_marker.as_str(),
        ] {
            assert!(
                !serialized_activity.contains(forbidden),
                "Prepared activity leaked sentinel {forbidden}"
            );
        }
        assert_eq!(events[0]["state"], "prepared");
        assert!(events[0].get("review").is_none());
        assert!(events[0].get("result").is_none());
        assert_ne!(prepared.ticket_id, activity_id);
        assert_eq!(prepared.review.operation(), "repo.delete-file");
        assert_eq!(prepared.review.target_count(), 1);
        assert_eq!(prepared.review.path(), "tracked.txt");
        assert_eq!(prepared.review.preimage(), source_escaped);
        assert_eq!(prepared.review.content_sha256(), live_sha256(source));
        assert_eq!(prepared.review.content_byte_length(), source.len());
        assert!(!prepared.review.non_effects().is_empty());
        assert_eq!(executions.load(Ordering::SeqCst), 0);
        assert_eq!(
            fs::read(selected.root.join("tracked.txt")).unwrap(),
            before_bytes
        );
        assert_eq!(
            live_file_identity(&selected.root.join("tracked.txt")).unwrap(),
            before_identity
        );
        assert_eq!(
            fs::read(selected.root.join(".git/index")).unwrap(),
            before_index
        );
        assert_eq!(
            live_git_state(&git, &selected.root, "__rah_no_excluded_branch__").unwrap(),
            before_git
        );
        assert!(commit_control.has_pending_authorization().await);
        assert_eq!(
            app.state::<DesktopAppState>()
                .host_invocation
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .state(),
            CoordinatorState::HostPrepared
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn deletion_hostexplicit_ticket_boundaries_use_real_preparation() {
        let fixture = TestRepository::git_repository(GitRepositoryState::Clean);
        let repository = fixture.deletion_repository();
        let executions = Arc::new(AtomicUsize::new(0));
        let registry = counting_delete_registry(&repository, Arc::clone(&executions));
        let storage = TestRepository::new();
        let app = tauri::Builder::default()
            .any_thread()
            .manage(DesktopAppState::new(storage.0.clone()))
            .build(tauri::generate_context!())
            .expect("deterministic Desktop app should build");
        replace_selected_repository(app.state::<DesktopAppState>().inner(), repository);
        let host_activity = listen_for_test_event(app.handle(), "host_activity_event");
        let (prepared, activity) = prepare_real_delete_ticket_with_activity(
            app.handle(),
            app.state::<DesktopAppState>().inner(),
            Arc::clone(&registry),
            &host_activity.0,
            1,
        )
        .await;
        assert!(
            app.state::<DesktopAppState>()
                .host_invocation
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .take_prepared(&activity, std::time::Instant::now())
                .is_err()
        );
        assert_eq!(
            app.state::<DesktopAppState>()
                .host_invocation
                .lock()
                .unwrap()
                .state(),
            CoordinatorState::Idle
        );
        assert!(fs::read(fixture.0.join("tracked.txt")).is_ok());
        let _ = prepared;

        let (prepared, activity) = prepare_real_delete_ticket_with_activity(
            app.handle(),
            app.state::<DesktopAppState>().inner(),
            Arc::clone(&registry),
            &host_activity.0,
            2,
        )
        .await;
        assert!(
            app.state::<DesktopAppState>()
                .host_invocation
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .cancel(&activity)
                .is_err()
        );
        assert_eq!(
            app.state::<DesktopAppState>()
                .host_invocation
                .lock()
                .unwrap()
                .state(),
            CoordinatorState::HostPrepared
        );
        assert!(
            app.state::<DesktopAppState>()
                .host_invocation
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .cancel(&prepared.ticket_id)
                .is_ok()
        );

        let (prepared, _) = prepare_real_delete_ticket_with_activity(
            app.handle(),
            app.state::<DesktopAppState>().inner(),
            Arc::clone(&registry),
            &host_activity.0,
            3,
        )
        .await;
        assert!(
            app.state::<DesktopAppState>()
                .host_invocation
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .take_prepared(
                    "RAH_WRONG_DELETE_TICKET_SENTINEL",
                    std::time::Instant::now()
                )
                .is_err()
        );
        assert_eq!(
            app.state::<DesktopAppState>()
                .host_invocation
                .lock()
                .unwrap()
                .state(),
            CoordinatorState::Idle
        );
        assert!(fs::read(fixture.0.join("tracked.txt")).is_ok());
        let _ = prepared;

        let (prepared, _) = prepare_real_delete_ticket_with_activity(
            app.handle(),
            app.state::<DesktopAppState>().inner(),
            Arc::clone(&registry),
            &host_activity.0,
            4,
        )
        .await;
        assert!(
            app.state::<DesktopAppState>()
                .host_invocation
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .take_prepared(
                    &prepared.ticket_id,
                    std::time::Instant::now() + Duration::from_secs(300),
                )
                .is_err()
        );
        assert_eq!(
            app.state::<DesktopAppState>()
                .host_invocation
                .lock()
                .unwrap()
                .state(),
            CoordinatorState::Idle
        );

        let (prepared, _) = prepare_real_delete_ticket_with_activity(
            app.handle(),
            app.state::<DesktopAppState>().inner(),
            Arc::clone(&registry),
            &host_activity.0,
            5,
        )
        .await;
        assert!(
            app.state::<DesktopAppState>()
                .host_invocation
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .take_prepared(&prepared.ticket_id, std::time::Instant::now())
                .is_ok()
        );
        app.state::<DesktopAppState>()
            .host_invocation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .finish_host();
        assert!(
            app.state::<DesktopAppState>()
                .host_invocation
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .take_prepared(&prepared.ticket_id, std::time::Instant::now())
                .is_err()
        );
        assert_eq!(executions.load(Ordering::SeqCst), 0);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn deletion_hostexplicit_rejects_stale_before_started_for_git_and_desktop_drift() {
        let fixture = TestRepository::git_repository(GitRepositoryState::Clean);
        fs::write(fixture.0.join("nested/ordinary.txt"), b"reviewed commit\n")
            .expect("reviewed Commit fixture should be written");
        assert!(
            Command::new(TestRepository::native_git())
                .args(["add", "nested/ordinary.txt"])
                .current_dir(&fixture.0)
                .status()
                .expect("reviewed Commit fixture should stage")
                .success()
        );
        let repository = fixture.deletion_repository();
        let executions = Arc::new(AtomicUsize::new(0));
        let registry = counting_delete_registry(&repository, Arc::clone(&executions));
        let storage = TestRepository::new();
        let app = tauri::Builder::default()
            .any_thread()
            .manage(DesktopAppState::new(storage.0.clone()))
            .build(tauri::generate_context!())
            .expect("deterministic Desktop app should build");
        replace_selected_repository(app.state::<DesktopAppState>().inner(), repository);
        let selected = app
            .state::<DesktopAppState>()
            .repository
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
            .expect("selected repository should be retained");
        let commit_control =
            authorize_test_commit(app.state::<DesktopAppState>().inner(), selected).await;
        app.state::<DesktopAppState>()
            .host_invocation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .begin_prepare()
            .expect("success Prepare should reserve the coordinator");
        let current = current_delete_composition(
            app.state::<DesktopAppState>().inner(),
            Arc::clone(&registry),
        );
        let prepared = prepare_repo_delete_file_with_current(
            HostPrepareDeleteFileRequest {
                path: "tracked.txt".to_owned(),
            },
            app.handle(),
            app.state::<DesktopAppState>().inner(),
            current,
        )
        .await
        .expect("success Prepare should succeed");
        let ticket = app
            .state::<DesktopAppState>()
            .host_invocation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take_prepared(&prepared.ticket_id, std::time::Instant::now())
            .expect("ticket should be consumed once");
        let target = fixture.0.join("tracked.txt");
        let bytes = fs::read(&target).expect("target should read");
        let identity = live_file_identity(&target).expect("identity should read");
        fs::remove_file(&target).expect("same-byte replacement should remove target");
        fs::write(&target, &bytes).expect("same-byte replacement should be written");
        assert_ne!(live_file_identity(&target).unwrap(), identity);
        let current = current_delete_composition(
            app.state::<DesktopAppState>().inner(),
            Arc::clone(&registry),
        );
        assert_eq!(
            validate_host_confirmation_ticket(&ticket, &current).await,
            Err(FrontendError::HostInvocationStale)
        );
        app.state::<DesktopAppState>()
            .host_invocation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .finish_host();
        assert!(commit_control.has_pending_authorization().await);
        assert_eq!(executions.load(Ordering::SeqCst), 0);
        assert_eq!(
            app.state::<DesktopAppState>()
                .host_invocation
                .lock()
                .unwrap()
                .state(),
            CoordinatorState::Idle
        );

        let fixture = TestRepository::git_repository(GitRepositoryState::Clean);
        let repository = fixture.deletion_repository();
        let registry = counting_delete_registry(&repository, Arc::clone(&executions));
        let storage = TestRepository::new();
        let app = tauri::Builder::default()
            .any_thread()
            .manage(DesktopAppState::new(storage.0.clone()))
            .build(tauri::generate_context!())
            .expect("second deterministic Desktop app should build");
        replace_selected_repository(app.state::<DesktopAppState>().inner(), repository);
        let prepared = prepare_real_delete_ticket(
            app.handle(),
            app.state::<DesktopAppState>().inner(),
            Arc::clone(&registry),
        )
        .await;
        let target = fixture.0.join("nested/ordinary.txt");
        fs::write(&target, b"index drift\n").expect("index drift should be written");
        assert!(
            Command::new(TestRepository::native_git())
                .args(["add", "nested/ordinary.txt"])
                .current_dir(&fixture.0)
                .status()
                .expect("index drift should stage")
                .success()
        );
        let ticket = app
            .state::<DesktopAppState>()
            .host_invocation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take_prepared(&prepared.ticket_id, std::time::Instant::now())
            .expect("second ticket should be consumed once");
        let current = current_delete_composition(
            app.state::<DesktopAppState>().inner(),
            Arc::clone(&registry),
        );
        assert!(
            validate_host_confirmation_ticket(&ticket, &current)
                .await
                .is_err()
        );
        app.state::<DesktopAppState>()
            .host_invocation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .finish_host();
        assert_eq!(executions.load(Ordering::SeqCst), 0);

        let fixture = TestRepository::git_repository(GitRepositoryState::Clean);
        let repository = fixture.deletion_repository();
        let registry = counting_delete_registry(&repository, Arc::clone(&executions));
        let storage = TestRepository::new();
        let app = tauri::Builder::default()
            .any_thread()
            .manage(DesktopAppState::new(storage.0.clone()))
            .build(tauri::generate_context!())
            .expect("third deterministic Desktop app should build");
        replace_selected_repository(app.state::<DesktopAppState>().inner(), repository);
        let prepared = prepare_real_delete_ticket(
            app.handle(),
            app.state::<DesktopAppState>().inner(),
            Arc::clone(&registry),
        )
        .await;
        let ticket = app
            .state::<DesktopAppState>()
            .host_invocation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take_prepared(&prepared.ticket_id, std::time::Instant::now())
            .expect("third ticket should be consumed once");
        let mut current = current_delete_composition(
            app.state::<DesktopAppState>().inner(),
            Arc::clone(&registry),
        );
        current.generations[1] += 1;
        assert!(
            validate_host_confirmation_ticket(&ticket, &current)
                .await
                .is_err()
        );
        app.state::<DesktopAppState>()
            .host_invocation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .finish_host();
        assert_eq!(executions.load(Ordering::SeqCst), 0);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn deletion_hostexplicit_proof_coupling_requires_independent_postcondition() {
        let output = |status: &str, is_error: bool| ToolOutput {
            content: vec![ToolContent::Json(serde_json::json!({
                "status": status,
                "uncertain": status == "uncertain",
                "path": "tracked.txt"
            }))],
            is_error,
        };

        let fixture = TestRepository::git_repository(GitRepositoryState::Clean);
        let preparer = Arc::new(
            rah_tools::RepositoryDeleteFilePreparer::new(TestRepository::native_git(), &fixture.0)
                .expect("proof preparer should construct"),
        );
        let preparation = Box::new(
            preparer
                .prepare(rah_tools::RepositoryDeleteFilePreparationRequest {
                    path: "tracked.txt".to_owned(),
                })
                .await
                .expect("proof preparation should succeed"),
        );
        fs::remove_file(fixture.0.join("tracked.txt")).expect("verified absence should be created");
        let proof = (preparation, Arc::clone(&preparer));
        assert_eq!(
            classify_repository_delete_file_result(
                &output("deleted_verified", false),
                "tracked.txt",
                Some(&proof),
            )
            .await,
            DeleteFileResultClassification::DeletedVerified
        );
        assert_eq!(
            delete_file_host_terminal_state(DeleteFileResultClassification::DeletedVerified),
            HostActivityState::ToolCompleted
        );

        let fixture = TestRepository::git_repository(GitRepositoryState::Clean);
        let preparer = Arc::new(
            rah_tools::RepositoryDeleteFilePreparer::new(TestRepository::native_git(), &fixture.0)
                .expect("second proof preparer should construct"),
        );
        let preparation = Box::new(
            preparer
                .prepare(rah_tools::RepositoryDeleteFilePreparationRequest {
                    path: "tracked.txt".to_owned(),
                })
                .await
                .expect("second proof preparation should succeed"),
        );
        let proof = (preparation, Arc::clone(&preparer));
        assert_eq!(
            classify_repository_delete_file_result(
                &output("deleted_verified", false),
                "tracked.txt",
                Some(&proof),
            )
            .await,
            DeleteFileResultClassification::Uncertain
        );

        let fixture = TestRepository::git_repository(GitRepositoryState::Clean);
        let preparer = Arc::new(
            rah_tools::RepositoryDeleteFilePreparer::new(TestRepository::native_git(), &fixture.0)
                .expect("third proof preparer should construct"),
        );
        let preparation = Box::new(
            preparer
                .prepare(rah_tools::RepositoryDeleteFilePreparationRequest {
                    path: "tracked.txt".to_owned(),
                })
                .await
                .expect("third proof preparation should succeed"),
        );
        let proof = (preparation, Arc::clone(&preparer));
        assert_eq!(
            classify_repository_delete_file_result(
                &output("known_no_effect", true),
                "tracked.txt",
                Some(&proof),
            )
            .await,
            DeleteFileResultClassification::KnownNoEffect
        );

        let fixture = TestRepository::git_repository(GitRepositoryState::Clean);
        let preparer = Arc::new(
            rah_tools::RepositoryDeleteFilePreparer::new(TestRepository::native_git(), &fixture.0)
                .expect("fourth proof preparer should construct"),
        );
        let preparation = Box::new(
            preparer
                .prepare(rah_tools::RepositoryDeleteFilePreparationRequest {
                    path: "tracked.txt".to_owned(),
                })
                .await
                .expect("fourth proof preparation should succeed"),
        );
        let bytes = fs::read(fixture.0.join("tracked.txt")).expect("replacement bytes should read");
        fs::remove_file(fixture.0.join("tracked.txt")).expect("replacement should remove target");
        fs::write(fixture.0.join("tracked.txt"), bytes)
            .expect("replacement should recreate target");
        let proof = (preparation, Arc::clone(&preparer));
        assert_eq!(
            classify_repository_delete_file_result(
                &output("known_no_effect", true),
                "tracked.txt",
                Some(&proof),
            )
            .await,
            DeleteFileResultClassification::Uncertain
        );

        let fixture = TestRepository::git_repository(GitRepositoryState::Clean);
        let preparer = Arc::new(
            rah_tools::RepositoryDeleteFilePreparer::new(TestRepository::native_git(), &fixture.0)
                .expect("parent replacement preparer should construct"),
        );
        let preparation = Box::new(
            preparer
                .prepare(rah_tools::RepositoryDeleteFilePreparationRequest {
                    path: "nested/ordinary.txt".to_owned(),
                })
                .await
                .expect("parent replacement preparation should succeed"),
        );
        fs::remove_file(fixture.0.join("nested/ordinary.txt"))
            .expect("parent replacement target should be removed");
        fs::rename(fixture.0.join("nested"), fixture.0.join("nested-old"))
            .expect("original parent should be replaced");
        fs::create_dir(fixture.0.join("nested")).expect("replacement parent should be created");
        let parent_replacement_output = ToolOutput {
            content: vec![ToolContent::Json(serde_json::json!({
                "status": "deleted_verified",
                "uncertain": false,
                "path": "nested/ordinary.txt"
            }))],
            is_error: false,
        };
        let proof = (preparation, Arc::clone(&preparer));
        assert_eq!(
            classify_repository_delete_file_result(
                &parent_replacement_output,
                "nested/ordinary.txt",
                Some(&proof),
            )
            .await,
            DeleteFileResultClassification::Uncertain
        );
        assert_eq!(
            safe_delete_file_activity_result(DeleteFileResultClassification::Uncertain).content,
            vec![ToolContent::Json(serde_json::json!({"status":"uncertain"}))]
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn deletion_hostexplicit_success_dispatches_once_and_invalidates_commit_review() {
        let fixture = TestRepository::git_repository(GitRepositoryState::Clean);
        fs::write(fixture.0.join("nested/ordinary.txt"), b"reviewed commit\n")
            .expect("reviewed Commit fixture should be written");
        assert!(
            Command::new(TestRepository::native_git())
                .args(["add", "nested/ordinary.txt"])
                .current_dir(&fixture.0)
                .status()
                .expect("reviewed Commit fixture should stage")
                .success()
        );
        let repository = fixture.deletion_repository();
        let executions = Arc::new(AtomicUsize::new(0));
        let registry = counting_delete_registry(&repository, Arc::clone(&executions));
        let storage = TestRepository::new();
        let app = tauri::Builder::default()
            .any_thread()
            .manage(DesktopAppState::new(storage.0.clone()))
            .build(tauri::generate_context!())
            .expect("deterministic Desktop app should build");
        replace_selected_repository(app.state::<DesktopAppState>().inner(), repository);
        let selected = app
            .state::<DesktopAppState>()
            .repository
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
            .expect("selected repository should be retained");
        let commit_control = authorize_test_commit(
            app.state::<DesktopAppState>().inner(),
            Arc::clone(&selected),
        )
        .await;
        let before = live_git_state(
            &TestRepository::native_git(),
            &fixture.0,
            "__rah_no_excluded_branch__",
        )
        .expect("Git baseline should read");
        let before_index = fs::read(selected.root.join(".git/index")).expect("index should read");
        let host_activity = listen_for_test_event(app.handle(), "host_activity_event");
        app.state::<DesktopAppState>()
            .host_invocation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .begin_prepare()
            .expect("success Prepare should reserve the coordinator");
        let current = current_delete_composition(
            app.state::<DesktopAppState>().inner(),
            Arc::clone(&registry),
        );
        let prepared = prepare_repo_delete_file_with_current(
            HostPrepareDeleteFileRequest {
                path: "tracked.txt".to_owned(),
            },
            app.handle(),
            app.state::<DesktopAppState>().inner(),
            current.clone(),
        )
        .await;
        let prepared = prepared.expect("success Prepare should succeed");
        let _prepare_events = wait_for_test_events(&host_activity.0, 1)
            .await
            .expect("Prepare event should arrive");
        let ticket = app
            .state::<DesktopAppState>()
            .host_invocation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take_prepared(&prepared.ticket_id, std::time::Instant::now())
            .expect("prepared deletion ticket should be consumed");
        validate_host_confirmation_ticket(&ticket, &current)
            .await
            .expect("current deletion ticket should pass production validation");
        assert!(commit_control.has_pending_authorization().await);
        let started_id = ticket.activity_id.clone();
        invalidate_repository_commit_review(app.state::<DesktopAppState>().inner()).await;
        assert!(!commit_control.has_pending_authorization().await);
        emit_host_activity(
            app.handle(),
            HostActivityEvent {
                source: "host_explicit",
                invocation_id: started_id,
                tool: "repo.delete-file".to_owned(),
                state: HostActivityState::Started,
                result: None,
                review: None,
            },
        );
        let PreparedHostInvocation {
            registry,
            expected_definition,
            call,
            allowed_permissions,
            kind,
            activity_id,
            repository_identity,
            generations,
            payload,
            ..
        } = ticket;
        let deletion_proof = match payload {
            PreparedHostPayload::DeleteFile {
                preparation,
                preparer,
            } => Some((preparation, preparer)),
            _ => panic!("ticket should retain deletion proof"),
        };
        run_host_tool(
            app.handle().clone(),
            registry,
            expected_definition,
            allowed_permissions,
            call,
            kind,
            activity_id,
            repository_identity,
            generations,
            None,
            None,
            deletion_proof,
            None,
        )
        .await;
        let events = wait_for_test_events(&host_activity.0, 3)
            .await
            .expect("Started and terminal events should arrive");
        require_host_event(&events[1], "started", "repo.delete-file").expect("Started event");
        require_host_event(&events[2], "tool_completed", "repo.delete-file")
            .expect("terminal event");
        assert_eq!(
            event_tool_output(&events[2]).unwrap().content,
            vec![ToolContent::Json(
                serde_json::json!({"status":"deleted_verified"})
            )]
        );
        assert!(!selected.root.join("tracked.txt").exists());
        let after = live_git_state(
            &TestRepository::native_git(),
            &fixture.0,
            "__rah_no_excluded_branch__",
        )
        .expect("Git terminal state should read");
        assert_eq!(
            fs::read(selected.root.join(".git/index")).unwrap(),
            before_index
        );
        assert_eq!(after.symbolic_head, before.symbolic_head);
        assert_eq!(after.head_oid, before.head_oid);
        assert_eq!(after.current_branch, before.current_branch);
        assert_eq!(after.index_semantics, before.index_semantics);
        assert_eq!(after.raw_staged_diff, before.raw_staged_diff);
        assert_eq!(after.tracking, before.tracking);
        assert_eq!(after.local_heads, before.local_heads);
        assert_eq!(after.tags_and_remotes, before.tags_and_remotes);
        assert_eq!(after.all_refs, before.all_refs);
        assert!(after.status.contains(" D tracked.txt"));
        assert!(!after.status.contains("D  tracked.txt"));
        assert_eq!(executions.load(Ordering::SeqCst), 1);
        assert!(!commit_control.has_pending_authorization().await);
        assert_eq!(
            app.state::<DesktopAppState>()
                .host_invocation
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .state(),
            CoordinatorState::Idle
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn deletion_hostexplicit_post_started_failures_are_uncertain_without_replay() {
        let fixture = TestRepository::git_repository(GitRepositoryState::Clean);
        fs::write(fixture.0.join("nested/ordinary.txt"), b"reviewed commit\n")
            .expect("reviewed Commit fixture should be written");
        assert!(
            Command::new(TestRepository::native_git())
                .args(["add", "nested/ordinary.txt"])
                .current_dir(&fixture.0)
                .status()
                .expect("reviewed Commit fixture should stage")
                .success()
        );
        let repository = fixture.deletion_repository();
        let preparer = Arc::new(
            rah_tools::RepositoryDeleteFilePreparer::new(TestRepository::native_git(), &fixture.0)
                .expect("failure proof preparer should construct"),
        );
        let preparation = Box::new(
            preparer
                .prepare(rah_tools::RepositoryDeleteFilePreparationRequest {
                    path: "tracked.txt".to_owned(),
                })
                .await
                .expect("failure proof preparation should succeed"),
        );
        let expected_definition =
            RepositoryFileDeletionTool::new(TestRepository::native_git(), &fixture.0)
                .expect("failure tool should construct")
                .definition();
        let executions = Arc::new(AtomicUsize::new(0));
        let mut registry = ToolRegistry::new();
        registry
            .register(Arc::new(FailingDeleteFileTool {
                inner: RepositoryFileDeletionTool::from_authority(
                    repository
                        .deletion_authority
                        .clone()
                        .expect("deletion authority should be present"),
                ),
                executions: Arc::clone(&executions),
            }))
            .expect("failing tool should register");
        let registry = Arc::new(registry);
        let storage = TestRepository::new();
        let app = tauri::Builder::default()
            .any_thread()
            .manage(DesktopAppState::new(storage.0.clone()))
            .build(tauri::generate_context!())
            .expect("failure Desktop app should build");
        replace_selected_repository(app.state::<DesktopAppState>().inner(), repository);
        let selected = app
            .state::<DesktopAppState>()
            .repository
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
            .expect("selected repository should be retained");
        let commit_control =
            authorize_test_commit(app.state::<DesktopAppState>().inner(), selected).await;
        assert!(commit_control.has_pending_authorization().await);
        invalidate_repository_commit_review(app.state::<DesktopAppState>().inner()).await;
        assert!(!commit_control.has_pending_authorization().await);
        let host_activity = listen_for_test_event(app.handle(), "host_activity_event");
        let generations = current_host_generation_tuple(app.state::<DesktopAppState>().inner());
        let repository_identity = Some(repository_context_fingerprint(&fixture.0));
        app.state::<DesktopAppState>()
            .host_invocation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .begin_read()
            .expect("post-start failure should own coordinator");
        emit_host_activity(
            app.handle(),
            HostActivityEvent {
                source: "host_explicit",
                invocation_id: "RAH_FAILURE_ACTIVITY_SENTINEL".to_owned(),
                tool: "repo.delete-file".to_owned(),
                state: HostActivityState::Started,
                result: None,
                review: None,
            },
        );
        run_host_tool(
            app.handle().clone(),
            registry,
            expected_definition.clone(),
            vec![
                PermissionLevel::None,
                PermissionLevel::Read,
                PermissionLevel::Execute,
            ],
            host_call(
                ToolName::new("repo.delete-file"),
                ToolInput(serde_json::json!({
                    "path": "tracked.txt",
                    "expected_file_sha256": live_sha256(b"base\n"),
                    "expected_file_byte_length": 5
                })),
            ),
            HostInvocationKind::RepoDeleteFile,
            "RAH_FAILURE_ACTIVITY_SENTINEL".to_owned(),
            repository_identity,
            generations,
            None,
            None,
            Some((preparation, preparer)),
            None,
        )
        .await;
        let events = wait_for_test_events(&host_activity.0, 2)
            .await
            .expect("runtime failure terminal event should arrive");
        require_host_event(&events[1], "possible_effect_unknown", "repo.delete-file")
            .expect("runtime failure should be uncertain");
        assert_eq!(
            event_tool_output(&events[1]).unwrap().content,
            vec![ToolContent::Json(serde_json::json!({"status":"uncertain"}))]
        );
        let serialized = serde_json::to_string(&events[1]).unwrap();
        assert!(!serialized.contains("RAH_RAW_DELETE_RUNTIME_FAILURE_SENTINEL"));
        assert!(fixture.0.join("tracked.txt").exists());
        assert_eq!(executions.load(Ordering::SeqCst), 1);
        assert_eq!(
            app.state::<DesktopAppState>()
                .host_invocation
                .lock()
                .unwrap()
                .state(),
            CoordinatorState::Idle
        );

        let fixture = TestRepository::git_repository(GitRepositoryState::Clean);
        let repository = fixture.deletion_repository();
        let executions = Arc::new(AtomicUsize::new(0));
        let registry = counting_delete_registry(&repository, Arc::clone(&executions));
        let storage = TestRepository::new();
        let app = tauri::Builder::default()
            .any_thread()
            .manage(DesktopAppState::new(storage.0.clone()))
            .build(tauri::generate_context!())
            .expect("rejection Desktop app should build");
        replace_selected_repository(app.state::<DesktopAppState>().inner(), repository);
        let host_activity = listen_for_test_event(app.handle(), "host_activity_event");
        let repository_identity = Some(repository_context_fingerprint(&fixture.0));
        let generations = current_host_generation_tuple(app.state::<DesktopAppState>().inner());
        let mut expected_definition = registry
            .definitions()
            .into_iter()
            .find(|definition| definition.name.as_str() == "repo.delete-file")
            .expect("rejection definition should exist");
        expected_definition.description = "RAH_REJECTED_DEFINITION_SENTINEL".to_owned();
        app.state::<DesktopAppState>()
            .host_invocation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .begin_read()
            .expect("rejected dispatch should own coordinator");
        emit_host_activity(
            app.handle(),
            HostActivityEvent {
                source: "host_explicit",
                invocation_id: "RAH_REJECTED_ACTIVITY".to_owned(),
                tool: "repo.delete-file".to_owned(),
                state: HostActivityState::Started,
                result: None,
                review: None,
            },
        );
        run_host_tool(
            app.handle().clone(),
            registry,
            expected_definition,
            vec![
                PermissionLevel::None,
                PermissionLevel::Read,
                PermissionLevel::Execute,
            ],
            host_call(
                ToolName::new("repo.delete-file"),
                ToolInput(serde_json::json!({
                    "path": "tracked.txt",
                    "expected_file_sha256": live_sha256(b"base\n"),
                    "expected_file_byte_length": 5
                })),
            ),
            HostInvocationKind::RepoDeleteFile,
            "RAH_REJECTED_ACTIVITY".to_owned(),
            repository_identity,
            generations,
            None,
            None,
            None,
            None,
        )
        .await;
        let events = wait_for_test_events(&host_activity.0, 2)
            .await
            .expect("rejected dispatch terminal event should arrive");
        require_host_event(&events[1], "possible_effect_unknown", "repo.delete-file")
            .expect("rejected dispatch should be uncertain after Started");
        assert_eq!(
            event_tool_output(&events[1]).unwrap().content,
            vec![ToolContent::Json(serde_json::json!({"status":"uncertain"}))]
        );
        assert_eq!(executions.load(Ordering::SeqCst), 0);
        assert!(fixture.0.join("tracked.txt").exists());
        assert_eq!(
            app.state::<DesktopAppState>()
                .host_invocation
                .lock()
                .unwrap()
                .state(),
            CoordinatorState::Idle
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
    async fn desktop_rename_file_uses_host_authority_and_preserves_repository_state() {
        let fixture = TestRepository::git_repository(GitRepositoryState::Clean);
        let without_authority = fixture.desktop_repository();
        let registry = desktop_tool_registry(Some(&without_authority), None)
            .expect("registry without rename authority should build");
        assert!(
            registry
                .definitions()
                .iter()
                .all(|definition| definition.name.as_str() != "repo.rename-file")
        );

        let repository = fixture.rename_repository();
        let registry = desktop_tool_registry(Some(&repository), None)
            .expect("registry with rename authority should build");
        let definition = registry
            .definitions()
            .into_iter()
            .find(|definition| definition.name.as_str() == "repo.rename-file")
            .expect("host authority exposes rename tool");
        assert_eq!(definition.permission, PermissionLevel::Execute);
        assert_eq!(
            definition.input_schema,
            serde_json::json!({
                "type": "object",
                "properties": {
                    "source_path": {"type": "string", "minLength": 1, "maxLength": 1024},
                    "destination_path": {"type": "string", "minLength": 1, "maxLength": 1024},
                    "expected_source_file_sha256": {"type": "string", "pattern": "^[0-9a-f]{64}$"},
                    "expected_source_file_byte_length": {"type": "integer", "minimum": 0, "maximum": 1024 * 1024}
                },
                "required": ["source_path", "destination_path", "expected_source_file_sha256", "expected_source_file_byte_length"],
                "additionalProperties": false
            })
        );

        let source = fixture.0.join("tracked.txt");
        let bytes = fs::read(&source).expect("source should be readable");
        let index_before = fs::read(fixture.0.join(".git/index")).expect("index should read");
        let head_before = Command::new(TestRepository::native_git())
            .args(["rev-parse", "HEAD"])
            .current_dir(&fixture.0)
            .output()
            .expect("HEAD should read")
            .stdout;
        let output = registry
            .execute(
                ToolCall {
                    id: ToolCallId::new(),
                    name: ToolName::new("repo.rename-file"),
                    input: ToolInput(serde_json::json!({
                        "source_path": "tracked.txt",
                        "destination_path": "nested/moved.txt",
                        "expected_source_file_sha256": format!("{:x}", Sha256::digest(&bytes)),
                        "expected_source_file_byte_length": bytes.len()
                    })),
                },
                ToolContext::default(),
            )
            .await
            .expect("rename dispatch should return a bounded result");
        assert!(!output.is_error);
        assert!(matches!(&output.content[0], ToolContent::Json(value)
            if value["status"] == "renamed_verified"));
        assert!(!source.exists());
        assert_eq!(fs::read(fixture.0.join("nested/moved.txt")).unwrap(), bytes);
        assert_eq!(
            fs::read(fixture.0.join(".git/index")).unwrap(),
            index_before
        );
        assert_eq!(
            Command::new(TestRepository::native_git())
                .args(["rev-parse", "HEAD"])
                .current_dir(&fixture.0)
                .output()
                .unwrap()
                .stdout,
            head_before
        );
        let status = String::from_utf8(
            Command::new(TestRepository::native_git())
                .args(["status", "--porcelain=v1"])
                .current_dir(&fixture.0)
                .output()
                .unwrap()
                .stdout,
        )
        .unwrap();
        assert!(status.contains(" D tracked.txt"));
        assert!(status.contains("?? nested/moved.txt"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn desktop_rename_file_refreshes_same_directory_activity_without_frontend_authority() {
        let fixture = TestRepository::git_repository(GitRepositoryState::Clean);
        let repository = fixture.rename_repository();
        let registry = desktop_tool_registry(Some(&repository), None).unwrap();
        let bytes = fs::read(fixture.0.join("tracked.txt")).unwrap();
        let output = registry
            .execute(
                ToolCall {
                    id: ToolCallId::new(),
                    name: ToolName::new("repo.rename-file"),
                    input: ToolInput(serde_json::json!({
                        "source_path": "tracked.txt",
                        "destination_path": "renamed.txt",
                        "expected_source_file_sha256": format!("{:x}", Sha256::digest(&bytes)),
                        "expected_source_file_byte_length": bytes.len()
                    })),
                },
                ToolContext::default(),
            )
            .await
            .unwrap();
        assert!(matches!(&output.content[0], ToolContent::Json(value)
            if value["status"] == "renamed_verified"));

        let id = ToolCallId::new();
        let requested = AgentEvent::ToolRequested {
            session_id: SessionId::new(),
            tool_call: ToolCall {
                id: id.clone(),
                name: ToolName::new("repo.rename-file"),
                input: ToolInput(serde_json::json!({})),
            },
        };
        let finished = AgentEvent::ToolFinished {
            session_id: SessionId::new(),
            tool_call_id: id,
            output,
        };
        let mut calls = HashMap::new();
        assert!(activity_event(&requested, &mut calls).is_some());
        let (event, refresh) = activity_event(&finished, &mut calls).unwrap();
        assert!(refresh);
        assert_eq!(
            serde_json::to_string(&event).unwrap(),
            r#"{"kind":"tool_finished","tool":"repo.rename-file","result":"success"}"#
        );
        assert!(!fixture.0.join("tracked.txt").exists());
        assert_eq!(fs::read(fixture.0.join("renamed.txt")).unwrap(), bytes);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn desktop_rename_file_rejects_stale_source_and_destination_collision_without_retry() {
        let fixture = TestRepository::git_repository(GitRepositoryState::Clean);
        let repository = fixture.rename_repository();
        let registry = desktop_tool_registry(Some(&repository), None).unwrap();
        let bytes = fs::read(fixture.0.join("tracked.txt")).unwrap();
        let request = |destination: &str| ToolCall {
            id: ToolCallId::new(),
            name: ToolName::new("repo.rename-file"),
            input: ToolInput(serde_json::json!({
            "source_path": "tracked.txt",
            "destination_path": destination,
            "expected_source_file_sha256": format!("{:x}", Sha256::digest(&bytes)),
            "expected_source_file_byte_length": bytes.len()
            })),
        };
        let case_only = registry
            .execute(request("TRACKED.TXT"), ToolContext::default())
            .await
            .unwrap();
        assert!(case_only.is_error);
        assert!(matches!(&case_only.content[0], ToolContent::Json(value)
            if value["status"] == "precondition_failed"));
        assert_eq!(fs::read(fixture.0.join("tracked.txt")).unwrap(), bytes);

        fs::write(fixture.0.join("tracked.txt"), b"newer source\n").unwrap();
        let stale = registry
            .execute(request("nested/stale.txt"), ToolContext::default())
            .await
            .unwrap();
        assert!(stale.is_error);
        assert!(matches!(&stale.content[0], ToolContent::Json(value)
            if value["status"] == "precondition_failed"));
        assert_eq!(
            fs::read(fixture.0.join("tracked.txt")).unwrap(),
            b"newer source\n"
        );
        assert!(!fixture.0.join("nested/stale.txt").exists());

        fs::write(fixture.0.join("tracked.txt"), &bytes).unwrap();
        fs::write(fixture.0.join("nested/collision.txt"), b"protected\n").unwrap();
        let collision = registry
            .execute(request("nested/collision.txt"), ToolContext::default())
            .await
            .unwrap();
        assert!(collision.is_error);
        assert!(matches!(&collision.content[0], ToolContent::Json(value)
            if value["status"] == "precondition_failed"));
        assert_eq!(fs::read(fixture.0.join("tracked.txt")).unwrap(), bytes);
        assert_eq!(
            fs::read(fixture.0.join("nested/collision.txt")).unwrap(),
            b"protected\n"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn desktop_successful_rename_revokes_reviewed_commit_authorization() {
        let fixture = TestRepository::git_repository(GitRepositoryState::Clean);
        let git = TestRepository::native_git();
        fs::write(fixture.0.join("nested/ordinary.txt"), b"reviewed\n").unwrap();
        assert!(
            Command::new(&git)
                .args(["add", "nested/ordinary.txt"])
                .current_dir(&fixture.0)
                .status()
                .unwrap()
                .success()
        );
        let repository = fixture.rename_repository();
        let (commit_tool, control) = RepositoryCommitTool::compose(
            &repository.git_executable,
            &repository.root,
            "RAH Host".to_owned(),
            "rah-host@example.invalid".to_owned(),
        )
        .unwrap();
        let control = Arc::new(control);
        let storage = TestRepository::new();
        let state = DesktopAppState::new(storage.0.clone());
        *state.commit_identity.lock().unwrap() = Some(DesktopCommitIdentity {
            name: "RAH Host".to_owned(),
            email: "rah-host@example.invalid".to_owned(),
        });
        replace_selected_repository(&state, repository);
        let selected = state.repository.lock().unwrap().clone().unwrap();
        let repository_generation = *state.repository_generation.lock().unwrap();
        let model_generation = state.model.lock().unwrap().generation;
        let identity_generation = *state.commit_identity_generation.lock().unwrap();
        *state.commit_capability.lock().unwrap() = Some(DesktopCommitCapability {
            repository_generation,
            model_generation,
            identity_generation,
            _tool: Arc::new(commit_tool),
            control: Arc::clone(&control),
        });
        let (snapshot, review) =
            desktop_repository_snapshot_with_review(&selected, Some(Arc::clone(&control)))
                .await
                .unwrap();
        let snapshot = install_repository_workflow(
            &state,
            &selected,
            repository_generation,
            snapshot,
            review,
            identity_generation,
        );
        let StagedReviewPresentation::ReviewAvailable {
            review_id: Some(review_id),
            ..
        } = snapshot.review
        else {
            panic!("staged review should be available");
        };
        authorize_repository_commit_review(&state, &review_id)
            .await
            .unwrap();
        assert!(control.has_pending_authorization().await);

        let bytes = fs::read(fixture.0.join("tracked.txt")).unwrap();
        let registry = desktop_tool_registry(Some(&selected), None).unwrap();
        let output = registry
            .execute(
                ToolCall {
                    id: ToolCallId::new(),
                    name: ToolName::new("repo.rename-file"),
                    input: ToolInput(serde_json::json!({
                        "source_path": "tracked.txt",
                        "destination_path": "renamed.txt",
                        "expected_source_file_sha256": format!("{:x}", Sha256::digest(&bytes)),
                        "expected_source_file_byte_length": bytes.len()
                    })),
                },
                ToolContext::default(),
            )
            .await
            .unwrap();
        assert!(matches!(&output.content[0], ToolContent::Json(value)
            if value["status"] == "renamed_verified"));
        invalidate_repository_commit_review(&state).await;
        assert!(!control.has_pending_authorization().await);
        assert!(matches!(
            authorize_repository_commit_review(&state, &review_id).await,
            Err(FrontendError::CommitAuthorizationStale)
        ));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn desktop_verified_directory_creation_revokes_reviewed_commit_authorization() {
        let fixture = TestRepository::git_repository(GitRepositoryState::Clean);
        let git = TestRepository::native_git();
        fs::write(fixture.0.join("reviewed.txt"), b"reviewed\n")
            .expect("review fixture should be written");
        assert!(
            Command::new(&git)
                .args(["add", "reviewed.txt"])
                .current_dir(&fixture.0)
                .status()
                .expect("Git add should start")
                .success()
        );
        fs::create_dir(fixture.0.join("parent")).expect("directory parent should exist");
        let repository = fixture.directory_repository();
        let (commit_tool, control) = RepositoryCommitTool::compose(
            &repository.git_executable,
            &repository.root,
            "RAH Host".to_owned(),
            "rah-host@example.invalid".to_owned(),
        )
        .expect("commit capability should compose");
        let control = Arc::new(control);
        let storage = TestRepository::new();
        let state = DesktopAppState::new(storage.0.clone());
        *state.commit_identity.lock().unwrap() = Some(DesktopCommitIdentity {
            name: "RAH Host".to_owned(),
            email: "rah-host@example.invalid".to_owned(),
        });
        replace_selected_repository(&state, repository);
        let selected = state.repository.lock().unwrap().clone().unwrap();
        let repository_generation = *state.repository_generation.lock().unwrap();
        let model_generation = state.model.lock().unwrap().generation;
        let identity_generation = *state.commit_identity_generation.lock().unwrap();
        *state.commit_capability.lock().unwrap() = Some(DesktopCommitCapability {
            repository_generation,
            model_generation,
            identity_generation,
            _tool: Arc::new(commit_tool),
            control: Arc::clone(&control),
        });
        let (snapshot, review) =
            desktop_repository_snapshot_with_review(&selected, Some(Arc::clone(&control)))
                .await
                .expect("review snapshot should be available");
        let snapshot = install_repository_workflow(
            &state,
            &selected,
            repository_generation,
            snapshot,
            review,
            identity_generation,
        );
        let StagedReviewPresentation::ReviewAvailable {
            review_id: Some(review_id),
            ..
        } = snapshot.review
        else {
            panic!("staged review should be available");
        };
        authorize_repository_commit_review(&state, &review_id)
            .await
            .expect("review should authorize");
        assert!(control.has_pending_authorization().await);

        let registry = desktop_tool_registry(Some(&selected), None).unwrap();
        let output = registry
            .execute(
                ToolCall {
                    id: ToolCallId::new(),
                    name: ToolName::new("repo.create-directory"),
                    input: ToolInput(serde_json::json!({"path":"parent/empty"})),
                },
                ToolContext::default(),
            )
            .await
            .expect("directory creation should return a result");
        assert!(matches!(&output.content[0], ToolContent::Json(value)
            if value["status"] == "directory_created_verified"));

        let id = ToolCallId::new();
        let requested = AgentEvent::ToolRequested {
            session_id: SessionId::new(),
            tool_call: ToolCall {
                id: id.clone(),
                name: ToolName::new("repo.create-directory"),
                input: ToolInput(serde_json::json!({"path":"parent/empty"})),
            },
        };
        let finished = AgentEvent::ToolFinished {
            session_id: SessionId::new(),
            tool_call_id: id,
            output,
        };
        let mut calls = HashMap::new();
        activity_event(&requested, &mut calls).expect("requested activity");
        let (_, refresh) = activity_event(&finished, &mut calls).expect("finished activity");
        assert!(refresh);
        invalidate_repository_commit_review(&state).await;
        assert!(!control.has_pending_authorization().await);
        assert!(matches!(
            authorize_repository_commit_review(&state, &review_id).await,
            Err(FrontendError::CommitAuthorizationStale)
        ));
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
    fn stale_generation_tuple_cannot_start_a_chat_turn() {
        assert!(super::connection_activation_publication_is_current(
            [4, 8, 6, 10],
            [4, 8, 6, 10],
        ));
        assert!(!super::connection_activation_publication_is_current(
            [4, 8, 6, 10],
            [5, 8, 6, 10],
        ));
        assert!(!super::connection_activation_publication_is_current(
            [4, 8, 6, 10],
            [4, 9, 6, 10],
        ));
        assert!(!super::connection_activation_publication_is_current(
            [4, 8, 6, 10],
            [4, 8, 7, 10],
        ));
        assert!(!super::connection_activation_publication_is_current(
            [4, 8, 6, 10],
            [4, 8, 6, 11],
        ));
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

    #[tokio::test(flavor = "current_thread")]
    async fn desktop_directory_creation_is_explicit_and_git_clean() {
        let fixture = TestRepository::git_repository(GitRepositoryState::Clean);
        fs::create_dir(fixture.0.join("parent")).expect("directory parent should exist");
        let repository = fixture.directory_repository();
        let registry = desktop_tool_registry(Some(&repository), None)
            .expect("directory-authorized registry should build");
        assert!(
            registry
                .definitions()
                .iter()
                .any(|definition| definition.name.as_str() == "repo.create-directory")
        );

        let output = registry
            .execute(
                ToolCall {
                    id: ToolCallId::new(),
                    name: ToolName::new("repo.create-directory"),
                    input: ToolInput(serde_json::json!({"path":"parent/empty"})),
                },
                ToolContext::default(),
            )
            .await
            .expect("directory dispatch should return a result");
        assert!(!output.is_error);
        assert!(matches!(&output.content[0], ToolContent::Json(value)
            if value["status"] == "directory_created_verified"
                && value["uncertain"] == false));
        assert!(fixture.0.join("parent/empty").is_dir());
        assert_eq!(
            String::from_utf8(
                Command::new(TestRepository::native_git())
                    .args(["status", "--porcelain=v1"])
                    .current_dir(&fixture.0)
                    .output()
                    .expect("Git status should run")
                    .stdout,
            )
            .expect("Git status should be UTF-8"),
            ""
        );

        let id = ToolCallId::new();
        let requested = AgentEvent::ToolRequested {
            session_id: SessionId::new(),
            tool_call: ToolCall {
                id: id.clone(),
                name: ToolName::new("repo.create-directory"),
                input: ToolInput(serde_json::json!({"path":"parent/empty"})),
            },
        };
        let finished = AgentEvent::ToolFinished {
            session_id: SessionId::new(),
            tool_call_id: id,
            output,
        };
        let mut calls = HashMap::new();
        assert!(activity_event(&requested, &mut calls).is_some());
        let (_, refresh) = activity_event(&finished, &mut calls).expect("finished activity");
        assert!(refresh);
    }

    #[test]
    fn desktop_directory_authority_is_not_inferred_from_other_repository_tools() {
        let fixture = TestRepository::new();
        let repository = fixture.desktop_repository();
        let registry = desktop_tool_registry(Some(&repository), None)
            .expect("authority-free registry should build");
        assert!(
            !registry
                .definitions()
                .iter()
                .any(|definition| definition.name.as_str() == "repo.create-directory")
        );
    }

    #[test]
    fn directory_authority_snapshot_changes_with_repository_generation() {
        let repository_a_fixture = TestRepository::git_repository(GitRepositoryState::Clean);
        let repository_b_fixture = TestRepository::git_repository(GitRepositoryState::Clean);
        let repository_a = repository_a_fixture.directory_repository();
        let repository_b = repository_b_fixture.directory_repository();
        let storage = TestRepository::new();
        let state = DesktopAppState::new(storage.0.clone());

        replace_selected_repository(&state, repository_a);
        let selected_a = state.repository.lock().unwrap().clone().unwrap();
        assert_eq!(*state.repository_generation.lock().unwrap(), 1);
        assert!(
            selected_a
                .directory_creation_authority
                .as_ref()
                .is_some_and(|authority| authority.matches_repository_root(&selected_a.root))
        );

        replace_selected_repository(&state, repository_b);
        let selected_b = state.repository.lock().unwrap().clone().unwrap();
        assert_eq!(*state.repository_generation.lock().unwrap(), 2);
        assert!(
            selected_b
                .directory_creation_authority
                .as_ref()
                .is_some_and(|authority| authority.matches_repository_root(&selected_b.root))
        );
        assert!(
            !selected_a
                .directory_creation_authority
                .as_ref()
                .is_some_and(|authority| authority.matches_repository_root(&selected_b.root))
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
    fn patch_terminal_matrix_is_strict_and_conservative() {
        let cases = [
            (
                serde_json::json!({
                    "status": "precondition_failed",
                    "changed": false,
                    "uncertain": false,
                    "reason": "precondition",
                }),
                true,
                RepositoryPatchResultClassification::PreconditionFailed,
                HostActivityState::ToolError,
            ),
            (
                serde_json::json!({
                    "status": "ok",
                    "changed": true,
                    "uncertain": false,
                    "reason": "none",
                }),
                false,
                RepositoryPatchResultClassification::ChangedVerified,
                HostActivityState::ToolCompleted,
            ),
            (
                serde_json::json!({
                    "status": "replacement_failed_known",
                    "changed": false,
                    "uncertain": false,
                    "reason": "replacement",
                }),
                true,
                RepositoryPatchResultClassification::ReplacementFailedKnown,
                HostActivityState::ToolError,
            ),
            (
                serde_json::json!({
                    "status": "uncertain",
                    "changed": false,
                    "uncertain": true,
                    "reason": "temporary",
                }),
                true,
                RepositoryPatchResultClassification::Uncertain,
                HostActivityState::PossibleEffectUnknown,
            ),
            (
                serde_json::json!({
                    "status": "ok",
                    "changed": true,
                    "uncertain": false,
                    "reason": "none",
                    "source": "must be rejected",
                }),
                true,
                RepositoryPatchResultClassification::Malformed,
                HostActivityState::PossibleEffectUnknown,
            ),
        ];
        for (value, is_error, expected_classification, expected_state) in cases {
            let output = ToolOutput {
                content: vec![ToolContent::Json(value)],
                is_error,
            };
            let classification = classify_repository_patch_output(&output);
            assert_eq!(classification, expected_classification);
            assert_eq!(patch_host_terminal_state(classification), expected_state);
            assert_eq!(
                classification == RepositoryPatchResultClassification::Malformed,
                expected_state == HostActivityState::PossibleEffectUnknown
                    && expected_classification == RepositoryPatchResultClassification::Malformed
            );
        }
    }

    #[test]
    fn patch_host_activity_does_not_serialize_review_source_content() {
        let event = HostActivityEvent {
            source: "host_explicit",
            invocation_id: "host-explicit-1".to_owned(),
            tool: "repo.patch".to_owned(),
            state: HostActivityState::Started,
            result: None,
            review: None,
        };
        let serialized = serde_json::to_string(&event).expect("host activity serializes");
        assert!(!serialized.contains("oldTextEscaped"));
        assert!(!serialized.contains("replacementTextEscaped"));
        assert!(!serialized.contains("review"));
    }

    #[test]
    fn prepared_host_activity_uses_separate_value_level_correlation_for_all_prepared_tools() {
        const TICKET: &str = "RAH_SECRET_AUTHORITY_TICKET_SENTINEL";
        let cases = [
            (
                "repo.create-branch",
                Some(HostInvocationReview::Branch(BranchReview {
                    operation: "Create local branch",
                    branch: "safe-topic".to_owned(),
                    target: "Current committed HEAD",
                    effect: "Creates one new local branch reference.",
                    non_effect: "Does not switch branches or modify HEAD/index/worktree.",
                    permission_category: "execute",
                    authority_category: "repository_local_branch_creation",
                })),
            ),
            ("repo.patch", None),
            ("repo.edit-files", None),
            ("repo.create-file", None),
        ];
        for (tool, review) in cases {
            let activity_id = format!("host-explicit-activity-{tool}");
            assert_ne!(activity_id, TICKET);
            let event = prepared_host_activity(activity_id.clone(), tool.to_owned(), review);
            let serialized = serde_json::to_string(&event).expect("prepared activity serializes");
            assert!(serialized.contains(&activity_id));
            assert!(!serialized.contains(TICKET));
            assert!(!serialized.contains("ticketId"));
        }
    }

    #[test]
    fn branch_activity_preserves_review_for_safe_results_and_refreshes_uncertain_results() {
        let safe_outputs = [
            ToolOutput {
                content: vec![ToolContent::Json(
                    serde_json::json!({"status": "invalid_input", "uncertain": false}),
                )],
                is_error: true,
            },
            ToolOutput {
                content: vec![ToolContent::Json(
                    serde_json::json!({"status": "precondition_failed", "uncertain": false}),
                )],
                is_error: true,
            },
            ToolOutput {
                content: vec![ToolContent::Json(
                    serde_json::json!({"status": "known_no_effect", "uncertain": false}),
                )],
                is_error: true,
            },
            ToolOutput {
                content: vec![ToolContent::Json(serde_json::json!({
                    "status": "branch_created_verified",
                    "uncertain": false,
                    "name": "branch",
                    "oid": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                }))],
                is_error: false,
            },
            ToolOutput {
                content: vec![ToolContent::Json(serde_json::json!({
                    "status": "desired_state_observed_after_uncertain_attempt",
                    "uncertain": true,
                    "name": "branch",
                    "oid": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                }))],
                is_error: true,
            },
        ];
        for output in safe_outputs {
            let id = ToolCallId::new();
            let requested = AgentEvent::ToolRequested {
                session_id: SessionId::new(),
                tool_call: ToolCall {
                    id: id.clone(),
                    name: ToolName::new(REPOSITORY_CREATE_BRANCH_TOOL_NAME),
                    input: ToolInput(serde_json::json!({"name": "branch"})),
                },
            };
            let finished = AgentEvent::ToolFinished {
                session_id: SessionId::new(),
                tool_call_id: id,
                output,
            };
            let mut calls = HashMap::new();
            activity_event_with_composition(
                &requested,
                &mut calls,
                &empty_composition_metadata(),
                true,
            )
            .expect("branch request activity");
            let outcome = activity_event_with_composition(
                &finished,
                &mut calls,
                &empty_composition_metadata(),
                true,
            )
            .expect("branch completion activity");
            assert!(!outcome.invalidate_review);
            assert_eq!(outcome.refresh_reason, None);
        }

        let assert_uncertain = |output: ToolOutput| {
            let id = ToolCallId::new();
            let requested = AgentEvent::ToolRequested {
                session_id: SessionId::new(),
                tool_call: ToolCall {
                    id: id.clone(),
                    name: ToolName::new(REPOSITORY_CREATE_BRANCH_TOOL_NAME),
                    input: ToolInput(serde_json::json!({"name": "branch"})),
                },
            };
            let finished = AgentEvent::ToolFinished {
                session_id: SessionId::new(),
                tool_call_id: id,
                output,
            };
            let mut calls = HashMap::new();
            activity_event_with_composition(
                &requested,
                &mut calls,
                &empty_composition_metadata(),
                true,
            )
            .expect("branch request activity");
            let outcome = activity_event_with_composition(
                &finished,
                &mut calls,
                &empty_composition_metadata(),
                true,
            )
            .expect("branch completion activity");
            assert!(outcome.invalidate_review);
            assert_eq!(
                outcome.refresh_reason,
                Some(RepositoryRefreshReason::FirstPartyMutation)
            );
        };

        for output in [
            ToolOutput {
                content: vec![ToolContent::Json(serde_json::json!({
                    "status": "branch_created_verified",
                    "uncertain": false,
                    "name": "branch",
                    "oid": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                }))],
                is_error: true,
            },
            ToolOutput {
                content: vec![ToolContent::Json(serde_json::json!({
                    "status": "branch_created_verified",
                    "uncertain": true,
                    "name": "branch",
                    "oid": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                }))],
                is_error: false,
            },
            ToolOutput {
                content: vec![ToolContent::Json(serde_json::json!({
                    "status": "branch_created_verified",
                    "uncertain": false,
                    "oid": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                }))],
                is_error: false,
            },
            ToolOutput {
                content: vec![ToolContent::Json(serde_json::json!({
                    "status": "branch_created_verified",
                    "uncertain": false,
                    "name": "branch",
                }))],
                is_error: false,
            },
            ToolOutput {
                content: vec![ToolContent::Json(serde_json::json!({
                    "status": "branch_created_verified",
                    "uncertain": false,
                    "name": "branch",
                    "oid": "zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz",
                }))],
                is_error: false,
            },
            ToolOutput {
                content: vec![ToolContent::Json(serde_json::json!({
                    "status": "desired_state_observed_after_uncertain_attempt",
                    "uncertain": false,
                    "name": "branch",
                    "oid": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                }))],
                is_error: true,
            },
            ToolOutput {
                content: vec![ToolContent::Json(serde_json::json!({
                    "status": "desired_state_observed_after_uncertain_attempt",
                    "uncertain": true,
                    "name": "branch",
                    "oid": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                }))],
                is_error: false,
            },
            ToolOutput {
                content: vec![ToolContent::Json(
                    serde_json::json!({"status": "invalid_input", "uncertain": true}),
                )],
                is_error: true,
            },
            ToolOutput {
                content: vec![ToolContent::Json(
                    serde_json::json!({"status": "known_no_effect", "uncertain": false}),
                )],
                is_error: false,
            },
            ToolOutput {
                content: vec![ToolContent::Json(serde_json::json!({
                    "status": "invalid_input",
                    "uncertain": false,
                    "extra": true,
                }))],
                is_error: true,
            },
            ToolOutput {
                content: vec![ToolContent::Json(serde_json::json!({
                    "status": "known_no_effect",
                }))],
                is_error: true,
            },
            ToolOutput {
                content: vec![ToolContent::Json(
                    serde_json::json!({"status": "future_status", "uncertain": false}),
                )],
                is_error: true,
            },
            ToolOutput {
                content: vec![ToolContent::Json(
                    serde_json::json!({"status": "uncertain"}),
                )],
                is_error: true,
            },
            ToolOutput {
                content: vec![ToolContent::Text("not branch JSON".to_owned())],
                is_error: true,
            },
            ToolOutput {
                content: vec![
                    ToolContent::Json(serde_json::json!({
                        "status": "invalid_input",
                        "uncertain": false,
                    })),
                    ToolContent::Json(serde_json::json!({
                        "status": "invalid_input",
                        "uncertain": false,
                    })),
                ],
                is_error: true,
            },
        ] {
            assert_uncertain(output);
        }
    }

    fn external_activity_composition(source_kind: SourceKind) -> DesktopToolComposition {
        let public_tool_name = match source_kind {
            SourceKind::Mcp => "mcp.host.echo",
            SourceKind::ProcessPlugin => "plugin.host.echo",
            _ => panic!("external activity fixture requires an external source"),
        };
        DesktopToolComposition {
            registry: Arc::new(rah_tools::ToolRegistry::new()),
            expected_definitions: Vec::new(),
            tools: vec![EffectiveToolEntry {
                public_tool_name: public_tool_name.to_owned(),
                source_kind,
                source_label: "host".to_owned(),
                effect_class: EffectClass::External,
                authority_category: super::effective_authority::AuthorityCategory::External,
                permission: PermissionLevel::Read,
                repository_bound: false,
                advertised: true,
                host_invocation: HostInvocationDescriptor {
                    eligible: false,
                    kind: None,
                    unavailable_reason: Some(HostInvocationUnavailableReason::ProviderNotSupported),
                },
            }],
            unavailable: Vec::new(),
            repository_patch_preparer: None,
            repository_multi_file_edit_preparer: None,
            repository_create_file_preparer: None,
            repository_delete_file_preparer: None,
            repository_rename_file_preparer: None,
        }
    }

    #[test]
    fn external_requested_without_start_has_no_repository_effect() {
        let composition = external_activity_composition(SourceKind::Mcp);
        let id = ToolCallId::new();
        let requested = AgentEvent::ToolRequested {
            session_id: SessionId::new(),
            tool_call: ToolCall {
                id: id.clone(),
                name: ToolName::new("mcp.host.echo"),
                input: ToolInput(serde_json::json!({})),
            },
        };
        let mut calls = HashMap::new();
        let outcome = activity_event_with_composition(&requested, &mut calls, &composition, true)
            .expect("requested activity");
        assert!(!outcome.invalidate_review);
        assert_eq!(outcome.refresh_reason, None);
    }

    #[test]
    fn branch_requested_without_start_has_no_repository_effect() {
        let id = ToolCallId::new();
        let requested = AgentEvent::ToolRequested {
            session_id: SessionId::new(),
            tool_call: ToolCall {
                id: id.clone(),
                name: ToolName::new(REPOSITORY_CREATE_BRANCH_TOOL_NAME),
                input: ToolInput(serde_json::json!({"name": "branch"})),
            },
        };
        let mut calls = HashMap::new();
        let outcome = activity_event_with_composition(
            &requested,
            &mut calls,
            &empty_composition_metadata(),
            true,
        )
        .expect("requested activity");
        assert!(!outcome.invalidate_review);
        assert_eq!(outcome.refresh_reason, None);
        assert!(!uncertain_repository_effect_pending(&calls));
    }

    #[test]
    fn started_branch_without_finish_is_an_uncertain_repository_effect() {
        let id = ToolCallId::new();
        let requested = AgentEvent::ToolRequested {
            session_id: SessionId::new(),
            tool_call: ToolCall {
                id: id.clone(),
                name: ToolName::new(REPOSITORY_CREATE_BRANCH_TOOL_NAME),
                input: ToolInput(serde_json::json!({"name": "branch"})),
            },
        };
        let started = AgentEvent::ToolStarted {
            session_id: SessionId::new(),
            tool_call_id: id,
        };
        let mut calls = HashMap::new();
        activity_event_with_composition(
            &requested,
            &mut calls,
            &empty_composition_metadata(),
            true,
        )
        .expect("requested activity");
        let started = activity_event_with_composition(
            &started,
            &mut calls,
            &empty_composition_metadata(),
            true,
        )
        .expect("started activity");
        assert!(!started.invalidate_review);
        assert_eq!(started.refresh_reason, None);
        assert!(uncertain_repository_effect_pending(&calls));

        let repository_selected = true;
        assert!(uncertain_repository_effect_requires_refresh(
            repository_selected,
            &calls
        ));
        calls.clear();
        assert!(calls.is_empty());
    }

    #[test]
    fn started_branch_without_repository_does_not_manufacture_refresh_state() {
        let id = ToolCallId::new();
        let requested = AgentEvent::ToolRequested {
            session_id: SessionId::new(),
            tool_call: ToolCall {
                id: id.clone(),
                name: ToolName::new(REPOSITORY_CREATE_BRANCH_TOOL_NAME),
                input: ToolInput(serde_json::json!({"name": "branch"})),
            },
        };
        let started = AgentEvent::ToolStarted {
            session_id: SessionId::new(),
            tool_call_id: id,
        };
        let mut calls = HashMap::new();
        activity_event_with_composition(
            &requested,
            &mut calls,
            &empty_composition_metadata(),
            false,
        )
        .expect("requested activity");
        let started = activity_event_with_composition(
            &started,
            &mut calls,
            &empty_composition_metadata(),
            false,
        )
        .expect("started activity");
        assert!(!started.invalidate_review);
        assert_eq!(started.refresh_reason, None);
        assert!(uncertain_repository_effect_pending(&calls));
        assert!(!uncertain_repository_effect_requires_refresh(false, &calls));
        calls.clear();
        assert!(calls.is_empty());
    }

    #[test]
    fn external_start_invalidates_review_and_finish_refreshes_selected_repository() {
        let composition = external_activity_composition(SourceKind::Mcp);
        let id = ToolCallId::new();
        let requested = AgentEvent::ToolRequested {
            session_id: SessionId::new(),
            tool_call: ToolCall {
                id: id.clone(),
                name: ToolName::new("mcp.host.echo"),
                input: ToolInput(serde_json::json!({})),
            },
        };
        let started = AgentEvent::ToolStarted {
            session_id: SessionId::new(),
            tool_call_id: id.clone(),
        };
        let finished = AgentEvent::ToolFinished {
            session_id: SessionId::new(),
            tool_call_id: id,
            output: ToolOutput {
                content: vec![],
                is_error: true,
            },
        };
        let mut calls = HashMap::new();
        activity_event_with_composition(&requested, &mut calls, &composition, true)
            .expect("requested activity");
        let started = activity_event_with_composition(&started, &mut calls, &composition, true)
            .expect("started activity");
        assert!(started.invalidate_review);
        assert_eq!(started.refresh_reason, None);
        assert!(uncertain_repository_effect_pending(&calls));
        let finished = activity_event_with_composition(&finished, &mut calls, &composition, true)
            .expect("finished activity");
        assert!(finished.invalidate_review);
        assert_eq!(
            finished.refresh_reason,
            Some(super::RepositoryRefreshReason::UncertainRepositoryEffect)
        );
        assert!(!uncertain_repository_effect_pending(&calls));
    }

    #[test]
    fn external_started_without_repository_does_not_manufacture_refresh_state() {
        let composition = external_activity_composition(SourceKind::ProcessPlugin);
        let id = ToolCallId::new();
        let requested = AgentEvent::ToolRequested {
            session_id: SessionId::new(),
            tool_call: ToolCall {
                id: id.clone(),
                name: ToolName::new("plugin.host.echo"),
                input: ToolInput(serde_json::json!({})),
            },
        };
        let started = AgentEvent::ToolStarted {
            session_id: SessionId::new(),
            tool_call_id: id.clone(),
        };
        let finished = AgentEvent::ToolFinished {
            session_id: SessionId::new(),
            tool_call_id: id,
            output: ToolOutput {
                content: vec![],
                is_error: false,
            },
        };
        let mut calls = HashMap::new();
        activity_event_with_composition(&requested, &mut calls, &composition, false)
            .expect("requested activity");
        let started = activity_event_with_composition(&started, &mut calls, &composition, false)
            .expect("started activity");
        assert!(!started.invalidate_review);
        let finished = activity_event_with_composition(&finished, &mut calls, &composition, false)
            .expect("finished activity");
        assert!(!finished.invalidate_review);
        assert_eq!(finished.refresh_reason, None);
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
    fn startup_remembers_profile_path_without_selecting_or_activating_it() {
        use super::desktop_preferences::{
            RememberedTrustedProfilePath, clear_test_state, test_lock,
        };
        let _guard = test_lock()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let storage = TestRepository::new();
        let raw_path = r"C:\missing\profile-source.json";
        let remembered = RememberedTrustedProfilePath::parse(PathBuf::from(raw_path))
            .expect("fixture path is lexically valid");
        let bytes = serde_json::to_vec(&serde_json::json!({
            "version": 3,
            "model": { "provider": "inherit" },
            "trusted_profile": { "path": raw_path },
        }))
        .unwrap();
        fs::write(storage.0.join("desktop-preferences.json"), bytes).unwrap();

        clear_test_state();
        super::reset_startup_activation_counters();
        let state = DesktopAppState::new(storage.0.clone());
        assert_eq!(
            state
                .preferences
                .lock()
                .unwrap()
                .remembered_trusted_profile_path(),
            Some(remembered)
        );
        assert!(state.trusted_profile.lock().unwrap().is_none());
        assert_eq!(*state.trusted_profile_generation.lock().unwrap(), 0);
        assert!(state.provider_activation.lock().unwrap().is_none());
        assert_eq!(state.status().profile_status, "not loaded");
        assert_eq!(
            super::startup_activation_snapshot(),
            super::StartupActivationCounters::default()
        );
        clear_test_state();
    }

    #[test]
    fn clear_selection_is_process_local_and_generation_is_exact() {
        use super::desktop_preferences::RememberedTrustedProfilePath;

        let storage = TestRepository::new();
        let profile_path = storage.0.join("provider-profile.json");
        fs::write(
            &profile_path,
            serde_json::json!({
                "profile_version": 1,
                "profile_id": "task215-profile",
                "resources": { "executables": {}, "repositories": {} },
                "capabilities": [],
                "mcp_providers": [],
                "process_plugins": []
            })
            .to_string(),
        )
        .unwrap();
        let remembered = RememberedTrustedProfilePath::parse(profile_path.clone()).unwrap();
        let state = DesktopAppState::new(storage.0.clone());
        state
            .preferences
            .lock()
            .unwrap()
            .save_trusted_profile_path(&DesktopModelSelection::default(), remembered.clone())
            .unwrap();
        let selection = load_provider_only_profile(profile_path).unwrap();

        publish_trusted_profile_selection(&state, selection);
        assert_eq!(*state.trusted_profile_generation.lock().unwrap(), 1);
        assert!(state.provider_activation.lock().unwrap().is_none());
        assert!(
            state
                .preferences
                .lock()
                .unwrap()
                .remembered_trusted_profile_path()
                .is_some()
        );

        assert!(clear_trusted_profile_selection(&state));
        assert_eq!(*state.trusted_profile_generation.lock().unwrap(), 2);
        assert!(state.trusted_profile.lock().unwrap().is_none());
        assert_eq!(
            state
                .preferences
                .lock()
                .unwrap()
                .remembered_trusted_profile_path(),
            Some(remembered)
        );
        assert!(!clear_trusted_profile_selection(&state));
        assert_eq!(*state.trusted_profile_generation.lock().unwrap(), 2);
    }

    #[test]
    fn restore_rereads_current_source_without_persisting_or_spawning() {
        use super::desktop_preferences::RememberedTrustedProfilePath;

        let storage = TestRepository::new();
        let profile_path = storage.0.join("provider-profile.json");
        let write_profile = |profile_id: &str| {
            fs::write(
                &profile_path,
                serde_json::json!({
                    "profile_version": 1,
                    "profile_id": profile_id,
                    "resources": { "executables": {}, "repositories": {} },
                    "capabilities": [],
                    "mcp_providers": [],
                    "process_plugins": []
                })
                .to_string(),
            )
            .unwrap();
        };
        write_profile("task215-before");
        let remembered = RememberedTrustedProfilePath::parse(profile_path.clone()).unwrap();
        let state = DesktopAppState::new(storage.0.clone());
        save_trusted_profile_preference(&state, remembered.clone()).unwrap();

        restore_trusted_profile_selection(&state).unwrap();
        let first = state
            .trusted_profile
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .presentation();
        assert_eq!(first.profile_id.as_deref(), Some("task215-before"));
        assert_eq!(*state.trusted_profile_generation.lock().unwrap(), 1);
        assert!(state.provider_activation.lock().unwrap().is_none());
        assert_eq!(
            state
                .preferences
                .lock()
                .unwrap()
                .remembered_trusted_profile_path(),
            Some(remembered.clone())
        );

        write_profile("task215-after");
        restore_trusted_profile_selection(&state).unwrap();
        let second = state
            .trusted_profile
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .presentation();
        assert_eq!(second.profile_id.as_deref(), Some("task215-after"));
        assert_eq!(*state.trusted_profile_generation.lock().unwrap(), 2);

        fs::write(&profile_path, b"{").unwrap();
        assert_eq!(
            restore_trusted_profile_selection(&state),
            Err(FrontendError::ProfileInvalid)
        );
        assert_eq!(*state.trusted_profile_generation.lock().unwrap(), 2);
        assert_eq!(
            state
                .trusted_profile
                .lock()
                .unwrap()
                .as_ref()
                .unwrap()
                .presentation()
                .profile_id
                .as_deref(),
            Some("task215-after")
        );
        assert_eq!(
            state
                .preferences
                .lock()
                .unwrap()
                .remembered_trusted_profile_path(),
            Some(remembered)
        );
    }

    #[test]
    fn forget_removes_only_preference_and_save_failure_is_non_destructive() {
        use super::desktop_preferences::{
            RememberedTrustedProfilePath, TestFault, clear_test_state, set_test_fault, test_lock,
        };

        let _guard = test_lock()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let storage = TestRepository::new();
        let path = storage.0.join("provider-profile.json");
        fs::write(
            &path,
            serde_json::json!({
                "profile_version": 1,
                "profile_id": "task215-forget",
                "resources": { "executables": {}, "repositories": {} },
                "capabilities": [],
                "mcp_providers": [],
                "process_plugins": []
            })
            .to_string(),
        )
        .unwrap();
        let remembered = RememberedTrustedProfilePath::parse(path.clone()).unwrap();
        let state = DesktopAppState::new(storage.0.clone());
        save_trusted_profile_preference(&state, remembered.clone()).unwrap();
        publish_trusted_profile_selection(&state, load_provider_only_profile(path).unwrap());
        assert_eq!(*state.trusted_profile_generation.lock().unwrap(), 1);

        forget_trusted_profile_preference(&state).unwrap();
        assert_eq!(
            state
                .preferences
                .lock()
                .unwrap()
                .remembered_trusted_profile_path(),
            None
        );
        assert!(state.trusted_profile.lock().unwrap().is_some());
        assert_eq!(*state.trusted_profile_generation.lock().unwrap(), 1);

        save_trusted_profile_preference(&state, remembered.clone()).unwrap();
        set_test_fault(
            storage.0.join("desktop-preferences.json"),
            TestFault::ReplaceOther,
        );
        assert_eq!(
            forget_trusted_profile_preference(&state),
            Err(FrontendError::TrustedProfilePreferenceSaveFailed)
        );
        assert_eq!(
            state
                .preferences
                .lock()
                .unwrap()
                .remembered_trusted_profile_path(),
            Some(remembered)
        );
        assert!(state.trusted_profile.lock().unwrap().is_some());
        assert_eq!(*state.trusted_profile_generation.lock().unwrap(), 1);
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

    #[test]
    fn live_completion_marker_requires_a_complete_marker() {
        assert!(super::contains_live_completion_marker(
            "RAH_REPO_RENAME_FILE_LIVE_OK"
        ));
        assert!(super::contains_live_completion_marker(
            "prelude.RAH_REPO_RENAME_FILE_LIVE_OK"
        ));
        assert!(!super::contains_live_completion_marker(
            "the marker is absent"
        ));
        assert!(!super::contains_live_completion_marker(
            "RAH_REPO_RENAME_FILE_LIVE_O"
        ));
        assert!(!super::contains_live_completion_marker(
            "RAH_REPO_RENAME_FILE_LIVE_OK_extra"
        ));
        assert!(!super::contains_live_completion_marker(
            "XRAH_REPO_RENAME_FILE_LIVE_OK"
        ));
        assert!(!super::contains_live_completion_marker("RAH__LIVE_OK"));
    }
}
