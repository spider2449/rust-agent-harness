//! Hardened, bounded Git invocations through the `sandbox` service.

use std::{collections::BTreeMap, path::PathBuf, sync::Arc, time::Duration};

use rah_api::{
    CancellationToken,
    sandbox::{ExecEnv, ExecError, OutputLimits, ProgramSpec, Termination},
    tools::ToolError,
};

/// Bytes kept from Git's stderr for error messages.
const STDERR_BYTES: usize = 16 * 1024;

/// The environment every Git invocation gets on top of the sandbox's base
/// environment: no system or global configuration, no fsmonitor or
/// untracked cache, no optional locks, no prompts, literal pathspecs.
pub(crate) fn hardened_env() -> BTreeMap<String, String> {
    let null_device = if cfg!(windows) { "NUL" } else { "/dev/null" };
    [
        ("GIT_CONFIG_NOSYSTEM", "1"),
        ("GIT_CONFIG_GLOBAL", null_device),
        ("GIT_CONFIG_COUNT", "2"),
        ("GIT_CONFIG_KEY_0", "core.fsmonitor"),
        ("GIT_CONFIG_VALUE_0", "false"),
        ("GIT_CONFIG_KEY_1", "core.untrackedCache"),
        ("GIT_CONFIG_VALUE_1", "false"),
        ("GIT_OPTIONAL_LOCKS", "0"),
        ("GIT_TERMINAL_PROMPT", "0"),
        ("GIT_LITERAL_PATHSPECS", "1"),
    ]
    .into_iter()
    .map(|(name, value)| (name.to_owned(), value.to_owned()))
    .collect()
}

pub(crate) struct Git {
    pub sandbox: Arc<dyn ExecEnv>,
    pub program: String,
    /// The repository directory, relative to the sandbox root.
    pub cwd: Option<PathBuf>,
    pub timeout: Duration,
    pub max_output_bytes: usize,
}

pub(crate) struct GitOutput {
    pub stdout: Vec<u8>,
    /// stdout hit `max_output_bytes`; the output is a prefix.
    pub truncated: bool,
}

impl Git {
    /// Runs `git --no-pager <args>`. Exit codes in `ok_codes` succeed; any
    /// other code is an error carrying Git's stderr.
    pub async fn run(
        &self,
        args: &[&str],
        ok_codes: &[i32],
        cancel: CancellationToken,
    ) -> Result<GitOutput, ToolError> {
        let mut argv = vec!["--no-pager".to_owned()];
        argv.extend(args.iter().map(|arg| (*arg).to_owned()));
        let mut spec = ProgramSpec::new(self.program.clone(), argv, self.timeout);
        spec.cwd = self.cwd.clone();
        spec.env = hardened_env();
        spec.limits = OutputLimits {
            stdout_bytes: self.max_output_bytes,
            stderr_bytes: STDERR_BYTES,
        };
        let output = self
            .sandbox
            .run(spec, cancel)
            .await
            .map_err(|error| match error {
                ExecError::InvalidSpec(_) | ExecError::Spawn { .. } => {
                    ToolError::Execution(format!("cannot run git: {error}"))
                }
                ExecError::Supervision(_) => ToolError::Execution(error.to_string()),
            })?;
        match output.termination {
            None => {}
            // Only a full stdout is a usable (truncated) result.
            Some(Termination::OutputLimit) if output.stdout.len() >= self.max_output_bytes => {
                return Ok(GitOutput {
                    stdout: output.stdout,
                    truncated: true,
                });
            }
            Some(Termination::OutputLimit) => {
                return Err(ToolError::Execution("git wrote too much to stderr".into()));
            }
            Some(Termination::TimedOut) => {
                return Err(ToolError::Execution(format!(
                    "git timed out after {} ms",
                    self.timeout.as_millis()
                )));
            }
            Some(Termination::Cancelled) => return Err(ToolError::Cancelled),
        }
        let exit_code = output.exit_code.unwrap_or(-1);
        if !ok_codes.contains(&exit_code) {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ToolError::Execution(format!(
                "git {} failed ({exit_code}): {}",
                args.first().copied().unwrap_or_default(),
                stderr.trim()
            )));
        }
        Ok(GitOutput {
            stdout: output.stdout,
            truncated: false,
        })
    }

    /// Fails unless the working directory is the top level of a work tree,
    /// so Git never reads a repository above the configured directory.
    pub async fn ensure_top_level(&self, cancel: CancellationToken) -> Result<(), ToolError> {
        let output = self
            .run(&["rev-parse", "--show-prefix"], &[0], cancel)
            .await?;
        if output.stdout.iter().any(|b| !b.is_ascii_whitespace()) {
            return Err(ToolError::Execution(
                "the configured directory is not the top level of a Git work tree".into(),
            ));
        }
        Ok(())
    }
}
