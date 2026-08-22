//! Bounded stdio MCP tools. Supervision reduces ambient authority; it is not a sandbox.
use async_trait::async_trait;
use rah_protocol::{PermissionLevel, ToolContent, ToolDefinition, ToolInput, ToolName, ToolOutput};
use rah_tools::{
    ExternalToolIdentity, ExternalToolPermissionError, ExternalToolPermissionPolicy, Tool,
    ToolContext, ToolError,
};
use serde_json::{Value, json};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, SystemTime},
};
use thiserror::Error;
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
    process::{Child, ChildStdin, Command as ProcessCommand},
    sync::{OwnedSemaphorePermit, Semaphore, mpsc, oneshot},
    task::JoinHandle,
    time::timeout,
};
pub const MCP_PROTOCOL_VERSION: &str = "2025-06-18";
const STARTUP_TIMEOUT: Duration = Duration::from_secs(2);
const SHUTDOWN_TIMEOUT: Duration = Duration::from_millis(500);
const RETIRED_ID_LIMIT: usize = 64;
static SEQ: AtomicU64 = AtomicU64::new(1);
#[derive(Clone, Debug)]
pub struct McpLimits {
    pub max_outstanding: usize,
    pub command_queue: usize,
    pub max_message_bytes: usize,
    pub max_result_bytes: usize,
    pub max_stderr_bytes: usize,
}
impl Default for McpLimits {
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
#[derive(Clone, Debug)]
pub struct McpExpectedTool {
    schema: Value,
    permission: PermissionLevel,
}
#[derive(Clone, Debug)]
pub struct McpServerConfig {
    server_id: String,
    program: PathBuf,
    args: Vec<String>,
    call_timeout: Duration,
    limits: McpLimits,
    permissions: ExternalToolPermissionPolicy,
    expected_names: HashSet<String>,
    expected: HashMap<String, McpExpectedTool>,
}
impl McpServerConfig {
    pub fn stdio(
        id: impl Into<String>,
        program: impl Into<PathBuf>,
    ) -> Result<Self, McpAdapterError> {
        let id = id.into();
        component(&id, "server ID")?;
        let program = program.into();
        if program.as_os_str().is_empty() {
            return Err(config("MCP server program must not be empty"));
        }
        Ok(Self {
            server_id: id,
            program,
            args: vec![],
            call_timeout: Duration::from_secs(30),
            limits: McpLimits::default(),
            permissions: ExternalToolPermissionPolicy::new(),
            expected_names: HashSet::new(),
            expected: HashMap::new(),
        })
    }
    #[must_use]
    pub fn with_arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }
    #[must_use]
    pub fn with_call_timeout(mut self, value: Duration) -> Self {
        self.call_timeout = value;
        self
    }
    pub fn with_limits(mut self, value: McpLimits) -> Result<Self, McpAdapterError> {
        limits(&value)?;
        self.limits = value;
        Ok(self)
    }
    pub fn with_tool_permission(
        mut self,
        name: impl Into<String>,
        level: PermissionLevel,
    ) -> Result<Self, McpAdapterError> {
        let name = name.into();
        component(&name, "remote tool name")?;
        self.permissions
            .assign(identity(&self.server_id, &name)?, level)
            .map_err(permission)?;
        self.expected_names.insert(name);
        Ok(self)
    }
    pub fn with_expected_tool(
        mut self,
        name: impl Into<String>,
        schema: Value,
        permission_level: PermissionLevel,
    ) -> Result<Self, McpAdapterError> {
        let name = name.into();
        component(&name, "remote tool name")?;
        schema_object(&schema)?;
        if self.expected.contains_key(&name) {
            return Err(config("expected MCP tool configured more than once"));
        }
        self.permissions
            .assign(identity(&self.server_id, &name)?, permission_level)
            .map_err(permission)?;
        self.expected_names.insert(name.clone());
        self.expected.insert(
            name,
            McpExpectedTool {
                schema,
                permission: permission_level,
            },
        );
        Ok(self)
    }
}
#[derive(Clone, Debug, Default)]
pub struct McpDiagnostics {
    pub stderr: String,
    pub truncated_bytes: u64,
}
#[derive(Debug, Error)]
pub enum McpAdapterError {
    #[error("invalid MCP configuration: {message}")]
    InvalidConfiguration { message: String },
    #[error("failed to start MCP server: {message}")]
    Startup { message: String },
    #[error("MCP initialization failed: {message}")]
    Initialization { message: String },
    #[error("failed to shut down MCP server: {message}")]
    Shutdown { message: String },
}
pub struct McpAdapter {
    client: Client,
    tools: Vec<Arc<dyn Tool>>,
    actor: Mutex<Option<JoinHandle<()>>>,
    diagnostics: Arc<Mutex<Diagnostics>>,
}
impl McpAdapter {
    pub async fn connect(cfg: McpServerConfig) -> Result<Self, McpAdapterError> {
        limits(&cfg.limits)?;
        if cfg.call_timeout.is_zero() {
            return Err(config("call timeout must be greater than zero"));
        }
        let executable = Executable::new(&cfg.program)?;
        let cwd = isolated_cwd().await?;
        executable.revalidate()?;
        let diagnostics = Arc::new(Mutex::new(Diagnostics::new(cfg.limits.max_stderr_bytes)));
        let mut cmd = ProcessCommand::new(&executable.path);
        cmd.args(&cfg.args)
            .current_dir(&cwd)
            .env_clear()
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);
        #[cfg(windows)]
        if let Some(root) = std::env::var_os("SystemRoot") {
            cmd.env("SystemRoot", root);
        }
        let mut child = cmd
            .spawn()
            .map_err(|_| startup("configured MCP executable could not be started"))?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| startup("child stdin was not captured"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| startup("child stdout was not captured"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| startup("child stderr was not captured"))?;
        let (commands, rx) = mpsc::channel(cfg.limits.command_queue);
        // A cancelled request must always be able to retire while every allowed
        // request is outstanding.  Otherwise a full control queue could leave a
        // timed-out request live and retain its permit indefinitely.
        let (control, crx) = mpsc::channel(cfg.limits.max_outstanding + 1);
        let client = Client {
            commands,
            control,
            next: Arc::new(AtomicU64::new(1)),
            capacity: Arc::new(Semaphore::new(cfg.limits.max_outstanding)),
            call_timeout: cfg.call_timeout,
        };
        let stderr_task = tokio::spawn(drain(stderr, Arc::clone(&diagnostics)));
        let actor = tokio::spawn(actor(
            child,
            stdin,
            Box::new(stdout),
            rx,
            crx,
            cfg.limits.max_message_bytes,
            stderr_task,
            cwd,
        ));
        let ready = async {
            let initialized = client
                .request(
                    "initialize",
                    json!({
                        "protocolVersion": MCP_PROTOCOL_VERSION,
                        "capabilities": {},
                        "clientInfo": {
                            "name": "rah-tools-mcp",
                            "version": env!("CARGO_PKG_VERSION")
                        }
                    }),
                    STARTUP_TIMEOUT,
                )
                .await
                .map_err(init_error)?;
            if initialized.get("protocolVersion")
                != Some(&Value::String(MCP_PROTOCOL_VERSION.to_owned()))
            {
                return Err(init("server did not accept pinned protocol version"));
            }
            client
                .notify("notifications/initialized", json!({}))
                .map_err(init_error)?;
            let listed = client
                .request("tools/list", json!({}), STARTUP_TIMEOUT)
                .await
                .map_err(init_error)?;
            map_tools(&cfg, listed, &client)
        }
        .await;
        match ready {
            Ok(tools) => Ok(Self {
                client,
                tools,
                actor: Mutex::new(Some(actor)),
                diagnostics,
            }),
            Err(e) => {
                let _ = client.control.try_send(Control::Stop);
                let _ = timeout(SHUTDOWN_TIMEOUT * 4, actor).await;
                Err(e)
            }
        }
    }
    #[must_use]
    pub fn tools(&self) -> Vec<Arc<dyn Tool>> {
        self.tools.clone()
    }
    #[must_use]
    pub fn diagnostics(&self) -> McpDiagnostics {
        self.diagnostics
            .lock()
            .map(|d| McpDiagnostics {
                stderr: d.text(),
                truncated_bytes: d.truncated,
            })
            .unwrap_or_default()
    }
    pub async fn shutdown(self) -> Result<(), McpAdapterError> {
        let _ = self
            .client
            .request("shutdown", json!({}), SHUTDOWN_TIMEOUT)
            .await;
        let _ = self.client.control.try_send(Control::Stop);
        let actor = self
            .actor
            .lock()
            .map_err(|_| McpAdapterError::Shutdown {
                message: "supervisor state was poisoned".to_owned(),
            })?
            .take();
        if let Some(a) = actor {
            timeout(SHUTDOWN_TIMEOUT * 4, a)
                .await
                .map_err(|_| McpAdapterError::Shutdown {
                    message: "supervisor did not reap child".to_owned(),
                })?
                .map_err(|e| McpAdapterError::Shutdown {
                    message: e.to_string(),
                })?;
        }
        Ok(())
    }
}
impl Drop for McpAdapter {
    fn drop(&mut self) {
        let _ = self.client.control.try_send(Control::Stop);
    }
}
fn map_tools(
    cfg: &McpServerConfig,
    result: Value,
    client: &Client,
) -> Result<Vec<Arc<dyn Tool>>, McpAdapterError> {
    let list = result
        .get("tools")
        .and_then(Value::as_array)
        .ok_or_else(|| init("tools/list did not return a tool array"))?;
    let expected = &cfg.expected_names;
    if list.len() != expected.len() {
        return Err(init("tools/list did not match exact expected tool set"));
    }
    let mut seen = HashSet::new();
    let mut tools = Vec::with_capacity(list.len());
    for item in list {
        let name = item
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| init("tools/list returned invalid name"))?;
        component(name, "remote tool name")
            .map_err(|_| init("tools/list returned invalid name"))?;
        if !seen.insert(name) || !expected.contains(name) {
            return Err(init("tools/list did not match exact expected tool set"));
        }
        let input_schema = item
            .get("inputSchema")
            .cloned()
            .ok_or_else(|| init("tools/list returned no input schema"))?;
        schema_object(&input_schema)
            .map_err(|_| init("tools/list returned unsupported input schema"))?;
        let p = cfg
            .permissions
            .permission_for(&identity(&cfg.server_id, name)?)
            .ok_or_else(|| init("remote tool has no explicit host permission"))?;
        if let Some(e) = cfg.expected.get(name)
            && (p != e.permission || normalized(&input_schema) != normalized(&e.schema))
        {
            return Err(init(
                "tools/list schema or permission did not match host expectation",
            ));
        }
        let description = item.get("description").and_then(Value::as_str).map_or_else(
            || format!("MCP tool `{name}` from configured server."),
            str::to_owned,
        );
        tools.push(Arc::new(McpTool {
            definition: ToolDefinition {
                name: ToolName::new(format!("mcp.{}.{}", cfg.server_id, name)),
                description,
                input_schema,
                permission: p,
            },
            remote: name.to_owned(),
            client: client.clone(),
            result_limit: cfg.limits.max_result_bytes,
        }) as Arc<dyn Tool>);
    }
    if seen.iter().copied().collect::<HashSet<_>>() != expected.iter().map(String::as_str).collect()
    {
        return Err(init("tools/list did not match exact expected tool set"));
    }
    tools.sort_by(|a, b| {
        a.definition()
            .name
            .as_str()
            .cmp(b.definition().name.as_str())
    });
    Ok(tools)
}
struct McpTool {
    definition: ToolDefinition,
    remote: String,
    client: Client,
    result_limit: usize,
}
#[async_trait]
impl Tool for McpTool {
    fn definition(&self) -> ToolDefinition {
        self.definition.clone()
    }
    async fn execute(&self, input: ToolInput, _: ToolContext) -> Result<ToolOutput, ToolError> {
        if !input.0.is_object() {
            return Err(ToolError::InvalidInput {
                message: "MCP tool arguments must be a JSON object".to_owned(),
            });
        }
        let result = self
            .client
            .request(
                "tools/call",
                json!({"name":self.remote,"arguments":input.0}),
                self.client.call_timeout,
            )
            .await
            .map_err(|e| ToolError::Execution {
                message: e.message(),
            })?;
        output(result, self.result_limit)
    }
}
fn output(value: Value, limit: usize) -> Result<ToolOutput, ToolError> {
    if serde_json::to_vec(&value).map_or(true, |v| v.len() > limit) {
        return Err(ToolError::Execution {
            message: "MCP tool result exceeded host result limit".to_owned(),
        });
    }
    let content = value
        .get("content")
        .and_then(Value::as_array)
        .ok_or_else(malformed)?;
    let mut out = Vec::with_capacity(content.len() + 1);
    for item in content {
        if item.get("type") != Some(&Value::String("text".to_owned())) {
            return Err(malformed());
        }
        out.push(ToolContent::Text(
            item.get("text")
                .and_then(Value::as_str)
                .ok_or_else(malformed)?
                .to_owned(),
        ));
    }
    if let Some(v) = value.get("structuredContent") {
        out.push(ToolContent::Json(v.clone()));
    }
    let output = ToolOutput {
        content: out,
        is_error: value
            .get("isError")
            .map_or(Ok(false), |v| v.as_bool().ok_or_else(malformed))?,
    };
    if serde_json::to_vec(&output).map_or(true, |v| v.len() > limit) {
        return Err(ToolError::Execution {
            message: "MCP tool output exceeded host result limit".to_owned(),
        });
    }
    Ok(output)
}
fn malformed() -> ToolError {
    ToolError::Execution {
        message: "MCP server returned malformed tool result".to_owned(),
    }
}
#[derive(Clone)]
struct Client {
    commands: mpsc::Sender<Command>,
    control: mpsc::Sender<Control>,
    next: Arc<AtomicU64>,
    capacity: Arc<Semaphore>,
    call_timeout: Duration,
}
impl Client {
    async fn request(
        &self,
        method: &'static str,
        params: Value,
        time: Duration,
    ) -> Result<Value, ClientError> {
        let permit = Arc::clone(&self.capacity)
            .try_acquire_owned()
            .map_err(|_| ClientError::Busy)?;
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        self.commands
            .try_send(Command::Request {
                id,
                method,
                params,
                response: tx,
                permit,
            })
            .map_err(|_| ClientError::Busy)?;
        let mut guard = Cancel {
            id,
            control: self.control.clone(),
            armed: true,
        };
        let result = match timeout(time, rx).await {
            Ok(Ok(v)) => v,
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
            .try_send(Command::Notify { method, params })
            .map_err(|_| ClientError::Busy)
    }
}
struct Cancel {
    id: u64,
    control: mpsc::Sender<Control>,
    armed: bool,
}
impl Drop for Cancel {
    fn drop(&mut self) {
        if self.armed {
            let _ = self.control.try_send(Control::Cancel(self.id));
        }
    }
}
enum Command {
    Request {
        id: u64,
        method: &'static str,
        params: Value,
        response: oneshot::Sender<Result<Value, ClientError>>,
        permit: OwnedSemaphorePermit,
    },
    Notify {
        method: &'static str,
        params: Value,
    },
}
enum Control {
    Cancel(u64),
    Stop,
}
struct Pending {
    response: oneshot::Sender<Result<Value, ClientError>>,
    _permit: OwnedSemaphorePermit,
}
#[derive(Clone)]
enum ClientError {
    Busy,
    Disconnected,
    Timeout,
    Protocol,
    Remote,
}
impl ClientError {
    fn message(&self) -> String {
        match self {
            Self::Busy => "MCP server is busy; request was not sent",
            Self::Disconnected => "MCP server disconnected; uncertain call was not replayed",
            Self::Timeout => "MCP request timed out and was not replayed",
            Self::Protocol => "MCP protocol failure",
            Self::Remote => "MCP server rejected the request",
        }
        .to_owned()
    }
}
enum Incoming {
    Message(Value),
    Failure,
    Disconnected,
}
#[allow(clippy::too_many_arguments)]
async fn actor(
    mut child: Child,
    mut stdin: ChildStdin,
    stdout: Box<dyn AsyncRead + Send + Unpin>,
    mut commands: mpsc::Receiver<Command>,
    mut control: mpsc::Receiver<Control>,
    max: usize,
    stderr: JoinHandle<()>,
    cwd: PathBuf,
) {
    let (tx, mut incoming) = mpsc::channel(1);
    let reader = tokio::spawn(read_frames(stdout, tx, max));
    let mut pending = HashMap::<u64, Pending>::new();
    let mut retired = VecDeque::new();
    let mut error = ClientError::Disconnected;
    'run: loop {
        tokio::select! {
            item = incoming.recv() => match item {
                Some(Incoming::Message(message)) => {
                    let Some(id) = message.get("id").and_then(Value::as_u64) else {
                        error = ClientError::Protocol;
                        break 'run;
                    };
                    if let Some(request) = pending.remove(&id) {
                        let result = if message.get("error").is_some() {
                            Err(ClientError::Remote)
                        } else {
                            message.get("result").cloned().ok_or(ClientError::Protocol)
                        };
                        let _ = request.response.send(result);
                    } else if !is_retired(&retired, id) {
                        error = ClientError::Protocol;
                        break 'run;
                    }
                }
                Some(Incoming::Failure) => {
                    error = ClientError::Protocol;
                    break 'run;
                }
                Some(Incoming::Disconnected) | None => break 'run,
            },
            item = control.recv() => match item {
                Some(Control::Cancel(id)) => {
                    if pending.remove(&id).is_some() {
                        retire(&mut retired, id);
                        let cancellation = json!({
                            "jsonrpc": "2.0",
                            "method": "notifications/cancelled",
                            "params": {"requestId": id, "reason": "RAH request ended"}
                        });
                        if write(&mut stdin, &cancellation, max).await.is_err() {
                            break 'run;
                        }
                    }
                }
                Some(Control::Stop) | None => break 'run,
            },
            item = commands.recv() => {
                let Some(item) = item else {
                    break 'run;
                };
                let message = match item {
                    Command::Request { id, method, params, response, permit } => {
                        pending.insert(id, Pending { response, _permit: permit });
                        json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params})
                    }
                    Command::Notify { method, params } => {
                        json!({"jsonrpc": "2.0", "method": method, "params": params})
                    }
                };
                if write(&mut stdin, &message, max).await.is_err() {
                    break 'run;
                }
            }
            _ = child.wait() => break 'run,
        }
    }
    for (_, p) in pending {
        let _ = p.response.send(Err(error.clone()));
    }
    let _ = stdin.shutdown().await;
    if timeout(SHUTDOWN_TIMEOUT, child.wait()).await.is_err() {
        let _ = child.kill().await;
        let _ = child.wait().await;
    }
    reader.abort();
    let _ = reader.await;
    let _ = timeout(SHUTDOWN_TIMEOUT, stderr).await;
    let _ = tokio::fs::remove_dir(cwd).await;
}
fn retire(retired: &mut VecDeque<u64>, id: u64) {
    retired.push_back(id);
    if retired.len() > RETIRED_ID_LIMIT {
        retired.pop_front();
    }
}
fn is_retired(retired: &VecDeque<u64>, id: u64) -> bool {
    retired.contains(&id)
}
async fn read_frames(
    mut reader: Box<dyn AsyncRead + Send + Unpin>,
    tx: mpsc::Sender<Incoming>,
    max: usize,
) {
    let mut frame = Vec::with_capacity(max.min(8192));
    let mut byte = [0];
    loop {
        match reader.read(&mut byte).await {
            Ok(0) | Err(_) => {
                let _ = tx.send(Incoming::Disconnected).await;
                return;
            }
            Ok(_) if byte[0] == b'\n' => {
                let event = serde_json::from_slice(&frame)
                    .map(Incoming::Message)
                    .unwrap_or(Incoming::Failure);
                if tx.send(event).await.is_err() {
                    return;
                }
                frame.clear()
            }
            Ok(_) if frame.len() == max => {
                let _ = tx.send(Incoming::Failure).await;
                return;
            }
            Ok(_) => frame.push(byte[0]),
        }
    }
}
async fn write(stdin: &mut ChildStdin, v: &Value, max: usize) -> Result<(), ClientError> {
    let mut bytes = serde_json::to_vec(v).map_err(|_| ClientError::Protocol)?;
    if bytes.len() > max {
        return Err(ClientError::Protocol);
    }
    bytes.push(b'\n');
    stdin
        .write_all(&bytes)
        .await
        .map_err(|_| ClientError::Disconnected)?;
    stdin.flush().await.map_err(|_| ClientError::Disconnected)
}
struct Diagnostics {
    bytes: VecDeque<u8>,
    max: usize,
    truncated: u64,
}
impl Diagnostics {
    fn new(max: usize) -> Self {
        Self {
            bytes: VecDeque::with_capacity(max),
            max,
            truncated: 0,
        }
    }
    fn push(&mut self, input: &[u8]) {
        for b in input {
            if self.bytes.len() == self.max {
                self.bytes.pop_front();
                self.truncated += 1;
            }
            self.bytes.push_back(*b);
        }
    }
    fn text(&self) -> String {
        String::from_utf8_lossy(&self.bytes.iter().copied().collect::<Vec<_>>())
            .chars()
            .flat_map(|c| {
                if c.is_control() && !matches!(c, '\n' | '\r' | '\t') {
                    c.escape_default().collect()
                } else {
                    vec![c]
                }
            })
            .collect()
    }
}
async fn drain(mut stream: impl AsyncRead + Unpin, d: Arc<Mutex<Diagnostics>>) {
    let mut b = [0; 8192];
    loop {
        match stream.read(&mut b).await {
            Ok(0) | Err(_) => return,
            Ok(n) => {
                if let Ok(mut d) = d.lock() {
                    d.push(&b[..n]);
                }
            }
        }
    }
}
struct Executable {
    path: PathBuf,
    len: u64,
    modified: Option<SystemTime>,
}
impl Executable {
    fn new(path: &Path) -> Result<Self, McpAdapterError> {
        if !path.is_absolute() {
            return Err(config(
                "MCP executable must be absolute; PATH lookup is disabled",
            ));
        }
        let link = std::fs::symlink_metadata(path)
            .map_err(|_| startup("configured MCP executable could not be inspected"))?;
        if link.file_type().is_symlink() {
            return Err(config("MCP executable must not be a symbolic link"));
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if link.file_attributes() & 0x400 != 0 {
                return Err(config("MCP executable must not be a reparse point"));
            }
        }
        let path = std::fs::canonicalize(path)
            .map_err(|_| startup("configured MCP executable could not be resolved"))?;
        let m = std::fs::metadata(&path)
            .map_err(|_| startup("configured MCP executable could not be inspected"))?;
        if !m.is_file() {
            return Err(startup("configured MCP executable is not a regular file"));
        }
        #[cfg(windows)]
        if path
            .extension()
            .is_none_or(|x| !x.eq_ignore_ascii_case("exe"))
        {
            return Err(config("MCP executable must be native .exe"));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if m.permissions().mode() & 0o111 == 0 {
                return Err(config("MCP executable is not executable"));
            }
        }
        Ok(Self {
            path,
            len: m.len(),
            modified: m.modified().ok(),
        })
    }
    fn revalidate(&self) -> Result<(), McpAdapterError> {
        let now = Self::new(&self.path)?;
        if now.path != self.path || now.len != self.len || now.modified != self.modified {
            return Err(startup("configured MCP executable identity changed"));
        }
        Ok(())
    }
}
async fn isolated_cwd() -> Result<PathBuf, McpAdapterError> {
    for _ in 0..8 {
        let path = std::env::temp_dir().join(format!(
            "rah-mcp-{}-{}",
            std::process::id(),
            SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        match tokio::fs::create_dir(&path).await {
            Ok(()) => {
                return tokio::fs::canonicalize(path)
                    .await
                    .map_err(|_| startup("isolated MCP cwd could not be resolved"));
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(_) => return Err(startup("isolated MCP cwd could not be created")),
        }
    }
    Err(startup("isolated MCP cwd could not be allocated"))
}
fn component(v: &str, label: &str) -> Result<(), McpAdapterError> {
    if v.is_empty()
        || v.len() > 64
        || !v
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"_-".contains(&b))
    {
        return Err(config(&format!(
            "{label} must contain 1-64 lowercase ASCII letters, digits, `_`, or `-`"
        )));
    }
    Ok(())
}
fn schema_object(v: &Value) -> Result<(), McpAdapterError> {
    if v.is_object() {
        Ok(())
    } else {
        Err(config("MCP input schema must be JSON object"))
    }
}
fn normalized(v: &Value) -> Value {
    match v {
        Value::Object(m) => {
            let mut k = m.keys().collect::<Vec<_>>();
            k.sort();
            Value::Object(
                k.into_iter()
                    .map(|x| (x.clone(), normalized(&m[x])))
                    .collect(),
            )
        }
        Value::Array(a) => Value::Array(a.iter().map(normalized).collect()),
        _ => v.clone(),
    }
}
fn limits(v: &McpLimits) -> Result<(), McpAdapterError> {
    if v.max_outstanding == 0
        || v.max_outstanding > 32
        || v.command_queue == 0
        || v.command_queue > 64
        || v.max_message_bytes == 0
        || v.max_message_bytes > 1024 * 1024
        || v.max_result_bytes == 0
        || v.max_result_bytes > v.max_message_bytes
        || v.max_stderr_bytes == 0
        || v.max_stderr_bytes > 64 * 1024
    {
        return Err(config("MCP resource limits exceeded hard maxima"));
    }
    Ok(())
}
fn identity(server: &str, name: &str) -> Result<ExternalToolIdentity, McpAdapterError> {
    ExternalToolIdentity::new(format!("mcp:{server}:{name}")).map_err(permission)
}
fn config(m: &str) -> McpAdapterError {
    McpAdapterError::InvalidConfiguration {
        message: m.to_owned(),
    }
}
fn startup(m: &str) -> McpAdapterError {
    McpAdapterError::Startup {
        message: m.to_owned(),
    }
}
fn init(m: &str) -> McpAdapterError {
    McpAdapterError::Initialization {
        message: m.to_owned(),
    }
}
fn init_error(e: ClientError) -> McpAdapterError {
    init(&e.message())
}
fn permission(e: ExternalToolPermissionError) -> McpAdapterError {
    config(&e.to_string())
}

#[cfg(test)]
mod tests {
    use super::{RETIRED_ID_LIMIT, is_retired, retire};
    use std::collections::VecDeque;

    #[test]
    fn retired_request_ids_are_bounded_and_evicted_ids_fail_closed() {
        let mut retired = VecDeque::new();
        for id in 1..=(RETIRED_ID_LIMIT as u64 + 1) {
            retire(&mut retired, id);
        }
        assert_eq!(retired.len(), RETIRED_ID_LIMIT);
        assert!(!is_retired(&retired, 1));
        assert!(is_retired(&retired, RETIRED_ID_LIMIT as u64 + 1));
    }
}
