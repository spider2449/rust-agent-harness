//! Closed, byte-safe `repo.diff-staged` observer.
//!
//! The host fixes the sole semantic difference from `repo.diff`: index versus
//! HEAD. Git's fixed `diff --cached` semantics select its object-format-correct
//! empty tree for an unborn HEAD; the model cannot select a revision or argv.

use std::path::Path;

use async_trait::async_trait;
use rah_protocol::{PermissionLevel, ToolDefinition, ToolInput, ToolName, ToolOutput};
use serde_json::json;

use crate::{
    Tool, ToolContext, ToolError,
    repository_diff::{DiffBaseline, execute_fixed_diff},
    repository_observer::RepositoryObserver,
};

/// Stable name for the fixed, read-only index-versus-HEAD diff observer.
pub const REPOSITORY_DIFF_STAGED_TOOL_NAME: &str = "repo.diff-staged";

/// Reports a bounded normalized index-versus-HEAD diff for one host-selected repository.
///
/// The model supplies only `{}`. The host owns the executable, repository,
/// fixed `--cached` commands, child environment, capture limits, timeout, and
/// one exclusive RAH repository lease spanning HEAD checks and all diff phases.
pub struct RepositoryDiffStagedTool {
    observer: RepositoryObserver,
}

impl RepositoryDiffStagedTool {
    /// Creates the observer for one host-selected native Git executable and repository.
    pub fn new(
        git_executable: impl AsRef<Path>,
        repository_root: impl AsRef<Path>,
    ) -> Result<Self, ToolError> {
        Ok(Self {
            observer: RepositoryObserver::new(git_executable.as_ref(), repository_root.as_ref())?,
        })
    }
}

#[async_trait]
impl Tool for RepositoryDiffStagedTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: ToolName::new(REPOSITORY_DIFF_STAGED_TOOL_NAME),
            description: "Reports a bounded read-only index-versus-HEAD Git diff for one host-authorized repository.".to_owned(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
            permission: PermissionLevel::Execute,
        }
    }

    async fn execute(
        &self,
        input: ToolInput,
        _context: ToolContext,
    ) -> Result<ToolOutput, ToolError> {
        execute_fixed_diff(&self.observer, &input, DiffBaseline::IndexVsHead).await
    }
}
