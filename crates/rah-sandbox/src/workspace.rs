use std::{
    fs,
    path::{Component, Path, PathBuf},
};

use thiserror::Error;

/// Error returned while validating a path against a workspace root.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum WorkspacePathError {
    /// The configured workspace root is unavailable.
    #[error("workspace root `{path}` is unavailable: {message}")]
    InvalidRoot {
        /// Configured workspace root.
        path: PathBuf,
        /// Filesystem failure detail.
        message: String,
    },
    /// The requested existing path does not exist.
    #[error("workspace path `{path}` does not exist")]
    NotFound {
        /// Missing path.
        path: PathBuf,
    },
    /// The resolved path escapes the workspace root.
    #[error("path `{path}` resolves outside workspace `{workspace_root}`")]
    OutsideWorkspace {
        /// Rejected requested path.
        path: PathBuf,
        /// Canonical workspace root.
        workspace_root: PathBuf,
    },
    /// The requested path could not be resolved safely.
    #[error("path `{path}` could not be resolved: {message}")]
    Resolution {
        /// Requested path.
        path: PathBuf,
        /// Resolution failure detail.
        message: String,
    },
}

/// Canonical workspace boundary for filesystem path validation.
///
/// This policy validates paths only; it is not strong operating-system isolation.
#[derive(Clone, Debug)]
pub struct WorkspacePolicy {
    root: PathBuf,
}

impl WorkspacePolicy {
    /// Creates a policy from an existing workspace directory.
    pub fn new(root: impl AsRef<Path>) -> Result<Self, WorkspacePathError> {
        let requested = root.as_ref();
        let canonical =
            fs::canonicalize(requested).map_err(|error| WorkspacePathError::InvalidRoot {
                path: requested.to_path_buf(),
                message: error.to_string(),
            })?;
        if !canonical.is_dir() {
            return Err(WorkspacePathError::InvalidRoot {
                path: requested.to_path_buf(),
                message: "root is not a directory".to_owned(),
            });
        }

        Ok(Self { root: canonical })
    }

    /// Returns the canonical workspace root.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Resolves and validates an existing path.
    pub fn resolve_existing(&self, path: impl AsRef<Path>) -> Result<PathBuf, WorkspacePathError> {
        let requested = path.as_ref();
        let candidate = self.candidate(requested);
        if !candidate.exists() {
            return Err(WorkspacePathError::NotFound { path: candidate });
        }
        let resolved =
            fs::canonicalize(&candidate).map_err(|error| WorkspacePathError::Resolution {
                path: candidate.clone(),
                message: error.to_string(),
            })?;
        self.ensure_inside(requested, resolved)
    }

    /// Resolves a path that may not exist yet by validating its nearest existing ancestor.
    pub fn resolve_write(&self, path: impl AsRef<Path>) -> Result<PathBuf, WorkspacePathError> {
        let requested = path.as_ref();
        let candidate = self.candidate(requested);
        if candidate.exists() {
            return self.resolve_existing(requested);
        }

        let mut ancestor = candidate.as_path();
        while !ancestor.exists() {
            ancestor = ancestor
                .parent()
                .ok_or_else(|| WorkspacePathError::Resolution {
                    path: candidate.clone(),
                    message: "no existing ancestor was found".to_owned(),
                })?;
        }
        let canonical_ancestor =
            fs::canonicalize(ancestor).map_err(|error| WorkspacePathError::Resolution {
                path: ancestor.to_path_buf(),
                message: error.to_string(),
            })?;
        self.ensure_inside(requested, canonical_ancestor.clone())?;
        let remainder =
            candidate
                .strip_prefix(ancestor)
                .map_err(|error| WorkspacePathError::Resolution {
                    path: candidate.clone(),
                    message: error.to_string(),
                })?;
        if remainder.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        }) {
            return Err(WorkspacePathError::OutsideWorkspace {
                path: requested.to_path_buf(),
                workspace_root: self.root.clone(),
            });
        }
        let resolved = canonical_ancestor.join(remainder);
        self.ensure_inside(requested, resolved)
    }

    fn candidate(&self, requested: &Path) -> PathBuf {
        if requested.is_absolute() {
            requested.to_path_buf()
        } else {
            self.root.join(requested)
        }
    }

    fn ensure_inside(
        &self,
        requested: &Path,
        resolved: PathBuf,
    ) -> Result<PathBuf, WorkspacePathError> {
        if resolved.starts_with(&self.root) {
            Ok(resolved)
        } else {
            Err(WorkspacePathError::OutsideWorkspace {
                path: requested.to_path_buf(),
                workspace_root: self.root.clone(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::{WorkspacePathError, WorkspacePolicy};

    static NEXT_TEST_DIR: AtomicU64 = AtomicU64::new(0);

    struct TestDirectory {
        path: PathBuf,
    }

    impl TestDirectory {
        fn new() -> Self {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time should follow Unix epoch")
                .as_nanos();
            let sequence = NEXT_TEST_DIR.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "rah-workspace-policy-{}-{timestamp}-{sequence}",
                std::process::id()
            ));
            fs::create_dir(&path).expect("test directory should be created");
            Self { path }
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn resolves_existing_child() {
        let base = TestDirectory::new();
        let workspace = base.path.join("workspace");
        fs::create_dir(&workspace).expect("workspace should be created");
        fs::write(workspace.join("child.txt"), "content").expect("file should be written");
        let policy = WorkspacePolicy::new(&workspace).expect("workspace should be valid");

        let resolved = policy
            .resolve_existing("child.txt")
            .expect("child should resolve");

        assert_eq!(
            resolved,
            fs::canonicalize(workspace.join("child.txt")).expect("child should canonicalize")
        );
    }

    #[test]
    fn rejects_parent_escape() {
        let (base, workspace, policy) = setup_workspace();
        fs::write(base.path.join("outside.txt"), "outside").expect("file should be written");

        let error = policy
            .resolve_existing("../outside.txt")
            .expect_err("parent escape should fail");

        assert!(matches!(error, WorkspacePathError::OutsideWorkspace { .. }));
        assert!(workspace.exists());
    }

    #[test]
    fn rejects_absolute_outside_path() {
        let (base, _workspace, policy) = setup_workspace();
        let outside = base.path.join("outside.txt");
        fs::write(&outside, "outside").expect("file should be written");

        let error = policy
            .resolve_existing(&outside)
            .expect_err("absolute outside path should fail");

        assert!(matches!(error, WorkspacePathError::OutsideWorkspace { .. }));
    }

    #[test]
    fn resolves_nonexistent_write_target_inside_workspace() {
        let (_base, workspace, policy) = setup_workspace();

        let resolved = policy
            .resolve_write("new/nested.txt")
            .expect("nonexistent child should resolve");

        assert_eq!(resolved, policy.root().join("new/nested.txt"));
        assert_eq!(
            policy.root(),
            fs::canonicalize(workspace).expect("workspace should canonicalize")
        );
    }

    #[test]
    fn rejects_parent_escape_after_nonexistent_component() {
        let (_base, _workspace, policy) = setup_workspace();

        let error = policy
            .resolve_write("missing/../../outside.txt")
            .expect_err("unresolved parent escape should fail");

        assert!(matches!(error, WorkspacePathError::OutsideWorkspace { .. }));
    }

    #[test]
    fn rejects_symlink_escape_when_supported() {
        let (base, workspace, policy) = setup_workspace();
        let outside = base.path.join("outside.txt");
        let link = workspace.join("outside-link.txt");
        fs::write(&outside, "outside").expect("file should be written");
        if create_file_symlink(&outside, &link).is_err() {
            return;
        }

        let error = policy
            .resolve_existing("outside-link.txt")
            .expect_err("symlink escape should fail");

        assert!(matches!(error, WorkspacePathError::OutsideWorkspace { .. }));
    }

    fn setup_workspace() -> (TestDirectory, PathBuf, WorkspacePolicy) {
        let base = TestDirectory::new();
        let workspace = base.path.join("workspace");
        fs::create_dir(&workspace).expect("workspace should be created");
        let policy = WorkspacePolicy::new(&workspace).expect("workspace should be valid");
        (base, workspace, policy)
    }

    #[cfg(unix)]
    fn create_file_symlink(target: &Path, link: &Path) -> std::io::Result<()> {
        std::os::unix::fs::symlink(target, link)
    }

    #[cfg(windows)]
    fn create_file_symlink(target: &Path, link: &Path) -> std::io::Result<()> {
        std::os::windows::fs::symlink_file(target, link)
    }
}
