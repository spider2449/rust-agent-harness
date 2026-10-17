//! A small Language Server Protocol client: JSON-RPC 2.0 over stdio with
//! `Content-Length` framing. One actor task, owned by the plugin scope, holds
//! the pipes and the process handle; dropping it terminates the server. The
//! client only follows `textDocument/publishDiagnostics`.

use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex, PoisonError,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use rah_api::sandbox::{ProcessHandle, Spawned};
use serde_json::{Value, json};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    sync::{mpsc, oneshot, watch},
};

/// Largest message accepted from or sent to a server.
const MAX_MESSAGE_BYTES: usize = 16 * 1024 * 1024;
/// Largest header block accepted.
const MAX_HEADER_BYTES: usize = 8 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Severity {
    Error = 1,
    Warning = 2,
    Information = 3,
    Hint = 4,
}

impl Severity {
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "error" => Some(Self::Error),
            "warning" => Some(Self::Warning),
            "information" => Some(Self::Information),
            "hint" => Some(Self::Hint),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Information => "info",
            Self::Hint => "hint",
        }
    }

    fn from_code(code: Option<u64>) -> Self {
        match code {
            Some(2) => Self::Warning,
            Some(3) => Self::Information,
            Some(4) => Self::Hint,
            // The specification leaves a missing severity to the client.
            _ => Self::Error,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Diagnostic {
    /// Zero-based, as on the wire.
    pub line: u64,
    pub column: u64,
    pub severity: Severity,
    pub message: String,
    pub source: Option<String>,
    pub code: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum LspError {
    /// The connection is closed; nothing was sent.
    Closed,
    /// The connection ended while the request was outstanding.
    Disconnected,
    TimedOut,
    /// The server answered with a JSON-RPC error.
    Remote(String),
    /// The server broke the protocol; the connection was closed.
    Protocol(String),
}

impl std::fmt::Display for LspError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Closed => f.write_str("the language server connection is closed"),
            Self::Disconnected => f.write_str("the language server disconnected"),
            Self::TimedOut => f.write_str("the language server did not answer in time"),
            Self::Remote(message) => write!(f, "the language server returned an error: {message}"),
            Self::Protocol(message) => write!(f, "language server protocol violation: {message}"),
        }
    }
}

struct Entry {
    /// Counts the publications for this document; a waiter compares it with
    /// the value it saw before it changed the document.
    generation: u64,
    diagnostics: Vec<Diagnostic>,
}

/// What the actor learns from the server, shared with the callers.
#[derive(Default)]
pub(crate) struct Shared {
    entries: Mutex<HashMap<String, Entry>>,
    /// The version last sent for each open document.
    versions: Mutex<HashMap<String, i64>>,
    publications: watch::Sender<u64>,
}

/// A key under which the spellings of one document URI meet: servers
/// percent-encode the drive colon on Windows and change the case of the
/// drive letter.
pub(crate) fn uri_key(uri: &str) -> String {
    let rest = uri.strip_prefix("file://").unwrap_or(uri);
    let bytes = rest.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        let hex = bytes
            .get(index + 1..index + 3)
            .and_then(|pair| std::str::from_utf8(pair).ok())
            .filter(|_| bytes[index] == b'%')
            .and_then(|pair| u8::from_str_radix(pair, 16).ok());
        match hex {
            Some(byte) => {
                decoded.push(byte);
                index += 3;
            }
            None => {
                decoded.push(bytes[index]);
                index += 1;
            }
        }
    }
    let text = String::from_utf8_lossy(&decoded).into_owned();
    let drive = text.len() > 2
        && text.starts_with('/')
        && text.as_bytes()[1].is_ascii_alphabetic()
        && text.as_bytes()[2] == b':';
    if drive {
        text[1..].to_ascii_lowercase()
    } else {
        text
    }
}

impl Shared {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// The number of publications seen for `uri`.
    pub fn generation(&self, uri: &str) -> u64 {
        let key = uri_key(uri);
        self.entries
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&key)
            .map_or(0, |entry| entry.generation)
    }

    pub fn diagnostics(&self, uri: &str) -> Vec<Diagnostic> {
        let key = uri_key(uri);
        self.entries
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&key)
            .map(|entry| entry.diagnostics.clone())
            .unwrap_or_default()
    }

    pub fn subscribe(&self) -> watch::Receiver<u64> {
        self.publications.subscribe()
    }

    /// Records the version about to be sent and returns the previous one.
    pub fn bump_version(&self, uri: &str) -> Option<i64> {
        let mut versions = self.versions.lock().unwrap_or_else(PoisonError::into_inner);
        let slot = versions.entry(uri_key(uri));
        match slot {
            std::collections::hash_map::Entry::Occupied(mut entry) => {
                let previous = *entry.get();
                entry.insert(previous + 1);
                Some(previous)
            }
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(1);
                None
            }
        }
    }

    pub fn version(&self, uri: &str) -> i64 {
        let key = uri_key(uri);
        self.versions
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&key)
            .copied()
            .unwrap_or(0)
    }

    fn publish(&self, uri: String, version: Option<i64>, diagnostics: Vec<Diagnostic>) {
        // A publication for an older version of the document describes text
        // that is no longer there.
        if version.is_some_and(|version| version < self.version(&uri)) {
            return;
        }
        {
            let mut entries = self.entries.lock().unwrap_or_else(PoisonError::into_inner);
            let entry = entries.entry(uri_key(&uri)).or_insert(Entry {
                generation: 0,
                diagnostics: Vec::new(),
            });
            entry.generation += 1;
            entry.diagnostics = diagnostics;
        }
        self.publications.send_modify(|count| *count += 1);
    }
}

enum Command {
    Request {
        id: u64,
        method: &'static str,
        params: Value,
        reply: oneshot::Sender<Result<Value, LspError>>,
    },
    Notify {
        method: &'static str,
        params: Value,
    },
}

#[derive(Clone)]
pub(crate) struct Client {
    commands: mpsc::UnboundedSender<Command>,
    next: Arc<AtomicU64>,
    pub shared: Arc<Shared>,
    process: Arc<dyn ProcessHandle>,
}

impl Client {
    /// Returns the client and the actor future, which the caller must run in
    /// a scope-owned task.
    pub fn new(
        spawned: Spawned,
        root_uri: String,
        shared: Arc<Shared>,
    ) -> (Self, impl Future<Output = ()> + Send) {
        let (commands, receiver) = mpsc::unbounded_channel();
        let process: Arc<dyn ProcessHandle> = Arc::from(spawned.process);
        let actor = actor(
            spawned.stdin,
            spawned.stdout,
            process.clone(),
            receiver,
            shared.clone(),
            root_uri,
        );
        let client = Self {
            commands,
            next: Arc::new(AtomicU64::new(1)),
            shared,
            process,
        };
        (client, actor)
    }

    /// Whether the connection has ended.
    pub fn is_closed(&self) -> bool {
        self.commands.is_closed()
    }

    /// The retained tail of the server's stderr.
    pub fn stderr(&self) -> String {
        self.process.stderr()
    }

    pub async fn request(
        &self,
        method: &'static str,
        params: Value,
        timeout: Duration,
    ) -> Result<Value, LspError> {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let (reply, response) = oneshot::channel();
        self.commands
            .send(Command::Request {
                id,
                method,
                params,
                reply,
            })
            .map_err(|_| LspError::Closed)?;
        match tokio::time::timeout(timeout, response).await {
            Ok(response) => response.unwrap_or(Err(LspError::Disconnected)),
            Err(_) => Err(LspError::TimedOut),
        }
    }

    pub fn notify(&self, method: &'static str, params: Value) -> Result<(), LspError> {
        self.commands
            .send(Command::Notify { method, params })
            .map_err(|_| LspError::Closed)
    }
}

async fn write_message(
    stdin: &mut (dyn AsyncWrite + Send + Unpin),
    message: &Value,
) -> Result<(), LspError> {
    let body = serde_json::to_vec(message).map_err(|_| LspError::Protocol("unencodable".into()))?;
    if body.len() > MAX_MESSAGE_BYTES {
        return Err(LspError::Protocol(
            "a message exceeds the size limit".into(),
        ));
    }
    let mut bytes = format!("Content-Length: {}\r\n\r\n", body.len()).into_bytes();
    bytes.extend_from_slice(&body);
    stdin
        .write_all(&bytes)
        .await
        .map_err(|_| LspError::Disconnected)?;
    stdin.flush().await.map_err(|_| LspError::Disconnected)
}

/// Splits the next complete message off `buffer`. `Ok(None)` means more bytes
/// are needed.
pub(crate) fn next_frame(buffer: &mut Vec<u8>) -> Result<Option<Vec<u8>>, LspError> {
    let Some(end) = buffer.windows(4).position(|window| window == b"\r\n\r\n") else {
        if buffer.len() > MAX_HEADER_BYTES {
            return Err(LspError::Protocol("the header block is too long".into()));
        }
        return Ok(None);
    };
    let header = std::str::from_utf8(&buffer[..end])
        .map_err(|_| LspError::Protocol("the header block is not text".into()))?;
    let length = header
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.trim()
                .eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().ok())
                .flatten()
        })
        .ok_or_else(|| LspError::Protocol("a message has no Content-Length".into()))?;
    if length > MAX_MESSAGE_BYTES {
        return Err(LspError::Protocol(
            "a message exceeds the size limit".into(),
        ));
    }
    let start = end + 4;
    if buffer.len() < start + length {
        return Ok(None);
    }
    let body = buffer[start..start + length].to_vec();
    buffer.drain(..start + length);
    Ok(Some(body))
}

/// What to do with one incoming message.
enum Incoming {
    Response {
        id: u64,
        result: Result<Value, LspError>,
    },
    /// A server-to-client request that needs this answer.
    Answer(Value),
    Handled,
}

fn classify(body: &[u8], shared: &Shared, root_uri: &str) -> Result<Incoming, LspError> {
    let message: Value = serde_json::from_slice(body)
        .map_err(|_| LspError::Protocol("the server sent a message that is not JSON".into()))?;
    let Some(object) = message.as_object() else {
        return Err(LspError::Protocol("message is not a JSON object".into()));
    };
    let params = object.get("params");
    match (
        object.get("method").and_then(Value::as_str),
        object.get("id"),
    ) {
        (Some(method), Some(id)) => Ok(Incoming::Answer(match method {
            "workspace/configuration" => {
                let count = params
                    .and_then(|params| params.get("items"))
                    .and_then(Value::as_array)
                    .map_or(0, Vec::len);
                json!({ "jsonrpc": "2.0", "id": id, "result": vec![Value::Null; count] })
            }
            "workspace/workspaceFolders" => json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": [{ "uri": root_uri, "name": "workspace" }]
            }),
            "client/registerCapability"
            | "client/unregisterCapability"
            | "window/workDoneProgress/create"
            | "window/showMessageRequest"
            | "workspace/diagnostic/refresh" => {
                json!({ "jsonrpc": "2.0", "id": id, "result": null })
            }
            _ => json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": { "code": -32601, "message": "method not supported by RAH" }
            }),
        })),
        (Some("textDocument/publishDiagnostics"), None) => {
            if let Some((uri, version, diagnostics)) = params.and_then(parse_publication) {
                shared.publish(uri, version, diagnostics);
            }
            Ok(Incoming::Handled)
        }
        (Some(_), None) => Ok(Incoming::Handled),
        (None, Some(id)) => {
            let Some(id) = id.as_u64() else {
                return Ok(Incoming::Handled);
            };
            let result = if let Some(error) = object.get("error") {
                Err(LspError::Remote(
                    error
                        .get("message")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .chars()
                        .take(500)
                        .collect(),
                ))
            } else {
                Ok(object.get("result").cloned().unwrap_or(Value::Null))
            };
            Ok(Incoming::Response { id, result })
        }
        (None, None) => Err(LspError::Protocol(
            "message has neither method nor id".into(),
        )),
    }
}

fn parse_publication(params: &Value) -> Option<(String, Option<i64>, Vec<Diagnostic>)> {
    let uri = params.get("uri")?.as_str()?.to_owned();
    let version = params.get("version").and_then(Value::as_i64);
    let diagnostics = params
        .get("diagnostics")?
        .as_array()?
        .iter()
        .filter_map(|item| {
            let start = item.get("range")?.get("start")?;
            Some(Diagnostic {
                line: start.get("line")?.as_u64()?,
                column: start.get("character")?.as_u64()?,
                severity: Severity::from_code(item.get("severity").and_then(Value::as_u64)),
                message: item.get("message")?.as_str()?.to_owned(),
                source: item
                    .get("source")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                code: item.get("code").and_then(|code| match code {
                    Value::String(text) => Some(text.clone()),
                    Value::Number(number) => Some(number.to_string()),
                    _ => None,
                }),
            })
        })
        .collect();
    Some((uri, version, diagnostics))
}

async fn actor(
    mut stdin: Box<dyn AsyncWrite + Send + Unpin>,
    mut stdout: Box<dyn AsyncRead + Send + Unpin>,
    process: Arc<dyn ProcessHandle>,
    mut commands: mpsc::UnboundedReceiver<Command>,
    shared: Arc<Shared>,
    root_uri: String,
) {
    type Reply = oneshot::Sender<Result<Value, LspError>>;
    let mut pending: HashMap<u64, Reply> = HashMap::new();
    let mut buffer: Vec<u8> = Vec::new();
    let mut chunk = vec![0_u8; 16 * 1024];
    let failure = 'run: loop {
        tokio::select! {
            read = stdout.read(&mut chunk) => {
                let count = match read {
                    Ok(0) | Err(_) => break 'run LspError::Disconnected,
                    Ok(count) => count,
                };
                buffer.extend_from_slice(&chunk[..count]);
                loop {
                    let body = match next_frame(&mut buffer) {
                        Ok(Some(body)) => body,
                        Ok(None) => break,
                        Err(error) => break 'run error,
                    };
                    match classify(&body, &shared, &root_uri) {
                        Ok(Incoming::Response { id, result }) => {
                            if let Some(reply) = pending.remove(&id) {
                                let _ = reply.send(result);
                            }
                        }
                        Ok(Incoming::Answer(answer)) => {
                            if let Err(error) = write_message(stdin.as_mut(), &answer).await {
                                break 'run error;
                            }
                        }
                        Ok(Incoming::Handled) => {}
                        Err(error) => break 'run error,
                    }
                }
            }
            command = commands.recv() => match command {
                // Every client handle is gone.
                None => break 'run LspError::Closed,
                Some(Command::Request { id, method, params, reply }) => {
                    let message = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });
                    match write_message(stdin.as_mut(), &message).await {
                        Ok(()) => {
                            pending.insert(id, reply);
                        }
                        Err(error) => {
                            let _ = reply.send(Err(error.clone()));
                            break 'run error;
                        }
                    }
                }
                Some(Command::Notify { method, params }) => {
                    let message = json!({ "jsonrpc": "2.0", "method": method, "params": params });
                    if let Err(error) = write_message(stdin.as_mut(), &message).await {
                        break 'run error;
                    }
                }
            },
            _ = process.wait() => break 'run LspError::Disconnected,
        }
    };
    for (_, reply) in pending.drain() {
        let _ = reply.send(Err(failure.clone()));
    }
    // Refuse later requests, then end the server.
    commands.close();
    while let Ok(command) = commands.try_recv() {
        if let Command::Request { reply, .. } = command {
            let _ = reply.send(Err(LspError::Closed));
        }
    }
    process.terminate();
}
