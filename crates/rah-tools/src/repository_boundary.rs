//! Repository-specific nested Git boundary validation.
//!
//! `WorkspacePolicy` intentionally remains Git-agnostic.  This module is the
//! narrower repository-authority layer used by repository-bound capabilities.

use std::{
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
};

use crate::{
    ToolError,
    git_support::git_error,
    host_execute::{is_beneath, paths_equivalent},
};

const MAX_OBSERVED_DIRECTORIES: usize = 100_000;

/// Repository A's descendant-boundary policy.
///
/// The policy recognizes observable child `.git` markers only.  A bare child
/// repository without such a marker is intentionally not inferred.
#[derive(Clone, Debug)]
pub(crate) struct RepositoryNestedBoundaryPolicy {
    root: PathBuf,
}

impl RepositoryNestedBoundaryPolicy {
    pub(crate) fn new(root: impl AsRef<Path>) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
        }
    }

    /// Validates an existing target or existing creation parent.
    pub(crate) fn validate_existing(&self, path: &Path) -> Result<(), ToolError> {
        let mut current = match fs::symlink_metadata(path) {
            Ok(metadata) if metadata.is_dir() => path.to_path_buf(),
            Ok(_) => path
                .parent()
                .ok_or_else(|| boundary_error("repository target ancestry is unavailable"))?
                .to_path_buf(),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => path
                .parent()
                .ok_or_else(|| boundary_error("repository target ancestry is unavailable"))?
                .to_path_buf(),
            Err(error) => return Err(boundary_observation_error(error)),
        };
        loop {
            if paths_equivalent(&current, &self.root) {
                return Ok(());
            }
            if !is_beneath(&current, &self.root) {
                return Err(boundary_error(
                    "repository target ancestry is outside the repository",
                ));
            }
            reject_ambiguous_component(&current)?;
            reject_nested_marker(&current)?;
            current = current
                .parent()
                .ok_or_else(|| boundary_error("repository target ancestry is unavailable"))?
                .to_path_buf();
        }
    }

    /// Validates the observable directory tree before a whole-repository Git
    /// observation.  A boundary or an ambiguous traversable entry rejects the
    /// complete observation before Git can read it.
    pub(crate) fn validate_observation(&self) -> Result<(), ToolError> {
        let mut pending = vec![self.root.clone()];
        let mut directories = 0usize;
        while let Some(directory) = pending.pop() {
            directories = directories.saturating_add(1);
            if directories > MAX_OBSERVED_DIRECTORIES {
                return Err(boundary_error(
                    "repository observation boundary is ambiguous",
                ));
            }
            if !paths_equivalent(&directory, &self.root) {
                reject_ambiguous_component(&directory)?;
                reject_nested_marker(&directory)?;
            }
            for entry in fs::read_dir(&directory).map_err(boundary_observation_error)? {
                let entry = entry.map_err(boundary_observation_error)?;
                let path = entry.path();
                let metadata = fs::symlink_metadata(&path).map_err(boundary_observation_error)?;
                if is_dot_git_name(entry.file_name().as_os_str()) {
                    if !paths_equivalent(&directory, &self.root) {
                        return Err(boundary_error(
                            "repository path crosses a nested repository boundary",
                        ));
                    }
                    continue;
                }
                if metadata.file_type().is_symlink() || is_reparse_point(&metadata) {
                    if metadata.is_dir() || !metadata.is_file() {
                        return Err(boundary_error(
                            "repository observation boundary is ambiguous",
                        ));
                    }
                    continue;
                }
                if metadata.is_dir() {
                    pending.push(path);
                }
            }
        }
        Ok(())
    }
}

fn reject_nested_marker(directory: &Path) -> Result<(), ToolError> {
    let marker = directory.join(".git");
    match fs::symlink_metadata(marker) {
        Ok(_) => Err(boundary_error(
            "repository path crosses a nested repository boundary",
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(boundary_observation_error(error)),
    }
}

fn reject_ambiguous_component(path: &Path) -> Result<(), ToolError> {
    let metadata = fs::symlink_metadata(path).map_err(boundary_observation_error)?;
    if metadata.file_type().is_symlink() || is_reparse_point(&metadata) {
        return Err(boundary_error("repository target ancestry is ambiguous"));
    }
    Ok(())
}

fn is_dot_git_name(name: &OsStr) -> bool {
    #[cfg(windows)]
    {
        name.to_string_lossy().eq_ignore_ascii_case(".git")
    }
    #[cfg(not(windows))]
    {
        name == OsStr::new(".git")
    }
}

#[cfg(windows)]
fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_attributes() & 0x400 != 0
}

#[cfg(not(windows))]
fn is_reparse_point(_: &fs::Metadata) -> bool {
    false
}

fn boundary_error(message: &'static str) -> ToolError {
    git_error(message)
}

fn boundary_observation_error(_: std::io::Error) -> ToolError {
    boundary_error("repository boundary observation failed")
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    use super::RepositoryNestedBoundaryPolicy;

    static NEXT_ID: AtomicU64 = AtomicU64::new(0);

    fn root(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "rah-repository-boundary-{name}-{}-{}",
            std::process::id(),
            NEXT_ID.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn root_metadata_is_allowed_but_descendant_marker_is_rejected() {
        let root = root("marker");
        fs::create_dir(root.join(".git")).unwrap();
        fs::create_dir_all(root.join("nested")).unwrap();
        fs::create_dir(root.join("nested/.git")).unwrap();
        fs::write(root.join("nested/secret.txt"), b"secret").unwrap();
        let policy = RepositoryNestedBoundaryPolicy::new(&root);
        assert!(
            policy
                .validate_existing(&root.join("nested/secret.txt"))
                .is_err()
        );
        assert!(policy.validate_existing(&root).is_ok());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn ordinary_nested_directory_remains_allowed() {
        let root = root("ordinary");
        fs::create_dir(root.join(".git")).unwrap();
        fs::create_dir_all(root.join("nested/deeper")).unwrap();
        let policy = RepositoryNestedBoundaryPolicy::new(&root);
        assert!(
            policy
                .validate_existing(&root.join("nested/deeper"))
                .is_ok()
        );
        assert!(policy.validate_observation().is_ok());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn file_form_marker_is_a_boundary_without_following_it() {
        let root = root("file-marker");
        fs::create_dir(root.join(".git")).unwrap();
        fs::create_dir(root.join("nested")).unwrap();
        fs::write(root.join("nested/.git"), b"gitdir: unsupported").unwrap();
        fs::write(root.join("nested/secret.txt"), b"secret").unwrap();
        let policy = RepositoryNestedBoundaryPolicy::new(&root);
        assert!(
            policy
                .validate_existing(&root.join("nested/secret.txt"))
                .is_err()
        );
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn symbolic_link_marker_is_an_ambiguous_boundary() {
        let root = root("symlink-marker");
        fs::create_dir(root.join(".git")).unwrap();
        fs::create_dir(root.join("nested")).unwrap();
        std::os::unix::fs::symlink("missing-target", root.join("nested/.git")).unwrap();
        fs::write(root.join("nested/secret.txt"), b"secret").unwrap();
        let policy = RepositoryNestedBoundaryPolicy::new(&root);
        assert!(
            policy
                .validate_existing(&root.join("nested/secret.txt"))
                .is_err()
        );
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(windows)]
    #[test]
    fn windows_case_equivalent_marker_is_a_boundary() {
        let root = root("case-marker");
        fs::create_dir(root.join(".git")).unwrap();
        fs::create_dir(root.join("nested")).unwrap();
        fs::write(root.join("nested/.GIT"), b"marker").unwrap();
        fs::write(root.join("nested/secret.txt"), b"secret").unwrap();
        let policy = RepositoryNestedBoundaryPolicy::new(&root);
        assert!(
            policy
                .validate_existing(&root.join("nested/secret.txt"))
                .is_err()
        );
        let _ = fs::remove_dir_all(root);
    }
}
