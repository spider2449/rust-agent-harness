//! A JSON-RPC 2.0 client over newline-delimited stdio (MCP stdio
//! transport). One actor task, owned by the plugin scope, holds the pipes and
//! the process handle; dropping it terminates the server.

use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use rah_api::{
    CancellationToken,
    sandbox::{ProcessHandle, Spawned},
};
use serde_json::{Value, json};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    sync::{OwnedSemaphorePermit, Semaphore, mpsc, oneshot},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum RpcError {
    /// Too many requests outstanding; nothing was sent.
    Busy,
    /// The connection was already closed; nothing was sent.
    Closed,
    /// The request could not be encoded within the message limit; nothing
    /// was sent.
    TooLarge,
    /// The connection ended while the request was outstanding.
    Disconnected,
    TimedOut,
    Cancelled,
    /// The server answered with a JSON-RPC error.
    Remote {
        code: i64,
        message: String,
    },
    /// The server broke the protocol; the connection was closed.
    Protocol(String),
}

impl RpcError {
    /// Whether the request certainly never reached the server.
    pub fn not_sent(&self) -> bool {
        matches!(self, Self::Busy | Self::Closed | Self::TooLarge)
    }
}

impl std::fmt::Display for RpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Busy => f.write_str("the MCP server has too many requests outstanding"),
            Self::Closed => f.write_str("the MCP server connection is closed"),
            Self::TooLarge => f.write_str("the request exceeds the message size limit"),
            Self::Disconnected => {
                f.write_str("the MCP server disconnected during the request; it was not replayed")
            }
            Self::TimedOut => f.write_str("the MCP request timed out; it was not replayed"),
            Self::Cancelled => f.write_str("the MCP request was cancelled"),
            Self::Remote { code, message } => {
                write!(f, "the MCP server returned error {code}: {message}")
            }
            Self::Protocol(message) => write!(f, "MCP protocol violation: {message}"),
        }
    }
}

type Reply = oneshot::Sender<Result<Value, RpcError>>;

enum Command {
    Request {
        id: u64,
        method: &'static str,
        params: Value,
        reply: Reply,
        permit: OwnedSemaphorePermit,
    },
    Notify {
        method: &'static str,
        params: Value,
    },
    Cancel(u64),
}

#[derive(Clone)]
pub(crate) struct Client {
    commands: mpsc::UnboundedSender<Command>,
    permits: Arc<Semaphore>,
    next: Arc<AtomicU64>,
}

/// Sends `notifications/cancelled` unless disarmed.
struct CancelOnDrop {
    id: u64,
    commands: mpsc::UnboundedSender<Command>,
    armed: bool,
}

impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        if self.armed {
            let _ = self.commands.send(Command::Cancel(self.id));
        }
    }
}

impl Client {
    /// Returns the client and the actor future, which the caller must run
    /// in a scope-owned task.
    pub fn new(
        spawned: Spawned,
        max_outstanding: usize,
        max_message_bytes: usize,
    ) -> (
        Self,
        Arc<dyn ProcessHandle>,
        impl Future<Output = ()> + Send,
    ) {
        let (commands, receiver) = mpsc::unbounded_channel();
        let process: Arc<dyn ProcessHandle> = Arc::from(spawned.process);
        let actor = actor(
            spawned.stdin,
            spawned.stdout,
            process.clone(),
            receiver,
            max_message_bytes,
        );
        let client = Self {
            commands,
            permits: Arc::new(Semaphore::new(max_outstanding)),
            next: Arc::new(AtomicU64::new(1)),
        };
        (client, process, actor)
    }

    /// Sends one request and waits for its result. A timeout or
    /// cancellation tells the server with `notifications/cancelled`.
    pub async fn request(
        &self,
        method: &'static str,
        params: Value,
        timeout: Duration,
        cancel: &CancellationToken,
    ) -> Result<Value, RpcError> {
        let permit = self
            .permits
            .clone()
            .try_acquire_owned()
            .map_err(|_| RpcError::Busy)?;
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let (reply, response) = oneshot::channel();
        self.commands
            .send(Command::Request {
                id,
                method,
                params,
                reply,
                permit,
            })
            .map_err(|_| RpcError::Closed)?;
        let mut guard = CancelOnDrop {
            id,
            commands: self.commands.clone(),
            armed: true,
        };
        let result = tokio::select! {
            response = response => {
                guard.armed = false;
                response.unwrap_or(Err(RpcError::Disconnected))
            }
            () = tokio::time::sleep(timeout) => Err(RpcError::TimedOut),
            () = cancel.cancelled() => Err(RpcError::Cancelled),
        };
        drop(guard);
        result
    }

    pub fn notify(&self, method: &'static str, params: Value) -> Result<(), RpcError> {
        self.commands
            .send(Command::Notify { method, params })
            .map_err(|_| RpcError::Closed)
    }
}

async fn write_message(
    stdin: &mut (dyn AsyncWrite + Send + Unpin),
    message: &Value,
    max: usize,
) -> Result<(), RpcError> {
    let mut bytes = serde_json::to_vec(message).map_err(|_| RpcError::TooLarge)?;
    if bytes.len() > max {
        return Err(RpcError::TooLarge);
    }
    bytes.push(b'\n');
    stdin
        .write_all(&bytes)
        .await
        .map_err(|_| RpcError::Disconnected)?;
    stdin.flush().await.map_err(|_| RpcError::Disconnected)
}

/// What to do with one incoming message.
enum Incoming {
    Response {
        id: u64,
        result: Result<Value, RpcError>,
    },
    /// A server-to-client request that needs this answer.
    Answer(Value),
    Ignore,
}

fn classify(line: &[u8]) -> Result<Incoming, RpcError> {
    let message: Value = serde_json::from_slice(line)
        .map_err(|_| RpcError::Protocol("stdout carried a line that is not JSON".into()))?;
    let Some(object) = message.as_object() else {
        return Err(RpcError::Protocol("message is not a JSON object".into()));
    };
    match (object.get("method"), object.get("id")) {
        // Server request: answer `ping`, refuse everything else.
        (Some(method), Some(id)) => {
            let response = if method == "ping" {
                json!({ "jsonrpc": "2.0", "id": id, "result": {} })
            } else {
                json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "error": { "code": -32601, "message": "method not supported by RAH" }
                })
            };
            Ok(Incoming::Answer(response))
        }
        (Some(_), None) => Ok(Incoming::Ignore),
        (None, Some(id)) => {
            // Responses to requests already given up on are ignored.
            let Some(id) = id.as_u64() else {
                return Ok(Incoming::Ignore);
            };
            let result = if let Some(error) = object.get("error") {
                Err(RpcError::Remote {
                    code: error.get("code").and_then(Value::as_i64).unwrap_or(0),
                    message: error
                        .get("message")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .chars()
                        .take(1000)
                        .collect(),
                })
            } else if let Some(result) = object.get("result") {
                Ok(result.clone())
            } else {
                Err(RpcError::Protocol(
                    "response has neither result nor error".into(),
                ))
            };
            Ok(Incoming::Response { id, result })
        }
        (None, None) => Err(RpcError::Protocol(
            "message has neither method nor id".into(),
        )),
    }
}

async fn actor(
    mut stdin: Box<dyn AsyncWrite + Send + Unpin>,
    mut stdout: Box<dyn AsyncRead + Send + Unpin>,
    process: Arc<dyn ProcessHandle>,
    mut commands: mpsc::UnboundedReceiver<Command>,
    max: usize,
) {
    let mut pending: HashMap<u64, (Reply, OwnedSemaphorePermit)> = HashMap::new();
    let mut buffer: Vec<u8> = Vec::new();
    let mut chunk = vec![0_u8; 8 * 1024];
    let failure = 'run: loop {
        tokio::select! {
            read = stdout.read(&mut chunk) => {
                let count = match read {
                    Ok(0) | Err(_) => break 'run RpcError::Disconnected,
                    Ok(count) => count,
                };
                buffer.extend_from_slice(&chunk[..count]);
                while let Some(end) = buffer.iter().position(|b| *b == b'\n') {
                    let line: Vec<u8> = buffer.drain(..=end).collect();
                    let line = line.trim_ascii();
                    if line.is_empty() {
                        continue;
                    }
                    match classify(line) {
                        Ok(Incoming::Response { id, result }) => {
                            if let Some((reply, _permit)) = pending.remove(&id) {
                                let _ = reply.send(result);
                            }
                        }
                        Ok(Incoming::Answer(answer)) => {
                            if let Err(error) = write_message(stdin.as_mut(), &answer, max).await {
                                break 'run error;
                            }
                        }
                        Ok(Incoming::Ignore) => {}
                        Err(error) => break 'run error,
                    }
                }
                if buffer.len() > max {
                    break 'run RpcError::Protocol("a message exceeds the size limit".into());
                }
            }
            command = commands.recv() => match command {
                // Every client handle is gone.
                None => break 'run RpcError::Closed,
                Some(Command::Request { id, method, params, reply, permit }) => {
                    let message = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });
                    match write_message(stdin.as_mut(), &message, max).await {
                        Ok(()) => {
                            pending.insert(id, (reply, permit));
                        }
                        Err(RpcError::TooLarge) => {
                            let _ = reply.send(Err(RpcError::TooLarge));
                        }
                        Err(error) => {
                            // The write may have been partial: uncertain.
                            let _ = reply.send(Err(RpcError::Disconnected));
                            break 'run error;
                        }
                    }
                }
                Some(Command::Notify { method, params }) => {
                    let message = json!({ "jsonrpc": "2.0", "method": method, "params": params });
                    if let Err(error @ RpcError::Disconnected) =
                        write_message(stdin.as_mut(), &message, max).await
                    {
                        break 'run error;
                    }
                }
                Some(Command::Cancel(id)) => {
                    if pending.remove(&id).is_some() {
                        let message = json!({
                            "jsonrpc": "2.0",
                            "method": "notifications/cancelled",
                            "params": { "requestId": id, "reason": "cancelled by RAH" }
                        });
                        if let Err(error @ RpcError::Disconnected) =
                            write_message(stdin.as_mut(), &message, max).await
                        {
                            break 'run error;
                        }
                    }
                }
            },
            _ = process.wait() => break 'run RpcError::Disconnected,
        }
    };
    for (_, (reply, _permit)) in pending.drain() {
        let _ = reply.send(Err(failure.clone()));
    }
    // Refuse later requests, including queued ones that were never written,
    // then end the server.
    commands.close();
    while let Ok(command) = commands.try_recv() {
        if let Command::Request { reply, .. } = command {
            let _ = reply.send(Err(RpcError::Closed));
        }
    }
    process.terminate();
}
