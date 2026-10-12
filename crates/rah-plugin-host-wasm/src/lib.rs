//! WASM plugin host: plugins as WebAssembly components of the WIT world
//! `rah:plugin/wasm-plugin` (see [`WIT`]).
//!
//! - The guest exports `manifest() -> string`, a JSON object
//!   `{version, requires, tools}` (tools as in the process protocol), and
//!   `call-tool(name, input) -> result<string, string>`. `ok` text becomes the
//!   tool output; an `err` starting with `invalid_input:` means nothing was
//!   done, any other `err` is an execution failure.
//! - The host provides `host.call-service(service, method, params) ->
//!   result<string, string>`, refused for any service outside the guest's
//!   `requires`. Callable services: `session` with `list` and
//!   `read {session}`.
//!
//! Isolation is the point of this host: a guest gets no WASI, so no files,
//! network, clock, randomness or environment; its only way out is
//! `call-service`. Every call runs in a fresh store on a blocking thread with
//! a fuel budget (`fuel`, per call) and a memory cap (`max_memory_mb`). A
//! guest that runs out of fuel traps, and the call fails.
//!
//! `rah` mounts a component with `use = "wasm"`, `id`, and `config = { path =
//! "plugin.wasm" }` (binary or text format). It reads the manifest once at
//! boot; the manifest is fixed for the plugin's lifetime.

use std::{collections::BTreeSet, path::PathBuf, str::FromStr, sync::Arc};

use rah_api::{
    Plugin, PluginError, PluginId, PluginManifest, Scope, async_trait,
    session::{SESSION, SessionLog},
    tools::{TOOLS_SERVICE, Tool, ToolContext, ToolError, provide_tools},
};
use rah_protocol::{SessionId, ToolContent, ToolDefinition, ToolOutput};
use serde::Deserialize;
use serde_json::Value;
use wasmtime::{
    Config, Engine, Store, StoreLimits, StoreLimitsBuilder,
    component::{Component, HasSelf, Linker},
};

/// The WIT world every WASM plugin implements.
pub const WIT: &str = r#"
package rah:plugin@0.1.0;

interface host {
  call-service: func(service: string, method: string, params: string) -> result<string, string>;
}

world wasm-plugin {
  import host;
  export manifest: func() -> string;
  export call-tool: func(name: string, input: string) -> result<string, string>;
}
"#;

mod bindings {
    wasmtime::component::bindgen!({
        inline: r#"
package rah:plugin@0.1.0;

interface host {
  call-service: func(service: string, method: string, params: string) -> result<string, string>;
}

world wasm-plugin {
  import host;
  export manifest: func() -> string;
  export call-tool: func(name: string, input: string) -> result<string, string>;
}
"#,
    });
}

use bindings::WasmPlugin as Guest;

/// Host services a WASM plugin may require and call.
pub const CALLABLE_SERVICES: [&str; 1] = ["session"];
const DEFAULT_FUEL: u64 = 100_000_000;
const DEFAULT_MAX_MEMORY_MB: usize = 64;
const MAX_TOOLS: usize = 128;

/// The `config` of a `use = "wasm"` entry.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WasmConfig {
    pub path: PathBuf,
    pub fuel: Option<u64>,
    pub max_memory_mb: Option<usize>,
}

/// What a guest declared.
#[derive(Clone, Debug, PartialEq)]
pub struct Described {
    pub version: String,
    pub requires: Vec<String>,
    pub tools: Vec<ToolDefinition>,
}

fn parse_manifest(text: &str) -> Result<Described, String> {
    #[derive(Deserialize)]
    struct Manifest {
        version: String,
        #[serde(default)]
        requires: Vec<String>,
        #[serde(default)]
        tools: Vec<ToolDefinition>,
    }
    let manifest: Manifest =
        serde_json::from_str(text).map_err(|error| format!("invalid manifest: {error}"))?;
    if let Some(service) = manifest
        .requires
        .iter()
        .find(|s| !CALLABLE_SERVICES.contains(&s.as_str()))
    {
        return Err(format!(
            "a WASM plugin cannot depend on `{service}`; only: session"
        ));
    }
    if manifest.tools.is_empty() || manifest.tools.len() > MAX_TOOLS {
        return Err(format!("a WASM plugin provides 1 to {MAX_TOOLS} tools"));
    }
    let mut names = BTreeSet::new();
    for tool in &manifest.tools {
        let name = tool.name.as_str();
        if name.is_empty()
            || name.len() > 128
            || name.chars().any(char::is_whitespace)
            || !names.insert(name)
        {
            return Err(format!("invalid or duplicate tool name `{name}`"));
        }
    }
    Ok(Described {
        version: manifest.version,
        requires: manifest.requires,
        tools: manifest.tools,
    })
}

/// Per-call host state: what the guest may reach, and its limits.
struct HostState {
    requires: Arc<Vec<String>>,
    session: Option<Arc<dyn SessionLog>>,
    runtime: Option<tokio::runtime::Handle>,
    limits: StoreLimits,
}

impl bindings::rah::plugin::host::Host for HostState {
    fn call_service(
        &mut self,
        service: String,
        method: String,
        params: String,
    ) -> Result<String, String> {
        if !self.requires.contains(&service) {
            return Err(format!(
                "service `{service}` is not in this plugin's requires"
            ));
        }
        let (Some(session), Some(runtime), "session") =
            (&self.session, &self.runtime, service.as_str())
        else {
            return Err(format!("service `{service}` is not available"));
        };
        let params: Value = serde_json::from_str(&params).unwrap_or(Value::Null);
        let session = session.clone();
        let result = runtime.block_on(async move {
            match method.as_str() {
                "list" => serde_json::to_value(session.list().await.map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string()),
                "read" => {
                    let id = params
                        .get("session")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    let id = SessionId::from_str(id)
                        .map_err(|_| format!("`{id}` is not a session id"))?;
                    serde_json::to_value(session.read(&id).await.map_err(|e| e.to_string())?)
                        .map_err(|e| e.to_string())
                }
                other => Err(format!("unknown method `session.{other}`")),
            }
        })?;
        Ok(result.to_string())
    }
}

/// A compiled component and how to run it.
struct Runtime {
    engine: Engine,
    component: Component,
    linker: Linker<HostState>,
    fuel: u64,
    max_memory: usize,
}

impl Runtime {
    fn load(config: &WasmConfig) -> Result<Self, String> {
        let mut engine_config = Config::new();
        engine_config.consume_fuel(true);
        let engine = Engine::new(&engine_config).map_err(|error| error.to_string())?;
        let component = Component::from_file(&engine, &config.path)
            .map_err(|error| format!("cannot load `{}`: {error:#}", config.path.display()))?;
        let mut linker = Linker::new(&engine);
        Guest::add_to_linker::<HostState, HasSelf<HostState>>(&mut linker, |state| state)
            .map_err(|error| error.to_string())?;
        Ok(Self {
            engine,
            component,
            linker,
            fuel: config.fuel.unwrap_or(DEFAULT_FUEL).max(1),
            max_memory: config.max_memory_mb.unwrap_or(DEFAULT_MAX_MEMORY_MB).max(1) * 1024 * 1024,
        })
    }

    /// Instantiates the component in a fresh store and runs `call` on it.
    fn run<R>(
        &self,
        requires: Arc<Vec<String>>,
        session: Option<Arc<dyn SessionLog>>,
        runtime: Option<tokio::runtime::Handle>,
        call: impl FnOnce(&Guest, &mut Store<HostState>) -> wasmtime::Result<R>,
    ) -> Result<R, String> {
        let state = HostState {
            requires,
            session,
            runtime,
            limits: StoreLimitsBuilder::new()
                .memory_size(self.max_memory)
                .build(),
        };
        let mut store = Store::new(&self.engine, state);
        store.limiter(|state| &mut state.limits);
        store
            .set_fuel(self.fuel)
            .map_err(|error| error.to_string())?;
        let guest = Guest::instantiate(&mut store, &self.component, &self.linker)
            .map_err(|error| format!("cannot instantiate the component: {error:#}"))?;
        call(&guest, &mut store).map_err(|error| format!("the WASM plugin trapped: {error:#}"))
    }
}

/// A plugin implemented by a WebAssembly component.
pub struct WasmPlugin {
    manifest: PluginManifest,
    described: Described,
    runtime: Arc<Runtime>,
}

impl WasmPlugin {
    /// Compiles the component and reads its manifest.
    pub async fn load(id: PluginId, config: &Value) -> Result<Self, String> {
        let config: WasmConfig = serde_json::from_value(config.clone())
            .map_err(|error| format!("WASM plugin `{id}`: {error}"))?;
        let described = tokio::task::spawn_blocking(move || {
            let runtime = Runtime::load(&config)?;
            let manifest = runtime.run(Arc::new(Vec::new()), None, None, |guest, store| {
                guest.call_manifest(store)
            })?;
            Ok::<_, String>((runtime, parse_manifest(&manifest)?))
        })
        .await
        .map_err(|error| error.to_string())?
        .map_err(|error| format!("WASM plugin `{id}`: {error}"))?;
        let (runtime, described) = described;
        let manifest = PluginManifest::new(id, described.version.clone())
            .provides([TOOLS_SERVICE])
            .requires(described.requires.clone());
        Ok(Self {
            manifest,
            described,
            runtime: Arc::new(runtime),
        })
    }

    pub fn described(&self) -> &Described {
        &self.described
    }
}

#[async_trait]
impl Plugin for WasmPlugin {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    async fn apply(&self, scope: &mut Scope) -> Result<(), PluginError> {
        let requires = Arc::new(self.described.requires.clone());
        let session = if requires.iter().any(|s| s == "session") {
            Some(scope.inject(SESSION)?)
        } else {
            None
        };
        let tools = self
            .described
            .tools
            .iter()
            .map(|definition| {
                Arc::new(WasmTool {
                    definition: definition.clone(),
                    runtime: self.runtime.clone(),
                    requires: requires.clone(),
                    session: session.clone(),
                }) as Arc<dyn Tool>
            })
            .collect();
        provide_tools(scope, tools)?;
        Ok(())
    }
}

struct WasmTool {
    definition: ToolDefinition,
    runtime: Arc<Runtime>,
    requires: Arc<Vec<String>>,
    session: Option<Arc<dyn SessionLog>>,
}

#[async_trait]
impl Tool for WasmTool {
    fn definition(&self) -> ToolDefinition {
        self.definition.clone()
    }

    async fn call(&self, input: Value, _cx: ToolContext) -> Result<ToolOutput, ToolError> {
        let name = self.definition.name.to_string();
        let input = input.to_string();
        let (runtime, requires, session) = (
            self.runtime.clone(),
            self.requires.clone(),
            self.session.clone(),
        );
        let handle = tokio::runtime::Handle::current();
        // Fuel bounds the call; a cancelled caller stops waiting for it.
        let result = tokio::task::spawn_blocking(move || {
            runtime.run(requires, session, Some(handle), |guest, store| {
                guest.call_call_tool(store, &name, &input)
            })
        })
        .await
        .map_err(|error| ToolError::Execution(error.to_string()))?
        .map_err(ToolError::Execution)?;
        match result {
            Ok(text) => Ok(ToolOutput {
                content: vec![ToolContent::Text(text)],
                is_error: false,
            }),
            Err(message) => match message.strip_prefix("invalid_input:") {
                Some(rest) => Err(ToolError::InvalidInput(rest.trim().to_owned())),
                None => Err(ToolError::Execution(message)),
            },
        }
    }
}
