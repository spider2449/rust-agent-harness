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
mod remembered_workspace;
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
    GitUnstageTool, PreparedRepositoryCommitAuthorizationError, REPOSITORY_CREATE_BRANCH_TOOL_NAME,
    RepositoryAdmissionIdentity, RepositoryAdmissionRelation, RepositoryBranchCreationAuthority,
    RepositoryBranchCreationTool, RepositoryCommitControl, RepositoryCommitReview,
    RepositoryCommitTool, RepositoryCreateFilePreparationError,
    RepositoryCreateFilePreparationRequest, RepositoryDeleteFilePreparationError,
    RepositoryDeleteFilePreparationRequest, RepositoryDiffStagedTool, RepositoryDiffTool,
    RepositoryDirectoryCreationAuthority, RepositoryDirectoryCreationTool,
    RepositoryFileCreationTool, RepositoryFileDeletionAuthority, RepositoryFileDeletionTool,
    RepositoryFileInfoTool, RepositoryFileRenameAuthority, RepositoryFileRenameTool,
    RepositoryListTool, RepositoryMultiFileEditPreparationError,
    RepositoryMultiFileEditPreparationRequest, RepositoryMultiFileEditPreparationTarget,
    RepositoryMultiFileEditTextReplacement, RepositoryMultiFileEditTool,
    RepositoryPatchPreparationError, RepositoryPatchPreparationRequest,
    RepositoryPatchResultClassification, RepositoryRenameFilePreparationError,
    RepositoryRenameFilePreparationRequest, RepositoryRenameFileProof, RepositorySearchTool,
    RepositoryStatusTool, RepositoryWorktreePatchTool, Tool, ToolContext, ToolError, ToolRegistry,
    authorize_tool_dispatch, authorized_tool_dispatch, classify_repository_patch_output,
};
#[cfg(target_os = "windows")]
use remembered_workspace::{
    RememberedCandidateId, RememberedLocationHint, RememberedWorkspaceLookupError,
    RememberedWorkspaceMutationError, RememberedWorkspaceStartupState, RememberedWorkspaceState,
};
#[cfg(target_os = "windows")]
use repository_membership::{
    InertRepositoryMember, RepositoryMemberId, WorkspaceMembershipDeactivation,
    WorkspaceMembershipRemoval, WorkspaceMembershipState,
};
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
        Arc, Mutex, MutexGuard,
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
        identity_generation: u64,
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
    /// Excludes activation publication from lifecycle transitions that can
    /// change connection, model, or HostExplicit authority state. Every
    /// mutating transition acquires this first, then its state locks.
    lifecycle_coordination: Mutex<()>,
    repository: Mutex<Option<Arc<DesktopRepository>>>,
    repository_generation: Mutex<u64>,
    repository_workflow: Mutex<RepositoryWorkflowState>,
    /// At most one process-local asynchronous Stage/Unstage effect may hold
    /// the active repository lifecycle epoch at a time.
    repository_index_effect_reservation: Mutex<Option<RepositoryIndexEffectReservation>>,
    next_repository_index_effect_token: Mutex<u64>,
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
    /// Mutable descriptive remembered-workspace state.
    /// It never participates in repository membership or authority composition.
    remembered_workspace: RememberedWorkspaceState,
    /// An app-owned non-project directory used only when no repository is selected.
    neutral_workspace: Option<PathBuf>,
    model: Mutex<DesktopModelState>,
    preferences: Mutex<Preferences>,
    preference_ordering: Mutex<()>,
    conversation: Mutex<DesktopConversationState>,
    persistence: Mutex<Persistence>,
    close_started: AtomicBool,
    #[cfg(test)]
    activation_test_hook: Mutex<Option<Arc<ActivationTestHook>>>,
    #[cfg(test)]
    authorization_test_hook: Mutex<Option<Arc<AuthorizationTestHook>>>,
    #[cfg(test)]
    connect_publication_test_hook: Mutex<Option<Arc<ConnectPublicationTestHook>>>,
    #[cfg(test)]
    index_effect_test_hook: Mutex<Option<Arc<IndexEffectTestHook>>>,
    #[cfg(test)]
    workflow_refresh_test_hook: Mutex<Option<Arc<WorkflowRefreshTestHook>>>,
}

#[cfg(target_os = "windows")]
#[cfg(test)]
struct ActivationTestHook {
    reached: tokio::sync::mpsc::UnboundedSender<()>,
    release: Arc<std::sync::Barrier>,
    target_member: Option<RepositoryMemberId>,
}

#[cfg(target_os = "windows")]
#[cfg(test)]
struct AuthorizationTestHook {
    reached: tokio::sync::mpsc::UnboundedSender<()>,
    release: Arc<std::sync::Barrier>,
}

#[cfg(target_os = "windows")]
#[cfg(test)]
struct ConnectPublicationTestHook {
    reached: tokio::sync::mpsc::UnboundedSender<()>,
    release: Arc<std::sync::Barrier>,
}

#[cfg(target_os = "windows")]
#[cfg(test)]
struct IndexEffectTestHook {
    reached: tokio::sync::mpsc::UnboundedSender<()>,
    release: Arc<std::sync::Barrier>,
}

#[cfg(target_os = "windows")]
#[cfg(test)]
struct WorkflowRefreshTestHook {
    reached: tokio::sync::mpsc::UnboundedSender<()>,
    release: Arc<std::sync::Barrier>,
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
        let remembered_workspace = RememberedWorkspaceState::open(storage_directory.clone());
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
            lifecycle_coordination: Mutex::new(()),
            repository: Mutex::new(None),
            repository_generation: Mutex::new(0),
            repository_workflow: Mutex::new(RepositoryWorkflowState::default()),
            repository_index_effect_reservation: Mutex::new(None),
            next_repository_index_effect_token: Mutex::new(0),
            commit_identity: Mutex::new(identity),
            commit_identity_generation: Mutex::new(0),
            commit_capability: Mutex::new(None),
            trusted_profile: Mutex::new(None),
            trusted_profile_generation: Mutex::new(0),
            host_invocation: Mutex::new(HostInvocationCoordinator::default()),
            provider_activation: Mutex::new(None),
            remembered_workspace,
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
            #[cfg(test)]
            activation_test_hook: Mutex::new(None),
            #[cfg(test)]
            authorization_test_hook: Mutex::new(None),
            #[cfg(test)]
            connect_publication_test_hook: Mutex::new(None),
            #[cfg(test)]
            index_effect_test_hook: Mutex::new(None),
            #[cfg(test)]
            workflow_refresh_test_hook: Mutex::new(None),
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
        let _lifecycle_coordination = self
            .lifecycle_coordination
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
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
        let _lifecycle_coordination = self
            .lifecycle_coordination
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
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
        let _lifecycle_coordination = self
            .lifecycle_coordination
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
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
        chat.terminal.claim(generation)
    }

    fn finish_claimed_chat(&self, generation: u64) -> bool {
        let _lifecycle_coordination = self
            .lifecycle_coordination
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut active = self
            .active_chat
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let claimed = active.as_ref().is_some_and(|chat| {
            chat.generation == generation && !chat.terminal.is_unclaimed(generation)
        });
        if !claimed {
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
        let _lifecycle_coordination = self
            .lifecycle_coordination
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
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
        let _lifecycle_coordination = self
            .lifecycle_coordination
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
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
        let _lifecycle_coordination = self
            .lifecycle_coordination
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
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
        let _lifecycle_coordination = self
            .lifecycle_coordination
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
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
#[derive(Clone)]
struct RepositoryIndexAction {
    kind: RepositoryIndexActionKind,
    repository_generation: u64,
    observation_generation: u64,
    target: PathBuf,
    target_observation: TargetObservation,
}

#[cfg(target_os = "windows")]
#[derive(Clone)]
struct RepositoryIndexEffectReservation {
    token: u64,
    repository_generation: u64,
    member_id: Option<RepositoryMemberId>,
    kind: RepositoryIndexActionKind,
    repository: Arc<DesktopRepository>,
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
    RepositoryMemberSelectorInvalid,
    RepositoryMemberActive,
    RepositoryObservationFailed,
    RepositoryUnavailable,
    RepositoryDialogFailed,
    RepositoryBusy,
    RepositoryActionInvalid,
    RepositoryActionStale,
    RememberedCatalogUnavailable,
    RememberedCatalogSaveFailed,
    RememberedCandidateIdInvalid,
    RememberedCandidateNotFound,
    RememberedCatalogRequestInvalid,
    RememberedLocationRequired,
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ConnectionPublicationCurrentness {
    repository_generation: u64,
    model_generation: u64,
    profile_generation: u64,
    connection_generation: u64,
    identity_generation: u64,
}

#[cfg(target_os = "windows")]
fn connection_publication_is_current(
    captured: ConnectionPublicationCurrentness,
    current: ConnectionPublicationCurrentness,
) -> bool {
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
    IndexEffectActive,
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
    identity_generation: u64,
    repository_fingerprint: Option<String>,
    composition: Arc<DesktopToolComposition>,
    allowed_permissions: Vec<PermissionLevel>,
    commit_capability: Option<DesktopCommitCapability>,
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
    let _lifecycle_coordination = state
        .lifecycle_coordination
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
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
    if let Some(reason) = connected_publication_index_effect_rejection(state) {
        let PendingConnectedPublication {
            runtime,
            activation,
            ..
        } = pending;
        return Err(Box::new(RejectedProviderPublication {
            runtime,
            activation,
            reason,
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
    if !connection_publication_is_current(
        ConnectionPublicationCurrentness {
            repository_generation: pending.repository_generation,
            model_generation: pending.model_generation,
            profile_generation: pending.profile_generation,
            connection_generation: pending.connection_generation,
            identity_generation: pending.identity_generation,
        },
        ConnectionPublicationCurrentness {
            repository_generation: *current_repository_generation,
            model_generation: current_model.generation,
            profile_generation: *current_profile_generation,
            connection_generation: *current_connection_generation,
            identity_generation: *state
                .commit_identity_generation
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        },
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
        identity_generation,
        repository_fingerprint,
        composition,
        allowed_permissions,
        commit_capability,
    } = pending;
    *published_provider = activation;
    *connection = ConnectionState::Connected {
        runtime,
        source,
        repository_generation,
        model_generation,
        profile_generation,
        connection_generation,
        identity_generation,
        repository_fingerprint,
        composition,
        allowed_permissions,
    };
    *state
        .commit_capability
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = commit_capability;
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
    effective_authority_snapshot_for_state(state.inner())
}

#[cfg(target_os = "windows")]
fn effective_authority_snapshot_for_state(state: &DesktopAppState) -> EffectiveAuthoritySnapshot {
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
            identity_generation,
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
            let current_identity_generation = *state
                .commit_identity_generation
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
            let publication_current = connection_publication_is_current(
                ConnectionPublicationCurrentness {
                    repository_generation: *repository_generation,
                    model_generation: *model_generation,
                    profile_generation: *profile_generation,
                    connection_generation: *connection_generation,
                    identity_generation: *identity_generation,
                },
                ConnectionPublicationCurrentness {
                    repository_generation: current_repository_generation,
                    model_generation: current_model_generation,
                    profile_generation: current_profile_generation,
                    connection_generation: current_connection_generation,
                    identity_generation: current_identity_generation,
                },
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
        identity_generation,
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
    let current_identity_generation = *state
        .commit_identity_generation
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if generations != current_generations
        || *identity_generation != current_identity_generation
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
    let _lifecycle_coordination = state
        .lifecycle_coordination
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
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
    let _lifecycle_coordination = state
        .lifecycle_coordination
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
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
    let _lifecycle_coordination = state
        .lifecycle_coordination
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
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
        let _lifecycle_coordination = state
            .lifecycle_coordination
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
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
        let _lifecycle_coordination = state
            .lifecycle_coordination
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
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
        let _lifecycle_coordination = state
            .lifecycle_coordination
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
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
        let _lifecycle_coordination = state
            .lifecycle_coordination
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
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
        let _lifecycle_coordination = state
            .lifecycle_coordination
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
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
        let _lifecycle_coordination = state
            .lifecycle_coordination
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
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
        let _lifecycle_coordination = state
            .lifecycle_coordination
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
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
        let _lifecycle_coordination = state
            .lifecycle_coordination
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
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
        let _lifecycle_coordination = state
            .lifecycle_coordination
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
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
        let _lifecycle_coordination = state
            .lifecycle_coordination
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
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
        let _lifecycle_coordination = state
            .lifecycle_coordination
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
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
    let _lifecycle_coordination = state
        .lifecycle_coordination
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
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
    let _lifecycle_coordination = state
        .lifecycle_coordination
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if repository_index_effect_is_active(state.inner()) {
        return Err(FrontendError::RepositoryBusy);
    }
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
    if chat != ChatState::Idle {
        return Err(FrontendError::ModelConfigurationBusy);
    }
    selection.validate()?;
    // Commit revocation is the first authoritative transition. A busy
    // pending slot leaves model, capability, and workflow state untouched.
    reserve_commit_revocation(state.inner())?;
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
    withdraw_commit_capability_and_workflow(state.inner());
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
    let _lifecycle_coordination = state
        .lifecycle_coordination
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if repository_index_effect_is_active(state.inner()) {
        return Err(FrontendError::RepositoryBusy);
    }
    let _ordering = state
        .preference_ordering
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let identity = DesktopCommitIdentity { name, email };
    identity
        .validate()
        .map_err(|_| FrontendError::CommitIdentityInvalid)?;
    reserve_commit_revocation(state.inner())?;
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
    withdraw_commit_capability_and_workflow(state.inner());
    Ok(())
}

#[cfg(target_os = "windows")]
#[tauri::command]
fn reset_model_preferences(
    app: AppHandle,
    state: State<'_, DesktopAppState>,
) -> Result<(), FrontendError> {
    let _lifecycle_coordination = state
        .lifecycle_coordination
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if repository_index_effect_is_active(state.inner()) {
        return Err(FrontendError::RepositoryBusy);
    }
    let _ordering = state
        .preference_ordering
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let chat = *state
        .chat
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if chat != ChatState::Idle {
        return Err(FrontendError::ModelConfigurationBusy);
    }
    reserve_commit_revocation(state.inner())?;
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
    withdraw_commit_capability_and_workflow(state.inner());
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
struct PreparedRepositoryWorkflowAction {
    status_entry_index: usize,
    kind: RepositoryIndexActionKind,
    target: PathBuf,
    target_observation: TargetObservation,
}

#[cfg(target_os = "windows")]
struct PreparedRepositoryWorkflow {
    repository_generation: u64,
    identity_generation: u64,
    snapshot: RepositorySnapshot,
    actions: Vec<PreparedRepositoryWorkflowAction>,
    review_digest: Option<String>,
    commit_review: Option<RepositoryCommitReview>,
}

#[cfg(target_os = "windows")]
fn prepare_repository_workflow(
    repository: &DesktopRepository,
    repository_generation: u64,
    mut snapshot: RepositorySnapshot,
    commit_review: Option<RepositoryCommitReview>,
    identity_generation: u64,
) -> PreparedRepositoryWorkflow {
    let review_digest = matches!(
        snapshot.review,
        StagedReviewPresentation::ReviewAvailable { .. }
    )
    .then(|| staged_review_digest(&snapshot.staged_diff));
    let mut actions = Vec::new();
    for (status_entry_index, entry) in snapshot.status_entries.iter_mut().enumerate() {
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
            actions.push(PreparedRepositoryWorkflowAction {
                status_entry_index,
                kind: RepositoryIndexActionKind::Stage,
                target: target.canonical_path.clone(),
                target_observation: target.clone(),
            });
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
            actions.push(PreparedRepositoryWorkflowAction {
                status_entry_index,
                kind: RepositoryIndexActionKind::Unstage,
                target: target.canonical_path.clone(),
                target_observation: target,
            });
        }
    }
    PreparedRepositoryWorkflow {
        repository_generation,
        identity_generation,
        snapshot,
        actions,
        review_digest,
        commit_review,
    }
}

#[cfg(target_os = "windows")]
fn publish_repository_workflow(
    state: &DesktopAppState,
    mut prepared: PreparedRepositoryWorkflow,
) -> RepositorySnapshot {
    let mut workflow = state
        .repository_workflow
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let action_count = u64::try_from(prepared.actions.len())
        .ok()
        .and_then(|count| {
            count.checked_add(if prepared.commit_review.is_some() {
                1
            } else {
                0
            })
        });
    let next_observation_generation = workflow.observation_generation.checked_add(1);
    let action_sequence_fits = action_count
        .and_then(|count| workflow.next_action.checked_add(count))
        .is_some();
    let Some(next_observation_generation) =
        next_observation_generation.filter(|_| action_sequence_fits)
    else {
        workflow.actions.clear();
        workflow.review = None;
        workflow.commit_review = None;
        workflow.review_selector = None;
        workflow.authorization = CommitAuthorizationPresentation::AuthorizationRevoked;
        for entry in &mut prepared.snapshot.status_entries {
            entry.stage_action_id = None;
        }
        for file in &mut prepared.snapshot.staged_diff {
            file.unstage_action_id = None;
        }
        prepared.snapshot.review = StagedReviewPresentation::ReviewUnavailable {
            reason: "repository workflow generation exhausted",
        };
        return prepared.snapshot;
    };

    workflow.observation_generation = next_observation_generation;
    workflow.actions.clear();
    workflow.review = None;
    workflow.commit_review = None;
    workflow.review_selector = None;
    workflow.authorization = CommitAuthorizationPresentation::AuthorizationRevoked;
    let observation_generation = workflow.observation_generation;
    for action in prepared.actions {
        workflow.next_action += 1;
        let action_id = format!(
            "index-{}-{observation_generation}-{}",
            prepared.repository_generation, workflow.next_action
        );
        workflow.actions.insert(
            action_id.clone(),
            RepositoryIndexAction {
                kind: action.kind,
                repository_generation: prepared.repository_generation,
                observation_generation,
                target: action.target,
                target_observation: action.target_observation,
            },
        );
        match action.kind {
            RepositoryIndexActionKind::Stage => {
                prepared.snapshot.status_entries[action.status_entry_index].stage_action_id =
                    Some(action_id);
            }
            RepositoryIndexActionKind::Unstage => {
                let entry_path = &prepared.snapshot.status_entries[action.status_entry_index].path;
                for file in &mut prepared.snapshot.staged_diff {
                    if file.new_path.as_deref() == Some(entry_path.as_str()) {
                        file.unstage_action_id = Some(action_id.clone());
                    }
                }
            }
        }
    }
    if let Some(digest) = prepared.review_digest {
        workflow.review = Some(StagedReviewDescriptor {
            repository_generation: prepared.repository_generation,
            observation_generation,
            digest,
            complete: true,
            binary_supported: true,
        });
    }
    if let Some(commit_review) = prepared.commit_review {
        workflow.next_action += 1;
        let selector = format!(
            "review-{}-{observation_generation}-{}-{}",
            prepared.repository_generation, prepared.identity_generation, workflow.next_action
        );
        workflow.commit_review = Some(commit_review);
        workflow.review_selector = Some(selector.clone());
        workflow.authorization = CommitAuthorizationPresentation::ReadyToAuthorize;
        prepared.snapshot.review = StagedReviewPresentation::ReviewAvailable {
            review_id: Some(selector),
            can_authorize: true,
            authorization_state: workflow.authorization,
        };
    }
    prepared.snapshot
}

#[cfg(target_os = "windows")]
#[cfg(test)]
fn install_repository_workflow(
    state: &DesktopAppState,
    repository: &DesktopRepository,
    repository_generation: u64,
    snapshot: RepositorySnapshot,
    commit_review: Option<RepositoryCommitReview>,
    identity_generation: u64,
) -> RepositorySnapshot {
    publish_repository_workflow(
        state,
        prepare_repository_workflow(
            repository,
            repository_generation,
            snapshot,
            commit_review,
            identity_generation,
        ),
    )
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
    let active_member = state
        .workspace_membership
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .active_member();
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
    let prepared = prepare_repository_workflow(
        &repository,
        generation,
        snapshot,
        review,
        identity_generation,
    );
    repository_workflow_before_publish_barrier(state);
    let _lifecycle_coordination = state
        .lifecycle_coordination
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let current_repository = state
        .repository
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    let current_active_member = state
        .workspace_membership
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .active_member();
    if *state
        .repository_generation
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        != generation
        || !current_repository
            .as_ref()
            .is_some_and(|current| Arc::ptr_eq(current, &repository))
        || current_active_member != active_member
    {
        return Err(FrontendError::RepositoryObservationFailed);
    }
    Ok(publish_repository_workflow(state, prepared))
}

#[cfg(target_os = "windows")]
async fn revoke_repository_commit_context(state: &DesktopAppState) {
    let control = {
        let _lifecycle_coordination = state
            .lifecycle_coordination
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let control = state
            .commit_capability
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
            .map(|capability| capability.control);
        *state
            .repository_workflow
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) =
            RepositoryWorkflowState::default();
        control
    };
    if let Some(control) = control {
        // The lifecycle gate already prevents stale writers from reaching
        // publication. If the shared pending slot is temporarily busy, wait
        // only after releasing the blocking lifecycle guard.
        if !control.try_clear_authorization_now() {
            control.clear_authorization().await;
        }
    }
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
    let directory_creation_authority = RepositoryDirectoryCreationAuthority::new(git, root)
        .map_err(|error| {
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

#[cfg(all(target_os = "windows", not(test)))]
async fn admit_repository(
    state: &DesktopAppState,
    git: &Path,
    selected_path: &Path,
) -> Result<RepositoryMemberId, FrontendError> {
    admit_repository_with_semantic_validation(state, git, selected_path).await
}

#[cfg(target_os = "windows")]
async fn admit_repository_with_semantic_validation(
    state: &DesktopAppState,
    git: &Path,
    selected_path: &Path,
) -> Result<RepositoryMemberId, FrontendError> {
    let identity = RepositoryAdmissionIdentity::capture(git, selected_path).map_err(|error| {
        let _ = error;
        tracing::warn!("repository admission identity capture failed");
        FrontendError::RepositoryInvalid
    })?;
    identity.validate_git().await.map_err(|error| {
        let _ = error;
        tracing::warn!("repository Git layout validation failed");
        FrontendError::RepositoryInvalid
    })?;
    let root = identity.canonical_root().to_path_buf();
    let _ = construct_repository_for_admission(git, &root)?;
    identity
        .revalidate_git(git, &root)
        .await
        .map_err(|_| FrontendError::RepositoryMemberStale)?;
    admit_repository_identity(state, identity)
}

#[cfg(all(target_os = "windows", test))]
fn admit_repository(
    state: &DesktopAppState,
    git: &Path,
    selected_path: &Path,
) -> Result<RepositoryMemberId, FrontendError> {
    if std::fs::symlink_metadata(selected_path.join(".git"))
        .is_ok_and(|metadata| metadata.is_file())
    {
        return Err(FrontendError::RepositoryInvalid);
    }
    let identity = RepositoryAdmissionIdentity::capture(git, selected_path).map_err(|error| {
        let _ = error;
        FrontendError::RepositoryInvalid
    })?;
    let root = identity.canonical_root().to_path_buf();
    let _ = construct_repository_for_admission(git, &root)?;
    identity
        .revalidate(git, &root)
        .map_err(|_| FrontendError::RepositoryMemberStale)?;
    admit_repository_identity(state, identity)
}

#[cfg(target_os = "windows")]
fn admit_repository_identity(
    state: &DesktopAppState,
    identity: RepositoryAdmissionIdentity,
) -> Result<RepositoryMemberId, FrontendError> {
    let root = identity.canonical_root().to_path_buf();
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
    repository_generation: u64,
) {
    let repository_fingerprint = repository_context_fingerprint(&repository.root);
    let repository = Arc::new(repository);
    let mut membership = state
        .workspace_membership
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    debug_assert!(membership.member(member_id).is_some());
    let published = membership.publish_active(member_id);
    assert!(
        published,
        "activation commit member precondition was violated"
    );
    *state
        .repository
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(repository);
    let mut generation = state
        .repository_generation
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    *generation = repository_generation;
    drop(generation);
    drop(membership);
    append_live_evidence(serde_json::json!({
        "event": "repository_selected",
        "repository_generation": repository_generation,
        "repository_fingerprint": repository_fingerprint,
    }));
    state.select_persistence_namespace();
    state
        .conversation
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .start_new();
}

#[cfg(target_os = "windows")]
fn next_repository_generation(current: u64) -> Option<u64> {
    current.checked_add(1)
}

#[cfg(target_os = "windows")]
fn close_model_turn_is_active(
    coordinator_state: CoordinatorState,
    chat_state: ChatState,
    active_chat_present: bool,
) -> bool {
    coordinator_state == CoordinatorState::ModelTurn
        || chat_state != ChatState::Idle
        || active_chat_present
}

#[cfg(target_os = "windows")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CloseConnectionState {
    NotConnected,
    Connecting,
    Connected,
    Disconnecting,
    Error,
}

#[cfg(target_os = "windows")]
fn close_connection_state(connection: &ConnectionState) -> CloseConnectionState {
    match connection {
        ConnectionState::NotConnected => CloseConnectionState::NotConnected,
        ConnectionState::Connecting => CloseConnectionState::Connecting,
        ConnectionState::Connected { .. } => CloseConnectionState::Connected,
        ConnectionState::Disconnecting => CloseConnectionState::Disconnecting,
        ConnectionState::Error(_) => CloseConnectionState::Error,
    }
}

#[cfg(target_os = "windows")]
fn close_runtime_is_disconnected(
    connection: CloseConnectionState,
    provider_activation_present: bool,
) -> bool {
    connection == CloseConnectionState::NotConnected && !provider_activation_present
}

#[cfg(target_os = "windows")]
#[derive(Clone)]
struct ActivationTransaction {
    target_member_id: RepositoryMemberId,
    target_admission_generation: u64,
    target_identity: RepositoryAdmissionIdentity,
    expected_active_member: Option<RepositoryMemberId>,
    expected_repository_generation: u64,
}

#[cfg(target_os = "windows")]
fn capture_activation_transaction(
    state: &DesktopAppState,
    member_id: RepositoryMemberId,
) -> Result<(ActivationTransaction, InertRepositoryMember), FrontendError> {
    // Lock order for activation is membership coordination, lifecycle
    // coordination, then the individual host state locks. The first two
    // locks are held only for synchronous state capture/checks.
    let _membership_coordination = state
        .membership_coordination
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let _lifecycle_coordination = state
        .lifecycle_coordination
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
        let coordinator = state
            .host_invocation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match coordinator.state() {
            CoordinatorState::Idle | CoordinatorState::HostPrepared => {}
            CoordinatorState::ModelTurn | CoordinatorState::HostRunning => {
                return Err(FrontendError::HostInvocationBusy);
            }
        }
    }
    let membership = state
        .workspace_membership
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let member = membership
        .member(member_id)
        .cloned()
        .ok_or(FrontendError::RepositoryMemberNotFound)?;
    let transaction = ActivationTransaction {
        target_member_id: member.id,
        target_admission_generation: member.admission_generation,
        target_identity: member.identity.clone(),
        expected_active_member: membership.active_member(),
        expected_repository_generation: *state
            .repository_generation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
    };
    Ok((transaction, member))
}

#[cfg(target_os = "windows")]
fn activation_pre_publication_barrier(
    _state: &DesktopAppState,
    _target_member: RepositoryMemberId,
) {
    #[cfg(test)]
    let hook = _state
        .activation_test_hook
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    #[cfg(test)]
    if let Some(hook) = hook.filter(|hook| {
        hook.target_member
            .is_none_or(|target_member| target_member == _target_member)
    }) {
        let _ = hook.reached.send(());
        hook.release.wait();
    }
}

#[cfg(target_os = "windows")]
fn repository_workflow_before_publish_barrier(_state: &DesktopAppState) {
    #[cfg(test)]
    let hook = _state
        .workflow_refresh_test_hook
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    #[cfg(test)]
    if let Some(hook) = hook {
        let _ = hook.reached.send(());
        hook.release.wait();
    }
}

#[cfg(target_os = "windows")]
fn authorization_pre_publication_barrier(_state: &DesktopAppState) {
    #[cfg(test)]
    let hook = _state
        .authorization_test_hook
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    #[cfg(test)]
    if let Some(hook) = hook {
        let _ = hook.reached.send(());
        hook.release.wait();
    }
}

#[cfg(target_os = "windows")]
fn connect_pre_publication_barrier(_state: &DesktopAppState) {
    #[cfg(test)]
    let hook = _state
        .connect_publication_test_hook
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    #[cfg(test)]
    if let Some(hook) = hook {
        let _ = hook.reached.send(());
        hook.release.wait();
    }
}

#[cfg(target_os = "windows")]
fn index_effect_pre_execution_barrier(_state: &DesktopAppState) {
    #[cfg(test)]
    let hook = _state
        .index_effect_test_hook
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    #[cfg(test)]
    if let Some(hook) = hook {
        let _ = hook.reached.send(());
        hook.release.wait();
    }
}

#[cfg(target_os = "windows")]
fn reserve_commit_revocation(state: &DesktopAppState) -> Result<(), FrontendError> {
    let control = state
        .commit_capability
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .as_ref()
        .map(|capability| Arc::clone(&capability.control));
    if control.is_some_and(|control| !control.try_clear_authorization_now()) {
        return Err(FrontendError::RepositoryBusy);
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn repository_index_effect_is_active(state: &DesktopAppState) -> bool {
    state
        .repository_index_effect_reservation
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .is_some()
}

#[cfg(target_os = "windows")]
fn connected_publication_index_effect_rejection(
    state: &DesktopAppState,
) -> Option<ProviderPublicationRejectionReason> {
    repository_index_effect_is_active(state)
        .then_some(ProviderPublicationRejectionReason::IndexEffectActive)
}

#[cfg(target_os = "windows")]
fn repository_index_effect_binding_is_current(
    state: &DesktopAppState,
    reservation: &RepositoryIndexEffectReservation,
) -> bool {
    let current_member = state
        .workspace_membership
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .active_member();
    let current_repository = state
        .repository
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    current_member == reservation.member_id
        && *state
            .repository_generation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            == reservation.repository_generation
        && current_repository
            .as_ref()
            .is_some_and(|current| Arc::ptr_eq(current, &reservation.repository))
}

#[cfg(target_os = "windows")]
fn begin_repository_index_effect(
    state: &DesktopAppState,
    action_id: &str,
    kind: RepositoryIndexActionKind,
) -> Result<(RepositoryIndexEffectReservation, RepositoryIndexAction), FrontendError> {
    let _lifecycle_coordination = state
        .lifecycle_coordination
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if repository_index_effect_is_active(state) {
        return Err(FrontendError::RepositoryBusy);
    }

    let repository = state
        .repository
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
        .ok_or(FrontendError::RepositoryNotSelected)?;
    let member_id = state
        .workspace_membership
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .active_member();
    let generation = *state
        .repository_generation
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let action = {
        let workflow = state
            .repository_workflow
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let action = workflow
            .actions
            .get(action_id)
            .cloned()
            .ok_or(FrontendError::RepositoryActionInvalid)?;
        if action.kind != kind
            || action.repository_generation != generation
            || action.observation_generation != workflow.observation_generation
        {
            return Err(FrontendError::RepositoryActionStale);
        }
        action
    };
    if !repository_index_effect_binding_is_current(
        state,
        &RepositoryIndexEffectReservation {
            token: 0,
            repository_generation: generation,
            member_id,
            kind,
            repository: Arc::clone(&repository),
        },
    ) {
        return Err(FrontendError::RepositoryActionStale);
    }

    // This is the last fallible transition before the reservation and action
    // consumption are published. A busy pending Commit slot leaves all state
    // intact and performs no index work.
    reserve_commit_revocation(state)?;
    let token = {
        let mut next = state
            .next_repository_index_effect_token
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *next = next.checked_add(1).ok_or(FrontendError::RepositoryBusy)?;
        *next
    };
    let reservation = RepositoryIndexEffectReservation {
        token,
        repository_generation: generation,
        member_id,
        kind,
        repository,
    };
    {
        let mut workflow = state
            .repository_workflow
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        workflow.actions.remove(action_id);
        workflow.actions.clear();
        workflow.review = None;
        workflow.commit_review = None;
        workflow.review_selector = None;
        workflow.authorization = CommitAuthorizationPresentation::AuthorizationRevoked;
    }
    *state
        .repository_index_effect_reservation
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(reservation.clone());
    Ok((reservation, action))
}

#[cfg(target_os = "windows")]
async fn refresh_repository_workflow_for_index_effect(
    state: &DesktopAppState,
    reservation: &RepositoryIndexEffectReservation,
) -> Result<RepositorySnapshot, FrontendError> {
    {
        let _lifecycle_coordination = state
            .lifecycle_coordination
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let current = state
            .repository_index_effect_reservation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .is_some_and(|current| {
                current.token == reservation.token
                    && current.kind == reservation.kind
                    && repository_index_effect_binding_is_current(state, current)
            });
        if !current {
            return Err(FrontendError::RepositoryBusy);
        }
    }
    let refreshed = refresh_repository_workflow(state).await;
    {
        let _lifecycle_coordination = state
            .lifecycle_coordination
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let current = state
            .repository_index_effect_reservation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .is_some_and(|current| {
                current.token == reservation.token
                    && current.kind == reservation.kind
                    && repository_index_effect_binding_is_current(state, current)
            });
        if !current {
            return Err(FrontendError::RepositoryBusy);
        }
    }
    refreshed
}

#[cfg(target_os = "windows")]
async fn complete_repository_index_effect(
    state: &DesktopAppState,
    reservation: &RepositoryIndexEffectReservation,
) -> Result<(), FrontendError> {
    let refresh_result = refresh_repository_workflow_for_index_effect(state, reservation).await;
    let current = {
        let _lifecycle_coordination = state
            .lifecycle_coordination
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut current_reservation = state
            .repository_index_effect_reservation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match current_reservation.as_ref() {
            Some(current) if current.token == reservation.token => {
                let binding_current = repository_index_effect_binding_is_current(state, current);
                current_reservation.take();
                binding_current
            }
            Some(_) | None => false,
        }
    };
    if !current {
        return Err(FrontendError::RepositoryBusy);
    }
    refresh_result.map(|_| ())
}

#[cfg(target_os = "windows")]
fn withdraw_commit_capability_and_workflow(state: &DesktopAppState) {
    state
        .commit_capability
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .take();
    *state
        .repository_workflow
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = RepositoryWorkflowState::default();
}

#[cfg(target_os = "windows")]
fn publish_activation_if_current(
    state: &DesktopAppState,
    transaction: &ActivationTransaction,
    repository: DesktopRepository,
) -> Result<(), FrontendError> {
    // This is the linearization point for activation. Lifecycle transitions
    // acquire the same exclusion before changing connection, chat/model, or
    // HostExplicit state. Prepared is invalidated while that exclusion is
    // held, so Confirm cannot consume it between invalidation and publication.
    let _membership_coordination = state
        .membership_coordination
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let _lifecycle_coordination = state
        .lifecycle_coordination
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if repository_index_effect_is_active(state) {
        return Err(FrontendError::RepositoryBusy);
    }

    let member = state
        .workspace_membership
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .member(transaction.target_member_id)
        .cloned()
        .ok_or(FrontendError::RepositoryMemberStale)?;
    if member.id != transaction.target_member_id
        || member.admission_generation != transaction.target_admission_generation
        || !member.identity.same_binding(&transaction.target_identity)
    {
        return Err(FrontendError::RepositoryMemberStale);
    }
    if state
        .workspace_membership
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .active_member()
        != transaction.expected_active_member
        || *state
            .repository_generation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            != transaction.expected_repository_generation
    {
        return Err(FrontendError::RepositoryBusy);
    }

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
        let coordinator = state
            .host_invocation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match coordinator.state() {
            CoordinatorState::Idle | CoordinatorState::HostPrepared => {}
            CoordinatorState::ModelTurn | CoordinatorState::HostRunning => {
                return Err(FrontendError::HostInvocationBusy);
            }
        }
    }
    let git = selected_git_executable().map_err(|_| FrontendError::RepositoryMemberStale)?;
    member
        .identity
        .revalidate(&git, &member.root)
        .map_err(|_| FrontendError::RepositoryMemberStale)?;
    let current_repository_generation = *state
        .repository_generation
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let next_repository_generation = next_repository_generation(current_repository_generation)
        .ok_or(FrontendError::RepositoryUnavailable)?;
    // This is the final fallible target check. Once Commit revocation is
    // reserved below, the commit point contains only infallible transitions.
    reserve_commit_revocation(state)?;

    // No ordinary failure is permitted after this point. The old active
    // state is invalidated and the new active composition is published as one
    // synchronous transition under the coordination locks above.
    state
        .host_invocation
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clear_prepared();
    state
        .commit_capability
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .take();
    *state
        .repository_workflow
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = RepositoryWorkflowState::default();
    publish_active_repository(state, member.id, repository, next_repository_generation);
    Ok(())
}

#[cfg(target_os = "windows")]
async fn activate_admitted_member(
    state: &DesktopAppState,
    member_id: RepositoryMemberId,
) -> Result<ActivationOutcome, FrontendError> {
    {
        let _membership_coordination = state
            .membership_coordination
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state
            .workspace_membership
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .active_member()
            == Some(member_id)
        {
            return Ok(ActivationOutcome::AlreadyActive);
        }
    }
    let (transaction, member) = capture_activation_transaction(state, member_id)?;
    let git = selected_git_executable().map_err(|_| FrontendError::RepositoryMemberStale)?;
    member
        .identity
        .revalidate_git(&git, &member.root)
        .await
        .map_err(|_| FrontendError::RepositoryMemberStale)?;
    let repository = construct_repository_for_admission(&git, &member.root)?;
    member
        .identity
        .revalidate_git(&git, &member.root)
        .await
        .map_err(|_| FrontendError::RepositoryMemberStale)?;
    // Test synchronization is deliberately before the final gate so stale
    // and loser candidates cannot pass through post-invalidation state.
    activation_pre_publication_barrier(state, member_id);
    member
        .identity
        .revalidate_git(&git, &member.root)
        .await
        .map_err(|_| FrontendError::RepositoryMemberStale)?;
    publish_activation_if_current(state, &transaction, repository)?;
    Ok(ActivationOutcome::Activated)
}

#[cfg(target_os = "windows")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ActivationOutcome {
    Activated,
    AlreadyActive,
}

#[cfg(target_os = "windows")]
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct RememberedCandidatePresentation {
    candidate_id: String,
    label: String,
    order: usize,
    has_location_hint: bool,
}

#[cfg(target_os = "windows")]
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct RememberedWorkspaceCatalogPresentation {
    status: &'static str,
    candidates: Vec<RememberedCandidatePresentation>,
    last_active_candidate_id: Option<String>,
}

#[cfg(target_os = "windows")]
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct RememberedLocationPresentation {
    candidate_id: String,
    location: String,
}

#[cfg(target_os = "windows")]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RememberedCandidateAddRequest {
    label: String,
    #[serde(default)]
    location_hint: Option<String>,
}

#[cfg(target_os = "windows")]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RememberedCandidateUpdateRequest {
    candidate_id: String,
    #[serde(default)]
    label: Option<String>,
    #[serde(
        default,
        deserialize_with = "deserialize_remembered_location_hint_update"
    )]
    location_hint: RememberedLocationHintUpdate,
}

#[cfg(target_os = "windows")]
#[derive(Debug, Default)]
enum RememberedLocationHintUpdate {
    #[default]
    Missing,
    Clear,
    Set(String),
}

#[cfg(target_os = "windows")]
fn deserialize_remembered_location_hint_update<'de, D>(
    deserializer: D,
) -> Result<RememberedLocationHintUpdate, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Option::<String>::deserialize(deserializer)?;
    Ok(match value {
        Some(value) => RememberedLocationHintUpdate::Set(value),
        None => RememberedLocationHintUpdate::Clear,
    })
}

#[cfg(target_os = "windows")]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RememberedCandidateReorderRequest {
    candidate_ids: Vec<String>,
}

#[cfg(target_os = "windows")]
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct RememberedAdmissionResult {
    candidate_id: String,
    member_id: String,
    membership: WorkspaceRepositoryMembershipPresentation,
}

#[cfg(target_os = "windows")]
fn remembered_catalog_presentation(
    snapshot: RememberedWorkspaceStartupState,
) -> RememberedWorkspaceCatalogPresentation {
    let RememberedWorkspaceStartupState::Available(catalog) = snapshot else {
        return RememberedWorkspaceCatalogPresentation {
            status: "unavailable",
            candidates: Vec::new(),
            last_active_candidate_id: None,
        };
    };
    let workspace = catalog.workspace();
    RememberedWorkspaceCatalogPresentation {
        status: "available",
        candidates: workspace
            .members()
            .iter()
            .enumerate()
            .map(|(order, candidate)| RememberedCandidatePresentation {
                candidate_id: candidate.id().as_str().to_owned(),
                label: candidate.label().to_owned(),
                order,
                has_location_hint: candidate.location_hint().is_some(),
            })
            .collect(),
        last_active_candidate_id: workspace
            .last_active_member_id()
            .map(|candidate_id| candidate_id.as_str().to_owned()),
    }
}

#[cfg(target_os = "windows")]
fn remembered_mutation_error(error: RememberedWorkspaceMutationError) -> FrontendError {
    match error {
        RememberedWorkspaceMutationError::Unavailable => {
            FrontendError::RememberedCatalogUnavailable
        }
        RememberedWorkspaceMutationError::StorageFailure => {
            FrontendError::RememberedCatalogSaveFailed
        }
        RememberedWorkspaceMutationError::InvalidCatalog
        | RememberedWorkspaceMutationError::Rejected(_) => {
            if matches!(
                error,
                RememberedWorkspaceMutationError::Rejected(
                    remembered_workspace::ValidationError::CandidateNotFound
                )
            ) {
                FrontendError::RememberedCandidateNotFound
            } else {
                FrontendError::RememberedCatalogRequestInvalid
            }
        }
    }
}

#[cfg(target_os = "windows")]
fn parse_remembered_candidate_id(value: String) -> Result<RememberedCandidateId, FrontendError> {
    RememberedCandidateId::parse(value).map_err(|_| FrontendError::RememberedCandidateIdInvalid)
}

#[cfg(target_os = "windows")]
fn parse_remembered_location_hint(
    value: Option<String>,
) -> Result<Option<RememberedLocationHint>, FrontendError> {
    value
        .map(|value| {
            RememberedLocationHint::parse(PathBuf::from(value))
                .map_err(|_| FrontendError::RememberedCatalogRequestInvalid)
        })
        .transpose()
}

#[cfg(target_os = "windows")]
#[tauri::command]
fn remembered_workspace_catalog(
    state: State<'_, DesktopAppState>,
) -> RememberedWorkspaceCatalogPresentation {
    remembered_catalog_presentation(state.remembered_workspace.snapshot())
}

#[cfg(target_os = "windows")]
fn reveal_remembered_location(
    state: &DesktopAppState,
    candidate_id: RememberedCandidateId,
) -> Result<RememberedLocationPresentation, FrontendError> {
    let location = match state.remembered_workspace.location_hint(&candidate_id) {
        Ok(Some(location)) => location,
        Ok(None) => return Err(FrontendError::RememberedLocationRequired),
        Err(RememberedWorkspaceLookupError::Unavailable) => {
            return Err(FrontendError::RememberedCatalogUnavailable);
        }
        Err(RememberedWorkspaceLookupError::NotFound) => {
            return Err(FrontendError::RememberedCandidateNotFound);
        }
    };
    let location = location
        .to_str()
        .ok_or(FrontendError::RememberedCatalogRequestInvalid)?
        .to_owned();
    Ok(RememberedLocationPresentation {
        candidate_id: candidate_id.as_str().to_owned(),
        location,
    })
}

#[cfg(target_os = "windows")]
#[tauri::command]
fn reveal_remembered_workspace_location(
    state: State<'_, DesktopAppState>,
    candidate_id: String,
) -> Result<RememberedLocationPresentation, FrontendError> {
    let candidate_id = parse_remembered_candidate_id(candidate_id)?;
    reveal_remembered_location(state.inner(), candidate_id)
}

#[cfg(target_os = "windows")]
#[tauri::command]
fn remember_workspace_candidate(
    state: State<'_, DesktopAppState>,
    request: RememberedCandidateAddRequest,
) -> Result<RememberedWorkspaceCatalogPresentation, FrontendError> {
    let location_hint = parse_remembered_location_hint(request.location_hint)?;
    let catalog = state
        .remembered_workspace
        .add_candidate(request.label, location_hint)
        .map_err(remembered_mutation_error)?;
    Ok(remembered_catalog_presentation(
        RememberedWorkspaceStartupState::Available(catalog),
    ))
}

#[cfg(target_os = "windows")]
#[tauri::command]
fn update_remembered_workspace_candidate(
    state: State<'_, DesktopAppState>,
    request: RememberedCandidateUpdateRequest,
) -> Result<RememberedWorkspaceCatalogPresentation, FrontendError> {
    let candidate_id = parse_remembered_candidate_id(request.candidate_id)?;
    let location_hint = match request.location_hint {
        RememberedLocationHintUpdate::Missing => None,
        RememberedLocationHintUpdate::Clear => Some(None),
        RememberedLocationHintUpdate::Set(value) => {
            Some(parse_remembered_location_hint(Some(value))?)
        }
    };
    let catalog = state
        .remembered_workspace
        .update_candidate(candidate_id, request.label, location_hint)
        .map_err(remembered_mutation_error)?;
    Ok(remembered_catalog_presentation(
        RememberedWorkspaceStartupState::Available(catalog),
    ))
}

#[cfg(target_os = "windows")]
#[tauri::command]
fn delete_remembered_workspace_candidate(
    state: State<'_, DesktopAppState>,
    candidate_id: String,
) -> Result<RememberedWorkspaceCatalogPresentation, FrontendError> {
    let candidate_id = parse_remembered_candidate_id(candidate_id)?;
    let catalog = state
        .remembered_workspace
        .delete_candidate(candidate_id)
        .map_err(remembered_mutation_error)?;
    Ok(remembered_catalog_presentation(
        RememberedWorkspaceStartupState::Available(catalog),
    ))
}

#[cfg(target_os = "windows")]
#[tauri::command]
fn reorder_remembered_workspace_candidates(
    state: State<'_, DesktopAppState>,
    request: RememberedCandidateReorderRequest,
) -> Result<RememberedWorkspaceCatalogPresentation, FrontendError> {
    let candidate_ids = request
        .candidate_ids
        .into_iter()
        .map(parse_remembered_candidate_id)
        .collect::<Result<Vec<_>, _>>()?;
    let catalog = state
        .remembered_workspace
        .reorder_candidates(candidate_ids)
        .map_err(remembered_mutation_error)?;
    Ok(remembered_catalog_presentation(
        RememberedWorkspaceStartupState::Available(catalog),
    ))
}

#[cfg(all(target_os = "windows", not(test)))]
async fn admit_remembered_candidate(
    state: &DesktopAppState,
    candidate_id: RememberedCandidateId,
) -> Result<RepositoryMemberId, FrontendError> {
    let location = match state.remembered_workspace.location_hint(&candidate_id) {
        Ok(Some(location)) => location,
        Ok(None) => return Err(FrontendError::RememberedLocationRequired),
        Err(RememberedWorkspaceLookupError::Unavailable) => {
            return Err(FrontendError::RememberedCatalogUnavailable);
        }
        Err(RememberedWorkspaceLookupError::NotFound) => {
            return Err(FrontendError::RememberedCandidateNotFound);
        }
    };
    let git = selected_git_executable()?;
    admit_repository(state, &git, &location).await
}

#[cfg(all(target_os = "windows", test))]
fn admit_remembered_candidate(
    state: &DesktopAppState,
    candidate_id: RememberedCandidateId,
) -> Result<RepositoryMemberId, FrontendError> {
    let location = match state.remembered_workspace.location_hint(&candidate_id) {
        Ok(Some(location)) => location,
        Ok(None) => return Err(FrontendError::RememberedLocationRequired),
        Err(RememberedWorkspaceLookupError::Unavailable) => {
            return Err(FrontendError::RememberedCatalogUnavailable);
        }
        Err(RememberedWorkspaceLookupError::NotFound) => {
            return Err(FrontendError::RememberedCandidateNotFound);
        }
    };
    let git = selected_git_executable()?;
    admit_repository(state, &git, &location)
}

#[cfg(target_os = "windows")]
#[tauri::command]
async fn admit_remembered_workspace_candidate(
    state: State<'_, DesktopAppState>,
    candidate_id: String,
) -> Result<RememberedAdmissionResult, FrontendError> {
    let candidate_id = parse_remembered_candidate_id(candidate_id)?;
    #[cfg(not(test))]
    let member_id = admit_remembered_candidate(state.inner(), candidate_id.clone()).await?;
    #[cfg(test)]
    let member_id = admit_remembered_candidate(state.inner(), candidate_id.clone())?;
    Ok(RememberedAdmissionResult {
        candidate_id: candidate_id.as_str().to_owned(),
        member_id: member_id.selector(),
        membership: repository_membership_presentation(state.inner()),
    })
}

#[cfg(target_os = "windows")]
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct RepositoryMemberPresentation {
    member_id: String,
    display_name: String,
    active: bool,
    availability: &'static str,
}

#[cfg(target_os = "windows")]
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct WorkspaceRepositoryMembershipPresentation {
    members: Vec<RepositoryMemberPresentation>,
    active_member_id: Option<String>,
    membership_generation: u64,
}

#[cfg(target_os = "windows")]
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum RepositoryMemberRemovalOutcomePresentation {
    Removed,
}

#[cfg(target_os = "windows")]
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct RepositoryMemberRemovalResult {
    outcome: RepositoryMemberRemovalOutcomePresentation,
    membership: WorkspaceRepositoryMembershipPresentation,
}

#[cfg(target_os = "windows")]
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum RepositoryActivationOutcomePresentation {
    Activated,
    AlreadyActive,
}

#[cfg(target_os = "windows")]
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct RepositoryActivationResult {
    outcome: RepositoryActivationOutcomePresentation,
    membership: WorkspaceRepositoryMembershipPresentation,
}

#[cfg(target_os = "windows")]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CloseRepositoryRequest {
    expected_active_member_id: String,
    expected_repository_generation: u64,
}

#[cfg(target_os = "windows")]
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum CloseRepositoryOutcomePresentation {
    Closed,
}

#[cfg(target_os = "windows")]
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum CloseRepositoryError {
    NoActive,
    ActiveChanged,
    ConnectedOrRuntimeBusy,
    ModelTurnBusy,
    RepositoryEffectBusy,
    InvalidGuard,
    Unavailable,
}

#[cfg(target_os = "windows")]
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct CloseRepositoryResult {
    outcome: CloseRepositoryOutcomePresentation,
    status: &'static str,
    membership: WorkspaceRepositoryMembershipPresentation,
}

#[cfg(target_os = "windows")]
fn bounded_repository_display_name(root: &Path) -> String {
    const MAX_DISPLAY_NAME_CHARS: usize = 128;
    let name = root
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("Repository");
    name.chars().take(MAX_DISPLAY_NAME_CHARS).collect()
}

#[cfg(target_os = "windows")]
fn repository_membership_presentation(
    state: &DesktopAppState,
) -> WorkspaceRepositoryMembershipPresentation {
    let membership = state
        .workspace_membership
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    repository_membership_presentation_from(&membership)
}

#[cfg(target_os = "windows")]
fn repository_membership_presentation_from(
    membership: &WorkspaceMembershipState,
) -> WorkspaceRepositoryMembershipPresentation {
    let active_member = membership.active_member();
    let members = membership
        .members()
        .map(|member| RepositoryMemberPresentation {
            member_id: member.id.selector(),
            display_name: bounded_repository_display_name(&member.root),
            active: active_member == Some(member.id),
            availability: if active_member == Some(member.id) {
                "active"
            } else {
                "inactive"
            },
        })
        .collect();
    WorkspaceRepositoryMembershipPresentation {
        members,
        active_member_id: active_member.map(RepositoryMemberId::selector),
        membership_generation: membership.membership_generation(),
    }
}

#[cfg(target_os = "windows")]
fn close_state_lock<T>(mutex: &Mutex<T>) -> Result<MutexGuard<'_, T>, CloseRepositoryError> {
    mutex.lock().map_err(|_| CloseRepositoryError::Unavailable)
}

#[cfg(target_os = "windows")]
fn close_repository_transition(
    state: &DesktopAppState,
    request: CloseRepositoryRequest,
) -> Result<CloseRepositoryResult, CloseRepositoryError> {
    let expected_member_id = RepositoryMemberId::parse_selector(&request.expected_active_member_id)
        .filter(|_| request.expected_repository_generation > 0)
        .ok_or(CloseRepositoryError::InvalidGuard)?;

    // Match activation and member removal: membership coordination is always
    // acquired before lifecycle coordination. Both are held through the final
    // active-member publication below.
    let _membership_coordination = close_state_lock(&state.membership_coordination)?;
    let _lifecycle_coordination = close_state_lock(&state.lifecycle_coordination)?;

    let mut membership = close_state_lock(&state.workspace_membership)?;
    if membership.member(expected_member_id).is_none() {
        return Err(CloseRepositoryError::InvalidGuard);
    }
    let Some(active_member_id) = membership.active_member() else {
        let repository_present = close_state_lock(&state.repository)?.is_some();
        let commit_present = close_state_lock(&state.commit_capability)?.is_some();
        let workflow = close_state_lock(&state.repository_workflow)?;
        let workflow_present = repository_workflow_has_state(&workflow);
        let index_effect_present =
            close_state_lock(&state.repository_index_effect_reservation)?.is_some();
        let host_owner_present =
            close_state_lock(&state.host_invocation)?.state() != CoordinatorState::Idle;
        let chat_active = close_model_turn_is_active(
            close_state_lock(&state.host_invocation)?.state(),
            *close_state_lock(&state.chat)?,
            close_state_lock(&state.active_chat)?.is_some(),
        );
        let connection = close_state_lock(&state.connection)?;
        let provider_activation_present = close_state_lock(&state.provider_activation)?.is_some();
        let runtime_state_present = matches!(
            &*connection,
            ConnectionState::Connecting
                | ConnectionState::Connected { .. }
                | ConnectionState::Disconnecting
        );
        let provider_activation_incoherent = provider_activation_present
            && !matches!(&*connection, ConnectionState::Connected { .. });
        if repository_present
            || commit_present
            || workflow_present
            || index_effect_present
            || host_owner_present
            || chat_active
            || runtime_state_present
            || provider_activation_incoherent
        {
            return Err(CloseRepositoryError::Unavailable);
        }
        return Err(CloseRepositoryError::NoActive);
    };
    if active_member_id != expected_member_id {
        return Err(CloseRepositoryError::ActiveChanged);
    }
    let active_member = membership
        .member(active_member_id)
        .ok_or(CloseRepositoryError::Unavailable)?;
    let repository = close_state_lock(&state.repository)?
        .clone()
        .ok_or(CloseRepositoryError::Unavailable)?;
    if repository.root != active_member.root {
        return Err(CloseRepositoryError::Unavailable);
    }
    drop(repository);
    let current_repository_generation = *close_state_lock(&state.repository_generation)?;
    if current_repository_generation == 0 {
        return Err(CloseRepositoryError::Unavailable);
    }
    if current_repository_generation != request.expected_repository_generation {
        return Err(CloseRepositoryError::ActiveChanged);
    }

    let coordinator_state = close_state_lock(&state.host_invocation)?.state();
    let chat_state = *close_state_lock(&state.chat)?;
    let active_chat_present = close_state_lock(&state.active_chat)?.is_some();
    let model_turn_active =
        close_model_turn_is_active(coordinator_state, chat_state, active_chat_present);
    if model_turn_active {
        return Err(CloseRepositoryError::ModelTurnBusy);
    }

    let index_effect_active =
        close_state_lock(&state.repository_index_effect_reservation)?.is_some();
    if index_effect_active || coordinator_state == CoordinatorState::HostRunning {
        return Err(CloseRepositoryError::RepositoryEffectBusy);
    }

    let connection = close_state_lock(&state.connection)?;
    let provider_activation_present = close_state_lock(&state.provider_activation)?.is_some();
    if matches!(&*connection, ConnectionState::NotConnected) && provider_activation_present {
        return Err(CloseRepositoryError::Unavailable);
    }
    if !close_runtime_is_disconnected(
        close_connection_state(&connection),
        provider_activation_present,
    ) {
        return Err(CloseRepositoryError::ConnectedOrRuntimeBusy);
    }
    drop(connection);

    let next_generation = next_repository_generation(current_repository_generation)
        .ok_or(CloseRepositoryError::Unavailable)?;
    let commit_control = close_state_lock(&state.commit_capability)?
        .as_ref()
        .map(|capability| Arc::clone(&capability.control));

    // Acquire every mutex needed by the mutation before the last fallible
    // operation. Poisoning or contention therefore cannot leave a partial
    // withdrawal after authority revocation begins.
    let mut repository = close_state_lock(&state.repository)?;
    let mut repository_generation = close_state_lock(&state.repository_generation)?;
    let mut repository_workflow = close_state_lock(&state.repository_workflow)?;
    let mut host_invocation = close_state_lock(&state.host_invocation)?;
    let mut commit_capability = close_state_lock(&state.commit_capability)?;
    let mut conversation = close_state_lock(&state.conversation)?;

    // This nonblocking clear is the final fallible operation. Everything
    // below is an in-memory, synchronous publication under both gates.
    if commit_control.is_some_and(|control| !control.try_clear_authorization_now()) {
        return Err(CloseRepositoryError::RepositoryEffectBusy);
    }

    host_invocation.clear_prepared();
    commit_capability.take();
    *repository_workflow = RepositoryWorkflowState::default();
    repository.take();
    *repository_generation = next_generation;
    conversation.start_new();
    let deactivation = membership.deactivate_expected(expected_member_id);
    debug_assert!(matches!(
        deactivation,
        WorkspaceMembershipDeactivation::Deactivated
    ));
    Ok(CloseRepositoryResult {
        outcome: CloseRepositoryOutcomePresentation::Closed,
        status: "Repository closed. No repository is active.",
        membership: repository_membership_presentation_from(&membership),
    })
}

#[cfg(target_os = "windows")]
#[tauri::command]
fn close_repository(
    state: State<'_, DesktopAppState>,
    request: CloseRepositoryRequest,
) -> Result<CloseRepositoryResult, CloseRepositoryError> {
    close_repository_transition(state.inner(), request)
}

#[cfg(target_os = "windows")]
#[tauri::command]
fn repository_membership(
    state: State<'_, DesktopAppState>,
) -> WorkspaceRepositoryMembershipPresentation {
    repository_membership_presentation(state.inner())
}

#[cfg(target_os = "windows")]
fn repository_workflow_has_state(workflow: &RepositoryWorkflowState) -> bool {
    workflow.observation_generation != 0
        || workflow.next_action != 0
        || !workflow.actions.is_empty()
        || workflow.review.is_some()
        || workflow.commit_review.is_some()
        || workflow.review_selector.is_some()
        || workflow.authorization != CommitAuthorizationPresentation::ReviewRequired
}

#[cfg(target_os = "windows")]
fn inactive_repository_member_removal_is_busy(
    state: &DesktopAppState,
    target_member_id: RepositoryMemberId,
    active_member_id: Option<RepositoryMemberId>,
) -> bool {
    if let Some(reservation) = state
        .repository_index_effect_reservation
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .as_ref()
    {
        // A current reservation is attributable to its active member. Any
        // target-bound reservation, or an owner that is no longer current,
        // fails closed without clearing the external-effect owner.
        if reservation.member_id == Some(target_member_id)
            || !repository_index_effect_binding_is_current(state, reservation)
        {
            return true;
        }
    }

    if active_member_id.is_some() {
        return false;
    }

    // Executable lifecycle state without an active member cannot be safely
    // attributed to an unrelated inactive target.
    if state
        .repository
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .is_some()
        || state
            .commit_capability
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_some()
        || repository_workflow_has_state(
            &state
                .repository_workflow
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        )
        || state
            .host_invocation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .state()
            != CoordinatorState::Idle
        || state
            .provider_activation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_some()
        || !matches!(
            *state
                .connection
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
            ConnectionState::NotConnected
        )
        || !matches!(
            *state
                .chat
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
            ChatState::Idle
        )
        || state
            .active_chat
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_some()
    {
        return true;
    }

    false
}

#[cfg(target_os = "windows")]
fn remove_repository_member_selector(
    state: &DesktopAppState,
    selector: &str,
) -> Result<RepositoryMemberRemovalResult, FrontendError> {
    let member_id = RepositoryMemberId::parse_selector(selector)
        .ok_or(FrontendError::RepositoryMemberSelectorInvalid)?;

    // Removal and activation/admission share the same ordering. The final
    // membership checks and the sole membership mutation occur under both
    // coordination locks, so activation cannot publish an absent member.
    let _membership_coordination = state
        .membership_coordination
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let _lifecycle_coordination = state
        .lifecycle_coordination
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);

    let active_member_id = {
        let membership = state
            .workspace_membership
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if membership.member(member_id).is_none() {
            return Err(FrontendError::RepositoryMemberNotFound);
        }
        membership.active_member()
    };
    if active_member_id == Some(member_id) {
        return Err(FrontendError::RepositoryMemberActive);
    }
    if inactive_repository_member_removal_is_busy(state, member_id, active_member_id) {
        return Err(FrontendError::RepositoryBusy);
    }

    let removal = state
        .workspace_membership
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .remove(member_id);
    match removal {
        WorkspaceMembershipRemoval::Removed => Ok(RepositoryMemberRemovalResult {
            outcome: RepositoryMemberRemovalOutcomePresentation::Removed,
            membership: repository_membership_presentation(state),
        }),
        WorkspaceMembershipRemoval::Active => Err(FrontendError::RepositoryMemberActive),
        WorkspaceMembershipRemoval::NotFound => Err(FrontendError::RepositoryMemberNotFound),
    }
}

#[cfg(target_os = "windows")]
#[tauri::command]
fn remove_repository_member(
    state: State<'_, DesktopAppState>,
    member_id: String,
) -> Result<RepositoryMemberRemovalResult, FrontendError> {
    remove_repository_member_selector(state.inner(), &member_id)
}

#[cfg(target_os = "windows")]
async fn activate_repository_member_selector(
    state: &DesktopAppState,
    selector: &str,
) -> Result<RepositoryActivationResult, FrontendError> {
    let member_id = RepositoryMemberId::parse_selector(selector)
        .ok_or(FrontendError::RepositoryMemberSelectorInvalid)?;
    let outcome = activate_admitted_member(state, member_id).await?;
    Ok(RepositoryActivationResult {
        outcome: match outcome {
            ActivationOutcome::Activated => RepositoryActivationOutcomePresentation::Activated,
            ActivationOutcome::AlreadyActive => {
                RepositoryActivationOutcomePresentation::AlreadyActive
            }
        },
        membership: repository_membership_presentation(state),
    })
}

#[cfg(target_os = "windows")]
#[tauri::command]
async fn activate_repository_member(
    state: State<'_, DesktopAppState>,
    member_id: String,
) -> Result<RepositoryActivationResult, FrontendError> {
    activate_repository_member_selector(state.inner(), &member_id).await
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
    #[cfg(not(test))]
    let member_id = admit_repository(state.inner(), &git, &path).await?;
    #[cfg(test)]
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
struct CommitAuthorizationCapture {
    repository_generation: u64,
    model_generation: u64,
    identity_generation: u64,
    observation_generation: u64,
    review_id: String,
    control: Arc<RepositoryCommitControl>,
}

#[cfg(target_os = "windows")]
fn commit_authorization_capture_is_current(
    state: &DesktopAppState,
    capture: &CommitAuthorizationCapture,
) -> bool {
    if *state
        .repository_generation
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        != capture.repository_generation
        || state
            .model
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .generation
            != capture.model_generation
        || *state
            .commit_identity_generation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            != capture.identity_generation
        || matches!(
            *state
                .connection
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
            ConnectionState::Disconnecting
        )
    {
        return false;
    }
    let capability_current = state
        .commit_capability
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .as_ref()
        .is_some_and(|capability| {
            capability.repository_generation == capture.repository_generation
                && capability.model_generation == capture.model_generation
                && capability.identity_generation == capture.identity_generation
                && Arc::ptr_eq(&capability.control, &capture.control)
        });
    let workflow_current = state
        .repository_workflow
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let review_current = workflow_current.review.as_ref().is_some_and(|review| {
        review.repository_generation == capture.repository_generation
            && review.observation_generation == capture.observation_generation
            && review.complete
            && review.binary_supported
    });
    capability_current
        && review_current
        && workflow_current.observation_generation == capture.observation_generation
        && workflow_current.review_selector.as_deref() == Some(capture.review_id.as_str())
}

#[cfg(target_os = "windows")]
async fn authorize_repository_commit_review(
    state: &DesktopAppState,
    review_id: &str,
) -> Result<CommitAuthorizationResult, FrontendError> {
    // Capture and supersession are synchronous. The lifecycle guard is
    // released before review validation so no blocking guard crosses await.
    let (capture, review) = {
        let _lifecycle_coordination = state
            .lifecycle_coordination
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if repository_index_effect_is_active(state) {
            return Err(FrontendError::RepositoryBusy);
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
        // Every explicit human attempt supersedes an older approval, but the
        // shared slot is cleared before any review is taken from workflow.
        if !control.try_clear_authorization_now() {
            return Err(FrontendError::RepositoryBusy);
        }
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
        let review = match workflow.commit_review.take() {
            Some(review) => review,
            None => {
                workflow.authorization = CommitAuthorizationPresentation::ReviewStale;
                return Err(FrontendError::CommitAuthorizationStale);
            }
        };
        (
            CommitAuthorizationCapture {
                repository_generation,
                model_generation,
                identity_generation,
                observation_generation: workflow.observation_generation,
                review_id: review_id.to_owned(),
                control,
            },
            review,
        )
    };

    let prepared = match capture.control.prepare_reviewed_authorization(review).await {
        Ok(prepared) => prepared,
        Err(_) => {
            let _lifecycle_coordination = state
                .lifecycle_coordination
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if commit_authorization_capture_is_current(state, &capture) {
                state
                    .repository_workflow
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .authorization = CommitAuthorizationPresentation::ReviewStale;
            }
            return Err(FrontendError::CommitAuthorizationFailed);
        }
    };
    authorization_pre_publication_barrier(state);

    let _lifecycle_coordination = state
        .lifecycle_coordination
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if !commit_authorization_capture_is_current(state, &capture) {
        return Err(FrontendError::CommitAuthorizationStale);
    }
    match capture
        .control
        .try_install_prepared_authorization_now(prepared)
    {
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
        Err(PreparedRepositoryCommitAuthorizationError::PendingSlotBusy) => {
            Err(FrontendError::RepositoryBusy)
        }
        Err(PreparedRepositoryCommitAuthorizationError::ControlMismatch) => {
            Err(FrontendError::CommitAuthorizationStale)
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
    let (reservation, action) = begin_repository_index_effect(state, &action_id, kind)?;
    if !target_is_current(&action.target_observation) {
        complete_repository_index_effect(state, &reservation).await?;
        return Err(FrontendError::RepositoryActionStale);
    }
    index_effect_pre_execution_barrier(state);
    let result = match reservation.kind {
        RepositoryIndexActionKind::Stage => match GitStageTool::new(
            &reservation.repository.git_executable,
            &reservation.repository.root,
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
            &reservation.repository.git_executable,
            &reservation.repository.root,
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
    complete_repository_index_effect(state, &reservation).await?;
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
        let search = RepositorySearchTool::new(&repository.git_executable, &repository.root)?;
        let list = RepositoryListTool::new(&repository.git_executable, &repository.root)?;
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
        registry.register(Arc::new(search))?;
        registry.register(Arc::new(list))?;
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
fn begin_connect(state: &DesktopAppState) -> Result<ConnectRequest, FrontendError> {
    let _lifecycle_coordination = state
        .lifecycle_coordination
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if state
        .host_invocation
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .state()
        != CoordinatorState::Idle
    {
        return Err(FrontendError::HostInvocationBusy);
    }
    let mut connection = state
        .connection
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if matches!(
        *connection,
        ConnectionState::NotConnected | ConnectionState::Error(_)
    ) && repository_index_effect_is_active(state)
    {
        return Err(FrontendError::RepositoryBusy);
    }
    Ok(request_connect(&mut connection))
}

#[cfg(target_os = "windows")]
#[tauri::command]
async fn connect_codex(
    state: State<'_, DesktopAppState>,
) -> Result<ConnectionResult, FrontendError> {
    match begin_connect(state.inner())? {
        ConnectRequest::AlreadyConnected => return Ok(ConnectionResult::connected()),
        ConnectRequest::InProgress => return Ok(ConnectionResult::connecting()),
        ConnectRequest::Start => {}
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
        .map(|(tool, control)| DesktopCommitCapability {
            repository_generation,
            model_generation,
            identity_generation,
            _tool: Arc::new(tool),
            control: Arc::new(control),
        }),
        _ => None,
    };
    let commit_tool = commit_capability
        .as_ref()
        .map(|capability| Arc::clone(&capability._tool));
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
                "identity_generation": identity_generation,
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
            connect_pre_publication_barrier(state.inner());
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
            let current_identity_generation = *state
                .commit_identity_generation
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let stale = !connection_publication_is_current(
                ConnectionPublicationCurrentness {
                    repository_generation,
                    model_generation,
                    profile_generation,
                    connection_generation,
                    identity_generation,
                },
                ConnectionPublicationCurrentness {
                    repository_generation: current_repository_generation,
                    model_generation: current_model_generation,
                    profile_generation: current_profile_generation,
                    connection_generation: current_connection_generation,
                    identity_generation: current_identity_generation,
                },
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
                    "captured_identity_generation": identity_generation,
                    "current_identity_generation": current_identity_generation,
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
                identity_generation,
                repository_fingerprint,
                composition: Arc::clone(&composition),
                allowed_permissions: retained_allowed_permissions,
                commit_capability,
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
                    ProviderPublicationRejectionReason::IndexEffectActive => {
                        let mut connection = state
                            .connection
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner);
                        if matches!(*connection, ConnectionState::Connecting) {
                            *connection = ConnectionState::NotConnected;
                        }
                        Err(FrontendError::RepositoryBusy)
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
    let runtime = {
        let _lifecycle_coordination = state
            .lifecycle_coordination
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if repository_index_effect_is_active(state.inner()) {
            return Err(FrontendError::RepositoryBusy);
        }
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
    revoke_repository_commit_context(state.inner()).await;

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

    let _lifecycle_coordination = state
        .lifecycle_coordination
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
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
    match connection {
        ConnectionState::NotConnected | ConnectionState::Error(_) => Ok(()),
        ConnectionState::Connecting
        | ConnectionState::Connected { .. }
        | ConnectionState::Disconnecting => Err(FrontendError::RepositoryBusy),
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
) -> bool {
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
    refresh
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
    let mut retain_model_owner = false;
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
                retain_model_owner |=
                    handle_uncertain_repository_effects(&app, &mut tool_calls, repository_selected)
                        .await;
                if !app.state::<DesktopAppState>().claim_terminal(
                    chat_generation,
                    &runtime,
                    &session_id,
                ) {
                    if !retain_model_owner {
                        app.state::<DesktopAppState>()
                            .finish_claimed_chat(chat_generation);
                    }
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
                retain_model_owner |=
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
                    if !retain_model_owner {
                        app.state::<DesktopAppState>()
                            .finish_claimed_chat(chat_generation);
                    }
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
                retain_model_owner |=
                    handle_uncertain_repository_effects(&app, &mut tool_calls, repository_selected)
                        .await;
                if !app.state::<DesktopAppState>().claim_terminal(
                    chat_generation,
                    &runtime,
                    &session_id,
                ) {
                    if !retain_model_owner {
                        app.state::<DesktopAppState>()
                            .finish_claimed_chat(chat_generation);
                    }
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
    retain_model_owner |=
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
    if !retain_model_owner {
        app.state::<DesktopAppState>()
            .finish_claimed_chat(chat_generation);
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
            remembered_workspace_catalog,
            reveal_remembered_workspace_location,
            remember_workspace_candidate,
            update_remembered_workspace_candidate,
            delete_remembered_workspace_candidate,
            reorder_remembered_workspace_candidates,
            admit_remembered_workspace_candidate,
            repository_membership,
            remove_repository_member,
            activate_repository_member,
            close_repository,
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
#[path = "main_tests.rs"]
mod tests;
