//! `tools-shell`: the opt-in `shell.exec` tool.
//!
//! It runs one program with an argument vector through the `sandbox`
//! service. There is no shell: the program is never given a command string to
//! interpret. The tool declares `execute` permission, so under
//! `policy-authority` it is at least as strict as the `execute` level (Ask in
//! the presets' policy, which a frontend must confirm).
//!
//! Configuration (all optional):
//!
//! - `allow_programs`: when set, only these exact program strings may run.
//! - `default_timeout_ms` (30 s) and `max_timeout_ms` (5 min): the model may
//!   choose a timeout up to the maximum, which must also fit the sandbox's.
//! - `max_output_bytes`: retained bytes per stream (64 KiB).
//!
//! Effects: a program that exits on its own reports `completed` (a non-zero
//! exit code is an error result). A program the sandbox had to terminate
//! (timeout, output limit, cancellation) or whose supervision failed is
//! reported as an error, which the `ToolInvoker` records as `uncertain`.

use std::{path::PathBuf, sync::Arc, time::Duration};

use rah_api::{
    Plugin, PluginError, PluginId, PluginManifest, Scope, async_trait,
    sandbox::{ExecEnv, ExecError, ExecOutput, OutputLimits, ProgramSpec, SANDBOX, Termination},
    tools::{TOOLS_SERVICE, Tool, ToolContext, ToolError, provide_tools},
};
use rah_protocol::{PermissionLevel, ToolContent, ToolDefinition, ToolName, ToolOutput};
use serde::Deserialize;
use serde_json::{Value, json};

const DEFAULT_TIMEOUT_MS: u64 = 30_000;
const DEFAULT_MAX_TIMEOUT_MS: u64 = 300_000;
const DEFAULT_MAX_OUTPUT_BYTES: usize = 64 * 1024;

pub struct ShellToolsPlugin {
    manifest: PluginManifest,
}

impl ShellToolsPlugin {
    pub fn new(id: impl Into<PluginId>) -> Self {
        Self {
            manifest: PluginManifest::new(id, env!("CARGO_PKG_VERSION"))
                .provides([TOOLS_SERVICE])
                .requires(["sandbox"]),
        }
    }
}

/// The plugin configuration.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShellConfig {
    pub allow_programs: Option<Vec<String>>,
    pub default_timeout_ms: Option<u64>,
    pub max_timeout_ms: Option<u64>,
    pub max_output_bytes: Option<usize>,
}

#[async_trait]
impl Plugin for ShellToolsPlugin {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    async fn apply(&self, scope: &mut Scope) -> Result<(), PluginError> {
        let config: ShellConfig = if scope.config().is_null() {
            ShellConfig::default()
        } else {
            scope.config_as()?
        };
        let sandbox = scope.inject(SANDBOX)?;
        let tool = ShellExec::new(sandbox, config)?;
        provide_tools(scope, vec![Arc::new(tool)])?;
        Ok(())
    }
}

/// The `shell.exec` tool.
pub struct ShellExec {
    sandbox: Arc<dyn ExecEnv>,
    allow_programs: Option<Vec<String>>,
    default_timeout: Duration,
    max_timeout: Duration,
    max_output_bytes: usize,
}

impl ShellExec {
    pub fn new(sandbox: Arc<dyn ExecEnv>, config: ShellConfig) -> Result<Self, String> {
        let default_ms = config.default_timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS);
        let max_ms = config.max_timeout_ms.unwrap_or(DEFAULT_MAX_TIMEOUT_MS);
        if default_ms == 0 || default_ms > max_ms {
            return Err("timeouts must satisfy 0 < default_timeout_ms <= max_timeout_ms".into());
        }
        let max_output_bytes = config.max_output_bytes.unwrap_or(DEFAULT_MAX_OUTPUT_BYTES);
        if max_output_bytes == 0 {
            return Err("max_output_bytes must be greater than zero".into());
        }
        Ok(Self {
            sandbox,
            allow_programs: config.allow_programs,
            default_timeout: Duration::from_millis(default_ms),
            max_timeout: Duration::from_millis(max_ms),
            max_output_bytes,
        })
    }

    /// Tells the model what the tool is for, in the words a user asks with
    /// ("run git pull"), and which programs it may name.
    fn description(&self) -> String {
        let programs = match &self.allow_programs {
            Some(allowed) if !allowed.is_empty() => {
                let names: Vec<String> = allowed.iter().map(|name| format!("`{name}`")).collect();
                format!("Allowed programs: {}. ", names.join(", "))
            }
            Some(_) => "No program is allowed in this configuration. ".to_owned(),
            None => String::new(),
        };
        format!(
            "Runs a command-line program in the workspace and returns its exit code, \
             stdout and stderr. Use it whenever the user asks to run a command, for \
             example `git pull` as {{\"program\": \"git\", \"args\": [\"pull\"]}}. \
             {programs}There is no shell: pipes, redirection, globbing and `&&` are not \
             interpreted, so run one program per call."
        )
    }

    fn spec(&self, input: Value) -> Result<ProgramSpec, ToolError> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Input {
            program: String,
            #[serde(default)]
            args: Vec<String>,
            cwd: Option<PathBuf>,
            timeout_ms: Option<u64>,
        }
        let input: Input = serde_json::from_value(input)
            .map_err(|error| ToolError::InvalidInput(error.to_string()))?;
        if let Some(allowed) = &self.allow_programs
            && !allowed.contains(&input.program)
        {
            return Err(ToolError::InvalidInput(format!(
                "program `{}` is not in this tool's allow_programs",
                input.program
            )));
        }
        let timeout = match input.timeout_ms {
            None => self.default_timeout,
            Some(ms) if ms > 0 && Duration::from_millis(ms) <= self.max_timeout => {
                Duration::from_millis(ms)
            }
            Some(_) => {
                return Err(ToolError::InvalidInput(format!(
                    "timeout_ms must be between 1 and {}",
                    self.max_timeout.as_millis()
                )));
            }
        };
        let mut spec = ProgramSpec::new(input.program, input.args, timeout);
        spec.cwd = input.cwd;
        spec.limits = OutputLimits {
            stdout_bytes: self.max_output_bytes,
            stderr_bytes: self.max_output_bytes,
        };
        Ok(spec)
    }
}

#[async_trait]
impl Tool for ShellExec {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: ToolName::new("shell.exec"),
            description: self.description(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "program": {
                        "type": "string",
                        "description": "The program to run, without arguments, such as `git`."
                    },
                    "args": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "One string per argument, such as [\"pull\"] or [\"commit\", \"-m\", \"message\"]."
                    },
                    "cwd": {
                        "type": "string",
                        "description": "Working directory relative to the workspace root; the root when omitted."
                    },
                    "timeout_ms": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": self.max_timeout.as_millis(),
                        "description": format!(
                            "Timeout in milliseconds; {} when omitted.",
                            self.default_timeout.as_millis()
                        )
                    }
                },
                "required": ["program"],
                "additionalProperties": false
            }),
            permission: PermissionLevel::Execute,
        }
    }

    async fn call(&self, input: Value, cx: ToolContext) -> Result<ToolOutput, ToolError> {
        let spec = self.spec(input)?;
        match self.sandbox.run(spec, cx.cancel).await {
            Ok(output) => match output.termination {
                None => Ok(ToolOutput {
                    is_error: output.exit_code != Some(0),
                    content: vec![ToolContent::Json(render(&output))],
                }),
                Some(termination) => Err(ToolError::Execution(format!(
                    "the process was terminated ({}); its effects are uncertain. \
                     Partial output: {}",
                    match termination {
                        Termination::TimedOut => "timed out",
                        Termination::OutputLimit => "output limit exceeded",
                        Termination::Cancelled => "cancelled",
                    },
                    render(&output)
                ))),
            },
            Err(error @ (ExecError::InvalidSpec(_) | ExecError::Spawn { .. })) => {
                Err(ToolError::NotStarted(error.to_string()))
            }
            Err(error @ ExecError::Supervision(_)) => Err(ToolError::Execution(error.to_string())),
        }
    }
}

fn render(output: &ExecOutput) -> Value {
    json!({
        "exit_code": output.exit_code,
        "stdout": String::from_utf8_lossy(&output.stdout),
        "stderr": String::from_utf8_lossy(&output.stderr),
    })
}

impl ShellToolsPlugin {
    /// An example configuration, mostly the defaults, for reference (the Web
    /// UI shows it in an empty configuration editor). It is never applied.
    pub const CONFIG_EXAMPLE: &str = r#"{
  "allow_programs": [
    "git",
    "cargo"
  ],
  "default_timeout_ms": 30000,
  "max_timeout_ms": 300000,
  "max_output_bytes": 65536
}"#;

    /// What each configuration key does, as `(key, help)` pairs, for the
    /// Web UI's configuration editor.
    pub const CONFIG_HELP: &[(&str, &str)] = &[
        (
            "allow_programs",
            "Exact program names shell.exec may run, for example [\"git\", \
             \"cargo\"]; any program when unset. Programs run through the \
             sandbox plugin (sandbox-process or sandbox-os), which must be \
             mounted, and the policy must assign \"shell.exec\" = \"execute\". \
             A program missing an environment variable needs it in the \
             sandbox's inherit_env_extra.",
        ),
        (
            "default_timeout_ms",
            "Timeout when the model does not choose one, in milliseconds \
             (default 30000).",
        ),
        (
            "max_timeout_ms",
            "Longest timeout the model may choose, in milliseconds (default \
             300000). It must also fit the sandbox's max_timeout_ms.",
        ),
        (
            "max_output_bytes",
            "Bytes kept from each of stdout and stderr (default 65536).",
        ),
    ];
}

#[cfg(test)]
mod tests;

#[cfg(test)]
#[test]
fn the_config_example_is_a_valid_configuration() {
    let parsed: Result<ShellConfig, _> = serde_json::from_str(ShellToolsPlugin::CONFIG_EXAMPLE);
    assert!(parsed.is_ok(), "{:?}", parsed.err());
}

#[cfg(test)]
#[test]
fn the_config_help_describes_every_example_key() {
    let example: serde_json::Map<String, Value> =
        serde_json::from_str(ShellToolsPlugin::CONFIG_EXAMPLE).unwrap();
    let mut example_keys: Vec<&str> = example.keys().map(String::as_str).collect();
    let mut help_keys: Vec<&str> = ShellToolsPlugin::CONFIG_HELP
        .iter()
        .map(|(key, _)| *key)
        .collect();
    example_keys.sort_unstable();
    help_keys.sort_unstable();
    assert_eq!(help_keys, example_keys);
}
