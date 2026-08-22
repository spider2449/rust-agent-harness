//! Supervised process-plugin implementations of the provider-neutral RAH tool boundary.
//!
//! Process supervision, environment minimization, and an isolated working directory
//! reduce ambient authority. They are not OS sandboxing.

use std::{
    collections::{HashMap, HashSet, VecDeque},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, SystemTime},
};

use async_trait::async_trait;
use rah_protocol::{PermissionLevel, ToolContent, ToolDefinition, ToolInput, ToolName, ToolOutput};
use rah_tools::{
    ExternalToolIdentity, ExternalToolPermissionError, ExternalToolPermissionPolicy, Tool,
    ToolContext, ToolError,
};
use serde::Deserialize;
use serde_json::{Value, json};
use thiserror::Error;
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
    process::{Child, ChildStdin, Command},
    sync::{OwnedSemaphorePermit, Semaphore, mpsc, oneshot},
    task::JoinHandle,
    time::timeout,
};

/// Exact RAH-owned process-plugin protocol implemented by this prototype.
pub const PLUGIN_PROTOCOL_VERSION: &str = "1";

const STARTUP_TIMEOUT: Duration = Duration::from_secs(2);
const SHUTDOWN_TIMEOUT: Duration = Duration::from_millis(500);
// Enough reserved capacity for every outstanding request plus lifecycle control.
const CONTROL_QUEUE_CAPACITY: usize = 64;
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(1);

/// Resource limits applied at the untrusted process boundary.
#[derive(Clone, Debug)]
pub struct PluginLimits {
    /// Maximum correlated requests awaiting responses.
    pub max_outstanding: usize,
    /// Maximum queued normal supervisor commands.
    pub command_queue: usize,
    /// Maximum bytes in one inbound or outbound NDJSON message.
    pub max_message_bytes: usize,
    /// Maximum serialized bytes in a tool result.
    pub max_result_bytes: usize,
    /// Maximum retained stderr bytes.
    pub max_stderr_bytes: usize,
}

impl Default for PluginLimits {
    fn default() -> Self {
        Self {
            max_outstanding: 32,
            command_queue: 64,
            max_message_bytes: 1024 * 1024,
            max_result_bytes: 1024 * 1024,
            max_stderr_bytes: 64 * 1024,
        }
    }
}

/// Trusted host configuration for one explicitly selected process plugin.
#[derive(Clone, Debug)]
pub struct PluginConfig {
    plugin_id: String,
    plugin_version: String,
    protocol_version: String,
    program: PathBuf,
    args: Vec<String>,
    environment: Vec<(String, String)>,
    call_timeout: Duration,
    limits: PluginLimits,
    tool_permissions: ExternalToolPermissionPolicy,
    expected_names: HashSet<String>,
    expected: HashMap<String, PluginExpectedTool>,
}

/// One host-owned admission contract for a discovered process-plugin tool.
#[derive(Clone, Debug)]
pub struct PluginExpectedTool {
    schema: Value,
    permission: PermissionLevel,
}

impl PluginConfig {
    /// Creates an explicit stdio process configuration.
    pub fn stdio(
        plugin_id: impl Into<String>,
        plugin_version: impl Into<String>,
        program: impl Into<PathBuf>,
    ) -> Result<Self, PluginAdapterError> {
        let plugin_id = plugin_id.into();
        validate_component(&plugin_id, "plugin ID")?;
        let plugin_version = plugin_version.into();
        if plugin_version.is_empty() || plugin_version.len() > 128 {
            return Err(invalid_configuration(
                "plugin version must contain 1-128 characters",
            ));
        }
        let program = program.into();
        if program.as_os_str().is_empty() {
            return Err(invalid_configuration("plugin executable must not be empty"));
        }
        Ok(Self {
            plugin_id,
            plugin_version,
            protocol_version: PLUGIN_PROTOCOL_VERSION.to_owned(),
            program,
            args: Vec::new(),
            environment: Vec::new(),
            call_timeout: Duration::from_secs(30),
            limits: PluginLimits::default(),
            tool_permissions: ExternalToolPermissionPolicy::new(),
            expected_names: HashSet::new(),
            expected: HashMap::new(),
        })
    }

    /// Appends one literal process argument. No shell is involved.
    #[must_use]
    pub fn with_arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }

    /// Pins the exact protocol version expected from the child.
    #[must_use]
    pub fn with_protocol_version(mut self, version: impl Into<String>) -> Self {
        self.protocol_version = version.into();
        self
    }

    /// Overrides the per-tool-call timeout.
    #[must_use]
    pub fn with_call_timeout(mut self, call_timeout: Duration) -> Self {
        self.call_timeout = call_timeout;
        self
    }

    /// Adds one exact environment name and value after the inherited environment is cleared.
    pub fn with_environment(
        mut self,
        name: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<Self, PluginAdapterError> {
        let name = name.into();
        if name.is_empty()
            || name.contains('=')
            || name.contains('\0')
            || self
                .environment
                .iter()
                .any(|(existing, _)| existing == &name)
        {
            return Err(invalid_configuration(
                "environment names must be non-empty, unique, and contain neither `=` nor NUL",
            ));
        }
        self.environment.push((name, value.into()));
        Ok(self)
    }

    /// Assigns the trusted host permission for one remote tool.
    pub fn with_tool_permission(
        mut self,
        remote_tool_name: impl Into<String>,
        permission: PermissionLevel,
    ) -> Result<Self, PluginAdapterError> {
        let remote_tool_name = remote_tool_name.into();
        validate_component(&remote_tool_name, "remote tool name")?;
        let identity = external_identity(&self.plugin_id, &remote_tool_name)?;
        self.tool_permissions
            .assign(identity, permission)
            .map_err(permission_configuration_error)?;
        self.expected_names.insert(remote_tool_name);
        Ok(self)
    }

    /// Pins one remote tool's exact host-owned schema and permission.
    ///
    /// Discovery succeeds only when its complete tool set equals the configured
    /// expected names. JSON object key order is normalized before comparison.
    pub fn with_expected_tool(
        mut self,
        remote_tool_name: impl Into<String>,
        input_schema: Value,
        permission: PermissionLevel,
    ) -> Result<Self, PluginAdapterError> {
        let remote_tool_name = remote_tool_name.into();
        validate_component(&remote_tool_name, "remote tool name")?;
        validate_schema_object(&input_schema)?;
        if self.expected.contains_key(&remote_tool_name) {
            return Err(invalid_configuration(
                "expected process-plugin tool configured more than once",
            ));
        }
        let identity = external_identity(&self.plugin_id, &remote_tool_name)?;
        self.tool_permissions
            .assign(identity, permission)
            .map_err(permission_configuration_error)?;
        self.expected_names.insert(remote_tool_name.clone());
        self.expected.insert(
            remote_tool_name,
            PluginExpectedTool {
                schema: input_schema,
                permission,
            },
        );
        Ok(self)
    }

    /// Applies caller-selected limits after checking every prototype hard maximum.
    pub fn with_limits(mut self, limits: PluginLimits) -> Result<Self, PluginAdapterError> {
        validate_limits(&limits)?;
        self.limits = limits;
        Ok(self)
    }
}

/// Bounded, host-only diagnostic snapshot. It must not be made model-visible.
#[derive(Clone, Debug, Default)]
pub struct PluginDiagnostics {
    /// Lossy, control-escaped tail of child stderr.
    pub stderr: String,
    /// Number of old stderr bytes discarded to retain the fixed bound.
    pub truncated_bytes: u64,
    /// Isolated working directory assigned to the child.
    pub cwd: PathBuf,
}

/// Adapter construction or lifecycle failure outside the RAH tool API.
#[derive(Debug, Error)]
pub enum PluginAdapterError {
    /// Trusted host configuration is invalid.
    #[error("invalid process plugin configuration: {message}")]
    InvalidConfiguration { message: String },
    /// The configured child could not be started.
    #[error("failed to start process plugin: {message}")]
    Startup { message: String },
    /// Handshake, discovery, identity, or protocol validation failed.
    #[error("process plugin initialization failed: {message}")]
    Initialization { message: String },
    /// The owned process could not be shut down and reaped cleanly.
    #[error("failed to shut down process plugin: {message}")]
    Shutdown { message: String },
}

/// One connected plugin generation and its immutable RAH tool proxies.
pub struct PluginAdapter {
    client: Client,
    tools: Vec<Arc<dyn Tool>>,
    actor: Mutex<Option<JoinHandle<()>>>,
    diagnostics: Arc<Mutex<DiagnosticBuffer>>,
    cwd: PathBuf,
}

impl std::fmt::Debug for PluginAdapter {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PluginAdapter")
            .field("tools", &self.tools.len())
            .field("cwd", &self.cwd)
            .finish_non_exhaustive()
    }
}

impl PluginAdapter {
    /// Spawns, handshakes, validates identity, and discovers one plugin generation.
    pub async fn connect(config: PluginConfig) -> Result<Self, PluginAdapterError> {
        validate_limits(&config.limits)?;
        if config.protocol_version != PLUGIN_PROTOCOL_VERSION {
            return Err(invalid_configuration(
                "the prototype requires process plugin protocol version `1`",
            ));
        }
        let executable = ExecutableIdentity::capture(&config.program)?;
        let cwd = create_isolated_cwd().await?;
        executable.revalidate()?;
        let diagnostics = Arc::new(Mutex::new(DiagnosticBuffer::new(
            config.limits.max_stderr_bytes,
        )));
        let mut command = Command::new(&executable.path);
        command
            .args(&config.args)
            .current_dir(&cwd)
            .env_clear()
            .env("RAH_PLUGIN_PROTOCOL", PLUGIN_PROTOCOL_VERSION)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);
        #[cfg(windows)]
        if let Some(system_root) = std::env::var_os("SystemRoot") {
            command.env("SystemRoot", system_root);
        }
        for (name, value) in &config.environment {
            command.env(name, value);
        }
        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(error) => {
                let _ = tokio::fs::remove_dir(&cwd).await;
                return Err(PluginAdapterError::Startup {
                    message: error.to_string(),
                });
            }
        };
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| PluginAdapterError::Startup {
                message: "child stdin was not captured".to_owned(),
            })?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| PluginAdapterError::Startup {
                message: "child stdout was not captured".to_owned(),
            })?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| PluginAdapterError::Startup {
                message: "child stderr was not captured".to_owned(),
            })?;

        let (commands, command_receiver) = mpsc::channel(config.limits.command_queue);
        let (control, control_receiver) = mpsc::channel(CONTROL_QUEUE_CAPACITY);
        let client = Client {
            commands,
            control,
            next_id: Arc::new(AtomicU64::new(1)),
            next_execution: Arc::new(AtomicU64::new(1)),
            capacity: Arc::new(Semaphore::new(config.limits.max_outstanding)),
            cancellation_supported: Arc::new(AtomicBool::new(false)),
            call_timeout: config.call_timeout,
        };
        let stderr_task = tokio::spawn(drain_stderr(stderr, Arc::clone(&diagnostics)));
        let actor_cwd = cwd.clone();
        let max_message_bytes = config.limits.max_message_bytes;
        let actor = tokio::spawn(run_actor(ActorResources {
            child,
            stdin,
            stdout: Box::new(stdout),
            command_receiver,
            control_receiver,
            max_message_bytes,
            stderr_task,
            cwd: actor_cwd,
        }));

        let startup = async {
            let initialized = client
                .request(
                    "initialize",
                    json!({
                        "protocol_versions": [PLUGIN_PROTOCOL_VERSION],
                        "configured_plugin_id": config.plugin_id,
                        "host": {"name": "rah-tools-plugin", "version": env!("CARGO_PKG_VERSION")},
                        "capabilities": {"cancellation": true}
                    }),
                    STARTUP_TIMEOUT,
                    None,
                )
                .await
                .map_err(initialization_error)?;
            let supports_cancellation = validate_handshake(&config, initialized)?;
            client
                .cancellation_supported
                .store(supports_cancellation, Ordering::Release);
            client
                .notify("initialized", json!({}))
                .map_err(initialization_error)?;
            let listed = client
                .request("tools/list", json!({}), STARTUP_TIMEOUT, None)
                .await
                .map_err(initialization_error)?;
            map_tools(&config, listed, &client)
        }
        .await;

        match startup {
            Ok(tools) => Ok(Self {
                client,
                tools,
                actor: Mutex::new(Some(actor)),
                diagnostics,
                cwd,
            }),
            Err(error) => {
                let _ = client.control.try_send(ControlCommand::Stop);
                let _ = timeout(SHUTDOWN_TIMEOUT * 2, actor).await;
                Err(error)
            }
        }
    }

    /// Returns cloneable proxies suitable for the ordinary `ToolRegistry`.
    #[must_use]
    pub fn tools(&self) -> Vec<Arc<dyn Tool>> {
        self.tools.clone()
    }

    /// Returns bounded host-only diagnostics.
    #[must_use]
    pub fn diagnostics(&self) -> PluginDiagnostics {
        let (stderr, truncated_bytes) = self
            .diagnostics
            .lock()
            .map(|buffer| (buffer.sanitized(), buffer.truncated_bytes))
            .unwrap_or_else(|_| (String::new(), 0));
        PluginDiagnostics {
            stderr,
            truncated_bytes,
            cwd: self.cwd.clone(),
        }
    }

    /// Requests graceful shutdown, then guarantees termination and reaping.
    pub async fn shutdown(self) -> Result<(), PluginAdapterError> {
        let _ = self
            .client
            .request("shutdown", json!({}), SHUTDOWN_TIMEOUT, None)
            .await;
        // A closed control queue means the supervisor already observed process
        // termination or a protocol failure. Awaiting its handle still proves reap.
        let _ = self.client.control.try_send(ControlCommand::Stop);
        let actor = self
            .actor
            .lock()
            .map_err(|_| PluginAdapterError::Shutdown {
                message: "supervisor state was poisoned".to_owned(),
            })?
            .take();
        if let Some(actor) = actor {
            timeout(SHUTDOWN_TIMEOUT * 4, actor)
                .await
                .map_err(|_| PluginAdapterError::Shutdown {
                    message: "supervisor did not reap the process in time".to_owned(),
                })?
                .map_err(|error| PluginAdapterError::Shutdown {
                    message: error.to_string(),
                })?;
        }
        Ok(())
    }
}

impl Drop for PluginAdapter {
    fn drop(&mut self) {
        let _ = self.client.control.try_send(ControlCommand::Stop);
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InitializeResult {
    protocol_version: String,
    plugin: ReportedPlugin,
    capabilities: PluginCapabilities,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReportedPlugin {
    id: String,
    version: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PluginCapabilities {
    cancellation: bool,
}

fn validate_handshake(config: &PluginConfig, value: Value) -> Result<bool, PluginAdapterError> {
    let result: InitializeResult =
        serde_json::from_value(value).map_err(|_| PluginAdapterError::Initialization {
            message: "plugin returned a malformed initialize result".to_owned(),
        })?;
    if result.protocol_version != PLUGIN_PROTOCOL_VERSION
        || result.protocol_version != config.protocol_version
    {
        return Err(PluginAdapterError::Initialization {
            message: "plugin did not select the exact configured protocol version".to_owned(),
        });
    }
    if result.plugin.id != config.plugin_id {
        return Err(PluginAdapterError::Initialization {
            message: "plugin-reported identity did not match configured identity".to_owned(),
        });
    }
    if result.plugin.version != config.plugin_version {
        return Err(PluginAdapterError::Initialization {
            message: "plugin-reported version did not match configured version".to_owned(),
        });
    }
    Ok(result.capabilities.cancellation)
}

fn map_tools(
    config: &PluginConfig,
    value: Value,
    client: &Client,
) -> Result<Vec<Arc<dyn Tool>>, PluginAdapterError> {
    const MAX_TOOLS: usize = 128;
    const MAX_DESCRIPTION: usize = 16 * 1024;
    const MAX_SCHEMA: usize = 256 * 1024;
    let object = value
        .as_object()
        .ok_or_else(|| initialization("tools/list result was not an object"))?;
    if object.len() != 1 || !object.contains_key("tools") {
        return Err(initialization(
            "tools/list result contained unexpected fields",
        ));
    }
    let listed = object["tools"]
        .as_array()
        .ok_or_else(|| initialization("tools/list did not return a tool array"))?;
    if listed.len() > MAX_TOOLS {
        return Err(initialization("tools/list exceeded the tool count limit"));
    }
    if config.expected_names.is_empty() {
        return Err(initialization(
            "process plugin has no host-configured expected tools",
        ));
    }
    let mut names = HashSet::new();
    let mut mapped = Vec::<Arc<dyn Tool>>::with_capacity(listed.len());
    for value in listed {
        let object = value
            .as_object()
            .ok_or_else(|| initialization("tool definition was not an object"))?;
        for key in object.keys() {
            if !matches!(
                key.as_str(),
                "name" | "description" | "input_schema" | "metadata"
            ) {
                return Err(initialization(
                    "tool definition contained an unexpected field",
                ));
            }
        }
        let remote_name = object
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| initialization("tool definition had no valid name"))?;
        validate_component(remote_name, "remote tool name")
            .map_err(|error| initialization(&error.to_string()))?;
        if !names.insert(remote_name.to_owned()) {
            return Err(initialization("tools/list returned a duplicate tool name"));
        }
        if !config.expected_names.contains(remote_name) {
            return Err(initialization(
                "tools/list did not match the exact host-configured tool set",
            ));
        }
        let description = object
            .get("description")
            .and_then(Value::as_str)
            .ok_or_else(|| initialization("tool definition had no description"))?;
        if description.len() > MAX_DESCRIPTION {
            return Err(initialization("tool description exceeded its size limit"));
        }
        let schema = object
            .get("input_schema")
            .filter(|schema| schema.is_object())
            .cloned()
            .ok_or_else(|| initialization("tool definition had no input schema object"))?;
        if serde_json::to_vec(&schema).map_or(true, |bytes| bytes.len() > MAX_SCHEMA) {
            return Err(initialization("tool input schema exceeded its size limit"));
        }
        let identity = external_identity(&config.plugin_id, remote_name)?;
        let permission = config
            .tool_permissions
            .permission_for(&identity)
            .ok_or_else(|| {
                initialization(&format!(
                    "remote tool `{remote_name}` has no explicit host permission assignment"
                ))
            })?;
        if let Some(expected) = config.expected.get(remote_name)
            && (permission != expected.permission
                || normalize_json(&schema) != normalize_json(&expected.schema))
        {
            return Err(initialization(
                "tool schema or permission did not match the host expectation",
            ));
        }
        mapped.push(Arc::new(PluginTool {
            definition: ToolDefinition {
                name: ToolName::new(format!("plugin.{}.{remote_name}", config.plugin_id)),
                description: description.to_owned(),
                input_schema: schema,
                permission,
            },
            remote_name: remote_name.to_owned(),
            client: client.clone(),
            max_result_bytes: config.limits.max_result_bytes,
        }));
    }
    if names != config.expected_names {
        return Err(initialization(
            "tools/list did not match the exact host-configured tool set",
        ));
    }
    mapped.sort_by(|left, right| {
        left.definition()
            .name
            .as_str()
            .cmp(right.definition().name.as_str())
    });
    Ok(mapped)
}

struct PluginTool {
    definition: ToolDefinition,
    remote_name: String,
    client: Client,
    max_result_bytes: usize,
}

#[async_trait]
impl Tool for PluginTool {
    fn definition(&self) -> ToolDefinition {
        self.definition.clone()
    }

    async fn execute(
        &self,
        input: ToolInput,
        _context: ToolContext,
    ) -> Result<ToolOutput, ToolError> {
        if !input.0.is_object() {
            return Err(ToolError::InvalidInput {
                message: "process plugin arguments must be a JSON object".to_owned(),
            });
        }
        let execution_id = self.client.next_execution_id();
        let result = self
            .client
            .request(
                "tools/call",
                json!({
                    "execution_id": execution_id,
                    "name": self.remote_name,
                    "arguments": input.0
                }),
                self.client.call_timeout,
                Some(execution_id),
            )
            .await
            .map_err(|error| ToolError::Execution {
                message: error.sanitized_message(),
            })?;
        map_call_result(result, self.max_result_bytes)
    }
}

fn map_call_result(result: Value, max_result_bytes: usize) -> Result<ToolOutput, ToolError> {
    if serde_json::to_vec(&result).map_or(true, |bytes| bytes.len() > max_result_bytes) {
        return Err(ToolError::Execution {
            message: "process plugin returned an oversized tool result".to_owned(),
        });
    }
    let object = result.as_object().ok_or_else(malformed_result)?;
    if object
        .keys()
        .any(|key| !matches!(key.as_str(), "content" | "is_error"))
    {
        return Err(malformed_result());
    }
    let blocks = object
        .get("content")
        .and_then(Value::as_array)
        .ok_or_else(malformed_result)?;
    let mut content = Vec::with_capacity(blocks.len());
    for block in blocks {
        let object = block.as_object().ok_or_else(malformed_result)?;
        let kind = object
            .get("type")
            .and_then(Value::as_str)
            .ok_or_else(malformed_result)?;
        let value = object.get("value").ok_or_else(malformed_result)?;
        if object.len() != 2 {
            return Err(malformed_result());
        }
        match kind {
            "text" => content.push(ToolContent::Text(
                value.as_str().ok_or_else(malformed_result)?.to_owned(),
            )),
            "json" => content.push(ToolContent::Json(value.clone())),
            _ => return Err(malformed_result()),
        }
    }
    let is_error = object
        .get("is_error")
        .and_then(Value::as_bool)
        .ok_or_else(malformed_result)?;
    Ok(ToolOutput { content, is_error })
}

fn malformed_result() -> ToolError {
    ToolError::Execution {
        message: "process plugin returned a malformed tool result".to_owned(),
    }
}

#[derive(Clone)]
struct Client {
    commands: mpsc::Sender<ActorCommand>,
    control: mpsc::Sender<ControlCommand>,
    next_id: Arc<AtomicU64>,
    next_execution: Arc<AtomicU64>,
    capacity: Arc<Semaphore>,
    cancellation_supported: Arc<AtomicBool>,
    call_timeout: Duration,
}

impl Client {
    fn next_execution_id(&self) -> String {
        let sequence = self.next_execution.fetch_add(1, Ordering::Relaxed);
        format!("execution-{sequence}")
    }

    async fn request(
        &self,
        method: &'static str,
        params: Value,
        request_timeout: Duration,
        execution_id: Option<String>,
    ) -> Result<Value, ClientError> {
        let permit = Arc::clone(&self.capacity)
            .try_acquire_owned()
            .map_err(|_| ClientError::Busy)?;
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (response, receiver) = oneshot::channel();
        self.commands
            .try_send(ActorCommand::Request {
                id,
                method,
                params,
                execution_id: execution_id.clone(),
                response,
                permit,
            })
            .map_err(|_| ClientError::Busy)?;
        let mut guard = RequestGuard {
            id,
            execution_id: execution_id
                .filter(|_| self.cancellation_supported.load(Ordering::Acquire)),
            control: self.control.clone(),
            armed: true,
        };
        let result = match timeout(request_timeout, receiver).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(ClientError::Disconnected),
            Err(_) => Err(ClientError::Timeout),
        };
        if !matches!(result, Err(ClientError::Timeout)) {
            guard.armed = false;
        }
        result
    }

    fn notify(&self, method: &'static str, params: Value) -> Result<(), ClientError> {
        self.commands
            .try_send(ActorCommand::Notification { method, params })
            .map_err(|_| ClientError::Busy)
    }
}

struct RequestGuard {
    id: u64,
    execution_id: Option<String>,
    control: mpsc::Sender<ControlCommand>,
    armed: bool,
}

impl Drop for RequestGuard {
    fn drop(&mut self) {
        if self.armed {
            let _ = self.control.try_send(ControlCommand::Cancel {
                id: self.id,
                execution_id: self.execution_id.clone(),
            });
        }
    }
}

enum ActorCommand {
    Request {
        id: u64,
        method: &'static str,
        params: Value,
        execution_id: Option<String>,
        response: oneshot::Sender<Result<Value, ClientError>>,
        permit: OwnedSemaphorePermit,
    },
    Notification {
        method: &'static str,
        params: Value,
    },
}

enum ControlCommand {
    Cancel {
        id: u64,
        execution_id: Option<String>,
    },
    Stop,
}

struct PendingRequest {
    execution_id: Option<String>,
    response: oneshot::Sender<Result<Value, ClientError>>,
    _permit: OwnedSemaphorePermit,
}

#[derive(Clone, Debug)]
enum ClientError {
    Busy,
    Disconnected,
    Timeout,
    Protocol,
    Remote,
}

impl ClientError {
    fn sanitized_message(&self) -> String {
        match self {
            Self::Busy => "process plugin is busy; request was not sent".to_owned(),
            Self::Disconnected => {
                "process plugin disconnected; uncertain call was not replayed".to_owned()
            }
            Self::Timeout => "process plugin request timed out and was not replayed".to_owned(),
            Self::Protocol => "process plugin protocol failure".to_owned(),
            Self::Remote => "process plugin rejected the request".to_owned(),
        }
    }
}

enum Incoming {
    Message(Value),
    ProtocolFailure,
    Disconnected,
}

struct ActorResources {
    child: Child,
    stdin: ChildStdin,
    stdout: Box<dyn AsyncRead + Unpin + Send>,
    command_receiver: mpsc::Receiver<ActorCommand>,
    control_receiver: mpsc::Receiver<ControlCommand>,
    max_message_bytes: usize,
    stderr_task: JoinHandle<()>,
    cwd: PathBuf,
}

async fn run_actor(resources: ActorResources) {
    let ActorResources {
        mut child,
        mut stdin,
        stdout,
        command_receiver: mut commands,
        control_receiver: mut control,
        max_message_bytes,
        stderr_task,
        cwd,
    } = resources;
    let (incoming_sender, mut incoming) = mpsc::channel(8);
    let stdout_task = tokio::spawn(read_stdout(stdout, max_message_bytes, incoming_sender));
    let mut pending = HashMap::<u64, PendingRequest>::new();
    let mut completed = RetiredIds::new(64);
    let mut retired = RetiredIds::new(64);
    let mut connection_error = ClientError::Disconnected;

    'supervisor: loop {
        tokio::select! {
            biased;
            command = control.recv() => {
                match command {
                    Some(ControlCommand::Cancel { id, execution_id }) => {
                        if pending.remove(&id).is_some() {
                            retired.insert(id);
                            if let Some(execution_id) = execution_id {
                                let message = json!({
                                    "jsonrpc": "2.0",
                                    "method": "tools/cancel",
                                    "params": {"execution_id": execution_id, "reason": "RAH request ended"}
                                });
                                if write_message(&mut stdin, &message, max_message_bytes).await.is_err() {
                                    break 'supervisor;
                                }
                            }
                        }
                    }
                    Some(ControlCommand::Stop) | None => break 'supervisor,
                }
            }
            event = incoming.recv() => {
                match event {
                    Some(Incoming::Message(message)) => {
                        if handle_response(message, &mut pending, &mut completed, &retired).is_err() {
                            connection_error = ClientError::Protocol;
                            break 'supervisor;
                        }
                    }
                    Some(Incoming::ProtocolFailure) => {
                        connection_error = ClientError::Protocol;
                        break 'supervisor;
                    }
                    Some(Incoming::Disconnected) | None => break 'supervisor,
                }
            }
            command = commands.recv() => {
                let Some(command) = command else { break 'supervisor };
                let message = match command {
                    ActorCommand::Request { id, method, params, execution_id, response, permit } => {
                        let message = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
                        pending.insert(id, PendingRequest { execution_id, response, _permit: permit });
                        message
                    }
                    ActorCommand::Notification { method, params } => {
                        json!({"jsonrpc": "2.0", "method": method, "params": params})
                    }
                };
                if write_message(&mut stdin, &message, max_message_bytes).await.is_err() {
                    break 'supervisor;
                }
            }
            status = child.wait() => {
                if let Ok(status) = status {
                    tracing::debug!(target: "rah", plugin_exit_status = %status, "process plugin exited");
                }
                break 'supervisor;
            }
        }
    }

    fail_pending(&mut pending, connection_error);
    let _ = stdin.shutdown().await;
    if timeout(SHUTDOWN_TIMEOUT, child.wait()).await.is_err() {
        let _ = child.kill().await;
        let _ = child.wait().await;
    }
    stdout_task.abort();
    let _ = stdout_task.await;
    let _ = timeout(SHUTDOWN_TIMEOUT, stderr_task).await;
    if let Err(error) = tokio::fs::remove_dir(&cwd).await {
        tracing::debug!(target: "rah", path = %cwd.display(), %error, "failed to remove plugin cwd");
    }
}

fn handle_response(
    message: Value,
    pending: &mut HashMap<u64, PendingRequest>,
    completed: &mut RetiredIds,
    retired: &RetiredIds,
) -> Result<(), ()> {
    let object = message.as_object().ok_or(())?;
    if object.keys().any(|key| {
        !matches!(
            key.as_str(),
            "jsonrpc" | "id" | "result" | "error" | "execution_id"
        )
    }) {
        return Err(());
    }
    if object.get("jsonrpc") != Some(&Value::String("2.0".to_owned()))
        || object.contains_key("method")
    {
        return Err(());
    }
    let id = object.get("id").and_then(Value::as_u64).ok_or(())?;
    let has_result = object.contains_key("result");
    let has_error = object.contains_key("error");
    if has_result == has_error {
        return Err(());
    }
    let Some(request) = pending.remove(&id) else {
        if retired.contains(id) {
            return Ok(());
        }
        return Err(());
    };
    if let Some(response_execution) = object.get("execution_id").and_then(Value::as_str)
        && request.execution_id.as_deref() != Some(response_execution)
    {
        return Err(());
    }
    completed.insert(id);
    let result = if has_error {
        validate_error(object.get("error").ok_or(())?)?;
        Err(ClientError::Remote)
    } else {
        Ok(object.get("result").cloned().ok_or(())?)
    };
    let _ = request.response.send(result);
    Ok(())
}

fn validate_error(value: &Value) -> Result<(), ()> {
    let object = value.as_object().ok_or(())?;
    if object.get("code").and_then(Value::as_i64).is_none()
        || object.get("message").and_then(Value::as_str).is_none()
        || object
            .get("message")
            .and_then(Value::as_str)
            .is_some_and(|message| message.len() > 16 * 1024)
    {
        return Err(());
    }
    Ok(())
}

async fn read_stdout(
    mut stdout: impl AsyncRead + Unpin,
    max_message_bytes: usize,
    sender: mpsc::Sender<Incoming>,
) {
    let mut read_buffer = [0_u8; 8192];
    let mut frame = Vec::new();
    loop {
        let count = match stdout.read(&mut read_buffer).await {
            Ok(0) => {
                let event = if frame.is_empty() {
                    Incoming::Disconnected
                } else {
                    Incoming::ProtocolFailure
                };
                let _ = sender.send(event).await;
                return;
            }
            Ok(count) => count,
            Err(_) => {
                let _ = sender.send(Incoming::Disconnected).await;
                return;
            }
        };
        for byte in &read_buffer[..count] {
            if *byte == b'\n' {
                if frame.is_empty() || frame.len() > max_message_bytes {
                    let _ = sender.send(Incoming::ProtocolFailure).await;
                    return;
                }
                let message = serde_json::from_slice::<Value>(&frame);
                frame.clear();
                match message {
                    Ok(message) => {
                        if sender.send(Incoming::Message(message)).await.is_err() {
                            return;
                        }
                    }
                    Err(_) => {
                        let _ = sender.send(Incoming::ProtocolFailure).await;
                        return;
                    }
                }
            } else {
                if frame.len() == max_message_bytes {
                    let _ = sender.send(Incoming::ProtocolFailure).await;
                    return;
                }
                frame.push(*byte);
            }
        }
    }
}

async fn write_message(
    stdin: &mut ChildStdin,
    message: &Value,
    max_message_bytes: usize,
) -> Result<(), ClientError> {
    let mut serialized = serde_json::to_vec(message).map_err(|_| ClientError::Protocol)?;
    if serialized.len() > max_message_bytes {
        return Err(ClientError::Protocol);
    }
    serialized.push(b'\n');
    stdin
        .write_all(&serialized)
        .await
        .map_err(|_| ClientError::Disconnected)?;
    stdin.flush().await.map_err(|_| ClientError::Disconnected)
}

fn fail_pending(pending: &mut HashMap<u64, PendingRequest>, error: ClientError) {
    for (_, request) in pending.drain() {
        let _ = request.response.send(Err(error.clone()));
    }
}

struct RetiredIds {
    capacity: usize,
    ids: VecDeque<u64>,
}

impl RetiredIds {
    fn new(capacity: usize) -> Self {
        Self {
            capacity,
            ids: VecDeque::with_capacity(capacity),
        }
    }

    fn insert(&mut self, id: u64) {
        if self.ids.len() == self.capacity {
            self.ids.pop_front();
        }
        self.ids.push_back(id);
    }

    fn contains(&self, id: u64) -> bool {
        self.ids.contains(&id)
    }
}

struct DiagnosticBuffer {
    bytes: VecDeque<u8>,
    capacity: usize,
    truncated_bytes: u64,
}

impl DiagnosticBuffer {
    fn new(capacity: usize) -> Self {
        Self {
            bytes: VecDeque::with_capacity(capacity),
            capacity,
            truncated_bytes: 0,
        }
    }

    fn push(&mut self, bytes: &[u8]) {
        for byte in bytes {
            if self.bytes.len() == self.capacity {
                self.bytes.pop_front();
                self.truncated_bytes += 1;
            }
            self.bytes.push_back(*byte);
        }
    }

    fn sanitized(&self) -> String {
        let bytes = self.bytes.iter().copied().collect::<Vec<_>>();
        let mut sanitized = String::from_utf8_lossy(&bytes)
            .chars()
            .flat_map(|character| match character {
                '\n' | '\r' | '\t' => vec![character],
                character if character.is_control() => character.escape_default().collect(),
                character => vec![character],
            })
            .collect::<String>();
        if sanitized.len() > self.capacity {
            let mut boundary = self.capacity;
            while !sanitized.is_char_boundary(boundary) {
                boundary -= 1;
            }
            sanitized.truncate(boundary);
        }
        sanitized
    }
}

async fn drain_stderr(
    mut stderr: impl AsyncRead + Unpin,
    diagnostics: Arc<Mutex<DiagnosticBuffer>>,
) {
    let mut bytes = [0_u8; 8192];
    loop {
        match stderr.read(&mut bytes).await {
            Ok(0) | Err(_) => return,
            Ok(count) => {
                if let Ok(mut diagnostics) = diagnostics.lock() {
                    diagnostics.push(&bytes[..count]);
                }
            }
        }
    }
}

fn validate_component(component: &str, label: &str) -> Result<(), PluginAdapterError> {
    if component.is_empty()
        || component.len() > 64
        || !component
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"_-".contains(&byte))
    {
        return Err(invalid_configuration(&format!(
            "{label} must contain 1-64 lowercase ASCII letters, digits, `_`, or `-`"
        )));
    }
    Ok(())
}

fn validate_limits(limits: &PluginLimits) -> Result<(), PluginAdapterError> {
    if limits.max_outstanding == 0
        || limits.max_outstanding > 32
        || limits.command_queue == 0
        || limits.command_queue > 64
        || limits.max_message_bytes == 0
        || limits.max_message_bytes > 1024 * 1024
        || limits.max_result_bytes == 0
        || limits.max_result_bytes > limits.max_message_bytes
        || limits.max_stderr_bytes == 0
        || limits.max_stderr_bytes > 64 * 1024
    {
        return Err(invalid_configuration(
            "plugin resource limits exceeded hard maxima",
        ));
    }
    Ok(())
}

fn external_identity(
    plugin_id: &str,
    remote_name: &str,
) -> Result<ExternalToolIdentity, PluginAdapterError> {
    ExternalToolIdentity::new(format!("plugin:{plugin_id}:{remote_name}"))
        .map_err(permission_configuration_error)
}

/// A deliberately adapter-local executable identity check.
///
/// This matches the MCP boundary semantically, but remains private here so RAH
/// does not grow a generic process-launch API. Revalidation narrows, but cannot
/// eliminate, the filesystem replacement race between this check and spawn.
struct ExecutableIdentity {
    path: PathBuf,
    length: u64,
    modified: Option<SystemTime>,
}

impl ExecutableIdentity {
    fn capture(path: &Path) -> Result<Self, PluginAdapterError> {
        if !path.is_absolute() {
            return Err(invalid_configuration(
                "process-plugin executable must be absolute; PATH lookup is disabled",
            ));
        }
        let link = std::fs::symlink_metadata(path)
            .map_err(|_| startup("configured process-plugin executable could not be inspected"))?;
        if link.file_type().is_symlink() {
            return Err(invalid_configuration(
                "process-plugin executable must not be a symbolic link",
            ));
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if link.file_attributes() & 0x400 != 0 {
                return Err(invalid_configuration(
                    "process-plugin executable must not be a reparse point",
                ));
            }
        }
        let path = std::fs::canonicalize(path)
            .map_err(|_| startup("configured process-plugin executable could not be resolved"))?;
        let metadata = std::fs::metadata(&path)
            .map_err(|_| startup("configured process-plugin executable could not be inspected"))?;
        if !metadata.is_file() {
            return Err(startup(
                "configured process-plugin executable is not a regular file",
            ));
        }
        #[cfg(windows)]
        if path
            .extension()
            .is_none_or(|extension| !extension.eq_ignore_ascii_case("exe"))
        {
            return Err(invalid_configuration(
                "process-plugin executable must be a native .exe",
            ));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if metadata.permissions().mode() & 0o111 == 0 {
                return Err(invalid_configuration(
                    "process-plugin executable must have an executable permission bit",
                ));
            }
        }
        Ok(Self {
            path,
            length: metadata.len(),
            modified: metadata.modified().ok(),
        })
    }

    fn revalidate(&self) -> Result<(), PluginAdapterError> {
        let current = Self::capture(&self.path)?;
        if !same_native_path(&current.path, &self.path)
            || current.length != self.length
            || current.modified != self.modified
        {
            return Err(startup(
                "configured process-plugin executable identity changed",
            ));
        }
        Ok(())
    }
}

#[cfg(windows)]
fn same_native_path(left: &Path, right: &Path) -> bool {
    left.as_os_str()
        .to_string_lossy()
        .eq_ignore_ascii_case(&right.as_os_str().to_string_lossy())
}

#[cfg(not(windows))]
fn same_native_path(left: &Path, right: &Path) -> bool {
    left == right
}

fn validate_schema_object(schema: &Value) -> Result<(), PluginAdapterError> {
    if schema.is_object() {
        Ok(())
    } else {
        Err(invalid_configuration(
            "process-plugin input schema must be a JSON object",
        ))
    }
}

fn normalize_json(value: &Value) -> Value {
    match value {
        Value::Object(object) => {
            let mut keys = object.keys().collect::<Vec<_>>();
            keys.sort();
            Value::Object(
                keys.into_iter()
                    .map(|key| (key.clone(), normalize_json(&object[key])))
                    .collect(),
            )
        }
        Value::Array(values) => Value::Array(values.iter().map(normalize_json).collect()),
        _ => value.clone(),
    }
}

async fn create_isolated_cwd() -> Result<PathBuf, PluginAdapterError> {
    for _ in 0..8 {
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "rah-process-plugin-{}-{sequence}",
            std::process::id()
        ));
        match tokio::fs::create_dir(&path).await {
            Ok(()) => {
                return tokio::fs::canonicalize(path).await.map_err(|error| {
                    PluginAdapterError::Startup {
                        message: format!("failed to resolve isolated working directory: {error}"),
                    }
                });
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(PluginAdapterError::Startup {
                    message: format!("failed to create isolated working directory: {error}"),
                });
            }
        }
    }
    Err(PluginAdapterError::Startup {
        message: "failed to allocate a unique isolated working directory".to_owned(),
    })
}

fn invalid_configuration(message: &str) -> PluginAdapterError {
    PluginAdapterError::InvalidConfiguration {
        message: message.to_owned(),
    }
}

fn startup(message: &str) -> PluginAdapterError {
    PluginAdapterError::Startup {
        message: message.to_owned(),
    }
}

fn initialization(message: &str) -> PluginAdapterError {
    PluginAdapterError::Initialization {
        message: message.to_owned(),
    }
}

fn initialization_error(error: ClientError) -> PluginAdapterError {
    initialization(&error.sanitized_message())
}

fn permission_configuration_error(error: ExternalToolPermissionError) -> PluginAdapterError {
    invalid_configuration(&error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_queue_is_bounded() {
        let (sender, _receiver) = mpsc::channel(1);
        let (response, _) = oneshot::channel();
        sender
            .try_send(ActorCommand::Notification {
                method: "initialized",
                params: json!({}),
            })
            .expect("first command should fit");
        let permit = Arc::new(Semaphore::new(1))
            .try_acquire_owned()
            .expect("permit should be available");
        assert!(
            sender
                .try_send(ActorCommand::Request {
                    id: 1,
                    method: "tools/call",
                    params: json!({}),
                    execution_id: Some("execution-1".to_owned()),
                    response,
                    permit,
                })
                .is_err(),
            "a full command queue must reject without allocating unbounded work"
        );
    }

    #[test]
    fn hard_limits_reject_expansion() {
        let limits = PluginLimits {
            max_outstanding: 33,
            ..PluginLimits::default()
        };
        let config = PluginConfig::stdio("test", "0.1.0", "unused")
            .expect("base config")
            .with_limits(limits);
        assert!(config.is_err());
    }

    #[test]
    fn executable_identity_requires_a_native_regular_absolute_file() {
        let current = std::env::current_exe().expect("test executable path");
        assert!(ExecutableIdentity::capture(&current).is_ok());
        assert!(ExecutableIdentity::capture(Path::new("relative-program")).is_err());
        let directory = std::env::temp_dir();
        assert!(ExecutableIdentity::capture(&directory).is_err());
    }

    #[cfg(windows)]
    #[test]
    fn windows_rejects_command_and_powershell_scripts() {
        let root =
            std::env::temp_dir().join(format!("rah-plugin-executable-test-{}", std::process::id()));
        std::fs::create_dir_all(&root).expect("temporary test directory");
        for extension in ["cmd", "ps1"] {
            let path = root.join(format!("plugin.{extension}"));
            std::fs::write(&path, "echo ignored").expect("test script");
            assert!(ExecutableIdentity::capture(&path).is_err());
        }
        let _ = std::fs::remove_dir_all(root);
    }

    #[cfg(windows)]
    #[test]
    fn windows_rejects_executable_symlink_when_symlinks_are_available() {
        let root =
            std::env::temp_dir().join(format!("rah-plugin-link-test-{}", std::process::id()));
        std::fs::create_dir_all(&root).expect("temporary test directory");
        let link = root.join("plugin.exe");
        let target = std::env::current_exe().expect("test executable");
        match std::os::windows::fs::symlink_file(target, &link) {
            Ok(()) => assert!(ExecutableIdentity::capture(&link).is_err()),
            Err(error)
                if matches!(error.kind(), std::io::ErrorKind::PermissionDenied)
                    || error.raw_os_error() == Some(1314) => {}
            Err(error) => panic!("unexpected symlink creation failure: {error}"),
        }
        let _ = std::fs::remove_dir_all(root);
    }

    #[cfg(windows)]
    #[test]
    fn windows_executable_identity_revalidation_detects_replacement() {
        let root =
            std::env::temp_dir().join(format!("rah-plugin-identity-test-{}", std::process::id()));
        std::fs::create_dir_all(&root).expect("temporary test directory");
        let executable = root.join("plugin.exe");
        std::fs::write(&executable, b"first").expect("test executable");
        let identity = ExecutableIdentity::capture(&executable).expect("capture identity");
        assert!(identity.revalidate().is_ok());
        std::fs::write(&executable, b"replacement").expect("replace test executable");
        assert!(identity.revalidate().is_err());
        let _ = std::fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn unix_requires_an_executable_regular_file() {
        use std::os::unix::fs::PermissionsExt;

        let path =
            std::env::temp_dir().join(format!("rah-plugin-executable-test-{}", std::process::id()));
        std::fs::write(&path, "fixture").expect("test executable");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
            .expect("non-executable mode");
        assert!(ExecutableIdentity::capture(&path).is_err());
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700))
            .expect("executable mode");
        assert!(ExecutableIdentity::capture(&path).is_ok());
        let _ = std::fs::remove_file(path);
    }
}
