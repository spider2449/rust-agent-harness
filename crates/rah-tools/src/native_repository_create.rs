//! Native, descriptor-relative create-new support for repository tools.
//!
//! This module is deliberately crate-private.  Higher-level policy decides
//! whether a repository path is authorized; this code only preserves that
//! authorization while acquiring one new directory entry.

use std::{
    fs, io,
    path::{Component, Path},
};

/// Stable identity captured from the native object used by a relative
/// operation. This representation never crosses the crate boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct NativeObjectIdentity {
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
    #[cfg(windows)]
    volume_serial: u32,
    #[cfg(windows)]
    file_index: u64,
}

impl NativeObjectIdentity {
    pub(crate) fn same_object(&self, other: &Self) -> bool {
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

#[derive(Debug)]
pub(crate) struct NativeCreatedObject {
    identity: NativeObjectIdentity,
}

impl NativeCreatedObject {
    pub(crate) fn same_identity(&self, identity: &NativeObjectIdentity) -> bool {
        self.identity.same_object(identity)
    }
}

#[allow(dead_code)]
#[derive(Debug)]
pub(crate) enum NativeCreateError {
    InvalidParent,
    InvalidName,
    AlreadyExists,
    WriteFailed {
        error: io::Error,
        created: NativeCreatedObject,
    },
    Io(io::Error),
}

#[allow(dead_code)]
pub(crate) struct NativeParent {
    #[cfg(unix)]
    fd: std::os::fd::OwnedFd,
    #[cfg(windows)]
    handle: std::fs::File,
    identities: Vec<NativeObjectIdentity>,
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

    pub(crate) fn identities(&self) -> &[NativeObjectIdentity] {
        &self.identities
    }

    pub(crate) fn same_identity(&self, identity: &NativeObjectIdentity) -> bool {
        self.identities
            .last()
            .is_some_and(|current| current.same_object(identity))
    }
}

#[allow(dead_code)]
pub(crate) fn create_new(
    parent: &NativeParent,
    name: &str,
    content: &[u8],
    fail_after: Option<usize>,
) -> Result<NativeCreatedObject, NativeCreateError> {
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

/// Captures the identity of an existing target without following a link.
pub(crate) fn capture_existing(path: &Path) -> Result<NativeObjectIdentity, NativeCreateError> {
    #[cfg(unix)]
    {
        unix::capture_existing(path)
    }
    #[cfg(windows)]
    {
        windows::capture_existing(path)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = path;
        Err(NativeCreateError::InvalidParent)
    }
}

/// Creates exactly one ordinary directory entry relative to an authorized parent.
pub(crate) fn create_directory(parent: &NativeParent, name: &str) -> Result<(), NativeCreateError> {
    if !valid_name(name) {
        return Err(NativeCreateError::InvalidName);
    }
    #[cfg(unix)]
    {
        unix::create_directory(parent, name)
    }
    #[cfg(windows)]
    {
        windows::create_directory(parent, name)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (parent, name);
        Err(NativeCreateError::InvalidParent)
    }
}

#[allow(dead_code)]
fn validate_relative(path: &Path) -> Result<(), NativeCreateError> {
    if path.is_absolute() {
        return Err(NativeCreateError::InvalidParent);
    }
    if path.as_os_str().is_empty() {
        return Ok(());
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

    fn identity(fd: std::os::fd::RawFd) -> Result<NativeObjectIdentity, NativeCreateError> {
        let mut stat = unsafe { std::mem::zeroed::<libc::stat>() };
        if unsafe { libc::fstat(fd, &mut stat) } != 0 {
            return Err(NativeCreateError::Io(io::Error::last_os_error()));
        }
        Ok(NativeObjectIdentity {
            device: stat.st_dev as u64,
            inode: stat.st_ino as u64,
        })
    }

    pub(super) fn capture_existing(path: &Path) -> Result<NativeObjectIdentity, NativeCreateError> {
        let path = CString::new(path.as_os_str().as_encoded_bytes())
            .map_err(|_| NativeCreateError::InvalidParent)?;
        let fd = unsafe {
            libc::open(
                path.as_ptr(),
                libc::O_RDONLY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
            )
        };
        if fd < 0 {
            return Err(NativeCreateError::Io(io::Error::last_os_error()));
        }
        let fd = unsafe { OwnedFd::from_raw_fd(fd) };
        identity(fd.as_raw_fd())
    }

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
        let mut identities = vec![identity(current.as_raw_fd())?];
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
            identities.push(identity(current.as_raw_fd())?);
        }
        Ok(NativeParent {
            fd: current,
            identities,
        })
    }
    pub(super) fn create(
        parent: &NativeParent,
        name: &str,
        content: &[u8],
        fail_after: Option<usize>,
    ) -> Result<NativeCreatedObject, NativeCreateError> {
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
        let created = NativeCreatedObject {
            identity: identity(file.as_raw_fd())?,
        };
        use io::Write as _;
        let count = fail_after.unwrap_or(content.len()).min(content.len());
        if let Err(error) = file.write_all(&content[..count]) {
            return Err(NativeCreateError::WriteFailed { error, created });
        }
        if fail_after.is_some() {
            return Err(NativeCreateError::WriteFailed {
                error: io::Error::other("injected write failure"),
                created,
            });
        }
        match file.sync_all() {
            Ok(()) => Ok(created),
            Err(error) => Err(NativeCreateError::WriteFailed { error, created }),
        }
    }

    pub(super) fn create_directory(
        parent: &NativeParent,
        name: &str,
    ) -> Result<(), NativeCreateError> {
        let name = CString::new(name).map_err(|_| NativeCreateError::InvalidName)?;
        let result = unsafe { libc::mkdirat(parent.fd.as_raw_fd(), name.as_ptr(), 0o777) };
        if result == 0 {
            Ok(())
        } else {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::AlreadyExists {
                Err(NativeCreateError::AlreadyExists)
            } else {
                Err(NativeCreateError::Io(error))
            }
        }
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
    const STATUS_OBJECT_NAME_COLLISION: i32 = 0xc000_0035u32 as i32;

    fn identity(file: &fs::File) -> Result<NativeObjectIdentity, NativeCreateError> {
        let mut info = unsafe { zeroed::<BY_HANDLE_FILE_INFORMATION>() };
        if unsafe { GetFileInformationByHandle(file.as_raw_handle() as HANDLE, &mut info) } == 0 {
            return Err(NativeCreateError::Io(io::Error::last_os_error()));
        }
        Ok(NativeObjectIdentity {
            volume_serial: info.dwVolumeSerialNumber,
            file_index: (u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow),
        })
    }

    fn is_name_collision(status: i32) -> bool {
        status == STATUS_OBJECT_NAME_COLLISION
    }

    pub(super) fn capture_existing(path: &Path) -> Result<NativeObjectIdentity, NativeCreateError> {
        let path = wide(path);
        let handle = unsafe {
            CreateFileW(
                path.as_ptr(),
                FILE_READ_ATTRIBUTES,
                FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                std::ptr::null(),
                OPEN_EXISTING,
                FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS,
                std::ptr::null_mut(),
            )
        };
        if handle == INVALID_HANDLE_VALUE {
            return Err(NativeCreateError::Io(io::Error::last_os_error()));
        }
        let file = unsafe { fs::File::from_raw_handle(handle) };
        identity(&file)
    }

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
        let mut identities = vec![identity(&current)?];
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
            identities.push(identity(&current)?);
        }
        Ok(NativeParent {
            handle: current,
            identities,
        })
    }
    pub(super) fn create(
        parent: &NativeParent,
        name: &str,
        content: &[u8],
        fail_after: Option<usize>,
    ) -> Result<NativeCreatedObject, NativeCreateError> {
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
                FILE_READ_ATTRIBUTES | 0x0010_0002,
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
            return if is_name_collision(status) {
                Err(NativeCreateError::AlreadyExists)
            } else {
                Err(NativeCreateError::Io(io::Error::last_os_error()))
            };
        };
        let mut f = unsafe { fs::File::from_raw_handle(out) };
        let created = NativeCreatedObject {
            identity: identity(&f)?,
        };
        use io::Write as _;
        let n = fail_after.unwrap_or(content.len()).min(content.len());
        if let Err(error) = f.write_all(&content[..n]) {
            return Err(NativeCreateError::WriteFailed { error, created });
        }
        if fail_after.is_some() {
            return Err(NativeCreateError::WriteFailed {
                error: io::Error::other("injected write failure"),
                created,
            });
        };
        match f.sync_all() {
            Ok(()) => Ok(created),
            Err(error) => Err(NativeCreateError::WriteFailed { error, created }),
        }
    }

    pub(super) fn create_directory(
        parent: &NativeParent,
        name: &str,
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
                FILE_READ_ATTRIBUTES | FILE_ADD_FILE | FILE_TRAVERSE | SYNCHRONIZE,
                &attrs,
                &mut ios,
                std::ptr::null(),
                0,
                FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                FILE_CREATE,
                FILE_DIRECTORY_FILE | FILE_OPEN_REPARSE_POINT | FILE_SYNCHRONOUS_IO_NONALERT,
                std::ptr::null(),
                0,
            )
        };
        if status < 0 {
            let error = io::Error::last_os_error();
            return if error.kind() == io::ErrorKind::AlreadyExists {
                Err(NativeCreateError::AlreadyExists)
            } else {
                Err(NativeCreateError::Io(error))
            };
        }
        if out != INVALID_HANDLE_VALUE {
            let _directory = unsafe { fs::File::from_raw_handle(out) };
        }
        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use super::{STATUS_OBJECT_NAME_COLLISION, is_name_collision};

        #[test]
        fn only_object_name_collision_is_a_collision() {
            assert!(is_name_collision(STATUS_OBJECT_NAME_COLLISION));
            assert!(!is_name_collision(0xc000_0022u32 as i32));
            assert!(!is_name_collision(0xc000_000fu32 as i32));
            assert!(!is_name_collision(0xc000_0008u32 as i32));
            assert!(!is_name_collision(-1));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{NativeCreateError, NativeParent, create_new};
    use std::{
        fs,
        path::{Path, PathBuf},
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
    fn empty_relative_parent_creates_only_in_the_authorized_root() {
        let root = root("root-parent");
        let parent = NativeParent::open(&root, Path::new("")).expect("root parent");
        create_new(&parent, "root.txt", b"root content", None).expect("root create");
        assert_eq!(
            fs::read(root.join("root.txt")).expect("root file"),
            b"root content"
        );
        assert!(matches!(
            create_new(&parent, "root.txt", b"replacement", None),
            Err(NativeCreateError::AlreadyExists)
        ));
        assert_eq!(
            fs::read(root.join("root.txt")).expect("unchanged root file"),
            b"root content"
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
            Err(NativeCreateError::WriteFailed { .. })
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
