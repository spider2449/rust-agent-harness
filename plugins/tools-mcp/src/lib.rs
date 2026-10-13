//! MCP client plugins: the tools of one MCP server.
//!
//! - `tools-mcp` ([`McpToolsPlugin`]) uses the stdio transport. The server
//!   is started through the `sandbox` service (`ExecEnv::spawn`), so it gets
//!   the sandbox's working-directory and environment rules and is terminated
//!   with this plugin's scope.
//! - `tools-mcp-http` ([`McpHttpToolsPlugin`]) uses the Streamable HTTP
//!   transport (ADR 0038) to a server it does not start, so it needs no
//!   sandbox. `url` must be `https`, or `http` for a loopback host; redirects
//!   and proxies are not used. `bearer_token_env` optionally names a variable
//!   whose token is read at start and sent to that endpoint only.
//!
//! Mount one instance per server.
//!
//! Only tools the host lists in `tools` are exposed, as
//! `mcp.<server>.<remote name>`, and each declares the permission level the
//! host assigned there. The server's own metadata (annotations such as
//! read-only hints) grants nothing. `policy-authority` must still assign the
//! exposed names, for example `"mcp.files.*" = "read"`. A listed tool the
//! server does not offer fails the plugin.
//!
//! A call that times out, is cancelled or loses the connection is reported as
//! an error and never replayed; under a `write` or `execute` declaration its
//! effect is recorded as `uncertain`. Requests the server never received
//! (busy, closed connection) record no effect. A crashed server stays down
//! until the plugin is restarted.
//!
//! Text content becomes text and image content (PNG, JPEG, GIF or WebP, at
//! most `rah_api::tools::MAX_IMAGE_DATA` base64 characters) an image, which
//! a model that takes images sees. An image that does not qualify becomes a
//! short note. Other content and `structuredContent` are passed on as JSON.
//!
//! Configuration:
//!
//! ```toml
//! config = { server = "files", program = "mcp-files", args = ["--root", "."],
//!            tools = { read_file = "read", write_file = "write" } }
//! # tools-mcp-http:
//! config = { server = "docs", url = "https://mcp.example.com/mcp",
//!            bearer_token_env = "DOCS_MCP_TOKEN", tools = { search = "read" } }
//! ```
//!
//! Optional for both: `call_timeout_ms` (30 s), `startup_timeout_ms` (10 s),
//! `max_message_bytes` (8 MiB, room for a screenshot) and `max_outstanding`
//! (16). For stdio also `cwd` (relative to the sandbox root) and `env`.

mod client;
mod http;

use std::{collections::BTreeMap, path::PathBuf, sync::Arc, time::Duration};

use rah_api::{
    CancellationToken, Plugin, PluginError, PluginId, PluginManifest, Scope, async_trait,
    sandbox::{SANDBOX, SpawnSpec},
    tools::{TOOLS_SERVICE, Tool, ToolContext, ToolError, image, provide_tools},
};
use rah_protocol::{PermissionLevel, ToolContent, ToolDefinition, ToolName, ToolOutput};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    client::{Client, RpcError},
    http::HttpClient,
};

/// The protocol version RAH requests.
pub const MCP_PROTOCOL_VERSION: &str = "2025-06-18";
/// Versions RAH accepts from a server; their `tools/*` methods agree.
pub const SUPPORTED_VERSIONS: [&str; 3] = ["2025-06-18", "2025-03-26", "2024-11-05"];
const MAX_LIST_PAGES: usize = 32;
const MAX_DESCRIPTION_CHARS: usize = 2000;

pub struct McpToolsPlugin {
    manifest: PluginManifest,
}

impl McpToolsPlugin {
    pub fn new(id: impl Into<PluginId>) -> Self {
        Self {
            manifest: PluginManifest::new(id, env!("CARGO_PKG_VERSION"))
                .provides([TOOLS_SERVICE])
                .requires(["sandbox"]),
        }
    }
}

pub struct McpHttpToolsPlugin {
    manifest: PluginManifest,
}

impl McpHttpToolsPlugin {
    pub fn new(id: impl Into<PluginId>) -> Self {
        Self {
            manifest: PluginManifest::new(id, env!("CARGO_PKG_VERSION")).provides([TOOLS_SERVICE]),
        }
    }
}

/// The `tools-mcp-http` configuration.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct McpHttpConfig {
    /// Names this server in tool names: 1-32 of `[a-z0-9_-]`.
    pub server: String,
    pub url: String,
    /// The environment variable that holds a bearer token.
    pub bearer_token_env: Option<String>,
    /// Remote tool name -> declared permission. Only these are exposed.
    pub tools: BTreeMap<String, PermissionLevel>,
    pub call_timeout_ms: Option<u64>,
    pub startup_timeout_ms: Option<u64>,
    pub max_message_bytes: Option<usize>,
    pub max_outstanding: Option<usize>,
}

/// The plugin configuration.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct McpConfig {
    /// Names this server in tool names: 1-32 of `[a-z0-9_-]`.
    pub server: String,
    pub program: String,
    #[serde(default)]
    pub args: Vec<String>,
    pub cwd: Option<PathBuf>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    /// Remote tool name -> declared permission. Only these are exposed.
    pub tools: BTreeMap<String, PermissionLevel>,
    pub call_timeout_ms: Option<u64>,
    pub startup_timeout_ms: Option<u64>,
    pub max_message_bytes: Option<usize>,
    pub max_outstanding: Option<usize>,
}

fn valid_name(name: &str, max: usize, extra: &[u8], upper: bool) -> bool {
    !name.is_empty()
        && name.len() <= max
        && name.bytes().all(|b| {
            b.is_ascii_lowercase()
                || b.is_ascii_digit()
                || (upper && b.is_ascii_uppercase())
                || extra.contains(&b)
        })
}

/// Remote tool names: 1-64 of `[A-Za-z0-9_-]`.
fn valid_remote_name(name: &str) -> bool {
    valid_name(name, 64, b"_-", true)
}

fn positive(value: Option<u64>, default: u64, field: &str) -> Result<u64, String> {
    match value {
        Some(0) => Err(format!("`{field}` must be greater than zero")),
        Some(value) => Ok(value),
        None => Ok(default),
    }
}

/// Limits shared by both transports.
struct Limits {
    call_timeout: Duration,
    startup_timeout: Duration,
    max_message: usize,
    max_outstanding: usize,
}

impl Limits {
    fn new(
        call_timeout_ms: Option<u64>,
        startup_timeout_ms: Option<u64>,
        max_message_bytes: Option<usize>,
        max_outstanding: Option<usize>,
    ) -> Result<Self, PluginError> {
        Ok(Self {
            call_timeout: Duration::from_millis(positive(
                call_timeout_ms,
                30_000,
                "call_timeout_ms",
            )?),
            startup_timeout: Duration::from_millis(positive(
                startup_timeout_ms,
                10_000,
                "startup_timeout_ms",
            )?),
            max_message: positive(
                max_message_bytes.map(|v| v as u64),
                8 * 1024 * 1024,
                "max_message_bytes",
            )? as usize,
            max_outstanding: positive(max_outstanding.map(|v| v as u64), 16, "max_outstanding")?
                as usize,
        })
    }
}

/// Checks the server name and the listed tools.
fn check_names(server: &str, tools: &BTreeMap<String, PermissionLevel>) -> Result<(), PluginError> {
    if !valid_name(server, 32, b"_-", false) {
        return Err("`server` must be 1-32 lowercase ASCII letters, digits, `_` or `-`".into());
    }
    if tools.is_empty() {
        return Err("`tools` must list at least one remote tool to expose".into());
    }
    if let Some(name) = tools.keys().find(|name| !valid_remote_name(name)) {
        return Err(format!("remote tool name `{name}` must be 1-64 of [A-Za-z0-9_-]").into());
    }
    Ok(())
}

#[async_trait]
impl Plugin for McpToolsPlugin {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    async fn apply(&self, scope: &mut Scope) -> Result<(), PluginError> {
        let config: McpConfig = scope.config_as()?;
        check_names(&config.server, &config.tools)?;
        let limits = Limits::new(
            config.call_timeout_ms,
            config.startup_timeout_ms,
            config.max_message_bytes,
            config.max_outstanding,
        )?;

        let sandbox = scope.inject(SANDBOX)?;
        let mut spec = SpawnSpec::new(config.program.clone(), config.args.clone());
        spec.cwd = config.cwd.clone();
        spec.env = config.env.clone();
        let spawned = sandbox
            .spawn(spec)
            .await
            .map_err(|error| format!("MCP server `{}`: {error}", config.server))?;
        let (client, process, actor) =
            Client::new(spawned, limits.max_outstanding, limits.max_message);
        if !scope.spawn(actor) {
            return Err("the plugin scope is already disposed".into());
        }
        let connection = Connection::Stdio(client);
        let tools = match expose(&connection, &config.server, &config.tools, &limits).await {
            Ok(tools) => tools,
            Err(message) => {
                process.terminate();
                let stderr = process.stderr();
                let stderr = stderr.trim();
                return Err(if stderr.is_empty() {
                    message
                } else {
                    format!("{message}; stderr: {stderr}")
                }
                .into());
            }
        };
        provide_tools(scope, tools)?;
        Ok(())
    }
}

#[async_trait]
impl Plugin for McpHttpToolsPlugin {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    async fn apply(&self, scope: &mut Scope) -> Result<(), PluginError> {
        let config: McpHttpConfig = scope.config_as()?;
        check_names(&config.server, &config.tools)?;
        let limits = Limits::new(
            config.call_timeout_ms,
            config.startup_timeout_ms,
            config.max_message_bytes,
            config.max_outstanding,
        )?;
        let (url, https) = http::endpoint(&config.url)?;
        let token = match &config.bearer_token_env {
            None => None,
            Some(variable) => match std::env::var(variable) {
                Ok(token) if !token.trim().is_empty() => Some(token.trim().to_owned()),
                _ => return Err(format!("the variable `{variable}` holds no token").into()),
            },
        };
        let client = HttpClient::new(
            url,
            https,
            token,
            limits.max_outstanding,
            limits.max_message,
            scope.spawner(),
        )?;
        let closing = client.clone();
        scope.on_dispose(move || async move { closing.close().await });
        let connection = Connection::Http(client);
        let tools = expose(&connection, &config.server, &config.tools, &limits).await?;
        provide_tools(scope, tools)?;
        Ok(())
    }
}

/// One connection to an MCP server.
#[derive(Clone)]
enum Connection {
    Stdio(Client),
    Http(HttpClient),
}

impl Connection {
    async fn request(
        &self,
        method: &'static str,
        params: Value,
        timeout: Duration,
        cancel: &CancellationToken,
    ) -> Result<Value, RpcError> {
        match self {
            Self::Stdio(client) => client.request(method, params, timeout, cancel).await,
            Self::Http(client) => client.request(method, params, timeout, cancel).await,
        }
    }

    async fn notify(&self, method: &'static str, params: Value) -> Result<(), RpcError> {
        match self {
            Self::Stdio(client) => client.notify(method, params),
            Self::Http(client) => client.notify(method, params).await,
        }
    }
}

/// Connects and builds the listed tools.
async fn expose(
    connection: &Connection,
    server: &str,
    configured: &BTreeMap<String, PermissionLevel>,
    limits: &Limits,
) -> Result<Vec<Arc<dyn Tool>>, String> {
    let listed = connect(connection, limits.startup_timeout)
        .await
        .map_err(|message| format!("MCP server `{server}`: {message}"))?;
    let mut tools: Vec<Arc<dyn Tool>> = Vec::new();
    for (remote, permission) in configured {
        let Some(item) = listed
            .iter()
            .find(|item| item.get("name") == Some(&json!(remote)))
        else {
            return Err(format!(
                "MCP server `{server}` does not offer the configured tool `{remote}`"
            ));
        };
        let input_schema = item.get("inputSchema").cloned().unwrap_or(Value::Null);
        if input_schema.get("type") != Some(&json!("object")) {
            return Err(format!("MCP tool `{remote}` has no object input schema"));
        }
        let description: String = item
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or("MCP tool")
            .chars()
            .take(MAX_DESCRIPTION_CHARS)
            .collect();
        tools.push(Arc::new(McpTool {
            definition: ToolDefinition {
                name: ToolName::new(format!("mcp.{server}.{remote}")),
                description,
                input_schema,
                permission: *permission,
            },
            remote: remote.clone(),
            connection: connection.clone(),
            timeout: limits.call_timeout,
        }));
    }
    Ok(tools)
}

/// Runs the initialize handshake and lists the server's tools.
async fn connect(client: &Connection, timeout: Duration) -> Result<Vec<Value>, String> {
    let never = CancellationToken::new();
    let initialized = client
        .request(
            "initialize",
            json!({
                "protocolVersion": MCP_PROTOCOL_VERSION,
                "capabilities": {},
                "clientInfo": { "name": "rah", "version": env!("CARGO_PKG_VERSION") }
            }),
            timeout,
            &never,
        )
        .await
        .map_err(|error| format!("initialize failed: {error}"))?;
    let version = initialized.get("protocolVersion").and_then(Value::as_str);
    let Some(version) = version.filter(|version| SUPPORTED_VERSIONS.contains(version)) else {
        return Err(format!("unsupported protocol version {version:?}"));
    };
    if let Connection::Http(http) = client {
        // Streamable HTTP starts with 2025-03-26.
        if version == "2024-11-05" {
            return Err("protocol version 2024-11-05 has no Streamable HTTP transport".into());
        }
        http.set_version(version);
    }
    client
        .notify("notifications/initialized", json!({}))
        .await
        .map_err(|error| error.to_string())?;
    let mut tools = Vec::new();
    let mut cursor: Option<Value> = None;
    for _ in 0..MAX_LIST_PAGES {
        let params = match &cursor {
            Some(cursor) => json!({ "cursor": cursor }),
            None => json!({}),
        };
        let page = client
            .request("tools/list", params, timeout, &never)
            .await
            .map_err(|error| format!("tools/list failed: {error}"))?;
        let items = page
            .get("tools")
            .and_then(Value::as_array)
            .ok_or("tools/list returned no tool array")?;
        tools.extend(items.iter().cloned());
        cursor = page.get("nextCursor").filter(|c| !c.is_null()).cloned();
        if cursor.is_none() {
            return Ok(tools);
        }
    }
    Err(format!(
        "tools/list returned more than {MAX_LIST_PAGES} pages"
    ))
}

struct McpTool {
    definition: ToolDefinition,
    remote: String,
    connection: Connection,
    timeout: Duration,
}

#[async_trait]
impl Tool for McpTool {
    fn definition(&self) -> ToolDefinition {
        self.definition.clone()
    }

    async fn call(&self, input: Value, cx: ToolContext) -> Result<ToolOutput, ToolError> {
        if !input.is_object() {
            return Err(ToolError::InvalidInput(
                "MCP tool arguments must be a JSON object".into(),
            ));
        }
        let result = self
            .connection
            .request(
                "tools/call",
                json!({ "name": self.remote, "arguments": input }),
                self.timeout,
                &cx.cancel,
            )
            .await
            .map_err(|error| match error {
                RpcError::Cancelled => ToolError::Cancelled,
                error if error.not_sent() => ToolError::NotStarted(error.to_string()),
                error => ToolError::Execution(error.to_string()),
            })?;
        tool_output(&result)
    }
}

/// Maps a `tools/call` result. Text items become text and image items
/// images; other content items and `structuredContent` are passed on as
/// JSON.
fn tool_output(result: &Value) -> Result<ToolOutput, ToolError> {
    let malformed =
        || ToolError::Execution("the MCP server returned a malformed tool result".into());
    let items = result
        .get("content")
        .and_then(Value::as_array)
        .ok_or_else(malformed)?;
    let mut content = Vec::with_capacity(items.len() + 1);
    for item in items {
        match (
            item.get("type").and_then(Value::as_str),
            item.get("text").and_then(Value::as_str),
        ) {
            (Some("text"), Some(text)) => content.push(ToolContent::Text(text.to_owned())),
            (Some("text"), None) | (None, _) => return Err(malformed()),
            (Some("image"), _) => content.push(mcp_image(item)),
            (Some(_), _) => content.push(ToolContent::Json(item.clone())),
        }
    }
    if let Some(structured) = result.get("structuredContent") {
        content.push(ToolContent::Json(structured.clone()));
    }
    let is_error = match result.get("isError") {
        None => false,
        Some(flag) => flag.as_bool().ok_or_else(malformed)?,
    };
    Ok(ToolOutput { content, is_error })
}

/// An MCP image item, or a note saying why it is left out.
fn mcp_image(item: &Value) -> ToolContent {
    let media_type = item.get("mimeType").and_then(Value::as_str).unwrap_or("");
    let data = item.get("data").and_then(Value::as_str).unwrap_or("");
    image(media_type, data)
        .unwrap_or_else(|reason| ToolContent::Text(format!("[image left out: {reason}]")))
}

#[cfg(test)]
mod http_tests;
#[cfg(test)]
mod tests;
