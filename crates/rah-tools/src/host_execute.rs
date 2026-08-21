use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs,
    path::{Component, Path, PathBuf},
    time::{Duration, SystemTime},
};

use async_trait::async_trait;
use rah_protocol::{PermissionLevel, ToolContent, ToolDefinition, ToolInput, ToolName, ToolOutput};
use rah_sandbox::{
    HostProcessOutput, HostProcessSpec, OutputLimits, OutputOverflow, execute_host_process,
};
use serde_json::{Map, Value, json};

use crate::{Tool, ToolContext, ToolError};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);
const DEFAULT_SERIALIZED_OUTPUT_LIMIT: usize = 768 * 1024;

/// Closed host-owned argument renderer for one executable capability.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HostArgumentPolicy {
    /// Always supplies one exact host-selected argument vector and accepts `{}`.
    Exact(Vec<String>),
    /// Accepts only `{"text":"..."}` and appends that value as one literal argument.
    Text {
        /// Exact host-selected arguments before the one typed text value.
        prefix: Vec<String>,
        /// Maximum UTF-8 byte length of the text value.
        max_bytes: usize,
    },
}

/// Immutable host authorization for one capability-specific native process.
///
/// This policy controls selection and process construction. It is not a
/// filesystem, network, or operating-system sandbox.
#[derive(Clone, Debug)]
pub struct HostExecutionPolicy {
    executable: PathBuf,
    executable_identity: ExecutableIdentity,
    arguments: HostArgumentPolicy,
    cwd_root: PathBuf,
    cwd: PathBuf,
    environment: BTreeMap<OsString, OsString>,
    timeout: Duration,
    output_limits: OutputLimits,
    serialized_output_limit: usize,
}

impl HostExecutionPolicy {
    /// Resolves a trusted executable and a fixed cwd beneath a canonical root.
    pub fn new(
        executable: impl AsRef<Path>,
        arguments: HostArgumentPolicy,
        cwd_root: impl AsRef<Path>,
        relative_cwd: impl AsRef<Path>,
    ) -> Result<Self, ToolError> {
        validate_argument_policy(&arguments)?;
        let executable = canonical_native_executable(executable.as_ref())?;
        let executable_identity = ExecutableIdentity::capture(&executable)?;
        let cwd_root = canonical_directory(cwd_root.as_ref(), "cwd root")?;
        let relative_cwd = relative_cwd.as_ref();
        validate_relative_cwd(relative_cwd)?;
        let cwd = canonical_directory(&cwd_root.join(relative_cwd), "working directory")?;
        if !cwd.starts_with(&cwd_root) {
            return Err(policy_error(
                "working directory resolves outside the configured root",
            ));
        }

        Ok(Self {
            executable,
            executable_identity,
            arguments,
            cwd_root,
            cwd,
            environment: BTreeMap::new(),
            timeout: DEFAULT_TIMEOUT,
            output_limits: OutputLimits::RECOMMENDED,
            serialized_output_limit: DEFAULT_SERIALIZED_OUTPUT_LIMIT,
        })
    }

    /// Replaces the cleared child environment with exact trusted host values.
    pub fn with_environment(
        mut self,
        environment: BTreeMap<OsString, OsString>,
    ) -> Result<Self, ToolError> {
        for (name, value) in &environment {
            let name = name.to_string_lossy();
            let value = value.to_string_lossy();
            if name.is_empty() || name.contains('=') || name.contains('\0') || value.contains('\0')
            {
                return Err(policy_error(
                    "environment contains an invalid name or value",
                ));
            }
        }
        self.environment = environment;
        Ok(self)
    }

    /// Replaces the host-fixed timeout. The model cannot alter it.
    pub fn with_timeout(mut self, timeout: Duration) -> Result<Self, ToolError> {
        if timeout.is_zero() {
            return Err(policy_error("timeout must be greater than zero"));
        }
        self.timeout = timeout;
        Ok(self)
    }

    /// Replaces the hard output limits for deterministic tests or stricter capabilities.
    pub fn with_output_limits(mut self, limits: OutputLimits) -> Result<Self, ToolError> {
        if limits.stdout_bytes == 0
            || limits.stderr_bytes == 0
            || limits.combined_bytes == 0
            || limits.combined_bytes > limits.stdout_bytes.saturating_add(limits.stderr_bytes)
        {
            return Err(policy_error("output limits are invalid"));
        }
        self.output_limits = limits;
        Ok(self)
    }

    /// Replaces the serialized result limit.
    pub fn with_serialized_output_limit(mut self, bytes: usize) -> Result<Self, ToolError> {
        if bytes == 0 {
            return Err(policy_error(
                "serialized output limit must be greater than zero",
            ));
        }
        self.serialized_output_limit = bytes;
        Ok(self)
    }

    fn input_schema(&self) -> Value {
        match self.arguments {
            HostArgumentPolicy::Exact(_) => json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
            HostArgumentPolicy::Text { max_bytes, .. } => json!({
                "type": "object",
                "properties": {
                    "text": {"type": "string", "maxLength": max_bytes}
                },
                "required": ["text"],
                "additionalProperties": false
            }),
        }
    }

    fn render_arguments(&self, input: &ToolInput) -> Result<Vec<OsString>, ToolError> {
        let object = input.0.as_object().ok_or_else(|| ToolError::InvalidInput {
            message: "input must be an object".to_owned(),
        })?;
        match &self.arguments {
            HostArgumentPolicy::Exact(arguments) => {
                reject_unknown_fields(object, &[])?;
                Ok(arguments.iter().map(OsString::from).collect())
            }
            HostArgumentPolicy::Text { prefix, max_bytes } => {
                reject_unknown_fields(object, &["text"])?;
                let text = object.get("text").and_then(Value::as_str).ok_or_else(|| {
                    ToolError::InvalidInput {
                        message: "`text` must be a string".to_owned(),
                    }
                })?;
                if text.len() > *max_bytes || text.contains('\0') {
                    return Err(ToolError::InvalidInput {
                        message: format!(
                            "`text` must contain at most {max_bytes} UTF-8 bytes and no NUL"
                        ),
                    });
                }
                let mut arguments = prefix.iter().map(OsString::from).collect::<Vec<_>>();
                arguments.push(OsString::from(text));
                Ok(arguments)
            }
        }
    }

    async fn execute(&self, input: &ToolInput) -> Result<ToolOutput, ToolError> {
        self.revalidate()?;
        let args = self.render_arguments(input)?;
        let output = execute_host_process(HostProcessSpec {
            executable: self.executable.clone(),
            args,
            cwd: self.cwd.clone(),
            environment: self.environment.clone(),
            timeout: self.timeout,
            output_limits: self.output_limits,
        })
        .await
        .map_err(|error| ToolError::Execution {
            message: error.to_string(),
        })?;
        self.map_output(output)
    }

    fn revalidate(&self) -> Result<(), ToolError> {
        let current = canonical_native_executable(&self.executable)?;
        if current != self.executable {
            return Err(policy_error("configured executable identity changed"));
        }
        if ExecutableIdentity::capture(&current)? != self.executable_identity {
            return Err(policy_error("configured executable identity changed"));
        }
        let cwd = canonical_directory(&self.cwd, "working directory")?;
        if cwd != self.cwd || !cwd.starts_with(&self.cwd_root) {
            return Err(policy_error(
                "configured working directory identity changed",
            ));
        }
        Ok(())
    }

    fn map_output(&self, output: HostProcessOutput) -> Result<ToolOutput, ToolError> {
        let reason = if output.timed_out {
            Some("timeout")
        } else if output.overflow.is_some() {
            Some("output_limit_exceeded")
        } else if output.exit_code != Some(0) {
            Some("nonzero_exit")
        } else {
            None
        };
        let content = json!({
            "status": reason.unwrap_or("exited"),
            "stdout": String::from_utf8_lossy(&output.stdout),
            "stderr": String::from_utf8_lossy(&output.stderr),
            "exit_code": output.exit_code,
            "timed_out": output.timed_out,
            "output_overflow": overflow_name(output.overflow),
            "termination_attempted": output.termination_attempted,
            "retained_stdout_bytes": output.stdout.len(),
            "retained_stderr_bytes": output.stderr.len()
        });
        let mut tool_output = ToolOutput {
            content: vec![ToolContent::Json(content)],
            is_error: reason.is_some(),
        };
        let serialized =
            serde_json::to_vec(&tool_output).map_err(|error| ToolError::Execution {
                message: format!("failed to serialize process result: {error}"),
            })?;
        if serialized.len() > self.serialized_output_limit {
            tool_output = ToolOutput {
                content: vec![ToolContent::Json(json!({
                    "status": "serialized_output_limit_exceeded",
                    "timed_out": false,
                    "output_overflow": "serialized",
                    "termination_attempted": output.termination_attempted,
                    "retained_stdout_bytes": output.stdout.len(),
                    "retained_stderr_bytes": output.stderr.len()
                }))],
                is_error: true,
            };
        }
        let final_size = serde_json::to_vec(&tool_output)
            .map_err(|error| ToolError::Execution {
                message: format!("failed to serialize bounded process result: {error}"),
            })?
            .len();
        if final_size > self.serialized_output_limit {
            return Err(ToolError::Execution {
                message: "serialized process result limit is too small for status metadata"
                    .to_owned(),
            });
        }
        Ok(tool_output)
    }
}

/// Capability-specific Execute tool backed by one immutable host policy.
pub struct HostExecutionTool {
    name: ToolName,
    description: String,
    policy: HostExecutionPolicy,
}

impl HostExecutionTool {
    /// Creates one host-preauthorized capability with no model-selected process fields.
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        policy: HostExecutionPolicy,
    ) -> Self {
        Self {
            name: ToolName::new(name),
            description: description.into(),
            policy,
        }
    }
}

#[async_trait]
impl Tool for HostExecutionTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: self.name.clone(),
            description: self.description.clone(),
            input_schema: self.policy.input_schema(),
            permission: PermissionLevel::Execute,
        }
    }

    async fn execute(
        &self,
        input: ToolInput,
        _context: ToolContext,
    ) -> Result<ToolOutput, ToolError> {
        self.policy.execute(&input).await
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ExecutableIdentity {
    length: u64,
    modified: Option<SystemTime>,
}

impl ExecutableIdentity {
    fn capture(path: &Path) -> Result<Self, ToolError> {
        let metadata = fs::metadata(path).map_err(|error| policy_error(error.to_string()))?;
        Ok(Self {
            length: metadata.len(),
            modified: metadata.modified().ok(),
        })
    }
}

fn canonical_native_executable(path: &Path) -> Result<PathBuf, ToolError> {
    if !path.is_absolute() {
        return Err(policy_error(
            "configured executable must be an absolute path; PATH lookup is disabled",
        ));
    }
    let canonical = fs::canonicalize(path).map_err(|error| policy_error(error.to_string()))?;
    let metadata = fs::metadata(&canonical).map_err(|error| policy_error(error.to_string()))?;
    if !metadata.is_file() {
        return Err(policy_error("configured executable must be a regular file"));
    }
    validate_native_executable(&canonical, &metadata)?;
    Ok(canonical)
}

#[cfg(windows)]
fn validate_native_executable(path: &Path, _metadata: &fs::Metadata) -> Result<(), ToolError> {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    if !extension.eq_ignore_ascii_case("exe") {
        return Err(policy_error(
            "Windows Execute capabilities require a native .exe file; scripts are rejected",
        ));
    }
    Ok(())
}

#[cfg(unix)]
fn validate_native_executable(path: &Path, metadata: &fs::Metadata) -> Result<(), ToolError> {
    use std::os::unix::fs::PermissionsExt;

    if metadata.permissions().mode() & 0o111 == 0 {
        return Err(policy_error("configured executable is not executable"));
    }
    let prefix = fs::read(path)
        .map_err(|error| policy_error(error.to_string()))?
        .into_iter()
        .take(2)
        .collect::<Vec<_>>();
    if prefix == b"#!" {
        return Err(policy_error(
            "script and shebang executables are not supported by the prototype",
        ));
    }
    Ok(())
}

#[cfg(not(any(unix, windows)))]
fn validate_native_executable(_path: &Path, _metadata: &fs::Metadata) -> Result<(), ToolError> {
    Ok(())
}

fn canonical_directory(path: &Path, label: &str) -> Result<PathBuf, ToolError> {
    let canonical = fs::canonicalize(path).map_err(|error| policy_error(error.to_string()))?;
    if !canonical.is_dir() {
        return Err(policy_error(format!(
            "{label} must be an existing directory"
        )));
    }
    Ok(canonical)
}

fn validate_relative_cwd(path: &Path) -> Result<(), ToolError> {
    if path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(policy_error(
            "working directory must be a relative path without traversal",
        ));
    }
    Ok(())
}

fn validate_argument_policy(arguments: &HostArgumentPolicy) -> Result<(), ToolError> {
    let values = match arguments {
        HostArgumentPolicy::Exact(values) | HostArgumentPolicy::Text { prefix: values, .. } => {
            values
        }
    };
    if values.iter().any(|value| value.contains('\0')) {
        return Err(policy_error("configured arguments must not contain NUL"));
    }
    if matches!(arguments, HostArgumentPolicy::Text { max_bytes: 0, .. }) {
        return Err(policy_error(
            "text argument limit must be greater than zero",
        ));
    }
    Ok(())
}

fn reject_unknown_fields(object: &Map<String, Value>, allowed: &[&str]) -> Result<(), ToolError> {
    if let Some(name) = object.keys().find(|name| !allowed.contains(&name.as_str())) {
        return Err(ToolError::InvalidInput {
            message: format!("unknown field `{name}`"),
        });
    }
    Ok(())
}

fn overflow_name(overflow: Option<OutputOverflow>) -> Option<&'static str> {
    overflow.map(|value| match value {
        OutputOverflow::Stdout => "stdout",
        OutputOverflow::Stderr => "stderr",
        OutputOverflow::Combined => "combined",
    })
}

fn policy_error(message: impl Into<String>) -> ToolError {
    ToolError::Execution {
        message: format!(
            "host execution policy rejected capability: {}",
            message.into()
        ),
    }
}
