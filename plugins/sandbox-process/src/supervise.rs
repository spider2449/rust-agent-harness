//! Supervision of one child process. Everything runs inside the caller's
//! future: there are no detached reader tasks, and dropping the future kills
//! the process tree (`kill_on_drop` plus [`ProcessTree`]'s `Drop`).

use std::time::Duration;

use rah_api::{
    CancellationToken,
    sandbox::{ExecError, ExecOutput, ProgramSpec, Termination},
};
use tokio::{
    io::{AsyncRead, AsyncReadExt},
    process::Command,
    time::{self, Instant},
};

use crate::process::Process;

const CHUNK_BYTES: usize = 8 * 1024;
/// How long pipes may stay open after the program exits (descendants holding
/// them), and how long a terminated program may take to be reaped.
pub(crate) const GRACE: Duration = Duration::from_secs(2);

pub(crate) async fn supervise(
    mut child: Process,
    spec: &ProgramSpec,
    cancel: &CancellationToken,
    stop: &CancellationToken,
) -> Result<ExecOutput, ExecError> {
    // Attach before reading anything, so the tree is owned from the start.
    let mut tree = match ProcessTree::attach(&child) {
        Ok(tree) => tree,
        Err(error) => {
            let _ = child.start_kill();
            return Err(supervision(error));
        }
    };
    child
        .resume()
        .map_err(|error| supervision(error.to_string()))?;
    let mut stdout = child.stdout.take();
    let mut stderr = child.stderr.take();
    let mut out = Vec::new();
    let mut err = Vec::new();
    let mut out_chunk = vec![0_u8; CHUNK_BYTES];
    let mut err_chunk = vec![0_u8; CHUNK_BYTES];
    let mut status: Option<Option<i32>> = None;
    let mut termination = None;
    let deadline = Instant::now() + spec.timeout;
    let mut pipe_deadline: Option<Instant> = None;

    while status.is_none() || stdout.is_some() || stderr.is_some() {
        tokio::select! {
            biased;
            () = cancel.cancelled() => {
                termination = Some(Termination::Cancelled);
                break;
            }
            () = stop.cancelled() => {
                termination = Some(Termination::Cancelled);
                break;
            }
            () = time::sleep_until(deadline), if status.is_none() => {
                termination = Some(Termination::TimedOut);
                break;
            }
            () = sleep_until(pipe_deadline) => break,
            read = read_some(stdout.as_mut(), &mut out_chunk) => match read {
                Ok(0) | Err(_) => stdout = None,
                Ok(count) => if !retain(&mut out, &out_chunk[..count], spec.limits.stdout_bytes) {
                    termination = Some(Termination::OutputLimit);
                    break;
                },
            },
            read = read_some(stderr.as_mut(), &mut err_chunk) => match read {
                Ok(0) | Err(_) => stderr = None,
                Ok(count) => if !retain(&mut err, &err_chunk[..count], spec.limits.stderr_bytes) {
                    termination = Some(Termination::OutputLimit);
                    break;
                },
            },
            waited = child.wait(), if status.is_none() => {
                status = Some(waited.map_err(|error| supervision(error.to_string()))?);
                pipe_deadline = Some(Instant::now() + GRACE);
            }
        }
    }

    // Stop the program when it was terminated, and descendants that still
    // hold its pipes after it exited.
    if termination.is_some() || stdout.is_some() || stderr.is_some() {
        tree.terminate().map_err(supervision)?;
    }
    if status.is_none() {
        let _ = child.start_kill();
        match time::timeout(GRACE, child.wait()).await {
            Ok(Ok(_)) => {}
            Ok(Err(error)) => return Err(supervision(error.to_string())),
            Err(_) => return Err(supervision("the process did not exit after termination")),
        }
    }
    tree.disarm();
    Ok(ExecOutput {
        stdout: out,
        stderr: err,
        exit_code: status.filter(|_| termination.is_none()).flatten(),
        termination,
    })
}

/// Appends up to `limit` bytes in total; returns `false` when `chunk` did not
/// fit.
fn retain(buffer: &mut Vec<u8>, chunk: &[u8], limit: usize) -> bool {
    let room = limit.saturating_sub(buffer.len());
    buffer.extend_from_slice(&chunk[..chunk.len().min(room)]);
    chunk.len() <= room
}

async fn read_some<R: AsyncRead + Unpin>(
    reader: Option<&mut R>,
    chunk: &mut [u8],
) -> std::io::Result<usize> {
    match reader {
        Some(reader) => reader.read(chunk).await,
        None => std::future::pending().await,
    }
}

async fn sleep_until(deadline: Option<Instant>) {
    match deadline {
        Some(deadline) => time::sleep_until(deadline).await,
        None => std::future::pending().await,
    }
}

fn supervision(message: impl Into<String>) -> ExecError {
    ExecError::Supervision(message.into())
}

#[cfg(windows)]
pub(crate) fn configure(command: &mut Command) {
    use windows_sys::Win32::System::Threading::CREATE_NO_WINDOW;
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(unix)]
pub(crate) fn configure(command: &mut Command) {
    command.process_group(0);
}

#[cfg(not(any(unix, windows)))]
pub(crate) fn configure(_command: &mut Command) {}

/// Owns the child's process tree until disarmed; dropping it terminates the
/// tree. Windows assigns the child to a kill-on-close Job Object right after
/// spawn, so a process the child starts before assignment can escape, unless
/// the child was started suspended (`sandbox-os`).
#[cfg(windows)]
pub(crate) struct ProcessTree {
    job: Option<std::os::windows::io::OwnedHandle>,
}

#[cfg(windows)]
impl ProcessTree {
    pub(crate) fn attach(child: &Process) -> Result<Self, String> {
        use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
        use windows_sys::Win32::System::JobObjects::{
            AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
            SetInformationJobObject,
        };

        // SAFETY: null attributes and name create an anonymous job object.
        let raw = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
        if raw.is_null() {
            return Err(last_os_error("CreateJobObjectW"));
        }
        // SAFETY: `raw` is a valid handle that nothing else owns.
        let job = unsafe { OwnedHandle::from_raw_handle(raw.cast()) };
        let mut information = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        information.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        // SAFETY: the pointer and size describe `information`, which outlives the call.
        let configured = unsafe {
            SetInformationJobObject(
                job.as_raw_handle().cast(),
                JobObjectExtendedLimitInformation,
                (&raw const information).cast(),
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        };
        if configured == 0 {
            return Err(last_os_error("SetInformationJobObject"));
        }
        let process = child
            .raw_handle()
            .ok_or_else(|| "the spawned process has no handle".to_owned())?;
        // SAFETY: both handles are valid for the duration of the call.
        let assigned =
            unsafe { AssignProcessToJobObject(job.as_raw_handle().cast(), process.cast()) };
        if assigned == 0 {
            return Err(last_os_error("AssignProcessToJobObject"));
        }
        Ok(Self { job: Some(job) })
    }

    pub(crate) fn terminate(&mut self) -> Result<(), String> {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::System::JobObjects::TerminateJobObject;

        let Some(job) = &self.job else {
            return Ok(());
        };
        // SAFETY: `job` is a valid job object handle owned by `self`.
        if unsafe { TerminateJobObject(job.as_raw_handle().cast(), 1) } == 0 {
            return Err(last_os_error("TerminateJobObject"));
        }
        Ok(())
    }

    /// Closing the job handle still kills members left behind
    /// (kill-on-close).
    pub(crate) fn disarm(&mut self) {
        self.job.take();
    }
}

#[cfg(unix)]
pub(crate) struct ProcessTree {
    group: Option<i32>,
}

#[cfg(unix)]
impl ProcessTree {
    pub(crate) fn attach(child: &Process) -> Result<Self, String> {
        let pid = child
            .id()
            .ok_or_else(|| "the spawned process has no id".to_owned())?;
        let group = i32::try_from(pid).map_err(|_| "process id out of range".to_owned())?;
        Ok(Self { group: Some(group) })
    }

    pub(crate) fn terminate(&mut self) -> Result<(), String> {
        let Some(group) = self.group else {
            return Ok(());
        };
        // SAFETY: signalling a process group has no memory-safety requirements.
        let result = unsafe { libc::kill(-group, libc::SIGKILL) };
        if result == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH) {
            Ok(())
        } else {
            Err(last_os_error("kill"))
        }
    }

    pub(crate) fn disarm(&mut self) {
        self.group.take();
    }
}

#[cfg(not(any(unix, windows)))]
pub(crate) struct ProcessTree;

#[cfg(not(any(unix, windows)))]
impl ProcessTree {
    pub(crate) fn attach(_child: &Process) -> Result<Self, String> {
        Ok(Self)
    }

    pub(crate) fn terminate(&mut self) -> Result<(), String> {
        Ok(())
    }

    pub(crate) fn disarm(&mut self) {}
}

impl Drop for ProcessTree {
    fn drop(&mut self) {
        let _ = self.terminate();
    }
}

#[cfg(any(unix, windows))]
fn last_os_error(operation: &str) -> String {
    format!("{operation}: {}", std::io::Error::last_os_error())
}
