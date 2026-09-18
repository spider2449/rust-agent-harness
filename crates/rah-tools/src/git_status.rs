use std::path::{Path, PathBuf};

use async_trait::async_trait;
use rah_protocol::{ToolDefinition, ToolInput, ToolOutput};

use crate::{
    HostArgumentPolicy, HostExecutionPolicy, HostExecutionTool, Tool, ToolContext, ToolError,
    git_support::{git_environment, git_error},
    repository_git_layout::RepositoryGitLayout,
};

/// Stable tool name for the repository-specific host Git status capability.
pub const GIT_STATUS_TOOL_NAME: &str = "host.git.status";

/// Runs exactly `git status --porcelain=v1` in one host-authorized repository.
///
/// Construction is trusted host setup. The native Git executable and repository
/// are canonicalized before registration and revalidated before every call.
/// Model input is restricted to an empty object and cannot select process or
/// repository details.
pub struct GitStatusTool {
    repository: RepositoryGitLayout,
    git: PathBuf,
    inner: HostExecutionTool,
}

impl GitStatusTool {
    /// Creates the capability from an absolute host-selected native Git
    /// executable and an absolute host-selected repository root.
    pub fn new(
        git_executable: impl AsRef<Path>,
        repository_root: impl AsRef<Path>,
    ) -> Result<Self, ToolError> {
        if !repository_root.as_ref().is_absolute() {
            return Err(git_error("repository root must be an absolute path"));
        }
        let policy = HostExecutionPolicy::new(
            git_executable.as_ref(),
            HostArgumentPolicy::Exact(vec!["status".to_owned(), "--porcelain=v1".to_owned()]),
            repository_root.as_ref(),
            ".",
        )?
        .with_environment(git_environment())?;
        let repository =
            RepositoryGitLayout::capture(git_executable.as_ref(), repository_root.as_ref())?;
        Ok(Self {
            repository,
            git: git_executable.as_ref().to_path_buf(),
            inner: HostExecutionTool::new(
                GIT_STATUS_TOOL_NAME,
                "Reports porcelain status for one host-authorized Git repository.",
                policy,
            ),
        })
    }
}

#[async_trait]
impl Tool for GitStatusTool {
    fn definition(&self) -> ToolDefinition {
        self.inner.definition()
    }

    async fn execute(
        &self,
        input: ToolInput,
        context: ToolContext,
    ) -> Result<ToolOutput, ToolError> {
        self.repository.revalidate()?;
        if self.repository.is_linked() {
            self.repository.validate_git(&self.git).await?;
        }
        self.inner.execute(input, context).await
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use rah_protocol::{ToolContent, ToolInput};
    use serde_json::json;

    use crate::{Tool, ToolContext};

    use super::GitStatusTool;

    #[tokio::test]
    async fn linked_status_reads_only_the_selected_worktree() {
        let fixture = crate::repository_git_layout::test_fixture::WorktreeFixture::new();
        fs::write(fixture.linked_b.join("sibling-only.txt"), "sibling\n").unwrap();
        let tool = GitStatusTool::new(&fixture.git, &fixture.linked_a).unwrap();
        let output = tool
            .execute(ToolInput(json!({})), ToolContext::default())
            .await
            .unwrap();
        let [ToolContent::Json(value)] = output.content.as_slice() else {
            panic!("Git status should return one JSON object")
        };
        assert_eq!(value["stdout"], "");

        fs::write(fixture.linked_a.join("selected-only.txt"), "selected\n").unwrap();
        let output = tool
            .execute(ToolInput(json!({})), ToolContext::default())
            .await
            .unwrap();
        let [ToolContent::Json(value)] = output.content.as_slice() else {
            panic!("Git status should return one JSON object")
        };
        assert_eq!(value["stdout"], "?? selected-only.txt\n");
    }
}
