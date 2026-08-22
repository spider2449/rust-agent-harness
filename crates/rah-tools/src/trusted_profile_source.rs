//! Private source validation for host-selected static capability profiles.
//!
//! This is intentionally narrower than a generic trusted-file abstraction.
//! It validates only the profile source before the profile parser can create
//! any host authority.

use std::{
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
};

#[cfg(unix)]
use std::fs::OpenOptions;

use crate::ProfileError;

/// The profile is host configuration, so keep its input bound independent of
/// profile-controlled capability limits.
pub(super) const MAX_PROFILE_BYTES: u64 = 1024 * 1024;

pub(super) fn read(path: &Path) -> Result<String, ProfileError> {
    if !path.is_absolute() {
        return Err(ProfileError::InvalidSource);
    }
    validate_path_syntax(path)?;
    validate_path_topology(path)?;

    let mut file = open_no_follow(path)?;
    let opened = file
        .metadata()
        .map_err(|_| ProfileError::UnreadableSource)?;
    if !opened.is_file() {
        return Err(ProfileError::NonRegularSource);
    }
    if opened.len() > MAX_PROFILE_BYTES {
        return Err(ProfileError::SourceTooLarge);
    }

    // Validate the pathname again after opening. On Unix, the object identity
    // comparison below additionally detects a final-component replacement.
    validate_path_topology(path)?;
    verify_opened_identity(path, &opened)?;

    let mut raw = Vec::new();
    file.by_ref()
        .take(MAX_PROFILE_BYTES + 1)
        .read_to_end(&mut raw)
        .map_err(|_| ProfileError::UnreadableSource)?;
    if raw.len() as u64 > MAX_PROFILE_BYTES {
        return Err(ProfileError::SourceTooLarge);
    }
    String::from_utf8(raw).map_err(|_| ProfileError::InvalidEncoding)
}

fn validate_path_topology(path: &Path) -> Result<(), ProfileError> {
    let mut components: Vec<PathBuf> = path.ancestors().map(Path::to_path_buf).collect();
    components.reverse();
    let last = components.len().saturating_sub(1);

    for (index, component) in components.iter().enumerate() {
        let metadata =
            fs::symlink_metadata(component).map_err(|_| ProfileError::UnreadableSource)?;
        if is_link_or_reparse(&metadata) {
            return Err(ProfileError::LinkedSource);
        }
        if index == last {
            if !metadata.is_file() {
                return Err(ProfileError::NonRegularSource);
            }
        } else if !metadata.is_dir() {
            return Err(ProfileError::NonRegularSource);
        }
    }
    Ok(())
}

#[cfg(unix)]
fn open_no_follow(path: &Path) -> Result<File, ProfileError> {
    use std::os::unix::fs::OpenOptionsExt;

    OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .map_err(|_| ProfileError::UnreadableSource)
}

#[cfg(not(unix))]
fn open_no_follow(path: &Path) -> Result<File, ProfileError> {
    File::open(path).map_err(|_| ProfileError::UnreadableSource)
}

#[cfg(unix)]
fn verify_opened_identity(path: &Path, opened: &fs::Metadata) -> Result<(), ProfileError> {
    use std::os::unix::fs::MetadataExt;

    let current = fs::symlink_metadata(path).map_err(|_| ProfileError::UnreadableSource)?;
    if current.dev() != opened.dev() || current.ino() != opened.ino() {
        return Err(ProfileError::SourceChanged);
    }
    Ok(())
}

#[cfg(not(unix))]
fn verify_opened_identity(_path: &Path, _opened: &fs::Metadata) -> Result<(), ProfileError> {
    Ok(())
}

#[cfg(unix)]
fn is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

#[cfg(windows)]
fn is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::{FileTypeExt, MetadataExt};

    let file_type = metadata.file_type();
    file_type.is_symlink_file()
        || file_type.is_symlink_dir()
        || metadata.file_attributes() & 0x0400 != 0
}

#[cfg(not(any(unix, windows)))]
fn is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

fn validate_path_syntax(path: &Path) -> Result<(), ProfileError> {
    if path.components().any(|component| {
        matches!(
            component,
            std::path::Component::CurDir | std::path::Component::ParentDir
        )
    }) {
        return Err(ProfileError::UnsupportedSource);
    }
    validate_windows_syntax(path)
}

#[cfg(windows)]
fn validate_windows_syntax(path: &Path) -> Result<(), ProfileError> {
    use std::path::{Component, Prefix};

    let mut components = path.components();
    match components.next() {
        Some(Component::Prefix(prefix)) if matches!(prefix.kind(), Prefix::Disk(_)) => {}
        _ => return Err(ProfileError::UnsupportedSource),
    }
    if path
        .components()
        .filter_map(|component| match component {
            Component::Normal(part) => Some(part),
            _ => None,
        })
        .any(|part| part.to_string_lossy().contains(':'))
    {
        return Err(ProfileError::UnsupportedSource);
    }
    Ok(())
}

#[cfg(not(windows))]
fn validate_windows_syntax(_path: &Path) -> Result<(), ProfileError> {
    Ok(())
}
