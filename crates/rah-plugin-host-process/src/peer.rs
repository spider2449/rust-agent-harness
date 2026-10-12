//! A bidirectional JSON-RPC 2.0 connection to a plugin process over
//! newline-delimited stdio. One actor future owns the child, its pipes and
//! the requests the plugin sends; dropping it kills the child.

use std::{
    collections::{HashMap, VecDeque},
    sync::{
        Arc, Mutex, PoisonError,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use futures::{FutureExt, StreamExt, future::BoxFuture, stream::FuturesUnordered};
use rah_api::CancellationToken;
use serde_json::{Value, json};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    process::{Child, ChildStdin},
    sync::{mpsc, oneshot},
};

/// A JSON-RPC error: code, message and optional data.
#[derive(Clone, Debug, PartialEq)]
pub struct RpcFault {
    pub code: i64,
    pub message: String,
    pub data: Option<Value>,
}

impl RpcFault {
    pub fn new(code: i64, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum PeerError {
    /// The connection is closed; nothing was sent.
    Closed,
    /// The request exceeds the message limit; nothing was sent.
    TooLarge,
    /// The connection ended while the request was outstanding.
    Disconnected,
    TimedOut,
    Cancelled,
    /// The plugin answered with an error.
    Remote(RpcFault),
    /// The plugin broke the protocol; the connection was closed.
    Protocol(String),
}

impl PeerError {
    pub fn not_sent(&self) -> bool {
        matches!(self, Self::Closed | Self::TooLarge)
    }
}

impl std::fmt::Display for PeerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Closed => f.write_str("the plugin process connection is closed"),
            Self::TooLarge => f.write_str("the request exceeds the message size limit"),
            Self::Disconnected => f.write_str(
                "the plugin process disconnected during the request; it was not replayed",
            ),
            Self::TimedOut => f.write_str("the plugin request timed out; it was not replayed"),
            Self::Cancelled => f.write_str("the plugin request was cancelled"),
            Self::Remote(fault) => write!(f, "{} ({})", fault.message, fault.code),
            Self::Protocol(message) => write!(f, "plugin protocol violation: {message}"),
        }
    }
}

/// Answers requests the plugin sends to the host.
pub type Inbound =
    Arc<dyn Fn(String, Value) -> BoxFuture<'static, Result<Value, RpcFault>> + Send + Sync>;

type Reply = oneshot::Sender<Result<Value, PeerError>>;

enum Command {
    Request {
        id: u64,
        method: &'static str,
        params: Value,
        reply: Reply,
    },
    Notify {
        method: &'static str,
        params: Value,
    },
    /// Gives up on a request and tells the plugin with `$/cancel`.
    Cancel(u64),
}

/// Sends [`Command::Cancel`] unless disarmed, also when the request future
/// is dropped.
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

/// The host side of a connection.
#[derive(Clone)]
pub struct Peer {
    commands: mpsc::UnboundedSender<Command>,
    next: Arc<AtomicU64>,
    stderr: Arc<Mutex<VecDeque<u8>>>,
}

/// Retained bytes of the plugin's stderr.
const STDERR_TAIL: usize = 16 * 1024;

impl Peer {
    /// Wraps a spawned child with piped stdin, stdout and stderr. The
    /// returned future is the connection; the caller must drive it.
    pub fn connect(
        mut child: Child,
        inbound: Inbound,
        max_message: usize,
    ) -> Result<(Self, BoxFuture<'static, ()>), String> {
        let (Some(stdin), Some(stdout), Some(stderr)) =
            (child.stdin.take(), child.stdout.take(), child.stderr.take())
        else {
            return Err("the plugin process pipes are missing".into());
        };
        let (commands, receiver) = mpsc::unbounded_channel();
        let tail = Arc::new(Mutex::new(VecDeque::new()));
        let peer = Self {
            commands,
            next: Arc::new(AtomicU64::new(1)),
            stderr: tail.clone(),
        };
        let io = Io {
            child,
            stdin,
            stdout: Box::new(stdout),
            stderr: Box::new(stderr),
            tail,
        };
        Ok((peer, actor(io, receiver, inbound, max_message).boxed()))
    }

    pub async fn request(
        &self,
        method: &'static str,
        params: Value,
        timeout: Duration,
        cancel: &CancellationToken,
    ) -> Result<Value, PeerError> {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let (reply, response) = oneshot::channel();
        self.commands
            .send(Command::Request {
                id,
                method,
                params,
                reply,
            })
            .map_err(|_| PeerError::Closed)?;
        let mut guard = CancelOnDrop {
            id,
            commands: self.commands.clone(),
            armed: true,
        };
        tokio::select! {
            response = response => {
                guard.armed = false;
                response.unwrap_or(Err(PeerError::Disconnected))
            }
            () = tokio::time::sleep(timeout) => Err(PeerError::TimedOut),
            () = cancel.cancelled() => Err(PeerError::Cancelled),
        }
    }

    pub fn notify(&self, method: &'static str, params: Value) -> Result<(), PeerError> {
        self.commands
            .send(Command::Notify { method, params })
            .map_err(|_| PeerError::Closed)
    }

    /// The retained tail of the plugin's stderr.
    pub fn stderr(&self) -> String {
        let tail = self.stderr.lock().unwrap_or_else(PoisonError::into_inner);
        let bytes: Vec<u8> = tail.iter().copied().collect();
        String::from_utf8_lossy(&bytes).trim().to_owned()
    }
}

struct Io {
    child: Child,
    stdin: ChildStdin,
    stdout: Box<dyn tokio::io::AsyncRead + Send + Unpin>,
    stderr: Box<dyn tokio::io::AsyncRead + Send + Unpin>,
    tail: Arc<Mutex<VecDeque<u8>>>,
}

async fn write_message(
    stdin: &mut ChildStdin,
    message: &Value,
    max: usize,
) -> Result<(), PeerError> {
    let mut bytes = serde_json::to_vec(message).map_err(|_| PeerError::TooLarge)?;
    if bytes.len() > max {
        return Err(PeerError::TooLarge);
    }
    bytes.push(b'\n');
    stdin
        .write_all(&bytes)
        .await
        .map_err(|_| PeerError::Disconnected)?;
    stdin.flush().await.map_err(|_| PeerError::Disconnected)
}

fn fault_of(error: &Value) -> RpcFault {
    RpcFault {
        code: error.get("code").and_then(Value::as_i64).unwrap_or(0),
        message: error
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("")
            .chars()
            .take(2000)
            .collect(),
        data: error.get("data").cloned(),
    }
}

async fn actor(
    mut io: Io,
    mut commands: mpsc::UnboundedReceiver<Command>,
    inbound: Inbound,
    max: usize,
) {
    let mut pending: HashMap<u64, Reply> = HashMap::new();
    let mut handling: FuturesUnordered<BoxFuture<'static, Value>> = FuturesUnordered::new();
    let mut buffer = Vec::new();
    let mut chunk = vec![0_u8; 8 * 1024];
    let mut err_chunk = vec![0_u8; 4 * 1024];
    let mut stderr_open = true;
    let failure = 'run: loop {
        tokio::select! {
            read = io.stdout.read(&mut chunk) => {
                let count = match read {
                    Ok(0) | Err(_) => break 'run PeerError::Disconnected,
                    Ok(count) => count,
                };
                buffer.extend_from_slice(&chunk[..count]);
                while let Some(end) = buffer.iter().position(|b| *b == b'\n') {
                    let line: Vec<u8> = buffer.drain(..=end).collect();
                    let line = line.trim_ascii();
                    if line.is_empty() {
                        continue;
                    }
                    let Ok(message) = serde_json::from_slice::<Value>(line) else {
                        break 'run PeerError::Protocol("stdout carried a line that is not JSON".into());
                    };
                    match (message.get("method").and_then(Value::as_str), message.get("id")) {
                        (Some(method), Some(id)) => {
                            let (id, method) = (id.clone(), method.to_owned());
                            let params = message.get("params").cloned().unwrap_or(Value::Null);
                            let answer = inbound(method, params);
                            handling.push(async move {
                                match answer.await {
                                    Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
                                    Err(fault) => json!({
                                        "jsonrpc": "2.0",
                                        "id": id,
                                        "error": { "code": fault.code, "message": fault.message, "data": fault.data }
                                    }),
                                }
                            }.boxed());
                        }
                        // Notifications from the plugin are not used yet.
                        (Some(_), None) => {}
                        (None, Some(id)) => {
                            // Answers to requests already given up on are ignored.
                            if let Some(reply) = id.as_u64().and_then(|id| pending.remove(&id)) {
                                let result = match (message.get("result"), message.get("error")) {
                                    (_, Some(error)) => Err(PeerError::Remote(fault_of(error))),
                                    (Some(result), None) => Ok(result.clone()),
                                    (None, None) => Err(PeerError::Protocol("response without result".into())),
                                };
                                let _ = reply.send(result);
                            }
                        }
                        (None, None) => {
                            break 'run PeerError::Protocol("message has neither method nor id".into());
                        }
                    }
                }
                if buffer.len() > max {
                    break 'run PeerError::Protocol("a message exceeds the size limit".into());
                }
            }
            read = io.stderr.read(&mut err_chunk), if stderr_open => match read {
                Ok(0) | Err(_) => stderr_open = false,
                Ok(count) => {
                    let mut tail = io.tail.lock().unwrap_or_else(PoisonError::into_inner);
                    tail.extend(&err_chunk[..count]);
                    let excess = tail.len().saturating_sub(STDERR_TAIL);
                    tail.drain(..excess);
                }
            },
            Some(answer) = handling.next() => {
                if let Err(error) = write_message(&mut io.stdin, &answer, max).await {
                    break 'run error;
                }
            }
            command = commands.recv() => match command {
                None => break 'run PeerError::Closed,
                Some(Command::Request { id, method, params, reply }) => {
                    let message = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });
                    match write_message(&mut io.stdin, &message, max).await {
                        Ok(()) => {
                            pending.insert(id, reply);
                        }
                        Err(PeerError::TooLarge) => {
                            let _ = reply.send(Err(PeerError::TooLarge));
                        }
                        Err(error) => {
                            let _ = reply.send(Err(PeerError::Disconnected));
                            break 'run error;
                        }
                    }
                }
                Some(Command::Notify { method, params }) => {
                    let message = json!({ "jsonrpc": "2.0", "method": method, "params": params });
                    if let Err(error @ PeerError::Disconnected) = write_message(&mut io.stdin, &message, max).await {
                        break 'run error;
                    }
                }
                Some(Command::Cancel(id)) => {
                    if pending.remove(&id).is_some() {
                        let message = json!({ "jsonrpc": "2.0", "method": "$/cancel", "params": { "id": id } });
                        if let Err(error @ PeerError::Disconnected) = write_message(&mut io.stdin, &message, max).await {
                            break 'run error;
                        }
                    }
                }
            },
            _ = io.child.wait() => break 'run PeerError::Disconnected,
        }
    };
    for (_, reply) in pending.drain() {
        let _ = reply.send(Err(failure.clone()));
    }
    commands.close();
    while let Ok(command) = commands.try_recv() {
        if let Command::Request { reply, .. } = command {
            let _ = reply.send(Err(PeerError::Closed));
        }
    }
    // Give the plugin a moment to exit after its stdin closes, then kill it.
    drop(io.stdin);
    if tokio::time::timeout(Duration::from_millis(500), io.child.wait())
        .await
        .is_err()
    {
        let _ = io.child.kill().await;
    }
}
