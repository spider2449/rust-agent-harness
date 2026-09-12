//! Narrow host-owned identity evidence for an explicitly admitted repository.
//!
//! This is deliberately smaller than a filesystem authority. It captures and
//! revalidates only the repository root, supported top-level `.git` form, and
//! selected Git executable needed by Desktop membership currentness.

use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::ToolError;

#[derive(Clone, Debug, Eq, PartialEq)]
struct FileIdentity {
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
    #[cfg(windows)]
    volume_serial: u32,
    #[cfg(windows)]
    file_index: u64,
}

impl FileIdentity {
    fn capture(path: &Path) -> Result<Self, ToolError> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let metadata = fs::metadata(path).map_err(identity_error)?;
            return Ok(Self {
                device: metadata.dev(),
                inode: metadata.ino(),
            });
        }
        #[cfg(windows)]
        {
            use std::os::windows::{fs::OpenOptionsExt, io::AsRawHandle};
            use windows_sys::Win32::Storage::FileSystem::{
                BY_HANDLE_FILE_INFORMATION, FILE_FLAG_BACKUP_SEMANTICS, GetFileInformationByHandle,
            };
            let mut options = fs::OpenOptions::new();
            options.read(true).custom_flags(FILE_FLAG_BACKUP_SEMANTICS);
            let file = options.open(path).map_err(identity_error)?;
            let mut information = std::mem::MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::zeroed();
            let result = unsafe {
                GetFileInformationByHandle(file.as_raw_handle(), information.as_mut_ptr())
            };
            if result == 0 {
                return Err(identity_error(std::io::Error::last_os_error()));
            }
            let information = unsafe { information.assume_init() };
            Ok(Self {
                volume_serial: information.dwVolumeSerialNumber,
                file_index: (u64::from(information.nFileIndexHigh) << 32)
                    | u64::from(information.nFileIndexLow),
            })
        }
        #[cfg(not(any(unix, windows)))]
        {
            let _ = fs::metadata(path).map_err(identity_error)?;
            Ok(Self {})
        }
    }

    fn same_object(&self, other: &Self) -> bool {
        #[cfg(unix)]
        {
            self.device == other.device && self.inode == other.inode
        }
        #[cfg(windows)]
        {
            self.volume_serial == other.volume_serial && self.file_index == other.file_index
        }
        #[cfg(not(any(unix, windows)))]
        {
            self == other
        }
    }
}

/// The bounded relationship between two safely captured repository roots.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RepositoryAdmissionRelation {
    Same,
    Nested,
    Distinct,
}

/// Private repository identity evidence used by host-owned Desktop membership.
///
/// The type is intentionally not serializable and exposes no filesystem IDs,
/// raw Git identity, or generic authority operations.
#[derive(Clone)]
pub struct RepositoryAdmissionIdentity {
    canonical_root: PathBuf,
    root_identity: FileIdentity,
    dot_git_identity: FileIdentity,
    git_executable: PathBuf,
    git_identity: FileIdentity,
}

impl RepositoryAdmissionIdentity {
    /// Captures a supported ordinary non-bare repository binding.
    pub fn capture(
        git_executable: impl AsRef<Path>,
        repository_root: impl AsRef<Path>,
    ) -> Result<Self, ToolError> {
        let requested_root = repository_root.as_ref();
        reject_ambiguous_ancestry(requested_root, "repository root")?;
        let canonical_root = canonical_directory(requested_root, "repository root")?;
        reject_ambiguous_ancestry(&canonical_root, "repository root")?;

        let dot_git = canonical_root.join(".git");
        reject_link_or_reparse(&dot_git, "repository metadata")?;
        if !fs::metadata(&dot_git).map_err(identity_error)?.is_dir() {
            return Err(identity_error(
                "repository metadata must be a supported directory",
            ));
        }

        let requested_git = git_executable.as_ref();
        reject_ambiguous_ancestry(requested_git, "Git executable")?;
        let canonical_git = canonical_file(requested_git, "Git executable")?;

        Ok(Self {
            root_identity: FileIdentity::capture(&canonical_root)?,
            dot_git_identity: FileIdentity::capture(&dot_git)?,
            git_identity: FileIdentity::capture(&canonical_git)?,
            canonical_root,
            git_executable: canonical_git,
        })
    }

    /// Returns the host-canonical root for constructing a fresh active object.
    pub fn canonical_root(&self) -> &Path {
        &self.canonical_root
    }

    /// Re-captures all bound evidence and fails if any part became stale.
    pub fn revalidate(
        &self,
        git_executable: impl AsRef<Path>,
        repository_root: impl AsRef<Path>,
    ) -> Result<(), ToolError> {
        let current = Self::capture(git_executable, repository_root)?;
        if current.canonical_root != self.canonical_root
            || !current.root_identity.same_object(&self.root_identity)
            || !current.dot_git_identity.same_object(&self.dot_git_identity)
            || current.git_executable != self.git_executable
            || !current.git_identity.same_object(&self.git_identity)
        {
            return Err(identity_error("repository admission identity is stale"));
        }
        Ok(())
    }

    /// Compares two safely captured roots without exposing their raw evidence.
    pub fn relation(&self, other: &Self) -> RepositoryAdmissionRelation {
        if self.root_identity.same_object(&other.root_identity)
            || self.dot_git_identity.same_object(&other.dot_git_identity)
        {
            return RepositoryAdmissionRelation::Same;
        }
        if is_parent(&self.canonical_root, &other.canonical_root)
            || is_parent(&other.canonical_root, &self.canonical_root)
        {
            RepositoryAdmissionRelation::Nested
        } else {
            RepositoryAdmissionRelation::Distinct
        }
    }
}

fn canonical_directory(path: &Path, label: &str) -> Result<PathBuf, ToolError> {
    let canonical = fs::canonicalize(path).map_err(identity_error)?;
    if !canonical.is_dir() {
        return Err(identity_error(format!(
            "{label} must be an existing directory"
        )));
    }
    Ok(canonical)
}

fn canonical_file(path: &Path, label: &str) -> Result<PathBuf, ToolError> {
    let canonical = fs::canonicalize(path).map_err(identity_error)?;
    let metadata = fs::metadata(&canonical).map_err(identity_error)?;
    if !metadata.is_file() {
        return Err(identity_error(format!("{label} must be a regular file")));
    }
    reject_link_or_reparse(&canonical, label)?;
    Ok(canonical)
}

fn reject_link_or_reparse(path: &Path, label: &str) -> Result<(), ToolError> {
    let metadata = fs::symlink_metadata(path).map_err(identity_error)?;
    if metadata.file_type().is_symlink() {
        return Err(identity_error(format!(
            "{label} must not be a symbolic link"
        )));
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err(identity_error(format!(
                "{label} must not be a reparse point"
            )));
        }
    }
    Ok(())
}

fn reject_ambiguous_ancestry(path: &Path, label: &str) -> Result<(), ToolError> {
    let mut current = path.to_path_buf();
    loop {
        if current.exists() {
            reject_link_or_reparse(&current, label)?;
        }
        let Some(parent) = current.parent() else {
            break;
        };
        if parent == current {
            break;
        }
        current = parent.to_path_buf();
    }
    Ok(())
}

fn is_parent(parent: &Path, child: &Path) -> bool {
    #[cfg(windows)]
    {
        let parent = parent.to_string_lossy().to_ascii_lowercase();
        let child = child.to_string_lossy().to_ascii_lowercase();
        let parent = Path::new(&parent);
        let child = Path::new(&child);
        child.starts_with(parent) && child != parent
    }
    #[cfg(not(windows))]
    {
        child.starts_with(parent) && child != parent
    }
}

fn identity_error(_: impl std::fmt::Display) -> ToolError {
    ToolError::Execution {
        message: "repository admission identity validation failed".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::{RepositoryAdmissionIdentity, RepositoryAdmissionRelation};
    use std::fs;

    #[test]
    fn relation_distinguishes_same_nested_and_sibling_roots() {
        let base =
            std::env::temp_dir().join(format!("rah-admission-identity-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let a = base.join("a");
        let child = a.join("child");
        let sibling = base.join("sibling");
        for path in [&a, &child, &sibling] {
            fs::create_dir_all(path).unwrap();
            fs::create_dir(path.join(".git")).unwrap();
        }
        let git = std::env::current_exe().unwrap();
        let ia = RepositoryAdmissionIdentity::capture(&git, &a).unwrap();
        let ichild = RepositoryAdmissionIdentity::capture(&git, &child).unwrap();
        let isibling = RepositoryAdmissionIdentity::capture(&git, &sibling).unwrap();
        assert_eq!(ia.relation(&ia), RepositoryAdmissionRelation::Same);
        assert_eq!(ia.relation(&ichild), RepositoryAdmissionRelation::Nested);
        assert_eq!(
            ia.relation(&isibling),
            RepositoryAdmissionRelation::Distinct
        );
        let _ = fs::remove_dir_all(base);
    }
}
