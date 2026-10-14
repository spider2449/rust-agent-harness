//! Linux confinement: a landlock ruleset for files and signals, and a
//! seccomp filter for sockets. Both are prepared in the RAH process and
//! enforced in the child between `fork` and `exec`, where only
//! async-signal-safe system calls run.

use std::{
    ffi::CString,
    io,
    os::{
        fd::{AsRawFd, FromRawFd, OwnedFd},
        unix::{ffi::OsStrExt, fs::DirBuilderExt},
    },
    path::{Path, PathBuf},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use rah_api::{PluginError, Scope, sandbox::SANDBOX};
use serde::Deserialize;
use tokio::process::Command;

use super::resolve;
use crate::{ProcessEnv, canonical_root, inherited_env, max_timeout};

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    root: Option<PathBuf>,
    inherit_env: Option<Vec<String>>,
    max_timeout_ms: Option<u64>,
    readable: Option<Vec<PathBuf>>,
    writable: Option<Vec<PathBuf>>,
    #[serde(default)]
    network: bool,
}

pub(super) async fn apply(scope: &mut Scope) -> Result<(), PluginError> {
    let config: Config = if scope.config().is_null() {
        Config::default()
    } else {
        scope.config_as()?
    };
    let root = canonical_root(config.root).await?;
    let mut base_env = inherited_env(config.inherit_env)?;
    let max_timeout = max_timeout(config.max_timeout_ms)?;
    let readable = config.readable.unwrap_or_else(|| vec![PathBuf::from("/")]);
    let readable = resolve(&root, readable, "readable").await?;
    let mut writable = resolve(&root, config.writable.unwrap_or_default(), "writable").await?;

    let temp = private_temp_dir()?;
    let removed = temp.clone();
    scope.on_dispose(move || async move {
        let _ = tokio::fs::remove_dir_all(removed).await;
    });
    writable.push(root.clone());
    writable.push(temp.clone());
    let confinement = Confinement::new(&readable, &writable, config.network)?;
    base_env.insert("TMPDIR".into(), temp.into_os_string());

    let env = ProcessEnv {
        root,
        base_env,
        max_timeout,
        stop: scope.spawner().cancellation(),
        spawner: Some(scope.spawner()),
        confinement: Some(Arc::new(confinement)),
    };
    scope.provide(SANDBOX, Arc::new(env))?;
    Ok(())
}

/// A new directory only the current user can enter, used as `TMPDIR`.
fn private_temp_dir() -> Result<PathBuf, String> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or_default();
    let path =
        std::env::temp_dir().join(format!("rah-sandbox-os-{}-{nanos:x}", std::process::id()));
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&path)
        .map_err(|error| format!("temporary directory `{}`: {error}", path.display()))?;
    std::fs::canonicalize(&path).map_err(|error| error.to_string())
}

// Landlock (include/uapi/linux/landlock.h).
const CREATE_RULESET_VERSION: libc::c_uint = 1 << 0;
const RULE_PATH_BENEATH: libc::c_int = 1;

const FS_EXECUTE: u64 = 1 << 0;
const FS_WRITE_FILE: u64 = 1 << 1;
const FS_READ_FILE: u64 = 1 << 2;
const FS_READ_DIR: u64 = 1 << 3;
/// `REMOVE_DIR` through `MAKE_SYM` (ABI 1), `REFER` (ABI 2), `TRUNCATE`
/// (ABI 3) and `IOCTL_DEV` (ABI 5).
const FS_ABI_3: u64 = (1 << 15) - 1;
const FS_IOCTL_DEV: u64 = 1 << 15;
const FS_TRUNCATE: u64 = 1 << 14;
/// Rights that apply to a file rather than a directory.
const FS_FILE: u64 = FS_EXECUTE | FS_WRITE_FILE | FS_READ_FILE | FS_TRUNCATE | FS_IOCTL_DEV;
const FS_READ: u64 = FS_EXECUTE | FS_READ_FILE | FS_READ_DIR;

const SCOPE_ABSTRACT_UNIX_SOCKET: u64 = 1 << 0;
const SCOPE_SIGNAL: u64 = 1 << 1;

/// The oldest ABI with `TRUNCATE`, without which a confined program could
/// still truncate any file it can read.
const MIN_ABI: libc::c_long = 3;

#[repr(C)]
struct RulesetAttr {
    handled_access_fs: u64,
    handled_access_net: u64,
    scoped: u64,
}

#[repr(C, packed)]
struct PathBeneathAttr {
    allowed_access: u64,
    parent_fd: i32,
}

/// Device files every program may use whatever `readable` says.
const DEVICES: [(&str, u64); 5] = [
    ("/dev/null", FS_READ_FILE | FS_WRITE_FILE | FS_TRUNCATE),
    ("/dev/zero", FS_READ_FILE | FS_WRITE_FILE | FS_TRUNCATE),
    ("/dev/full", FS_READ_FILE | FS_WRITE_FILE | FS_TRUNCATE),
    ("/dev/random", FS_READ_FILE),
    ("/dev/urandom", FS_READ_FILE),
];

/// A prepared landlock ruleset and, when the network is denied, a seccomp
/// filter.
pub(crate) struct Confinement {
    ruleset: OwnedFd,
    filter: Option<Vec<libc::sock_filter>>,
}

impl Confinement {
    pub(crate) fn new(
        readable: &[PathBuf],
        writable: &[PathBuf],
        network: bool,
    ) -> Result<Self, String> {
        let abi = landlock_abi()?;
        if abi < MIN_ABI {
            return Err(format!(
                "landlock ABI {abi} is too old; sandbox-os needs ABI {MIN_ABI} (Linux 6.2) or later"
            ));
        }
        let handled = if abi >= 5 {
            FS_ABI_3 | FS_IOCTL_DEV
        } else {
            FS_ABI_3
        };
        let scoped = match (abi >= 6, network) {
            (false, _) => 0,
            (true, true) => SCOPE_SIGNAL,
            (true, false) => SCOPE_SIGNAL | SCOPE_ABSTRACT_UNIX_SOCKET,
        };
        let attr = RulesetAttr {
            handled_access_fs: handled,
            handled_access_net: 0,
            scoped,
        };
        // Older kernels reject fields they do not know.
        let size = match abi {
            ..4 => size_of::<u64>(),
            4 | 5 => 2 * size_of::<u64>(),
            _ => size_of::<RulesetAttr>(),
        };
        // SAFETY: `attr` outlives the call and `size` does not exceed it.
        let fd = unsafe {
            libc::syscall(
                libc::SYS_landlock_create_ruleset,
                &raw const attr,
                size,
                0_u32,
            )
        };
        let ruleset = owned_fd(fd).map_err(|error| format!("landlock_create_ruleset: {error}"))?;
        for path in readable {
            add_rule(&ruleset, path, handled & FS_READ)?;
        }
        for path in writable {
            add_rule(&ruleset, path, handled)?;
        }
        for (device, access) in DEVICES {
            if Path::new(device).exists() {
                add_rule(&ruleset, Path::new(device), handled & access)?;
            }
        }
        let filter = if network {
            None
        } else {
            Some(socket_filter()?)
        };
        // SAFETY: `PR_GET_SECCOMP` takes no pointer arguments.
        if filter.is_some() && unsafe { libc::prctl(libc::PR_GET_SECCOMP, 0, 0, 0, 0) } < 0 {
            return Err(format!(
                "seccomp is not available: {}",
                io::Error::last_os_error()
            ));
        }
        Ok(Self { ruleset, filter })
    }

    /// Confines the program `command` starts, before it executes.
    pub(crate) fn configure(self: &Arc<Self>, command: &mut Command) {
        let this = Arc::clone(self);
        // SAFETY: `enter` makes only async-signal-safe system calls and does
        // not allocate.
        unsafe {
            command.pre_exec(move || this.enter());
        }
    }

    /// Runs in the child after `fork`.
    fn enter(&self) -> io::Result<()> {
        // SAFETY: plain system calls; every pointer refers to memory owned by
        // `self` or the stack for the duration of the call.
        unsafe {
            if libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) != 0 {
                return Err(io::Error::last_os_error());
            }
            if libc::syscall(
                libc::SYS_landlock_restrict_self,
                self.ruleset.as_raw_fd(),
                0_u32,
            ) != 0
            {
                return Err(io::Error::last_os_error());
            }
            if let Some(filter) = &self.filter {
                let program = libc::sock_fprog {
                    len: filter.len() as libc::c_ushort,
                    filter: filter.as_ptr().cast_mut(),
                };
                if libc::prctl(
                    libc::PR_SET_SECCOMP,
                    libc::SECCOMP_MODE_FILTER,
                    &raw const program,
                    0,
                    0,
                ) != 0
                {
                    return Err(io::Error::last_os_error());
                }
            }
        }
        Ok(())
    }
}

pub(crate) fn landlock_abi() -> Result<libc::c_long, String> {
    // SAFETY: the version query takes no attribute.
    let abi = unsafe {
        libc::syscall(
            libc::SYS_landlock_create_ruleset,
            std::ptr::null::<RulesetAttr>(),
            0_usize,
            CREATE_RULESET_VERSION,
        )
    };
    if abi < 0 {
        let error = io::Error::last_os_error();
        return Err(match error.raw_os_error() {
            Some(libc::ENOSYS) => "landlock is not supported by this kernel".to_owned(),
            Some(libc::EOPNOTSUPP) => "landlock is disabled on this system".to_owned(),
            _ => format!("landlock: {error}"),
        });
    }
    Ok(abi)
}

fn owned_fd(result: libc::c_long) -> io::Result<OwnedFd> {
    let fd = libc::c_int::try_from(result).map_err(|_| io::Error::other("bad descriptor"))?;
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: the kernel returned a new descriptor that nothing else owns.
    Ok(unsafe { OwnedFd::from_raw_fd(fd) })
}

fn add_rule(ruleset: &OwnedFd, path: &Path, access: u64) -> Result<(), String> {
    let failed = |error: io::Error| format!("landlock rule for `{}`: {error}", path.display());
    let metadata = std::fs::metadata(path).map_err(failed)?;
    let access = if metadata.is_dir() {
        access
    } else {
        access & FS_FILE
    };
    if access == 0 {
        return Ok(());
    }
    let name = CString::new(path.as_os_str().as_bytes())
        .map_err(|_| format!("`{}` contains NUL", path.display()))?;
    // SAFETY: `name` is a valid C string for the duration of the call.
    let fd = unsafe { libc::open(name.as_ptr(), libc::O_PATH | libc::O_CLOEXEC) };
    let parent = owned_fd(fd.into()).map_err(failed)?;
    let attr = PathBeneathAttr {
        allowed_access: access,
        parent_fd: parent.as_raw_fd(),
    };
    // SAFETY: `attr` outlives the call; both descriptors are valid.
    let added = unsafe {
        libc::syscall(
            libc::SYS_landlock_add_rule,
            ruleset.as_raw_fd(),
            RULE_PATH_BENEATH,
            &raw const attr,
            0_u32,
        )
    };
    if added != 0 {
        return Err(failed(io::Error::last_os_error()));
    }
    Ok(())
}

// Classic BPF (include/uapi/linux/filter.h, bpf_common.h).
/// `BPF_LD | BPF_W | BPF_ABS`.
const BPF_LD_W_ABS: u16 = 0x20;
const BPF_JMP_JEQ_K: u16 = 0x05 | 0x10;
const BPF_JMP_JGE_K: u16 = 0x05 | 0x30;
const BPF_RET_K: u16 = 0x06;

// Offsets into `struct seccomp_data`.
const DATA_NR: u32 = 0;
const DATA_ARCH: u32 = 4;
/// The low 32 bits of the first argument on little-endian hosts.
const DATA_ARG0: u32 = 16;

#[cfg(target_arch = "x86_64")]
const AUDIT_ARCH: u32 = 0xC000_003E;
#[cfg(target_arch = "aarch64")]
const AUDIT_ARCH: u32 = 0xC000_00B7;
/// x32 system calls on x86-64; never valid numbers on AArch64.
const X32_SYSCALL_BIT: u32 = 0x4000_0000;

/// Denies `socket` for every family but `AF_UNIX` (`EACCES`) and
/// `io_uring_setup` (`EPERM`), whose operations could create sockets without
/// `socket`. System calls of another architecture (32-bit or x32 programs)
/// kill the process, since they would bypass the check.
#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
fn socket_filter() -> Result<Vec<libc::sock_filter>, String> {
    let statement = |code, k| libc::sock_filter {
        code,
        jt: 0,
        jf: 0,
        k,
    };
    let jump = |code, k, jt, jf| libc::sock_filter { code, jt, jf, k };
    let errno = |code: i32| libc::SECCOMP_RET_ERRNO | code as u32;
    let number = |nr: libc::c_long| nr as u32;
    Ok(vec![
        /* 0 */ statement(BPF_LD_W_ABS, DATA_ARCH),
        /* 1 */ jump(BPF_JMP_JEQ_K, AUDIT_ARCH, 0, 10), // -> 12
        /* 2 */ statement(BPF_LD_W_ABS, DATA_NR),
        /* 3 */ jump(BPF_JMP_JGE_K, X32_SYSCALL_BIT, 8, 0), // -> 12
        /* 4 */ jump(BPF_JMP_JEQ_K, number(libc::SYS_socket), 2, 0), // -> 7
        /* 5 */
        jump(BPF_JMP_JEQ_K, number(libc::SYS_io_uring_setup), 5, 0), // -> 11
        /* 6 */ statement(BPF_RET_K, libc::SECCOMP_RET_ALLOW),
        /* 7 */ statement(BPF_LD_W_ABS, DATA_ARG0),
        /* 8 */ jump(BPF_JMP_JEQ_K, libc::AF_UNIX as u32, 0, 1), // -> 10
        /* 9 */ statement(BPF_RET_K, libc::SECCOMP_RET_ALLOW),
        /* 10 */ statement(BPF_RET_K, errno(libc::EACCES)),
        /* 11 */ statement(BPF_RET_K, errno(libc::EPERM)),
        /* 12 */ statement(BPF_RET_K, libc::SECCOMP_RET_KILL_PROCESS),
    ])
}

#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
fn socket_filter() -> Result<Vec<libc::sock_filter>, String> {
    Err("denying the network needs an x86-64 or AArch64 host; set `network = true`".into())
}

#[cfg(test)]
#[test]
fn the_config_example_is_a_valid_configuration() {
    let parsed: Result<Config, _> = serde_json::from_str(crate::SandboxOsPlugin::CONFIG_EXAMPLE);
    assert!(parsed.is_ok(), "{:?}", parsed.err());
}
