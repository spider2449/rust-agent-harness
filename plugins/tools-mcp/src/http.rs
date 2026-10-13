//! The MCP Streamable HTTP transport (protocol 2025-03-26 and later, ADR
//! 0038). Every JSON-RPC message is one `POST`; the reply is one JSON body or
//! an SSE stream that ends with the response. No `GET` stream is opened, and
//! a broken stream is never resumed.

use std::{
    net::IpAddr,
    sync::{
        Arc, Mutex, PoisonError,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Duration,
};

use rah_api::{CancellationToken, kernel::Spawner};
use reqwest::{
    StatusCode, Url,
    header::{ACCEPT, CONTENT_TYPE, HeaderValue},
};
use serde_json::{Value, json};
use tokio::sync::Semaphore;

use crate::client::{Incoming, RpcError, classify};

const SESSION_HEADER: &str = "mcp-session-id";
const VERSION_HEADER: &str = "mcp-protocol-version";
/// How long a cancellation notice, a `ping` answer or the closing `DELETE`
/// may take.
const NOTICE_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_SESSION_ID: usize = 512;

/// Checks a configured endpoint: `https` for any host, `http` only for a
/// loopback host, without userinfo or fragment. Returns the URL and whether
/// it is `https`.
pub fn endpoint(url: &str) -> Result<(Url, bool), String> {
    let parsed = Url::parse(url).map_err(|error| format!("`url` is not a valid URL: {error}"))?;
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err("`url` must not contain a user name or password".into());
    }
    if parsed.fragment().is_some() {
        return Err("`url` must not contain a fragment".into());
    }
    let host = parsed.host_str().unwrap_or_default();
    match parsed.scheme() {
        "https" if !host.is_empty() => Ok((parsed, true)),
        "http" if is_loopback(host) => Ok((parsed, false)),
        "http" => Err("an `http` url must name a loopback host; use `https` otherwise".into()),
        _ => Err("`url` must use `https`, or `http` for a loopback host".into()),
    }
}

fn is_loopback(host: &str) -> bool {
    host.eq_ignore_ascii_case("localhost")
        || host
            .trim_start_matches('[')
            .trim_end_matches(']')
            .parse::<IpAddr>()
            .is_ok_and(|address| address.is_loopback())
}

/// A Streamable HTTP connection to one MCP endpoint. The bearer token is
/// never shown: this type has no `Debug`, and errors do not include it.
#[derive(Clone)]
pub(crate) struct HttpClient {
    inner: Arc<Inner>,
}

struct Inner {
    client: reqwest::Client,
    url: Url,
    authorization: Option<HeaderValue>,
    session: Mutex<Option<HeaderValue>>,
    version: Mutex<Option<HeaderValue>>,
    ended: AtomicBool,
    permits: Arc<Semaphore>,
    next: AtomicU64,
    max: usize,
    /// Runs cancellation notices in tasks owned by the plugin scope.
    spawner: Spawner,
}

impl HttpClient {
    pub fn new(
        url: Url,
        https: bool,
        token: Option<String>,
        max_outstanding: usize,
        max_message_bytes: usize,
        spawner: Spawner,
    ) -> Result<Self, String> {
        let client = reqwest::Client::builder()
            .https_only(https)
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .no_proxy()
            .connect_timeout(Duration::from_secs(30))
            .build()
            .map_err(|_| "cannot build the HTTP client".to_owned())?;
        let authorization = token
            .map(|token| {
                let mut value = HeaderValue::from_str(&format!("Bearer {token}"))
                    .map_err(|_| "the bearer token is not a valid header value".to_owned())?;
                value.set_sensitive(true);
                Ok::<_, String>(value)
            })
            .transpose()?;
        Ok(Self {
            inner: Arc::new(Inner {
                client,
                url,
                authorization,
                session: Mutex::new(None),
                version: Mutex::new(None),
                ended: AtomicBool::new(false),
                permits: Arc::new(Semaphore::new(max_outstanding)),
                next: AtomicU64::new(1),
                max: max_message_bytes,
                spawner,
            }),
        })
    }

    /// Sends one request and waits for its response. A timeout or
    /// cancellation tells the server with `notifications/cancelled`; the
    /// request is never sent again.
    pub async fn request(
        &self,
        method: &'static str,
        params: Value,
        timeout: Duration,
        cancel: &CancellationToken,
    ) -> Result<Value, RpcError> {
        if self.inner.ended.load(Ordering::Acquire) {
            return Err(RpcError::SessionEnded);
        }
        let _permit = self
            .inner
            .permits
            .clone()
            .try_acquire_owned()
            .map_err(|_| RpcError::Busy)?;
        let id = self.inner.next.fetch_add(1, Ordering::Relaxed);
        let body = self.inner.encode(&json!({
            "jsonrpc": "2.0", "id": id, "method": method, "params": params
        }))?;
        tokio::select! {
            result = self.inner.exchange(body, id) => result,
            () = tokio::time::sleep(timeout) => {
                self.cancelled(id);
                Err(RpcError::TimedOut)
            }
            () = cancel.cancelled() => {
                self.cancelled(id);
                Err(RpcError::Cancelled)
            }
        }
    }

    pub async fn notify(&self, method: &'static str, params: Value) -> Result<(), RpcError> {
        let body = self.inner.encode(&json!({
            "jsonrpc": "2.0", "method": method, "params": params
        }))?;
        self.inner.send_notice(body).await
    }

    /// The protocol version `initialize` agreed on, sent with every later
    /// request.
    pub fn set_version(&self, version: &str) {
        if let Ok(value) = HeaderValue::from_str(version) {
            *lock(&self.inner.version) = Some(value);
        }
    }

    /// Ends the session with `DELETE`, best effort.
    pub async fn close(&self) {
        let Some(session) = lock(&self.inner.session).clone() else {
            return;
        };
        let mut request = self
            .inner
            .client
            .delete(self.inner.url.clone())
            .header(SESSION_HEADER, session);
        if let Some(authorization) = &self.inner.authorization {
            request = request.header(reqwest::header::AUTHORIZATION, authorization.clone());
        }
        let _ = tokio::time::timeout(NOTICE_TIMEOUT, request.send()).await;
    }

    fn cancelled(&self, id: u64) {
        let Ok(body) = self.inner.encode(&json!({
            "jsonrpc": "2.0",
            "method": "notifications/cancelled",
            "params": { "requestId": id, "reason": "cancelled by RAH" }
        })) else {
            return;
        };
        let inner = self.inner.clone();
        self.inner.spawner.spawn(async move {
            let _ = inner.send_notice(body).await;
        });
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Inner {
    fn encode(&self, message: &Value) -> Result<Vec<u8>, RpcError> {
        let body = serde_json::to_vec(message).map_err(|_| RpcError::TooLarge)?;
        if body.len() > self.max {
            return Err(RpcError::TooLarge);
        }
        Ok(body)
    }

    fn post(&self, body: Vec<u8>) -> reqwest::RequestBuilder {
        let mut request = self
            .client
            .post(self.url.clone())
            .header(ACCEPT, "application/json, text/event-stream")
            .header(CONTENT_TYPE, "application/json")
            .body(body);
        if let Some(authorization) = &self.authorization {
            request = request.header(reqwest::header::AUTHORIZATION, authorization.clone());
        }
        if let Some(session) = lock(&self.session).clone() {
            request = request.header(SESSION_HEADER, session);
        }
        if let Some(version) = lock(&self.version).clone() {
            request = request.header(VERSION_HEADER, version);
        }
        request
    }

    /// Sends a request and reads its response from a JSON body or an SSE
    /// stream.
    async fn exchange(&self, body: Vec<u8>, id: u64) -> Result<Value, RpcError> {
        let had_session = lock(&self.session).is_some();
        let mut response = self.post(body).send().await.map_err(send_error)?;
        self.check_status(response.status(), had_session)?;
        if !had_session && let Some(session) = response.headers().get(SESSION_HEADER) {
            let valid = (1..=MAX_SESSION_ID).contains(&session.len())
                && session.as_bytes().iter().all(|b| (0x21..=0x7e).contains(b));
            if !valid {
                return Err(RpcError::Protocol("the session id is not valid".into()));
            }
            *lock(&self.session) = Some(session.clone());
        }
        let kind = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_ascii_lowercase();
        if kind.starts_with("text/event-stream") {
            return self.stream(&mut response, id).await;
        }
        if !kind.starts_with("application/json") {
            return Err(RpcError::Protocol(format!(
                "the reply has content type `{}`",
                kind.chars().take(100).collect::<String>()
            )));
        }
        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|_| RpcError::Disconnected)? {
            if body.len() + chunk.len() > self.max {
                return Err(RpcError::Protocol(
                    "a message exceeds the size limit".into(),
                ));
            }
            body.extend_from_slice(&chunk);
        }
        match classify(&body)? {
            Incoming::Response { id: got, result } if got == id => result,
            _ => Err(RpcError::Protocol(
                "the JSON reply is not the response to the request".into(),
            )),
        }
    }

    fn check_status(&self, status: StatusCode, had_session: bool) -> Result<(), RpcError> {
        if status.is_success() {
            return Ok(());
        }
        if status == StatusCode::NOT_FOUND && had_session {
            self.ended.store(true, Ordering::Release);
            return Err(RpcError::SessionEnded);
        }
        Err(RpcError::Http(status.as_u16()))
    }

    /// Reads SSE events until the response to `id`. Server requests on the
    /// stream are answered like over stdio; other messages are ignored.
    async fn stream(&self, response: &mut reqwest::Response, id: u64) -> Result<Value, RpcError> {
        let mut buffer: Vec<u8> = Vec::new();
        let mut data: Vec<u8> = Vec::new();
        loop {
            let Some(chunk) = response.chunk().await.map_err(|_| RpcError::Disconnected)? else {
                // The stream ended before the response: the request may
                // have run.
                return Err(RpcError::Disconnected);
            };
            buffer.extend_from_slice(&chunk);
            while let Some(end) = buffer.iter().position(|b| *b == b'\n') {
                let line: Vec<u8> = buffer.drain(..=end).collect();
                let line = line.strip_suffix(b"\n").unwrap_or(&line);
                let line = line.strip_suffix(b"\r").unwrap_or(line);
                if line.is_empty() {
                    if data.is_empty() {
                        continue;
                    }
                    let event = std::mem::take(&mut data);
                    match classify(&event)? {
                        Incoming::Response { id: got, result } if got == id => return result,
                        Incoming::Response { .. } | Incoming::Ignore => {}
                        Incoming::Answer(answer) => {
                            // A lost answer only makes the server's own
                            // request fail.
                            if let Ok(body) = self.encode(&answer) {
                                let _ = self.send_notice(body).await;
                            }
                        }
                    }
                } else if let Some(rest) = line.strip_prefix(b"data:") {
                    let rest = rest.strip_prefix(b" ").unwrap_or(rest);
                    if !data.is_empty() {
                        data.push(b'\n');
                    }
                    data.extend_from_slice(rest);
                    if data.len() > self.max {
                        return Err(RpcError::Protocol(
                            "a message exceeds the size limit".into(),
                        ));
                    }
                }
                // `event:`, `id:`, `retry:` and comments are not used.
            }
            if buffer.len() > self.max {
                return Err(RpcError::Protocol(
                    "a message exceeds the size limit".into(),
                ));
            }
        }
    }

    /// Posts a notification or an answer, which expects no response body.
    async fn send_notice(&self, body: Vec<u8>) -> Result<(), RpcError> {
        let had_session = lock(&self.session).is_some();
        let response = tokio::time::timeout(NOTICE_TIMEOUT, self.post(body).send())
            .await
            .map_err(|_| RpcError::TimedOut)?
            .map_err(send_error)?;
        self.check_status(response.status(), had_session)
    }
}

fn send_error(error: reqwest::Error) -> RpcError {
    if error.is_connect() {
        RpcError::Unreachable
    } else {
        RpcError::Disconnected
    }
}
