use std::path::Path;

use async_trait::async_trait;
use rah_protocol::{ToolDefinition, ToolInput, ToolOutput};

use crate::{
    HostArgumentPolicy, HostExecutionPolicy, HostExecutionTool, Tool, ToolContext, ToolError,
};

/// Stable tool name for the host-preauthorized Cargo version capability.
pub const CARGO_VERSION_TOOL_NAME: &str = "host.cargo.version";

/// Runs exactly one host-selected native Cargo executable with `--version`.
///
/// Construction is trusted host setup. The executable and isolated working
/// directory are canonicalized by [`HostExecutionPolicy`], while model input is
/// restricted to an empty object. This capability does not authorize any other
/// Cargo command or generic process execution.
pub struct CargoVersionTool {
    inner: HostExecutionTool,
}

impl CargoVersionTool {
    /// Creates the capability from an absolute host-selected Cargo executable
    /// and an existing host-owned non-sensitive working directory.
    pub fn new(
        cargo_executable: impl AsRef<Path>,
        isolated_cwd: impl AsRef<Path>,
    ) -> Result<Self, ToolError> {
        let policy = HostExecutionPolicy::new(
            cargo_executable,
            HostArgumentPolicy::Exact(vec!["--version".to_owned()]),
            isolated_cwd,
            ".",
        )?;
        Ok(Self {
            inner: HostExecutionTool::new(
                CARGO_VERSION_TOOL_NAME,
                "Reports the version of the host-preauthorized Cargo executable.",
                policy,
            ),
        })
    }
}

#[async_trait]
impl Tool for CargoVersionTool {
    fn definition(&self) -> ToolDefinition {
        self.inner.definition()
    }

    async fn execute(
        &self,
        input: ToolInput,
        context: ToolContext,
    ) -> Result<ToolOutput, ToolError> {
        self.inner.execute(input, context).await
    }
}
