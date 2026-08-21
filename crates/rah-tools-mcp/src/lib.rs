//! MCP-backed implementations of the provider-neutral RAH tool boundary.

use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use async_trait::async_trait;
use rah_protocol::{PermissionLevel, ToolContent, ToolDefinition, ToolInput, ToolName, ToolOutput};
use rah_tools::{Tool, ToolContext, ToolError};
use serde_json::{Value, json};
use thiserror::Error;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines},
    process::{Child, ChildStdout, Command},
    sync::{mpsc, oneshot},
    task::JoinHandle,
    time::timeout,
};

/// Exact MCP protocol revision implemented by this prototype.
pub const MCP_PROTOCOL_VERSION: &str = "2025-06-18";

const STARTUP_TIMEOUT: Duration = Duration::from_secs(2);
const SHUTDOWN_TIMEOUT: Duration = Duration::from_millis(500);

/// Trusted host configuration for one local stdio MCP server.
#[derive(Clone, Debug)]
pub struct McpServerConfig {
    server_id: String,
    program: PathBuf,
    args: Vec<String>,
    call_timeout: Duration,
}

impl McpServerConfig {
    /// Creates a stdio server configuration with a stable RAH-owned identity.
    pub fn stdio(
        server_id: impl Into<String>,
        program: impl Into<PathBuf>,
    ) -> Result<Self, McpAdapterError> {
        let server_id = server_id.into();
        validate_server_id(&server_id)?;
        let program = program.into();
        if program.as_os_str().is_empty() {
            return Err(McpAdapterError::InvalidConfiguration {
                message: "MCP server program must not be empty".to_owned(),
            });
        }

        Ok(Self {
            server_id,
            program,
            args: Vec::new(),
            call_timeout: Duration::from_secs(30),
        })
    }

    /// Appends one direct process argument without invoking a shell.
    #[must_use]
    pub fn with_arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }

    /// Overrides the per-call timeout.
    #[must_use]
    pub fn with_call_timeout(mut self, call_timeout: Duration) -> Self {
        self.call_timeout = call_timeout;
        self
    }
}

/// Adapter construction or lifecycle failure that does not cross RAH tool APIs.
#[derive(Debug, Error)]
pub enum McpAdapterError {
    /// Trusted host configuration is invalid.
    #[error("invalid MCP configuration: {message}")]
    InvalidConfiguration { message: String },
    /// The configured local child could not be started.
    #[error("failed to start MCP server: {message}")]
    Startup { message: String },
    /// The pinned MCP handshake or discovery contract failed.
    #[error("MCP initialization failed: {message}")]
    Initialization { message: String },
    /// The owned supervisor could not shut down cleanly.
    #[error("failed to shut down MCP server: {message}")]
    Shutdown { message: String },
}

/// Connected MCP generation and its immutable RAH tool proxies.
pub struct McpAdapter {
    client: Client,
    tools: Vec<Arc<dyn Tool>>,
    actor: Mutex<Option<JoinHandle<()>>>,
}

impl McpAdapter {
    /// Starts, initializes, and discovers one local stdio MCP server.
    pub async fn connect(config: McpServerConfig) -> Result<Self, McpAdapterError> {
        let mut command = Command::new(&config.program);
        command
            .args(&config.args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(true);
        let mut child = command.spawn().map_err(|error| McpAdapterError::Startup {
            message: error.to_string(),
        })?;
        let stdin = child.stdin.take().ok_or_else(|| McpAdapterError::Startup {
            message: "child stdin was not captured".to_owned(),
        })?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| McpAdapterError::Startup {
                message: "child stdout was not captured".to_owned(),
            })?;

        let (commands, receiver) = mpsc::unbounded_channel();
        let client = Client {
            commands,
            next_id: Arc::new(AtomicU64::new(1)),
            call_timeout: config.call_timeout,
        };
        let actor = tokio::spawn(run_actor(
            child,
            stdin,
            BufReader::new(stdout).lines(),
            receiver,
        ));

        let initialization = client
            .request(
                "initialize",
                json!({
                    "protocolVersion": MCP_PROTOCOL_VERSION,
                    "capabilities": {},
                    "clientInfo": {"name": "rah-tools-mcp", "version": env!("CARGO_PKG_VERSION")}
                }),
                STARTUP_TIMEOUT,
            )
            .await
            .map_err(initialization_error)?;
        if initialization["protocolVersion"] != MCP_PROTOCOL_VERSION {
            return Err(McpAdapterError::Initialization {
                message: "server did not accept the pinned protocol version".to_owned(),
            });
        }
        client
            .notify("notifications/initialized", json!({}))
            .map_err(initialization_error)?;

        let listed = client
            .request("tools/list", json!({}), STARTUP_TIMEOUT)
            .await
            .map_err(initialization_error)?;
        let tools = map_tools(&config.server_id, listed, &client)?;

        Ok(Self {
            client,
            tools,
            actor: Mutex::new(Some(actor)),
        })
    }

    /// Returns cloneable immutable proxies suitable for `ToolRegistry`.
    #[must_use]
    pub fn tools(&self) -> Vec<Arc<dyn Tool>> {
        self.tools.clone()
    }

    /// Gracefully closes and reaps the owned local child process.
    pub async fn shutdown(self) -> Result<(), McpAdapterError> {
        let (response, completion) = oneshot::channel();
        let sent = self.client.commands.send(ActorCommand::Shutdown(response));
        if sent.is_ok() {
            let _ = timeout(SHUTDOWN_TIMEOUT * 2, completion).await;
        }

        let actor = self
            .actor
            .lock()
            .map_err(|_| McpAdapterError::Shutdown {
                message: "supervisor state was poisoned".to_owned(),
            })?
            .take();
        if let Some(actor) = actor {
            timeout(SHUTDOWN_TIMEOUT * 2, actor)
                .await
                .map_err(|_| McpAdapterError::Shutdown {
                    message: "supervisor did not stop in time".to_owned(),
                })?
                .map_err(|error| McpAdapterError::Shutdown {
                    message: error.to_string(),
                })?;
        }
        Ok(())
    }
}

fn validate_server_id(server_id: &str) -> Result<(), McpAdapterError> {
    if server_id.is_empty()
        || server_id.len() > 64
        || !server_id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"_-".contains(&byte))
    {
        return Err(McpAdapterError::InvalidConfiguration {
            message: "server ID must contain 1-64 lowercase ASCII letters, digits, `_`, or `-`"
                .to_owned(),
        });
    }
    Ok(())
}

fn initialization_error(error: ClientError) -> McpAdapterError {
    McpAdapterError::Initialization {
        message: error.sanitized_message(),
    }
}

fn map_tools(
    server_id: &str,
    result: Value,
    client: &Client,
) -> Result<Vec<Arc<dyn Tool>>, McpAdapterError> {
    let listed = result["tools"]
        .as_array()
        .ok_or_else(|| McpAdapterError::Initialization {
            message: "tools/list did not return a tool array".to_owned(),
        })?;
    let mut mapped = Vec::<Arc<dyn Tool>>::with_capacity(listed.len());
    let mut remote_names = HashMap::new();
    for value in listed {
        let remote_name = value["name"]
            .as_str()
            .filter(|name| !name.is_empty())
            .ok_or_else(|| McpAdapterError::Initialization {
                message: "tools/list returned a tool without a valid name".to_owned(),
            })?;
        if remote_names.insert(remote_name.to_owned(), ()).is_some() {
            return Err(McpAdapterError::Initialization {
                message: "tools/list returned a duplicate tool name".to_owned(),
            });
        }
        let input_schema = value
            .get("inputSchema")
            .filter(|schema| schema.is_object())
            .cloned()
            .ok_or_else(|| McpAdapterError::Initialization {
                message: "tools/list returned a tool without an input schema object".to_owned(),
            })?;
        let description = value["description"].as_str().map_or_else(
            || format!("MCP tool `{remote_name}` from configured server `{server_id}`."),
            str::to_owned,
        );
        let tool = McpTool {
            definition: ToolDefinition {
                name: ToolName::new(format!("mcp.{server_id}.{remote_name}")),
                description,
                input_schema,
                permission: PermissionLevel::None,
            },
            remote_name: remote_name.to_owned(),
            client: client.clone(),
        };
        mapped.push(Arc::new(tool));
    }
    mapped.sort_by(|left, right| {
        left.definition()
            .name
            .as_str()
            .cmp(right.definition().name.as_str())
    });
    Ok(mapped)
}

struct McpTool {
    definition: ToolDefinition,
    remote_name: String,
    client: Client,
}

#[async_trait]
impl Tool for McpTool {
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
                message: "MCP tool arguments must be a JSON object".to_owned(),
            });
        }
        let result = self
            .client
            .request(
                "tools/call",
                json!({"name": self.remote_name, "arguments": input.0}),
                self.client.call_timeout,
            )
            .await
            .map_err(|error| ToolError::Execution {
                message: error.sanitized_message(),
            })?;
        map_call_result(result)
    }
}

fn map_call_result(result: Value) -> Result<ToolOutput, ToolError> {
    let content = result["content"]
        .as_array()
        .ok_or_else(|| ToolError::Execution {
            message: "MCP server returned a malformed tool result".to_owned(),
        })?;
    let mut mapped = Vec::with_capacity(content.len() + 1);
    for block in content {
        if block["type"] != "text" {
            return Err(ToolError::Execution {
                message: "MCP server returned unsupported tool content".to_owned(),
            });
        }
        let text = block["text"].as_str().ok_or_else(|| ToolError::Execution {
            message: "MCP server returned malformed text content".to_owned(),
        })?;
        mapped.push(ToolContent::Text(text.to_owned()));
    }
    if let Some(structured) = result.get("structuredContent") {
        mapped.push(ToolContent::Json(structured.clone()));
    }
    let is_error = match result.get("isError") {
        Some(value) => value.as_bool().ok_or_else(|| ToolError::Execution {
            message: "MCP server returned an invalid error flag".to_owned(),
        })?,
        None => false,
    };
    Ok(ToolOutput {
        content: mapped,
        is_error,
    })
}

#[derive(Clone)]
struct Client {
    commands: mpsc::UnboundedSender<ActorCommand>,
    next_id: Arc<AtomicU64>,
    call_timeout: Duration,
}

impl Client {
    async fn request(
        &self,
        method: &'static str,
        params: Value,
        request_timeout: Duration,
    ) -> Result<Value, ClientError> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (sender, receiver) = oneshot::channel();
        self.commands
            .send(ActorCommand::Request {
                id,
                method,
                params,
                response: sender,
            })
            .map_err(|_| ClientError::Disconnected)?;
        let mut guard = RequestGuard {
            id,
            commands: self.commands.clone(),
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
            .send(ActorCommand::Notification { method, params })
            .map_err(|_| ClientError::Disconnected)
    }
}

struct RequestGuard {
    id: u64,
    commands: mpsc::UnboundedSender<ActorCommand>,
    armed: bool,
}

impl Drop for RequestGuard {
    fn drop(&mut self) {
        if self.armed {
            let _ = self.commands.send(ActorCommand::Cancel { id: self.id });
        }
    }
}

enum ActorCommand {
    Request {
        id: u64,
        method: &'static str,
        params: Value,
        response: oneshot::Sender<Result<Value, ClientError>>,
    },
    Notification {
        method: &'static str,
        params: Value,
    },
    Cancel {
        id: u64,
    },
    Shutdown(oneshot::Sender<()>),
}

#[derive(Clone, Debug)]
enum ClientError {
    Disconnected,
    Timeout,
    Protocol,
    Remote,
}

impl ClientError {
    fn sanitized_message(&self) -> String {
        match self {
            Self::Disconnected => "MCP server disconnected".to_owned(),
            Self::Timeout => "MCP request timed out and was not replayed".to_owned(),
            Self::Protocol => "MCP protocol failure".to_owned(),
            Self::Remote => "MCP server rejected the request".to_owned(),
        }
    }
}

async fn run_actor(
    mut child: Child,
    mut stdin: tokio::process::ChildStdin,
    mut stdout: Lines<BufReader<ChildStdout>>,
    mut commands: mpsc::UnboundedReceiver<ActorCommand>,
) {
    let mut pending = HashMap::<u64, oneshot::Sender<Result<Value, ClientError>>>::new();
    loop {
        tokio::select! {
            command = commands.recv() => {
                let Some(command) = command else {
                    close_child(&mut child, &mut stdin).await;
                    break;
                };
                match command {
                    ActorCommand::Request { id, method, params, response } => {
                        let message = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
                        tracing::debug!(
                            target: "rah",
                            transport = "stdio",
                            payload = %message,
                            "sending MCP request"
                        );
                        pending.insert(id, response);
                        if write_message(&mut stdin, &message).await.is_err() {
                            fail_pending(&mut pending, ClientError::Disconnected);
                            close_child(&mut child, &mut stdin).await;
                            break;
                        }
                    }
                    ActorCommand::Notification { method, params } => {
                        let message = json!({"jsonrpc": "2.0", "method": method, "params": params});
                        if write_message(&mut stdin, &message).await.is_err() {
                            fail_pending(&mut pending, ClientError::Disconnected);
                            close_child(&mut child, &mut stdin).await;
                            break;
                        }
                    }
                    ActorCommand::Cancel { id } => {
                        if pending.remove(&id).is_some() {
                            let message = json!({
                                "jsonrpc": "2.0",
                                "method": "notifications/cancelled",
                                "params": {"requestId": id, "reason": "RAH request ended"}
                            });
                            if write_message(&mut stdin, &message).await.is_err() {
                                fail_pending(&mut pending, ClientError::Disconnected);
                                close_child(&mut child, &mut stdin).await;
                                break;
                            }
                        }
                    }
                    ActorCommand::Shutdown(response) => {
                        fail_pending(&mut pending, ClientError::Disconnected);
                        close_child(&mut child, &mut stdin).await;
                        let _ = response.send(());
                        break;
                    }
                }
            }
            line = stdout.next_line() => {
                match line {
                    Ok(Some(line)) => {
                        let Ok(message) = serde_json::from_str::<Value>(&line) else {
                            fail_pending(&mut pending, ClientError::Protocol);
                            close_child(&mut child, &mut stdin).await;
                            break;
                        };
                        let Some(id) = message["id"].as_u64() else {
                            continue;
                        };
                        let Some(response) = pending.remove(&id) else {
                            continue;
                        };
                        let result = if message.get("error").is_some() {
                            Err(ClientError::Remote)
                        } else if let Some(result) = message.get("result") {
                            Ok(result.clone())
                        } else {
                            Err(ClientError::Protocol)
                        };
                        let _ = response.send(result);
                    }
                    Ok(None) | Err(_) => {
                        fail_pending(&mut pending, ClientError::Disconnected);
                        close_child(&mut child, &mut stdin).await;
                        break;
                    }
                }
            }
            _ = child.wait() => {
                fail_pending(&mut pending, ClientError::Disconnected);
                break;
            }
        }
    }
}

async fn write_message(
    stdin: &mut tokio::process::ChildStdin,
    message: &Value,
) -> std::io::Result<()> {
    let mut serialized = serde_json::to_vec(message)?;
    serialized.push(b'\n');
    stdin.write_all(&serialized).await?;
    stdin.flush().await
}

fn fail_pending(
    pending: &mut HashMap<u64, oneshot::Sender<Result<Value, ClientError>>>,
    error: ClientError,
) {
    for (_, sender) in pending.drain() {
        let _ = sender.send(Err(error.clone()));
    }
}

async fn close_child(child: &mut Child, stdin: &mut tokio::process::ChildStdin) {
    let _ = stdin.shutdown().await;
    if timeout(SHUTDOWN_TIMEOUT, child.wait()).await.is_err() {
        let _ = child.kill().await;
        let _ = child.wait().await;
    }
}
