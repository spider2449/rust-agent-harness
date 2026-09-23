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
        if is_active_git_metadata(&self.root, path) {
            return Err(boundary_error(
                "repository path targets active Git metadata",
            ));
        }
        let mut current = match fs::symlink_metadata(path) {
            Ok(metadata) if metadata.is_dir() => path.to_path_buf(),
            Ok(_) => path
                .parent()
                .ok_or_else(|| boundary_error("repository target ancestry is unavailable"))?
                .to_path_buf(),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                nearest_existing_ancestor(path)?
            }
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
                if metadata.file_type().is_symlink() {
                    // Git observes a symlink as a directory entry and does
                    // not dereference it during the repository-wide scan.
                    // The `.git` name was handled above, so an ordinary
                    // symlink file does not make the observation ambiguous.
                    continue;
                }
                if is_reparse_point(&metadata) {
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

fn nearest_existing_ancestor(path: &Path) -> Result<PathBuf, ToolError> {
    let mut current = path.to_path_buf();
    loop {
        match fs::symlink_metadata(&current) {
            Ok(metadata) => {
                reject_ambiguous_component(&current)?;
                if metadata.is_dir() {
                    return Ok(current);
                }
                return Err(boundary_error(
                    "repository target ancestry is not a directory",
                ));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                current = current
                    .parent()
                    .ok_or_else(|| boundary_error("repository target ancestry is unavailable"))?
                    .to_path_buf();
            }
            Err(error) => return Err(boundary_observation_error(error)),
        }
    }
}

fn is_active_git_metadata(root: &Path, path: &Path) -> bool {
    let Ok(relative) = path.strip_prefix(root) else {
        return false;
    };
    relative
        .components()
        .next()
        .is_some_and(|component| is_dot_git_name(component.as_os_str()))
}

fn reject_nested_marker(directory: &Path) -> Result<(), ToolError> {
    let names = fs::read_dir(directory)
        .map_err(boundary_observation_error)?
        .map(|entry| {
            entry
                .map(|entry| entry.file_name())
                .map_err(boundary_observation_error)
        })
        .collect::<Result<Vec<_>, _>>()?;
    reject_marker_entries(names)
}

fn reject_marker_entries<I>(names: I) -> Result<(), ToolError>
where
    I: IntoIterator<Item = std::ffi::OsString>,
{
    if names.into_iter().any(|name| is_dot_git_name(&name)) {
        return Err(boundary_error(
            "repository path crosses a nested repository boundary",
        ));
    }
    Ok(())
}

fn reject_ambiguous_component(path: &Path) -> Result<(), ToolError> {
    let metadata = fs::symlink_metadata(path).map_err(boundary_observation_error)?;
    if metadata.file_type().is_symlink() || is_reparse_point(&metadata) {
        return Err(boundary_error("repository target ancestry is ambiguous"));
    }
    Ok(())
}

/// Matches the repository metadata marker using RAH's platform contract.
///
/// Windows uses ASCII-insensitive matching so this remains correct for
/// case-sensitive directories. Unix-like systems retain exact-case `.git`
/// semantics.
pub(crate) fn is_dot_git_name(name: &OsStr) -> bool {
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
pub(crate) fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_attributes() & 0x400 != 0
}

#[cfg(not(windows))]
pub(crate) fn is_reparse_point(_: &fs::Metadata) -> bool {
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
        ffi::OsStr,
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    use super::{RepositoryNestedBoundaryPolicy, is_dot_git_name, nearest_existing_ancestor};

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
    fn missing_descendant_uses_existing_ancestor_without_bypassing_boundaries() {
        let root = root("missing-ancestor");
        fs::create_dir(root.join(".git")).unwrap();
        let policy = RepositoryNestedBoundaryPolicy::new(&root);
        fs::create_dir(root.join("safe")).unwrap();
        assert_eq!(
            nearest_existing_ancestor(&root.join("safe/omitted/file.txt")).unwrap(),
            root.join("safe")
        );
        assert!(
            policy
                .validate_existing(&root.join("safe/omitted/file.txt"))
                .is_ok()
        );
        assert!(
            policy
                .validate_existing(&root.join("sparse/omitted/file.txt"))
                .is_ok()
        );

        fs::create_dir_all(root.join("nested/.git")).unwrap();
        assert!(
            policy
                .validate_existing(&root.join("nested/omitted/file.txt"))
                .is_err()
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn existing_regular_file_ancestor_fails_closed_without_climbing_past_it() {
        let root = root("file-ancestor");
        fs::create_dir(root.join(".git")).unwrap();
        fs::write(root.join("a"), b"not a directory").unwrap();
        let policy = RepositoryNestedBoundaryPolicy::new(&root);

        assert!(
            policy
                .validate_existing(&root.join("a/b/c/file.txt"))
                .is_err()
        );
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn existing_symlink_ancestor_rejects_all_missing_descendant_depths() {
        let root = root("symlink-ancestor");
        fs::create_dir(root.join(".git")).unwrap();
        let external = root("symlink-ancestor-target");
        fs::write(external.join("sentinel.txt"), b"outside").unwrap();
        std::os::unix::fs::symlink(&external, root.join("a")).unwrap();
        let policy = RepositoryNestedBoundaryPolicy::new(&root);

        for descendant in [
            "a/b/c/file.txt",
            "a/x/missing.txt",
            "a/x/y/missing.txt",
            "a/x/y/z/missing.txt",
        ] {
            assert!(policy.validate_existing(&root.join(descendant)).is_err());
        }

        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(external);
    }

    #[cfg(windows)]
    #[test]
    fn existing_junction_ancestor_rejects_missing_descendant() {
        let root_path = root("junction-ancestor");
        fs::create_dir(root_path.join(".git")).unwrap();
        let external = root("junction-ancestor-target");
        fs::write(external.join("sentinel.txt"), b"outside").unwrap();
        let status = std::process::Command::new("cmd.exe")
            .args(["/d", "/c", "mklink", "/J"])
            .arg(root_path.join("a"))
            .arg(&external)
            .status()
            .unwrap();
        assert!(status.success());
        let policy = RepositoryNestedBoundaryPolicy::new(&root_path);

        assert!(
            policy
                .validate_existing(&root_path.join("a/b/c/file.txt"))
                .is_err()
        );

        let _ = fs::remove_dir(root_path.join("a"));
        let _ = fs::remove_dir_all(root_path);
        let _ = fs::remove_dir_all(external);
    }

    #[test]
    fn root_is_the_last_existing_ancestor_and_is_not_bypassed() {
        let root = root("root-termination");
        fs::create_dir(root.join(".git")).unwrap();

        assert_eq!(
            nearest_existing_ancestor(&root.join("a/b/c/file.txt")).unwrap(),
            root
        );
        assert!(
            RepositoryNestedBoundaryPolicy::new(&root)
                .validate_existing(&root.join("a/b/c/file.txt"))
                .is_ok()
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn non_not_found_metadata_error_fails_closed() {
        let invalid = std::path::Path::new("invalid\0path");
        assert!(nearest_existing_ancestor(invalid).is_err());
    }

    #[test]
    fn marker_name_matching_is_platform_explicit() {
        assert!(is_dot_git_name(OsStr::new(".git")));
        #[cfg(windows)]
        {
            assert!(is_dot_git_name(OsStr::new(".GIT")));
            assert!(is_dot_git_name(OsStr::new(".Git")));
            assert!(is_dot_git_name(OsStr::new(".gIt")));
        }
        #[cfg(not(windows))]
        {
            assert!(!is_dot_git_name(OsStr::new(".GIT")));
            assert!(!is_dot_git_name(OsStr::new(".Git")));
            assert!(!is_dot_git_name(OsStr::new(".gIt")));
        }
        assert!(!is_dot_git_name(OsStr::new(".git ")));
        assert!(!is_dot_git_name(OsStr::new("git")));
    }

    #[test]
    fn enumerated_marker_names_are_the_authoritative_boundary_input() {
        let names = if cfg!(windows) {
            vec![std::ffi::OsString::from(".GIT")]
        } else {
            vec![std::ffi::OsString::from(".git")]
        };
        assert!(super::reject_marker_entries(names).is_err());
        assert!(
            super::reject_marker_entries([
                std::ffi::OsString::from("ordinary"),
                std::ffi::OsString::from("another"),
            ])
            .is_ok()
        );
    }

    #[test]
    fn existing_validation_uses_enumerated_marker_names() {
        let root = root("enumerated-marker");
        fs::create_dir(root.join(".git")).unwrap();
        fs::create_dir(root.join("nested")).unwrap();
        let marker_name = if cfg!(windows) { ".GIT" } else { ".git" };
        fs::write(root.join("nested").join(marker_name), b"marker").unwrap();
        let policy = RepositoryNestedBoundaryPolicy::new(&root);
        assert!(policy.validate_existing(&root.join("nested")).is_err());
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

    #[test]
    fn active_root_git_directory_or_gitfile_cannot_be_a_repository_path_target() {
        let first_root = root("active-git-dir");
        fs::create_dir(first_root.join(".git")).unwrap();
        fs::write(first_root.join(".git/config"), b"private configuration").unwrap();
        let policy = RepositoryNestedBoundaryPolicy::new(&first_root);
        assert!(policy.validate_existing(&first_root.join(".git")).is_err());
        assert!(
            policy
                .validate_existing(&first_root.join(".git/config"))
                .is_err()
        );
        let _ = fs::remove_dir_all(&first_root);

        let second_root = root("active-git-file");
        fs::write(second_root.join(".git"), b"gitdir: private\n").unwrap();
        let policy = RepositoryNestedBoundaryPolicy::new(&second_root);
        assert!(policy.validate_existing(&second_root.join(".git")).is_err());
        let _ = fs::remove_dir_all(second_root);
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
