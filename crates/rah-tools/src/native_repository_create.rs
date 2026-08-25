//! Native, descriptor-relative create-new support for repository tools.
//!
//! This module is deliberately crate-private.  Higher-level policy decides
//! whether a repository path is authorized; this code only preserves that
//! authorization while acquiring one new directory entry.

use std::{
    fs, io,
    path::{Component, Path},
};

#[allow(dead_code)]
#[derive(Debug)]
pub(crate) enum NativeCreateError {
    InvalidParent,
    InvalidName,
    AlreadyExists,
    WriteFailed(io::Error),
    Io(io::Error),
}

#[allow(dead_code)]
pub(crate) struct NativeParent {
    #[cfg(unix)]
    fd: std::os::fd::OwnedFd,
    #[cfg(windows)]
    handle: std::fs::File,
}

#[allow(dead_code)]
impl NativeParent {
    /// Opens `relative` by walking only existing directories from `root`.
    pub(crate) fn open(root: &Path, relative: &Path) -> Result<Self, NativeCreateError> {
        validate_relative(relative)?;
        #[cfg(unix)]
        {
            unix::open_parent(root, relative)
        }
        #[cfg(windows)]
        {
            windows::open_parent(root, relative)
        }
        #[cfg(not(any(unix, windows)))]
        {
            let _ = (root, relative);
            Err(NativeCreateError::InvalidParent)
        }
    }
}

#[allow(dead_code)]
pub(crate) fn create_new(
    parent: &NativeParent,
    name: &str,
    content: &[u8],
    fail_after: Option<usize>,
) -> Result<(), NativeCreateError> {
    if !valid_name(name) {
        return Err(NativeCreateError::InvalidName);
    }
    #[cfg(unix)]
    {
        unix::create(parent, name, content, fail_after)
    }
    #[cfg(windows)]
    {
        windows::create(parent, name, content, fail_after)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (parent, name, content, fail_after);
        Err(NativeCreateError::InvalidParent)
    }
}

#[allow(dead_code)]
fn validate_relative(path: &Path) -> Result<(), NativeCreateError> {
    if path.as_os_str().is_empty() || path.is_absolute() {
        return Err(NativeCreateError::InvalidParent);
    }
    for component in path.components() {
        if !matches!(component, Component::Normal(_)) {
            return Err(NativeCreateError::InvalidParent);
        }
    }
    Ok(())
}
#[allow(dead_code)]
fn valid_name(name: &str) -> bool {
    !name.is_empty() && !name.contains(['/', '\\', '\0']) && name != "." && name != ".."
}

#[cfg(unix)]
mod unix {
    use super::*;
    use std::{
        ffi::CString,
        os::fd::{AsRawFd, FromRawFd, OwnedFd},
    };

    pub(super) fn open_parent(
        root: &Path,
        relative: &Path,
    ) -> Result<NativeParent, NativeCreateError> {
        let root = CString::new(root.as_os_str().as_encoded_bytes())
            .map_err(|_| NativeCreateError::InvalidParent)?;
        let fd = unsafe {
            libc::open(
                root.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
            )
        };
        if fd < 0 {
            return Err(NativeCreateError::Io(io::Error::last_os_error()));
        }
        let mut current = unsafe { OwnedFd::from_raw_fd(fd) };
        for c in relative.components() {
            let Component::Normal(c) = c else {
                return Err(NativeCreateError::InvalidParent);
            };
            let c =
                CString::new(c.as_encoded_bytes()).map_err(|_| NativeCreateError::InvalidParent)?;
            let next = unsafe {
                libc::openat(
                    current.as_raw_fd(),
                    c.as_ptr(),
                    libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
                )
            };
            if next < 0 {
                return Err(NativeCreateError::Io(io::Error::last_os_error()));
            }
            current = unsafe { OwnedFd::from_raw_fd(next) };
        }
        Ok(NativeParent { fd: current })
    }
    pub(super) fn create(
        parent: &NativeParent,
        name: &str,
        content: &[u8],
        fail_after: Option<usize>,
    ) -> Result<(), NativeCreateError> {
        let name = CString::new(name).map_err(|_| NativeCreateError::InvalidName)?;
        let fd = unsafe {
            libc::openat(
                parent.fd.as_raw_fd(),
                name.as_ptr(),
                libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                0o600,
            )
        };
        if fd < 0 {
            let e = io::Error::last_os_error();
            return if e.kind() == io::ErrorKind::AlreadyExists {
                Err(NativeCreateError::AlreadyExists)
            } else {
                Err(NativeCreateError::Io(e))
            };
        }
        let mut file = unsafe { fs::File::from_raw_fd(fd) };
        use io::Write as _;
        let count = fail_after.unwrap_or(content.len()).min(content.len());
        file.write_all(&content[..count])
            .map_err(NativeCreateError::WriteFailed)?;
        if fail_after.is_some() {
            return Err(NativeCreateError::WriteFailed(io::Error::other(
                "injected write failure",
            )));
        }
        file.sync_all().map_err(NativeCreateError::WriteFailed)
    }
}

#[cfg(windows)]
mod windows {
    use super::*;
    use std::{
        mem::{size_of, zeroed},
        os::windows::{
            ffi::OsStrExt,
            io::{AsRawHandle, FromRawHandle},
        },
    };
    use windows_sys::{
        Wdk::{
            Foundation::OBJECT_ATTRIBUTES,
            Storage::FileSystem::{
                FILE_CREATE, FILE_DIRECTORY_FILE, FILE_NON_DIRECTORY_FILE, FILE_OPEN,
                FILE_OPEN_REPARSE_POINT, FILE_SYNCHRONOUS_IO_NONALERT, NtCreateFile,
            },
        },
        Win32::{
            Foundation::{HANDLE, INVALID_HANDLE_VALUE, UNICODE_STRING},
            Storage::FileSystem::{
                BY_HANDLE_FILE_INFORMATION, CreateFileW, FILE_ADD_FILE,
                FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_BACKUP_SEMANTICS,
                FILE_FLAG_OPEN_REPARSE_POINT, FILE_READ_ATTRIBUTES, FILE_SHARE_DELETE,
                FILE_SHARE_READ, FILE_SHARE_WRITE, FILE_TRAVERSE, GetFileInformationByHandle,
                OPEN_EXISTING, SYNCHRONIZE,
            },
            System::IO::{IO_STATUS_BLOCK, IO_STATUS_BLOCK_0},
        },
    };
    fn wide(path: &Path) -> Vec<u16> {
        path.as_os_str().encode_wide().chain(Some(0)).collect()
    }
    fn reject_reparse(file: &fs::File) -> Result<(), NativeCreateError> {
        let mut info = unsafe { zeroed::<BY_HANDLE_FILE_INFORMATION>() };
        if unsafe { GetFileInformationByHandle(file.as_raw_handle() as HANDLE, &mut info) } == 0 {
            return Err(NativeCreateError::Io(io::Error::last_os_error()));
        };
        if info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(NativeCreateError::InvalidParent);
        };
        Ok(())
    }
    pub(super) fn open_parent(
        root: &Path,
        relative: &Path,
    ) -> Result<NativeParent, NativeCreateError> {
        let w = wide(root);
        let h = unsafe {
            CreateFileW(
                w.as_ptr(),
                FILE_READ_ATTRIBUTES | FILE_ADD_FILE | FILE_TRAVERSE | SYNCHRONIZE,
                FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                std::ptr::null(),
                OPEN_EXISTING,
                FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
                std::ptr::null_mut(),
            )
        };
        if h == INVALID_HANDLE_VALUE {
            return Err(NativeCreateError::Io(io::Error::last_os_error()));
        };
        let mut current = unsafe { fs::File::from_raw_handle(h) };
        reject_reparse(&current)?;
        for c in relative.components() {
            let Component::Normal(c) = c else {
                return Err(NativeCreateError::InvalidParent);
            };
            let mut name = c.encode_wide().collect::<Vec<_>>();
            let mut us = UNICODE_STRING {
                Length: (name.len() * 2) as u16,
                MaximumLength: (name.len() * 2) as u16,
                Buffer: name.as_mut_ptr(),
            };
            let attrs = OBJECT_ATTRIBUTES {
                Length: size_of::<OBJECT_ATTRIBUTES>() as u32,
                RootDirectory: current.as_raw_handle() as HANDLE,
                ObjectName: &mut us,
                Attributes: 0,
                SecurityDescriptor: std::ptr::null(),
                SecurityQualityOfService: std::ptr::null(),
            };
            let mut out = INVALID_HANDLE_VALUE;
            let mut ios = IO_STATUS_BLOCK {
                Anonymous: IO_STATUS_BLOCK_0 { Status: 0 },
                Information: 0,
            };
            let status = unsafe {
                NtCreateFile(
                    &mut out,
                    FILE_READ_ATTRIBUTES | FILE_ADD_FILE | FILE_TRAVERSE | SYNCHRONIZE,
                    &attrs,
                    &mut ios,
                    std::ptr::null(),
                    0,
                    FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                    FILE_OPEN,
                    FILE_DIRECTORY_FILE | FILE_OPEN_REPARSE_POINT,
                    std::ptr::null(),
                    0,
                )
            };
            if status < 0 {
                return Err(NativeCreateError::InvalidParent);
            };
            current = unsafe { fs::File::from_raw_handle(out) };
            reject_reparse(&current)?;
        }
        Ok(NativeParent { handle: current })
    }
    pub(super) fn create(
        parent: &NativeParent,
        name: &str,
        content: &[u8],
        fail_after: Option<usize>,
    ) -> Result<(), NativeCreateError> {
        let mut n = name.encode_utf16().collect::<Vec<_>>();
        let mut us = UNICODE_STRING {
            Length: (n.len() * 2) as u16,
            MaximumLength: (n.len() * 2) as u16,
            Buffer: n.as_mut_ptr(),
        };
        let attrs = OBJECT_ATTRIBUTES {
            Length: size_of::<OBJECT_ATTRIBUTES>() as u32,
            RootDirectory: parent.handle.as_raw_handle() as HANDLE,
            ObjectName: &mut us,
            Attributes: 0,
            SecurityDescriptor: std::ptr::null(),
            SecurityQualityOfService: std::ptr::null(),
        };
        let mut out = INVALID_HANDLE_VALUE;
        let mut ios = IO_STATUS_BLOCK {
            Anonymous: IO_STATUS_BLOCK_0 { Status: 0 },
            Information: 0,
        };
        let status = unsafe {
            NtCreateFile(
                &mut out,
                0x0010_0002,
                &attrs,
                &mut ios,
                std::ptr::null(),
                0,
                FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                FILE_CREATE,
                FILE_NON_DIRECTORY_FILE | FILE_SYNCHRONOUS_IO_NONALERT,
                std::ptr::null(),
                0,
            )
        };
        if status < 0 {
            return Err(NativeCreateError::AlreadyExists);
        };
        let mut f = unsafe { fs::File::from_raw_handle(out) };
        use io::Write as _;
        let n = fail_after.unwrap_or(content.len()).min(content.len());
        f.write_all(&content[..n])
            .map_err(NativeCreateError::WriteFailed)?;
        if fail_after.is_some() {
            return Err(NativeCreateError::WriteFailed(io::Error::other(
                "injected write failure",
            )));
        };
        f.sync_all().map_err(NativeCreateError::WriteFailed)
    }
}

#[cfg(test)]
mod tests {
    use super::{NativeCreateError, NativeParent, create_new};
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn root(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "rah-native-{name}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        fs::create_dir_all(root.join("parent/nested")).expect("fixture");
        root
    }
    #[test]
    fn exclusive_create_preserves_racing_target_and_exact_content() {
        let root = root("exclusive");
        let parent = NativeParent::open(&root, PathBuf::from("parent").as_path()).expect("parent");
        create_new(&parent, "new.txt", b"exact\\n", None).expect("create");
        assert_eq!(
            fs::read(root.join("parent/new.txt")).expect("read"),
            b"exact\\n"
        );
        assert!(matches!(
            create_new(&parent, "new.txt", b"replace", None),
            Err(NativeCreateError::AlreadyExists)
        ));
        assert_eq!(
            fs::read(root.join("parent/new.txt")).expect("read"),
            b"exact\\n"
        );
        fs::remove_dir_all(root).expect("cleanup");
    }
    #[test]
    fn target_race_and_partial_write_never_cleanup_or_retry() {
        let root = root("race");
        let parent = NativeParent::open(&root, PathBuf::from("parent").as_path()).expect("parent");
        fs::write(root.join("parent/race.txt"), b"external").expect("race");
        assert!(matches!(
            create_new(&parent, "race.txt", b"rah", None),
            Err(NativeCreateError::AlreadyExists)
        ));
        assert_eq!(
            fs::read(root.join("parent/race.txt")).expect("read"),
            b"external"
        );
        assert!(matches!(
            create_new(&parent, "partial.txt", b"abcdef", Some(3)),
            Err(NativeCreateError::WriteFailed(_))
        ));
        assert_eq!(
            fs::read(root.join("parent/partial.txt")).expect("partial"),
            b"abc"
        );
        assert!(matches!(
            create_new(&parent, "partial.txt", b"retry", None),
            Err(NativeCreateError::AlreadyExists)
        ));
        fs::remove_dir_all(root).expect("cleanup");
    }
    #[test]
    fn parent_replacement_after_handle_open_cannot_redirect_creation() {
        let root = root("parent-race");
        let parent = NativeParent::open(&root, PathBuf::from("parent").as_path()).expect("parent");
        let moved = root.join("moved");
        fs::rename(root.join("parent"), &moved).expect("move validated parent");
        fs::create_dir(root.join("parent")).expect("replacement parent");
        create_new(&parent, "bound.txt", b"bound", None).expect("bound create");
        assert_eq!(
            fs::read(moved.join("bound.txt")).expect("original identity"),
            b"bound"
        );
        assert!(!root.join("parent/bound.txt").exists());
        fs::remove_dir_all(root).expect("cleanup");
    }
    #[cfg(unix)]
    #[test]
    fn unix_dirfd_rejects_link_parent_and_creates_non_executable_file() {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let root = root("unix");
        symlink(root.join("parent"), root.join("link")).expect("link");
        assert!(NativeParent::open(&root, PathBuf::from("link").as_path()).is_err());
        let parent =
            NativeParent::open(&root, PathBuf::from("parent/nested").as_path()).expect("parent");
        create_new(&parent, "mode.txt", b"x", None).expect("create");
        assert_eq!(
            fs::metadata(root.join("parent/nested/mode.txt"))
                .expect("metadata")
                .permissions()
                .mode()
                & 0o111,
            0
        );
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[cfg(windows)]
    #[test]
    fn windows_handle_walk_rejects_junction_parent() {
        use std::process::Command;

        let root = root("windows-junction");
        let target = root.join("parent");
        let junction = root.join("junction");
        let status = Command::new("cmd.exe")
            .args(["/c", "mklink", "/J"])
            .arg(&junction)
            .arg(&target)
            .status()
            .expect("mklink command should start");
        assert!(status.success(), "junction fixture should be created");
        assert!(NativeParent::open(&root, PathBuf::from("junction").as_path()).is_err());
        fs::remove_dir_all(root).expect("cleanup");
    }
}
