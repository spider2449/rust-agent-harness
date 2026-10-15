//! Process plugin host, protocol v3: plugins as child processes speaking
//! newline-delimited JSON-RPC 2.0 on stdin and stdout (stderr is kept as a
//! bounded diagnostic tail).
//!
//! Host to plugin:
//!
//! - `initialize {protocol, kernel_api, plugin_id, config}` returns the
//!   manifest `{protocol: 3, version, provides, requires, optional, tools,
//!   hooks, events}`. A manifest answering protocol 2 is accepted without
//!   `hooks` and `events`.
//! - `tools/call {name, arguments, session}` returns
//!   `{content: [{type: "text", text} | {type: "json", json}], is_error}`. An
//!   error whose `data.kind` is `invalid_input` or `not_started` means nothing
//!   was done.
//! - `hooks/call {hook, input}` for `tool.before_invoke`,
//!   `tool.after_invoke` and `model.before_request` (see [`hooks`]).
//! - `event {event, data}` (notification) for `session.appended`.
//! - `$/cancel {id}` (notification) and `shutdown`.
//!
//! Plugin to host:
//!
//! - `service/call {service, method, params}`. The host refuses any service
//!   not in the plugin's own `requires` (error `-32003`). Callable services:
//!   `session` with `list` and `read {session}`; `storage` with `get {key}`,
//!   `put {key, value}`, `delete {key}` and `list {prefix?}`, always in the
//!   namespace of the plugin's id ([`rah_api::storage::call_json`]).
//!
//! A process plugin may provide `tools` only; other services are not
//! available over the protocol yet. `rah` learns the manifest by running
//! the plugin once at boot ([`ProcessPlugin::describe`]); every start then
//! checks that the manifest is unchanged. The child gets a cleared
//! environment (refilled from `inherit_env` and `env`), runs while its plugin
//! scope lives and is killed with it. Like any mounted plugin it is trusted
//! code; this is process supervision, not a sandbox: descendants of the
//! child are not tracked.

pub mod hooks;
mod peer;

use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    process::Stdio,
    str::FromStr,
    sync::Arc,
    time::Duration,
};

use futures::FutureExt;
use rah_api::{
    CancellationToken, Plugin, PluginError, PluginId, PluginManifest, Scope, async_trait,
    kernel::KERNEL_API,
    session::{SESSION, SessionLog},
    storage::{self, KvStore, STORAGE, StorageError},
    tools::{TOOLS_SERVICE, Tool, ToolContext, ToolError, image, provide_tools},
};
use rah_protocol::{SessionId, ToolContent, ToolDefinition, ToolOutput};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::process::{Child, Command};

use crate::hooks::{EVENTS, HOOKABLE, HookSpec};
pub use crate::peer::{Inbound, Peer, PeerError, RpcFault};

pub const PROTOCOL_VERSION: u64 = 3;
/// The tools-only protocol, still accepted from plugins.
pub const PROTOCOL_V2: u64 = 2;
/// Services a process plugin may provide.
pub const PROVIDABLE_SERVICES: [&str; 1] = [TOOLS_SERVICE];
/// Host services a process plugin may require and call.
pub const CALLABLE_SERVICES: [&str; 2] = ["session", "storage"];
/// JSON-RPC error code for a service outside the plugin's `requires`.
pub const FORBIDDEN: i64 = -32003;
const MAX_TOOLS: usize = 128;

#[cfg(windows)]
const DEFAULT_INHERIT_ENV: &[&str] = &["PATH", "PATHEXT", "SYSTEMROOT", "WINDIR", "TEMP", "TMP"];
#[cfg(not(windows))]
const DEFAULT_INHERIT_ENV: &[&str] = &["PATH", "LANG", "TMPDIR", "HOME"];

/// How to start a process plugin: the `config` of a `use = "process"` entry.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProcessConfig {
    pub program: String,
    #[serde(default)]
    pub args: Vec<String>,
    /// Working directory of the child; defaults to that of `rah`.
    pub cwd: Option<PathBuf>,
    /// Added to the cleared environment after `inherit_env`.
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    /// Variables copied from `rah`'s environment; defaults to a short list.
    pub inherit_env: Option<Vec<String>>,
    /// Passed to the plugin as `initialize.config`.
    #[serde(default)]
    pub plugin: Value,
    pub startup_timeout_ms: Option<u64>,
    pub call_timeout_ms: Option<u64>,
    pub hook_timeout_ms: Option<u64>,
    pub max_message_bytes: Option<usize>,
}

impl ProcessConfig {
    fn startup_timeout(&self) -> Duration {
        Duration::from_millis(self.startup_timeout_ms.unwrap_or(10_000).max(1))
    }

    fn call_timeout(&self) -> Duration {
        Duration::from_millis(self.call_timeout_ms.unwrap_or(60_000).max(1))
    }

    fn hook_timeout(&self) -> Duration {
        Duration::from_millis(self.hook_timeout_ms.unwrap_or(5_000).max(1))
    }

    fn max_message(&self) -> usize {
        self.max_message_bytes.unwrap_or(4 * 1024 * 1024).max(1024)
    }

    fn spawn(&self) -> Result<Child, String> {
        let mut command = Command::new(&self.program);
        command.args(&self.args).env_clear();
        let inherit: Vec<String> = self.inherit_env.clone().unwrap_or_else(|| {
            DEFAULT_INHERIT_ENV
                .iter()
                .map(|name| (*name).to_owned())
                .collect()
        });
        for name in inherit {
            if let Some(value) = std::env::var_os(&name) {
                command.env(name, value);
            }
        }
        command
            .envs(&self.env)
            .env("RAH_PLUGIN_PROTOCOL", PROTOCOL_VERSION.to_string())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        if let Some(cwd) = &self.cwd {
            command.current_dir(cwd);
        }
        command
            .spawn()
            .map_err(|error| format!("cannot start `{}`: {error}", self.program))
    }
}

/// What a plugin declared in `initialize`.
#[derive(Clone, Debug, PartialEq)]
pub struct Described {
    pub version: String,
    pub provides: Vec<String>,
    pub requires: Vec<String>,
    pub optional: Vec<String>,
    pub tools: Vec<ToolDefinition>,
    pub hooks: Vec<HookSpec>,
    pub events: Vec<String>,
}

type Shape<'a> = (
    BTreeSet<&'a String>,
    BTreeSet<&'a String>,
    BTreeSet<&'a String>,
    BTreeSet<&'a HookSpec>,
    BTreeSet<&'a String>,
);

impl Described {
    /// What must not change between starts: services, hooks and events.
    fn shape(&self) -> Shape<'_> {
        (
            self.provides.iter().collect(),
            self.requires.iter().collect(),
            self.optional.iter().collect(),
            self.hooks.iter().collect(),
            self.events.iter().collect(),
        )
    }
}

fn parse_initialize(result: Value) -> Result<Described, String> {
    #[derive(Deserialize)]
    struct Reply {
        protocol: u64,
        version: String,
        #[serde(default)]
        provides: Vec<String>,
        #[serde(default)]
        requires: Vec<String>,
        #[serde(default)]
        optional: Vec<String>,
        #[serde(default)]
        tools: Vec<ToolDefinition>,
        #[serde(default)]
        hooks: Vec<HookSpec>,
        #[serde(default)]
        events: Vec<String>,
    }
    let reply: Reply = serde_json::from_value(result)
        .map_err(|error| format!("invalid initialize result: {error}"))?;
    if reply.protocol != PROTOCOL_VERSION && reply.protocol != PROTOCOL_V2 {
        return Err(format!(
            "the plugin speaks protocol {}, the host speaks {PROTOCOL_V2} or {PROTOCOL_VERSION}",
            reply.protocol
        ));
    }
    if reply.protocol == PROTOCOL_V2 && !(reply.hooks.is_empty() && reply.events.is_empty()) {
        return Err(format!(
            "`hooks` and `events` need protocol {PROTOCOL_VERSION}"
        ));
    }
    let mut hooks = BTreeSet::new();
    for spec in &reply.hooks {
        if !HOOKABLE.contains(&spec.hook.as_str()) {
            return Err(format!(
                "a process plugin cannot attach to `{}`; only: {}",
                spec.hook,
                HOOKABLE.join(", ")
            ));
        }
        if !hooks.insert(spec.hook.as_str()) {
            return Err(format!("duplicate hook `{}`", spec.hook));
        }
    }
    let mut events = BTreeSet::new();
    for event in &reply.events {
        if !EVENTS.contains(&event.as_str()) {
            return Err(format!(
                "a process plugin cannot subscribe to `{event}`; only: {}",
                EVENTS.join(", ")
            ));
        }
        if !events.insert(event.as_str()) {
            return Err(format!("duplicate event `{event}`"));
        }
    }
    if let Some(service) = reply
        .provides
        .iter()
        .find(|s| !PROVIDABLE_SERVICES.contains(&s.as_str()))
    {
        return Err(format!(
            "a process plugin cannot provide `{service}`; only: tools"
        ));
    }
    if let Some(service) = reply
        .requires
        .iter()
        .chain(&reply.optional)
        .find(|s| !CALLABLE_SERVICES.contains(&s.as_str()))
    {
        return Err(format!(
            "a process plugin cannot depend on `{service}`; only: {}",
            CALLABLE_SERVICES.join(", ")
        ));
    }
    let provides_tools = reply.provides.iter().any(|s| s == TOOLS_SERVICE);
    if provides_tools == reply.tools.is_empty() {
        return Err("a plugin lists tools exactly when it provides `tools`".into());
    }
    if reply.tools.len() > MAX_TOOLS {
        return Err(format!("more than {MAX_TOOLS} tools"));
    }
    let mut names = BTreeSet::new();
    for tool in &reply.tools {
        let name = tool.name.as_str();
        if name.is_empty() || name.len() > 128 || name.chars().any(char::is_whitespace) {
            return Err(format!("invalid tool name `{name}`"));
        }
        if !names.insert(name) {
            return Err(format!("duplicate tool `{name}`"));
        }
    }
    Ok(Described {
        version: reply.version,
        provides: reply.provides,
        requires: reply.requires,
        optional: reply.optional,
        tools: reply.tools,
        hooks: reply.hooks,
        events: reply.events,
    })
}

async fn initialize(
    peer: &Peer,
    id: &PluginId,
    config: &ProcessConfig,
) -> Result<Described, String> {
    let params = json!({
        "protocol": PROTOCOL_VERSION,
        "kernel_api": KERNEL_API,
        "plugin_id": id.as_str(),
        "config": config.plugin,
    });
    let result = peer
        .request(
            "initialize",
            params,
            config.startup_timeout(),
            &CancellationToken::new(),
        )
        .await
        .map_err(|error| format!("initialize failed: {error}"))?;
    parse_initialize(result)
}

fn with_stderr(message: String, peer: &Peer) -> String {
    match peer.stderr() {
        stderr if stderr.is_empty() => message,
        stderr => format!("{message}; stderr: {stderr}"),
    }
}

/// A plugin implemented by a child process.
pub struct ProcessPlugin {
    manifest: PluginManifest,
    described: Described,
}

impl ProcessPlugin {
    /// Starts the plugin once to read its manifest, then shuts it down.
    pub async fn describe(id: PluginId, config: &Value) -> Result<Self, String> {
        let launch: ProcessConfig = serde_json::from_value(config.clone())
            .map_err(|error| format!("process plugin `{id}`: {error}"))?;
        let child = launch
            .spawn()
            .map_err(|error| format!("process plugin `{id}`: {error}"))?;
        let refuse: Inbound = Arc::new(|_, _| {
            async {
                Err(RpcFault::new(
                    FORBIDDEN,
                    "no services are available during describe",
                ))
            }
            .boxed()
        });
        let (peer, connection) = Peer::connect(child, refuse, launch.max_message())?;
        let handshake = async {
            let described = initialize(&peer, &id, &launch).await;
            let _ = peer
                .request(
                    "shutdown",
                    json!({}),
                    Duration::from_millis(500),
                    &CancellationToken::new(),
                )
                .await;
            let described = described.map_err(|error| with_stderr(error, &peer));
            drop(peer);
            described
        };
        let (described, ()) = tokio::join!(handshake, connection);
        let described = described.map_err(|error| format!("process plugin `{id}`: {error}"))?;
        let manifest = PluginManifest::new(id, described.version.clone())
            .provides(described.provides.clone())
            .requires(described.requires.clone())
            .optional(described.optional.clone());
        Ok(Self {
            manifest,
            described,
        })
    }

    pub fn described(&self) -> &Described {
        &self.described
    }
}

#[async_trait]
impl Plugin for ProcessPlugin {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    fn check_config(&self, config: &Value) -> Result<(), PluginError> {
        ProcessConfig::deserialize(config)?;
        Ok(())
    }

    async fn apply(&self, scope: &mut Scope) -> Result<(), PluginError> {
        let launch: ProcessConfig = scope.config_as()?;
        let requires = self.described.requires.clone();
        let session = if requires.iter().any(|s| s == "session") {
            Some(scope.inject(SESSION)?)
        } else {
            None
        };
        let storage = if requires.iter().any(|s| s == "storage") {
            Some(scope.inject(STORAGE)?)
        } else {
            None
        };
        let services = HostServices {
            requires,
            session,
            storage,
            namespace: scope.id().to_string(),
        };
        let child = launch.spawn()?;
        let (peer, connection) =
            Peer::connect(child, host_services(services), launch.max_message())?;
        if !scope.spawn(connection) {
            return Err("the plugin scope is already disposed".into());
        }
        let described = initialize(&peer, scope.id(), &launch)
            .await
            .map_err(|error| with_stderr(error, &peer))?;
        if described.shape() != self.described.shape() {
            return Err("the plugin's manifest changed since `rah` started; restart `rah`".into());
        }
        if !described.tools.is_empty() {
            let tools = described
                .tools
                .into_iter()
                .map(|definition| {
                    Arc::new(ProcessTool {
                        definition,
                        peer: peer.clone(),
                        timeout: launch.call_timeout(),
                    }) as Arc<dyn Tool>
                })
                .collect();
            provide_tools(scope, tools)?;
        }
        hooks::attach(
            scope,
            &peer,
            &described.hooks,
            &described.events,
            launch.hook_timeout(),
        );
        scope.on_dispose(move || async move {
            let _ = peer
                .request(
                    "shutdown",
                    json!({}),
                    Duration::from_millis(500),
                    &CancellationToken::new(),
                )
                .await;
        });
        Ok(())
    }
}

/// What a running process plugin may call.
struct HostServices {
    requires: Vec<String>,
    session: Option<Arc<dyn SessionLog>>,
    storage: Option<Arc<dyn KvStore>>,
    /// The plugin's id: its only storage namespace.
    namespace: String,
}

fn storage_fault(error: StorageError) -> RpcFault {
    let code = match error {
        StorageError::UnknownMethod(_) => -32601,
        StorageError::InvalidName(_)
        | StorageError::InvalidParams(_)
        | StorageError::TooLarge { .. } => -32602,
        StorageError::Io(_) => -32000,
    };
    RpcFault::new(code, error.to_string())
}

/// Answers `service/call` for the services in `requires`.
fn host_services(services: HostServices) -> Inbound {
    let services = Arc::new(services);
    Arc::new(move |method, params| {
        let services = services.clone();
        async move {
            let (requires, session) = (&services.requires, services.session.clone());
            if method != "service/call" {
                return Err(RpcFault::new(-32601, format!("unknown method `{method}`")));
            }
            let service = params
                .get("service")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let call = params
                .get("method")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let args = params.get("params").cloned().unwrap_or(Value::Null);
            if !requires.iter().any(|s| s == service) {
                return Err(RpcFault::new(
                    FORBIDDEN,
                    format!("service `{service}` is not in this plugin's requires"),
                ));
            }
            match (service, call, session) {
                ("session", "list", Some(session)) => {
                    let list = session
                        .list()
                        .await
                        .map_err(|e| RpcFault::new(-32000, e.to_string()))?;
                    serde_json::to_value(list).map_err(|e| RpcFault::new(-32603, e.to_string()))
                }
                ("session", "read", Some(session)) => {
                    let id = args
                        .get("session")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    let id = SessionId::from_str(id).map_err(|_| {
                        RpcFault::new(-32602, format!("`{id}` is not a session id"))
                    })?;
                    let records = session
                        .read(&id)
                        .await
                        .map_err(|e| RpcFault::new(-32000, e.to_string()))?;
                    serde_json::to_value(records).map_err(|e| RpcFault::new(-32603, e.to_string()))
                }
                ("storage", call, _) => match &services.storage {
                    Some(store) => {
                        storage::call_json(store.as_ref(), &services.namespace, call, &args)
                            .await
                            .map_err(storage_fault)
                    }
                    None => Err(RpcFault::new(-32601, "service `storage` is not available")),
                },
                _ => Err(RpcFault::new(
                    -32601,
                    format!("unknown method `{service}.{call}`"),
                )),
            }
        }
        .boxed()
    })
}

struct ProcessTool {
    definition: ToolDefinition,
    peer: Peer,
    timeout: Duration,
}

#[async_trait]
impl Tool for ProcessTool {
    fn definition(&self) -> ToolDefinition {
        self.definition.clone()
    }

    async fn call(&self, input: Value, cx: ToolContext) -> Result<ToolOutput, ToolError> {
        let params = json!({
            "name": self.definition.name,
            "arguments": input,
            "session": cx.session.to_string(),
        });
        let result = self
            .peer
            .request("tools/call", params, self.timeout, &cx.cancel)
            .await
            .map_err(|error| match error {
                PeerError::Cancelled => ToolError::Cancelled,
                PeerError::Remote(fault) => {
                    let kind = fault
                        .data
                        .as_ref()
                        .and_then(|d| d.get("kind"))
                        .and_then(Value::as_str);
                    match kind {
                        Some("invalid_input") => ToolError::InvalidInput(fault.message),
                        Some("not_started") => ToolError::NotStarted(fault.message),
                        _ => ToolError::Execution(fault.message),
                    }
                }
                error if error.not_sent() => ToolError::NotStarted(error.to_string()),
                error => ToolError::Execution(error.to_string()),
            })?;
        tool_output(&result)
    }
}

/// The protocol form of a tool output, as a `tools/call` result.
fn wire_output(output: &ToolOutput) -> Value {
    let content: Vec<Value> = output
        .content
        .iter()
        .map(|item| match item {
            ToolContent::Text(text) => json!({ "type": "text", "text": text }),
            ToolContent::Json(value) => json!({ "type": "json", "json": value }),
            ToolContent::Image(picture) => json!({
                "type": "image",
                "media_type": picture.media_type,
                "data": picture.data,
            }),
        })
        .collect();
    json!({ "content": content, "is_error": output.is_error })
}

fn tool_output(result: &Value) -> Result<ToolOutput, ToolError> {
    let malformed = || ToolError::Execution("the plugin returned a malformed tool result".into());
    let items = result
        .get("content")
        .and_then(Value::as_array)
        .ok_or_else(malformed)?;
    let content = items
        .iter()
        .map(|item| match item.get("type").and_then(Value::as_str) {
            Some("text") => item
                .get("text")
                .and_then(Value::as_str)
                .map(|t| ToolContent::Text(t.to_owned())),
            Some("json") => item.get("json").cloned().map(ToolContent::Json),
            Some("image") => image(
                item.get("media_type").and_then(Value::as_str)?,
                item.get("data").and_then(Value::as_str)?,
            )
            .ok(),
            _ => None,
        })
        .collect::<Option<Vec<_>>>()
        .ok_or_else(malformed)?;
    let is_error = match result.get("is_error") {
        None => false,
        Some(flag) => flag.as_bool().ok_or_else(malformed)?,
    };
    Ok(ToolOutput { content, is_error })
}

impl ProcessPlugin {
    /// An example configuration, mostly the defaults, for reference (the Web
    /// UI shows it in an empty configuration editor). It is never applied.
    pub const CONFIG_EXAMPLE: &str = r#"{
  "program": "python",
  "args": [
    "my_plugin.py"
  ],
  "plugin": {}
}"#;
}

#[cfg(test)]
mod tests {
    use rah_protocol::ImageContent;

    use super::*;

    #[test]
    fn tool_results_carry_text_json_and_checked_images() {
        let output = tool_output(&json!({
            "content": [
                {"type": "text", "text": "t"},
                {"type": "json", "json": {"a": 1}},
                {"type": "image", "media_type": "image/png", "data": "AA=="}
            ]
        }))
        .unwrap();
        assert_eq!(
            output.content,
            vec![
                ToolContent::Text("t".into()),
                ToolContent::Json(json!({"a": 1})),
                ToolContent::Image(ImageContent {
                    media_type: "image/png".into(),
                    data: "AA==".into()
                }),
            ]
        );
        for item in [
            json!({"type": "image", "media_type": "image/svg+xml", "data": "AA=="}),
            json!({"type": "image", "media_type": "image/png", "data": "not base64"}),
            json!({"type": "image", "data": "AA=="}),
        ] {
            assert!(
                tool_output(&json!({ "content": [item] })).is_err(),
                "{item}"
            );
        }
    }
}

#[cfg(test)]
#[test]
fn the_config_example_is_a_valid_configuration() {
    let parsed: Result<ProcessConfig, _> = serde_json::from_str(ProcessPlugin::CONFIG_EXAMPLE);
    assert!(parsed.is_ok(), "{:?}", parsed.err());
}
