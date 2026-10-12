//! `tools-mcp`: tools of one MCP server over the stdio transport.
//!
//! The server is started through the `sandbox` service (`ExecEnv::spawn`),
//! so it gets the sandbox's working-directory and environment rules and is
//! terminated with this plugin's scope. Mount one instance per server.
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
//! Configuration:
//!
//! ```toml
//! config = { server = "files", program = "mcp-files", args = ["--root", "."],
//!            tools = { read_file = "read", write_file = "write" } }
//! ```
//!
//! Optional: `cwd` (relative to the sandbox root), `env`,
//! `call_timeout_ms` (30 s), `startup_timeout_ms` (10 s),
//! `max_message_bytes` (1 MiB) and `max_outstanding` (16).

mod client;

use std::{collections::BTreeMap, path::PathBuf, sync::Arc, time::Duration};

use rah_api::{
    CancellationToken, Plugin, PluginError, PluginId, PluginManifest, Scope, async_trait,
    sandbox::{SANDBOX, SpawnSpec},
    tools::{TOOLS_SERVICE, Tool, ToolContext, ToolError, provide_tools},
};
use rah_protocol::{PermissionLevel, ToolContent, ToolDefinition, ToolName, ToolOutput};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::client::{Client, RpcError};

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

#[async_trait]
impl Plugin for McpToolsPlugin {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    async fn apply(&self, scope: &mut Scope) -> Result<(), PluginError> {
        let config: McpConfig = scope.config_as()?;
        if !valid_name(&config.server, 32, b"_-", false) {
            return Err("`server` must be 1-32 lowercase ASCII letters, digits, `_` or `-`".into());
        }
        if config.tools.is_empty() {
            return Err("`tools` must list at least one remote tool to expose".into());
        }
        if let Some(name) = config.tools.keys().find(|name| !valid_remote_name(name)) {
            return Err(format!("remote tool name `{name}` must be 1-64 of [A-Za-z0-9_-]").into());
        }
        let positive = |value: Option<u64>, default: u64, field: &str| match value {
            Some(0) => Err(format!("`{field}` must be greater than zero")),
            Some(value) => Ok(value),
            None => Ok(default),
        };
        let call_timeout =
            Duration::from_millis(positive(config.call_timeout_ms, 30_000, "call_timeout_ms")?);
        let startup_timeout = Duration::from_millis(positive(
            config.startup_timeout_ms,
            10_000,
            "startup_timeout_ms",
        )?);
        let max_message = positive(
            config.max_message_bytes.map(|v| v as u64),
            1024 * 1024,
            "max_message_bytes",
        )? as usize;
        let max_outstanding = positive(
            config.max_outstanding.map(|v| v as u64),
            16,
            "max_outstanding",
        )? as usize;

        let sandbox = scope.inject(SANDBOX)?;
        let mut spec = SpawnSpec::new(config.program.clone(), config.args.clone());
        spec.cwd = config.cwd.clone();
        spec.env = config.env.clone();
        let spawned = sandbox
            .spawn(spec)
            .await
            .map_err(|error| format!("MCP server `{}`: {error}", config.server))?;
        let (client, process, actor) = Client::new(spawned, max_outstanding, max_message);
        if !scope.spawn(actor) {
            return Err("the plugin scope is already disposed".into());
        }
        let listed = match connect(&client, startup_timeout).await {
            Ok(listed) => listed,
            Err(message) => {
                process.terminate();
                let stderr = process.stderr();
                let stderr = stderr.trim();
                return Err(if stderr.is_empty() {
                    format!("MCP server `{}`: {message}", config.server)
                } else {
                    format!(
                        "MCP server `{}`: {message}; stderr: {stderr}",
                        config.server
                    )
                }
                .into());
            }
        };
        let mut tools: Vec<Arc<dyn Tool>> = Vec::new();
        for (remote, permission) in &config.tools {
            let Some(item) = listed
                .iter()
                .find(|item| item.get("name") == Some(&json!(remote)))
            else {
                process.terminate();
                return Err(format!(
                    "MCP server `{}` does not offer the configured tool `{remote}`",
                    config.server
                )
                .into());
            };
            let input_schema = item.get("inputSchema").cloned().unwrap_or(Value::Null);
            if input_schema.get("type") != Some(&json!("object")) {
                process.terminate();
                return Err(format!("MCP tool `{remote}` has no object input schema").into());
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
                    name: ToolName::new(format!("mcp.{}.{remote}", config.server)),
                    description,
                    input_schema,
                    permission: *permission,
                },
                remote: remote.clone(),
                client: client.clone(),
                timeout: call_timeout,
            }));
        }
        provide_tools(scope, tools)?;
        Ok(())
    }
}

/// Runs the initialize handshake and lists the server's tools.
async fn connect(client: &Client, timeout: Duration) -> Result<Vec<Value>, String> {
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
    if !version.is_some_and(|version| SUPPORTED_VERSIONS.contains(&version)) {
        return Err(format!("unsupported protocol version {version:?}"));
    }
    client
        .notify("notifications/initialized", json!({}))
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
    client: Client,
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
            .client
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

/// Maps a `tools/call` result. Text items become text; other content items
/// and `structuredContent` are passed on as JSON.
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

#[cfg(test)]
mod tests;
