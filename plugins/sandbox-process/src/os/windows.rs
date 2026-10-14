//! Windows confinement: every program runs with a Low integrity token.
//!
//! Windows denies a Low integrity process write access to anything whose
//! mandatory label is higher, and objects without a label count as Medium.
//! The root, a private temporary directory and the `writable` paths get an
//! inheritable Low label (`S:(ML;OICI;NW;;;LW)`); everything else the user
//! owns stays unwritable. The token also has no privileges and keeps
//! Administrators only as a deny-only group.
//!
//! std cannot start a process with another token (its attribute support is
//! unstable), so programs are started here with `CreateProcessAsUserW`:
//! suspended, so the Job Object owns the tree before the program runs, with
//! named pipes for the standard streams and an explicit inherited-handle
//! list.

use std::{
    collections::BTreeMap,
    ffi::{OsStr, OsString, c_void},
    io,
    os::windows::{
        ffi::{OsStrExt, OsStringExt},
        io::{AsRawHandle, FromRawHandle, OwnedHandle, RawHandle},
    },
    path::{Path, PathBuf},
    pin::Pin,
    ptr::{null, null_mut},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    task::{Context, Poll},
    time::{SystemTime, UNIX_EPOCH},
};

use rah_api::{PluginError, Scope, sandbox::SANDBOX};
use serde::Deserialize;
use tokio::{
    io::{AsyncRead, ReadBuf},
    net::windows::named_pipe::{NamedPipeServer, ServerOptions},
    sync::watch,
};
use windows_sys::Win32::{
    Foundation::{
        GENERIC_READ, GENERIC_WRITE, HANDLE, INVALID_HANDLE_VALUE, LocalFree, STILL_ACTIVE,
    },
    Security::{
        ACL,
        Authorization::{
            ConvertStringSecurityDescriptorToSecurityDescriptorW, ConvertStringSidToSidW,
            GetNamedSecurityInfoW, SDDL_REVISION_1, SE_FILE_OBJECT, SetNamedSecurityInfoW,
        },
        CONTAINER_INHERIT_ACE, CreateRestrictedToken, DISABLE_MAX_PRIVILEGE, EqualSid, GetAce,
        GetLengthSid, GetSecurityDescriptorSacl, GetTokenInformation, LABEL_SECURITY_INFORMATION,
        OBJECT_INHERIT_ACE, PSECURITY_DESCRIPTOR, PSID, SECURITY_ATTRIBUTES, SID_AND_ATTRIBUTES,
        SYSTEM_MANDATORY_LABEL_ACE, SetTokenInformation, TOKEN_ADJUST_DEFAULT,
        TOKEN_ASSIGN_PRIMARY, TOKEN_DUPLICATE, TOKEN_GROUPS, TOKEN_MANDATORY_LABEL, TOKEN_QUERY,
        TokenGroups, TokenIntegrityLevel,
    },
    Storage::FileSystem::{
        CreateFileW, FILE_READ_ATTRIBUTES, FILE_SHARE_READ, FILE_SHARE_WRITE,
        FILE_WRITE_ATTRIBUTES, OPEN_EXISTING,
    },
    System::{
        SystemInformation::{GetSystemDirectoryW, GetWindowsDirectoryW},
        SystemServices::{
            SE_GROUP_INTEGRITY, SYSTEM_MANDATORY_LABEL_ACE_TYPE, SYSTEM_MANDATORY_LABEL_NO_WRITE_UP,
        },
        Threading::{
            CREATE_NO_WINDOW, CREATE_SUSPENDED, CREATE_UNICODE_ENVIRONMENT, CreateProcessAsUserW,
            DeleteProcThreadAttributeList, EXTENDED_STARTUPINFO_PRESENT, GetCurrentProcess,
            GetExitCodeProcess, INFINITE, InitializeProcThreadAttributeList,
            LPPROC_THREAD_ATTRIBUTE_LIST, OpenProcessToken, PROC_THREAD_ATTRIBUTE_HANDLE_LIST,
            PROCESS_INFORMATION, RegisterWaitForSingleObject, ResumeThread, STARTF_USESTDHANDLES,
            STARTUPINFOEXW, TerminateProcess, UnregisterWaitEx, UpdateProcThreadAttribute,
            WT_EXECUTEONLYONCE,
        },
    },
};

use super::resolve;
use crate::{
    ProcessEnv, Stdin, canonical_root, inherited_env, max_timeout,
    process::{Process, Reader, Writer},
};

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

/// An inheritable Low label that denies writes from below Low (`NW`).
const LOW_LABEL_SDDL: &str = "S:(ML;OICI;NW;;;LW)";

pub(super) async fn apply(scope: &mut Scope) -> Result<(), PluginError> {
    let config: Config = if scope.config().is_null() {
        Config::default()
    } else {
        scope.config_as()?
    };
    if config.readable.is_some() {
        return Err("readable: sandbox-os cannot restrict reads on Windows; \
                    a Low integrity program can read whatever the user can"
            .into());
    }
    if !config.network {
        return Err("sandbox-os cannot deny the network on Windows; \
                    set `network = true` to run programs with network access"
            .into());
    }
    let root = canonical_root(config.root).await?;
    let mut base_env = inherited_env(config.inherit_env)?;
    let max_timeout = max_timeout(config.max_timeout_ms)?;
    let mut labeled = vec![root.clone()];
    labeled.extend(resolve(&root, config.writable.unwrap_or_default(), "writable").await?);
    for path in &labeled {
        refuse_broad(path)?;
    }
    let confinement =
        Confinement::new().map_err(|error| format!("Low integrity token: {error}"))?;

    let temp = private_temp_dir()?;
    let removed = temp.clone();
    scope.on_dispose(move || async move {
        let _ = tokio::fs::remove_dir_all(removed).await;
    });
    labeled.push(temp.clone());
    // Labeling propagates to every existing file, which takes a while on a
    // large tree the first time.
    tokio::task::spawn_blocking(move || {
        labeled.iter().try_for_each(|path| {
            label_low(path).map_err(|error| format!("Low label on `{}`: {error}", path.display()))
        })
    })
    .await
    .map_err(|error| format!("labeling failed: {error}"))??;

    base_env
        .retain(|name, _| !name.eq_ignore_ascii_case("TEMP") && !name.eq_ignore_ascii_case("TMP"));
    base_env.insert("TEMP".into(), temp.clone().into_os_string());
    base_env.insert("TMP".into(), temp.into_os_string());

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

/// Refuses to label a drive root, the user profile or the Windows directory
/// (or a directory holding them): every Low integrity process of the user
/// could write there afterwards.
fn refuse_broad(path: &Path) -> Result<(), String> {
    let shown = plain(path);
    if path.parent().is_none() {
        return Err(format!(
            "`{}` is a drive root; sandbox-os labels its root and writable paths Low",
            shown.display()
        ));
    }
    for name in ["USERPROFILE", "SystemRoot"] {
        let Some(protected) =
            std::env::var_os(name).and_then(|dir| std::fs::canonicalize(dir).ok())
        else {
            continue;
        };
        if protected.starts_with(path) || (name == "SystemRoot" && path.starts_with(&protected)) {
            return Err(format!(
                "`{}` holds or is inside %{name}%; sandbox-os labels its root and writable \
                 paths Low",
                shown.display()
            ));
        }
    }
    Ok(())
}

/// A new directory under the user's temporary directory, used as `TEMP`.
fn private_temp_dir() -> Result<PathBuf, String> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or_default();
    let path =
        std::env::temp_dir().join(format!("rah-sandbox-os-{}-{nanos:x}", std::process::id()));
    std::fs::create_dir(&path)
        .map_err(|error| format!("temporary directory `{}`: {error}", path.display()))?;
    Ok(path)
}

/// Gives `path` and everything below it the inheritable Low label, unless it
/// already has it.
fn label_low(path: &Path) -> io::Result<()> {
    let name = wide(plain(path).as_os_str());
    if has_low_label(&name)? {
        return Ok(());
    }
    let label = wide(OsStr::new(LOW_LABEL_SDDL));
    let mut descriptor: PSECURITY_DESCRIPTOR = null_mut();
    // SAFETY: `label` is NUL-terminated; the descriptor is freed by `Local`.
    let converted = unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            label.as_ptr(),
            SDDL_REVISION_1,
            &mut descriptor,
            null_mut(),
        )
    };
    if converted == 0 {
        return Err(io::Error::last_os_error());
    }
    let descriptor = Local(descriptor);
    let mut present = 0;
    let mut defaulted = 0;
    let mut sacl: *mut ACL = null_mut();
    // SAFETY: `descriptor` is valid; `sacl` points into it and is used while
    // it lives.
    if unsafe { GetSecurityDescriptorSacl(descriptor.0, &mut present, &mut sacl, &mut defaulted) }
        == 0
    {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: `name` is NUL-terminated and `sacl` valid. Setting the label
    // also propagates it to the existing children.
    let error = unsafe {
        SetNamedSecurityInfoW(
            name.as_ptr(),
            SE_FILE_OBJECT,
            LABEL_SECURITY_INFORMATION,
            null_mut(),
            null_mut(),
            null(),
            sacl,
        )
    };
    win32_result(error)
}

/// Whether `name` carries an inheritable no-write-up Low label, explicit or
/// inherited.
pub(crate) fn has_low_label(name: &[u16]) -> io::Result<bool> {
    let mut sacl: *mut ACL = null_mut();
    let mut descriptor: PSECURITY_DESCRIPTOR = null_mut();
    // SAFETY: `name` is NUL-terminated; the descriptor is freed by `Local`.
    let error = unsafe {
        GetNamedSecurityInfoW(
            name.as_ptr(),
            SE_FILE_OBJECT,
            LABEL_SECURITY_INFORMATION,
            null_mut(),
            null_mut(),
            null_mut(),
            &mut sacl,
            &mut descriptor,
        )
    };
    win32_result(error)?;
    let _descriptor = Local(descriptor);
    if sacl.is_null() {
        return Ok(false);
    }
    let low = Sid::parse("LW")?;
    let inherit = (OBJECT_INHERIT_ACE | CONTAINER_INHERIT_ACE) as u8;
    // SAFETY: `sacl` points into the live descriptor.
    let count = unsafe { (*sacl).AceCount };
    for index in 0..u32::from(count) {
        let mut ace: *mut c_void = null_mut();
        // SAFETY: `index` is below the ACE count of a valid ACL.
        if unsafe { GetAce(sacl, index, &mut ace) } == 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: every ACE starts with a header; a label ACE has the
        // mandatory label layout, with its SID at `SidStart`.
        let label = unsafe { &*ace.cast::<SYSTEM_MANDATORY_LABEL_ACE>() };
        if u32::from(label.Header.AceType) != SYSTEM_MANDATORY_LABEL_ACE_TYPE {
            continue;
        }
        let sid = (&raw const label.SidStart).cast_mut().cast::<c_void>();
        return Ok(label.Header.AceFlags & inherit == inherit
            && label.Mask & SYSTEM_MANDATORY_LABEL_NO_WRITE_UP != 0
            // SAFETY: both SIDs are valid.
            && unsafe { EqualSid(sid, low.0) } != 0);
    }
    Ok(false)
}

/// The Low integrity token every program runs with.
pub(crate) struct Confinement {
    token: OwnedHandle,
    /// Makes pipe names unique within this plugin instance.
    pipes: AtomicU64,
}

impl Confinement {
    fn new() -> io::Result<Self> {
        let mut raw: HANDLE = null_mut();
        // SAFETY: the pseudo handle of the current process is always valid.
        let opened = unsafe {
            OpenProcessToken(
                GetCurrentProcess(),
                TOKEN_DUPLICATE | TOKEN_QUERY | TOKEN_ASSIGN_PRIMARY | TOKEN_ADJUST_DEFAULT,
                &mut raw,
            )
        };
        if opened == 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: a new token handle that nothing else owns.
        let process_token = unsafe { OwnedHandle::from_raw_handle(raw) };
        let administrators = Sid::parse("BA")?;
        let disable: Vec<SID_AND_ATTRIBUTES> = if has_group(&process_token, &administrators)? {
            vec![SID_AND_ATTRIBUTES {
                Sid: administrators.0,
                Attributes: 0,
            }]
        } else {
            Vec::new()
        };
        let mut restricted: HANDLE = null_mut();
        // SAFETY: `disable` outlives the call and holds valid SIDs.
        let created = unsafe {
            CreateRestrictedToken(
                process_token.as_raw_handle(),
                DISABLE_MAX_PRIVILEGE,
                disable.len() as u32,
                disable.as_ptr(),
                0,
                null(),
                0,
                null(),
                &mut restricted,
            )
        };
        if created == 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: a new token handle that nothing else owns.
        let token = unsafe { OwnedHandle::from_raw_handle(restricted) };
        let low = Sid::parse("LW")?;
        let label = TOKEN_MANDATORY_LABEL {
            Label: SID_AND_ATTRIBUTES {
                Sid: low.0,
                Attributes: SE_GROUP_INTEGRITY as u32,
            },
        };
        // SAFETY: `label` and the SID it points to outlive the call.
        let set = unsafe {
            SetTokenInformation(
                token.as_raw_handle(),
                TokenIntegrityLevel,
                (&raw const label).cast(),
                size_of::<TOKEN_MANDATORY_LABEL>() as u32 + GetLengthSid(low.0),
            )
        };
        if set == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Self {
            token,
            pipes: AtomicU64::new(0),
        })
    }

    /// Starts `program` suspended with the Low integrity token; supervision
    /// resumes it once the process tree is owned.
    pub(crate) async fn spawn(
        &self,
        program: &str,
        args: &[String],
        cwd: &Path,
        base_env: &BTreeMap<OsString, OsString>,
        env: &BTreeMap<String, String>,
        stdin: Stdin,
    ) -> io::Result<Process> {
        let env = merged_env(base_env, env);
        let path = env
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case("PATH"))
            .map(|(_, value)| value.as_os_str());
        let cwd = plain(cwd);
        let application = resolve_program(program, &cwd, path)?;
        let mut command_line = command_line(&application, args);
        let environment = environment_block(&env);

        let (stdout, stdout_child) = self.pipe(true).await?;
        let (stderr, stderr_child) = self.pipe(true).await?;
        let (stdin, stdin_child) = match stdin {
            Stdin::Null => (None, open_null()?),
            Stdin::Piped => {
                let (server, child) = self.pipe(false).await?;
                (Some(Box::new(server) as Writer), child)
            }
        };
        let handles = [
            stdin_child.as_raw_handle(),
            stdout_child.as_raw_handle(),
            stderr_child.as_raw_handle(),
        ];
        let child = self.create(
            &wide(application.as_os_str()),
            &mut command_line,
            &environment,
            &wide(cwd.as_os_str()),
            handles,
        )?;
        // The child's ends close here, so the pipes end when the program and
        // its descendants are gone.
        drop((stdin_child, stdout_child, stderr_child));
        Ok(Process::native(
            child,
            stdin,
            Box::new(PipeReader(stdout)) as Reader,
            Box::new(PipeReader(stderr)) as Reader,
        ))
    }

    /// A named pipe: the server end for RAH and an inheritable client end for
    /// the program. `from_child` pipes carry output.
    async fn pipe(&self, from_child: bool) -> io::Result<(NamedPipeServer, OwnedHandle)> {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or_default();
        let name = format!(
            r"\\.\pipe\rah-sandbox-os-{}-{nanos:x}-{}",
            std::process::id(),
            self.pipes.fetch_add(1, Ordering::Relaxed)
        );
        // One instance, created first: nothing else can serve or take it.
        let server = ServerOptions::new()
            .first_pipe_instance(true)
            .reject_remote_clients(true)
            .max_instances(1)
            .access_inbound(from_child)
            .access_outbound(!from_child)
            .create(&name)?;
        let access = if from_child {
            GENERIC_WRITE | FILE_READ_ATTRIBUTES
        } else {
            GENERIC_READ | FILE_WRITE_ATTRIBUTES
        };
        let client = open_inheritable(OsStr::new(&name), access, 0)?;
        server.connect().await?;
        Ok((server, client))
    }

    fn create(
        &self,
        application: &[u16],
        command_line: &mut [u16],
        environment: &[u16],
        cwd: &[u16],
        handles: [RawHandle; 3],
    ) -> io::Result<NativeChild> {
        let attributes = AttributeList::new()?;
        // SAFETY: `handles` outlives `attributes`' use in `CreateProcessAsUserW`.
        let updated = unsafe {
            UpdateProcThreadAttribute(
                attributes.as_ptr(),
                0,
                PROC_THREAD_ATTRIBUTE_HANDLE_LIST as usize,
                handles.as_ptr().cast(),
                size_of_val(&handles),
                null_mut(),
                null(),
            )
        };
        if updated == 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: an all-zero `STARTUPINFOEXW` is valid.
        let mut startup: STARTUPINFOEXW = unsafe { std::mem::zeroed() };
        startup.StartupInfo.cb = size_of::<STARTUPINFOEXW>() as u32;
        startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
        startup.StartupInfo.hStdInput = handles[0];
        startup.StartupInfo.hStdOutput = handles[1];
        startup.StartupInfo.hStdError = handles[2];
        startup.lpAttributeList = attributes.as_ptr();
        // SAFETY: an all-zero `PROCESS_INFORMATION` is valid.
        let mut information: PROCESS_INFORMATION = unsafe { std::mem::zeroed() };
        // SAFETY: every string is NUL-terminated (the environment doubly),
        // the token and handles are valid, and all outlive the call.
        let created = unsafe {
            CreateProcessAsUserW(
                self.token.as_raw_handle(),
                application.as_ptr(),
                command_line.as_mut_ptr(),
                null(),
                null(),
                1,
                CREATE_SUSPENDED
                    | CREATE_NO_WINDOW
                    | CREATE_UNICODE_ENVIRONMENT
                    | EXTENDED_STARTUPINFO_PRESENT,
                environment.as_ptr().cast(),
                cwd.as_ptr(),
                &raw const startup.StartupInfo,
                &mut information,
            )
        };
        if created == 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: new handles that nothing else owns.
        let (process, thread) = unsafe {
            (
                OwnedHandle::from_raw_handle(information.hProcess),
                OwnedHandle::from_raw_handle(information.hThread),
            )
        };
        Ok(NativeChild {
            process,
            thread: Some(thread),
            exit: None,
            watch: None,
        })
    }
}

fn has_group(token: &OwnedHandle, sid: &Sid) -> io::Result<bool> {
    let mut size = 0;
    // SAFETY: a size query; nothing is written.
    unsafe { GetTokenInformation(token.as_raw_handle(), TokenGroups, null_mut(), 0, &mut size) };
    // `u64` elements keep the buffer aligned for `TOKEN_GROUPS`.
    let mut buffer = vec![0_u64; (size as usize).div_ceil(8)];
    // SAFETY: the buffer holds `size` bytes.
    let read = unsafe {
        GetTokenInformation(
            token.as_raw_handle(),
            TokenGroups,
            buffer.as_mut_ptr().cast(),
            size,
            &mut size,
        )
    };
    if read == 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: the buffer now holds a `TOKEN_GROUPS` with `GroupCount` entries.
    let groups = unsafe {
        let groups = &*buffer.as_ptr().cast::<TOKEN_GROUPS>();
        std::slice::from_raw_parts(groups.Groups.as_ptr(), groups.GroupCount as usize)
    };
    // SAFETY: both SIDs are valid.
    Ok(groups
        .iter()
        .any(|group| unsafe { EqualSid(group.Sid, sid.0) } != 0))
}

/// The base environment overridden by the program's, with names compared
/// case-insensitively as Windows does, sorted by upper-case name.
fn merged_env(
    base: &BTreeMap<OsString, OsString>,
    overrides: &BTreeMap<String, String>,
) -> Vec<(OsString, OsString)> {
    let mut merged = BTreeMap::new();
    let entries = base
        .iter()
        .map(|(name, value)| (name.clone(), value.clone()));
    let overrides = overrides
        .iter()
        .map(|(name, value)| (OsString::from(name), OsString::from(value)));
    for (name, value) in entries.chain(overrides) {
        merged.insert(name.to_ascii_uppercase(), (name, value));
    }
    merged.into_values().collect()
}

/// `NAME=value` strings, each NUL-terminated, then one more NUL.
fn environment_block(env: &[(OsString, OsString)]) -> Vec<u16> {
    let mut block = Vec::new();
    for (name, value) in env {
        block.extend(name.encode_wide());
        block.push(u16::from(b'='));
        block.extend(value.encode_wide());
        block.push(0);
    }
    if block.is_empty() {
        block.push(0);
    }
    block.push(0);
    block
}

/// Finds the program as std does on Windows: `.exe` is appended when it has
/// no extension; a path is relative to the working directory; a bare name is
/// looked up in the program's `PATH`, then the system and Windows
/// directories. Batch files are refused: they run through `cmd.exe`, whose
/// quoting differs.
fn resolve_program(program: &str, cwd: &Path, path: Option<&OsStr>) -> io::Result<PathBuf> {
    let mut name = OsString::from(program);
    if Path::new(program).extension().is_none() {
        name.push(".exe");
    }
    let found = if program.contains(['/', '\\', ':']) {
        let candidate = cwd.join(&name);
        if !candidate.is_absolute() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "a drive-relative program path is not supported",
            ));
        }
        candidate.is_file().then_some(candidate)
    } else {
        let mut dirs: Vec<PathBuf> = path
            .map(|path| {
                std::env::split_paths(path)
                    .filter(|dir| dir.is_absolute())
                    .collect()
            })
            .unwrap_or_default();
        dirs.extend(system_directory(GetSystemDirectoryW));
        dirs.extend(system_directory(GetWindowsDirectoryW));
        dirs.into_iter()
            .map(|dir| dir.join(&name))
            .find(|candidate| candidate.is_file())
    };
    let found =
        found.ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "program not found"))?;
    let batch = found.extension().is_some_and(|extension| {
        extension.eq_ignore_ascii_case("bat") || extension.eq_ignore_ascii_case("cmd")
    });
    if batch {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "batch files are not started directly; run `cmd.exe` with `/c` instead",
        ));
    }
    Ok(plain(&found))
}

fn system_directory(query: unsafe extern "system" fn(*mut u16, u32) -> u32) -> Option<PathBuf> {
    let mut buffer = vec![0_u16; 512];
    // SAFETY: the buffer holds `len` characters.
    let len = unsafe { query(buffer.as_mut_ptr(), buffer.len() as u32) } as usize;
    if len == 0 || len >= buffer.len() {
        return None;
    }
    buffer.truncate(len);
    Some(PathBuf::from(OsString::from_wide(&buffer)))
}

/// The quoted application followed by the arguments, quoted as the C
/// runtime parses them, and a NUL.
fn command_line(application: &Path, args: &[String]) -> Vec<u16> {
    let mut line = vec![u16::from(b'"')];
    line.extend(application.as_os_str().encode_wide());
    line.push(u16::from(b'"'));
    for arg in args {
        line.push(u16::from(b' '));
        append_arg(&mut line, arg);
    }
    line.push(0);
    line
}

fn append_arg(line: &mut Vec<u16>, arg: &str) {
    let quote = arg.is_empty() || arg.contains([' ', '\t']);
    if quote {
        line.push(u16::from(b'"'));
    }
    let mut backslashes = 0;
    for unit in arg.encode_utf16() {
        if unit == u16::from(b'\\') {
            backslashes += 1;
        } else {
            if unit == u16::from(b'"') {
                // 2n + 1 backslashes before a literal quote.
                line.extend(std::iter::repeat_n(u16::from(b'\\'), backslashes + 1));
            }
            backslashes = 0;
        }
        line.push(unit);
    }
    if quote {
        // 2n backslashes before the closing quote.
        line.extend(std::iter::repeat_n(u16::from(b'\\'), backslashes));
        line.push(u16::from(b'"'));
    }
}

fn open_null() -> io::Result<OwnedHandle> {
    open_inheritable(
        OsStr::new("NUL"),
        GENERIC_READ | GENERIC_WRITE,
        FILE_SHARE_READ | FILE_SHARE_WRITE,
    )
}

fn open_inheritable(name: &OsStr, access: u32, share: u32) -> io::Result<OwnedHandle> {
    let attributes = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: null_mut(),
        bInheritHandle: 1,
    };
    let name = wide(name);
    // SAFETY: `name` is NUL-terminated and `attributes` outlives the call.
    let handle = unsafe {
        CreateFileW(
            name.as_ptr(),
            access,
            share,
            &raw const attributes,
            OPEN_EXISTING,
            0,
            null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: a new handle that nothing else owns.
    Ok(unsafe { OwnedHandle::from_raw_handle(handle) })
}

/// A program started by [`Confinement::spawn`]. Dropping it before the
/// program has been seen to exit terminates the program.
pub(crate) struct NativeChild {
    /// Declared first, so the wait is unregistered before `process` closes.
    watch: Option<ExitWatch>,
    process: OwnedHandle,
    /// The suspended main thread, until resumed.
    thread: Option<OwnedHandle>,
    exit: Option<i32>,
}

impl NativeChild {
    pub(crate) fn raw_handle(&self) -> RawHandle {
        self.process.as_raw_handle()
    }

    pub(crate) fn resume(&mut self) -> io::Result<()> {
        if let Some(thread) = self.thread.take() {
            // SAFETY: `thread` is a valid thread handle.
            if unsafe { ResumeThread(thread.as_raw_handle()) } == u32::MAX {
                return Err(io::Error::last_os_error());
            }
        }
        Ok(())
    }

    pub(crate) fn kill(&mut self) -> io::Result<()> {
        if self.exit.is_some() {
            return Ok(());
        }
        // SAFETY: `process` is a valid process handle.
        if unsafe { TerminateProcess(self.process.as_raw_handle(), 1) } == 0 {
            let error = io::Error::last_os_error();
            // Terminating a process that already exited fails.
            if exit_code(&self.process)? == STILL_ACTIVE as u32 {
                return Err(error);
            }
        }
        Ok(())
    }

    /// Waits for the exit; cancel-safe.
    pub(crate) async fn wait(&mut self) -> io::Result<i32> {
        if let Some(code) = self.exit {
            return Ok(code);
        }
        let watch = match &self.watch {
            Some(watch) => watch,
            None => self.watch.insert(ExitWatch::register(&self.process)?),
        };
        let mut exited = watch.exited.clone();
        exited
            .wait_for(|exited| *exited)
            .await
            .map_err(|_| io::Error::other("the exit notification was lost"))?;
        let code = exit_code(&self.process)? as i32;
        self.exit = Some(code);
        self.watch = None;
        Ok(code)
    }
}

impl Drop for NativeChild {
    fn drop(&mut self) {
        if self.exit.is_none() {
            // SAFETY: `process` is a valid process handle.
            unsafe { TerminateProcess(self.process.as_raw_handle(), 1) };
        }
    }
}

fn exit_code(process: &OwnedHandle) -> io::Result<u32> {
    let mut code = 0;
    // SAFETY: `process` is a valid process handle.
    if unsafe { GetExitCodeProcess(process.as_raw_handle(), &mut code) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(code)
}

/// A thread-pool wait on a process handle that flips `exited` to `true`.
struct ExitWatch {
    wait: HANDLE,
    sender: *mut watch::Sender<bool>,
    exited: watch::Receiver<bool>,
}

// SAFETY: the wait handle may be unregistered from any thread, and the
// sender it points to is `Send + Sync`.
unsafe impl Send for ExitWatch {}

impl ExitWatch {
    fn register(process: &OwnedHandle) -> io::Result<Self> {
        let (sender, exited) = watch::channel(false);
        let sender = Box::into_raw(Box::new(sender));
        let mut wait: HANDLE = null_mut();
        // SAFETY: `sender` stays valid until `drop` has unregistered the wait
        // and waited for a running callback.
        let registered = unsafe {
            RegisterWaitForSingleObject(
                &mut wait,
                process.as_raw_handle(),
                Some(exited_callback),
                sender.cast_const().cast(),
                INFINITE,
                WT_EXECUTEONLYONCE,
            )
        };
        if registered == 0 {
            let error = io::Error::last_os_error();
            // SAFETY: the wait was not registered; `sender` is unused.
            drop(unsafe { Box::from_raw(sender) });
            return Err(error);
        }
        Ok(Self {
            wait,
            sender,
            exited,
        })
    }
}

unsafe extern "system" fn exited_callback(context: *mut c_void, _timed_out: bool) {
    // SAFETY: `context` is the sender of a registered `ExitWatch`, alive
    // until the wait is unregistered.
    let sender = unsafe { &*context.cast::<watch::Sender<bool>>() };
    sender.send_replace(true);
}

impl Drop for ExitWatch {
    fn drop(&mut self) {
        // SAFETY: `INVALID_HANDLE_VALUE` waits for a running callback, after
        // which nothing uses `sender` again.
        unsafe {
            UnregisterWaitEx(self.wait, INVALID_HANDLE_VALUE);
            drop(Box::from_raw(self.sender));
        }
    }
}

/// The server end of an output pipe. A disconnected client is the end of
/// the stream, as with an anonymous pipe.
struct PipeReader(NamedPipeServer);

impl AsyncRead for PipeReader {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        match Pin::new(&mut self.0).poll_read(cx, buf) {
            Poll::Ready(Err(error)) if error.kind() == io::ErrorKind::BrokenPipe => {
                Poll::Ready(Ok(()))
            }
            other => other,
        }
    }
}

/// A process-thread attribute list with room for one attribute.
struct AttributeList {
    /// `u64` elements keep the list aligned.
    buffer: Vec<u64>,
}

impl AttributeList {
    fn new() -> io::Result<Self> {
        let mut size = 0;
        // SAFETY: a size query with a null list.
        unsafe { InitializeProcThreadAttributeList(null_mut(), 1, 0, &mut size) };
        let mut buffer = vec![0_u64; size.div_ceil(8)];
        // SAFETY: the buffer holds `size` bytes.
        if unsafe { InitializeProcThreadAttributeList(buffer.as_mut_ptr().cast(), 1, 0, &mut size) }
            == 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(Self { buffer })
    }

    fn as_ptr(&self) -> LPPROC_THREAD_ATTRIBUTE_LIST {
        self.buffer.as_ptr().cast_mut().cast()
    }
}

impl Drop for AttributeList {
    fn drop(&mut self) {
        // SAFETY: the list was initialized in `new`.
        unsafe { DeleteProcThreadAttributeList(self.as_ptr()) };
    }
}

/// A SID allocated by `ConvertStringSidToSidW`.
struct Sid(PSID);

impl Sid {
    fn parse(text: &str) -> io::Result<Self> {
        let text = wide(OsStr::new(text));
        let mut sid: PSID = null_mut();
        // SAFETY: `text` is NUL-terminated; the SID is freed in `drop`.
        if unsafe { ConvertStringSidToSidW(text.as_ptr(), &mut sid) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Self(sid))
    }
}

impl Drop for Sid {
    fn drop(&mut self) {
        // SAFETY: allocated with `LocalAlloc` by `ConvertStringSidToSidW`.
        unsafe { LocalFree(self.0) };
    }
}

/// Memory the system allocated with `LocalAlloc` for the caller to free.
struct Local(*mut c_void);

impl Drop for Local {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: allocated with `LocalAlloc` by the system.
            unsafe { LocalFree(self.0) };
        }
    }
}

fn win32_result(error: u32) -> io::Result<()> {
    if error == 0 {
        Ok(())
    } else {
        Err(io::Error::from_raw_os_error(error as i32))
    }
}

fn wide(text: &OsStr) -> Vec<u16> {
    text.encode_wide().chain(Some(0)).collect()
}

/// `path` without the verbatim `\\?\` prefix of a drive path, which some
/// programs (`cmd.exe`) cannot use as their working directory.
fn plain(path: &Path) -> PathBuf {
    let text = path.as_os_str().to_string_lossy();
    match text.strip_prefix(r"\\?\") {
        Some(rest) if !rest.starts_with("UNC\\") => PathBuf::from(rest),
        _ => path.to_path_buf(),
    }
}
