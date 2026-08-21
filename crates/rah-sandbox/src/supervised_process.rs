use std::{
    collections::BTreeMap,
    ffi::OsString,
    path::PathBuf,
    process::Stdio,
    sync::{Arc, Mutex},
    time::Duration,
};

use tokio::{
    io::{AsyncRead, AsyncReadExt},
    process::{Child, Command},
    sync::mpsc,
    task::JoinHandle,
    time,
};

use crate::SandboxError;

const PIPE_CHUNK_BYTES: usize = 8 * 1024;
const TERMINATION_GRACE: Duration = Duration::from_secs(2);

/// Hard byte limits applied while draining a host-authorized process.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OutputLimits {
    /// Maximum retained stdout bytes.
    pub stdout_bytes: usize,
    /// Maximum retained stderr bytes.
    pub stderr_bytes: usize,
    /// Maximum retained bytes across both streams.
    pub combined_bytes: usize,
}

impl OutputLimits {
    /// Recommended initial limits from ADR 0009.
    pub const RECOMMENDED: Self = Self {
        stdout_bytes: 256 * 1024,
        stderr_bytes: 256 * 1024,
        combined_bytes: 512 * 1024,
    };

    fn validate(self) -> Result<(), SandboxError> {
        if self.stdout_bytes == 0 || self.stderr_bytes == 0 || self.combined_bytes == 0 {
            return Err(execution_error("output limits must be greater than zero"));
        }
        if self.combined_bytes > self.stdout_bytes.saturating_add(self.stderr_bytes) {
            return Err(execution_error(
                "combined output limit cannot exceed the sum of stream limits",
            ));
        }
        Ok(())
    }
}

/// Trusted-host process specification. No field is sourced directly from model input.
#[derive(Clone, Debug)]
pub struct HostProcessSpec {
    /// Absolute canonical executable selected by the host policy.
    pub executable: PathBuf,
    /// Argument vector produced by the capability policy.
    pub args: Vec<OsString>,
    /// Absolute canonical working directory selected by the host policy.
    pub cwd: PathBuf,
    /// Exact environment supplied after clearing the parent environment.
    pub environment: BTreeMap<OsString, OsString>,
    /// Host-fixed execution timeout.
    pub timeout: Duration,
    /// Hard limits enforced while reading child output.
    pub output_limits: OutputLimits,
}

/// Stream whose configured or combined output limit was exceeded.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputOverflow {
    /// Standard output exceeded its individual limit.
    Stdout,
    /// Standard error exceeded its individual limit.
    Stderr,
    /// The combined stdout and stderr limit was reached first.
    Combined,
}

/// Bounded result from one direct host-authorized process spawn.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostProcessOutput {
    /// Retained standard output prefix.
    pub stdout: Vec<u8>,
    /// Retained standard error prefix.
    pub stderr: Vec<u8>,
    /// Exit code when the platform reported one.
    pub exit_code: Option<i32>,
    /// Whether the host timeout ended normal waiting.
    pub timed_out: bool,
    /// Output limit that ended normal execution, if any.
    pub overflow: Option<OutputOverflow>,
    /// Whether process termination was attempted.
    pub termination_attempted: bool,
}

/// Spawns exactly one trusted-host process with bounded, abort-safe supervision.
///
/// This function does not provide filesystem or network isolation. Platform
/// process-tree ownership is best effort and is not an OS sandbox.
pub async fn execute_host_process(
    spec: HostProcessSpec,
) -> Result<HostProcessOutput, SandboxError> {
    spec.output_limits.validate()?;
    if spec.timeout.is_zero() {
        return Err(execution_error("timeout must be greater than zero"));
    }
    if !spec.executable.is_absolute() || !spec.cwd.is_absolute() {
        return Err(execution_error(
            "executable and working directory must be absolute canonical paths",
        ));
    }

    let mut command = Command::new(&spec.executable);
    command
        .args(&spec.args)
        .current_dir(&spec.cwd)
        .env_clear()
        .envs(&spec.environment)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    configure_process_group(&mut command);

    let mut child = command.spawn().map_err(|error| SandboxError::Execution {
        program: "configured capability executable".to_owned(),
        message: error.to_string(),
    })?;
    let mut supervisor = ProcessSupervisor::attach(&child)?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| execution_error("stdout pipe was not created"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| execution_error("stderr pipe was not created"))?;

    let budget = Arc::new(Mutex::new(CombinedBudget {
        retained: 0,
        limit: spec.output_limits.combined_bytes,
    }));
    let (overflow_tx, mut overflow_rx) = mpsc::channel(2);
    let stdout_task = tokio::spawn(read_bounded(
        stdout,
        spec.output_limits.stdout_bytes,
        Arc::clone(&budget),
        OutputOverflow::Stdout,
        overflow_tx.clone(),
    ));
    let stderr_task = tokio::spawn(read_bounded(
        stderr,
        spec.output_limits.stderr_bytes,
        budget,
        OutputOverflow::Stderr,
        overflow_tx,
    ));

    let deadline = time::sleep(spec.timeout);
    tokio::pin!(deadline);
    let completion = tokio::select! {
        status = child.wait() => Completion::Exited(status),
        () = &mut deadline => Completion::TimedOut,
        Some(overflow) = overflow_rx.recv() => Completion::Overflow(overflow),
    };

    let (mut exit_code, timed_out, mut overflow, mut termination_attempted) = match completion {
        Completion::Exited(status) => {
            let status = status.map_err(|error| execution_error(error.to_string()))?;
            (status.code(), false, None, false)
        }
        Completion::TimedOut => {
            terminate_and_reap(&mut child, &mut supervisor).await?;
            (None, true, None, true)
        }
        Completion::Overflow(kind) => {
            terminate_and_reap(&mut child, &mut supervisor).await?;
            (None, false, Some(kind), true)
        }
    };

    let stdout = join_reader(stdout_task).await?;
    let stderr = join_reader(stderr_task).await?;
    if overflow.is_none() {
        overflow = stdout.overflow.or(stderr.overflow);
        if overflow.is_some() {
            supervisor.terminate()?;
            exit_code = None;
            termination_attempted = true;
        }
    }
    supervisor.disarm();

    Ok(HostProcessOutput {
        stdout: stdout.bytes,
        stderr: stderr.bytes,
        exit_code,
        timed_out,
        overflow,
        termination_attempted,
    })
}

enum Completion {
    Exited(std::io::Result<std::process::ExitStatus>),
    TimedOut,
    Overflow(OutputOverflow),
}

struct CombinedBudget {
    retained: usize,
    limit: usize,
}

async fn read_bounded(
    mut reader: impl AsyncRead + Unpin,
    stream_limit: usize,
    budget: Arc<Mutex<CombinedBudget>>,
    stream_kind: OutputOverflow,
    overflow_tx: mpsc::Sender<OutputOverflow>,
) -> Result<ReadResult, std::io::Error> {
    let mut retained = Vec::with_capacity(stream_limit.min(PIPE_CHUNK_BYTES));
    let mut chunk = [0_u8; PIPE_CHUNK_BYTES];
    loop {
        let count = reader.read(&mut chunk).await?;
        if count == 0 {
            return Ok(ReadResult {
                bytes: retained,
                overflow: None,
            });
        }

        let stream_remaining = stream_limit.saturating_sub(retained.len());
        let (accepted, combined_exhausted) = {
            let mut budget = budget
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let combined_remaining = budget.limit.saturating_sub(budget.retained);
            let accepted = count.min(stream_remaining).min(combined_remaining);
            budget.retained += accepted;
            (
                accepted,
                accepted < count && combined_remaining <= stream_remaining,
            )
        };
        retained.extend_from_slice(&chunk[..accepted]);

        if accepted < count {
            let overflow = if combined_exhausted {
                OutputOverflow::Combined
            } else {
                stream_kind
            };
            let _ = overflow_tx.send(overflow).await;
            return Ok(ReadResult {
                bytes: retained,
                overflow: Some(overflow),
            });
        }
    }
}

struct ReadResult {
    bytes: Vec<u8>,
    overflow: Option<OutputOverflow>,
}

async fn join_reader(
    mut task: JoinHandle<Result<ReadResult, std::io::Error>>,
) -> Result<ReadResult, SandboxError> {
    match time::timeout(TERMINATION_GRACE, &mut task).await {
        Ok(Ok(Ok(bytes))) => Ok(bytes),
        Ok(Ok(Err(error))) => Err(execution_error(error.to_string())),
        Ok(Err(error)) => Err(execution_error(format!("output reader failed: {error}"))),
        Err(_) => {
            task.abort();
            Err(execution_error(
                "output pipe did not close after termination",
            ))
        }
    }
}

async fn terminate_and_reap(
    child: &mut Child,
    supervisor: &mut ProcessSupervisor,
) -> Result<(), SandboxError> {
    supervisor.terminate()?;
    let _ = child.start_kill();
    time::timeout(TERMINATION_GRACE, child.wait())
        .await
        .map_err(|_| execution_error("process did not exit after termination attempt"))?
        .map_err(|error| execution_error(error.to_string()))?;
    Ok(())
}

fn execution_error(message: impl Into<String>) -> SandboxError {
    SandboxError::Execution {
        program: "configured capability executable".to_owned(),
        message: message.into(),
    }
}

#[cfg(windows)]
fn configure_process_group(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    use windows_sys::Win32::System::Threading::CREATE_NO_WINDOW;

    command.as_std_mut().creation_flags(CREATE_NO_WINDOW);
}

#[cfg(unix)]
fn configure_process_group(command: &mut Command) {
    use std::os::unix::process::CommandExt;

    command.as_std_mut().process_group(0);
}

#[cfg(not(any(unix, windows)))]
fn configure_process_group(_command: &mut Command) {}

#[cfg(windows)]
struct ProcessSupervisor {
    job: Option<std::os::windows::io::OwnedHandle>,
}

#[cfg(windows)]
impl ProcessSupervisor {
    fn attach(child: &Child) -> Result<Self, SandboxError> {
        use std::mem::size_of;
        use std::os::windows::io::{AsRawHandle, FromRawHandle};
        use windows_sys::Win32::System::JobObjects::{
            AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
            SetInformationJobObject,
        };

        let raw_job = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
        if raw_job.is_null() {
            return Err(execution_error(std::io::Error::last_os_error().to_string()));
        }
        let job = unsafe { std::os::windows::io::OwnedHandle::from_raw_handle(raw_job.cast()) };
        let mut information = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        information.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let configured = unsafe {
            SetInformationJobObject(
                job.as_raw_handle().cast(),
                JobObjectExtendedLimitInformation,
                (&raw const information).cast(),
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        };
        if configured == 0 {
            return Err(execution_error(std::io::Error::last_os_error().to_string()));
        }
        let child_handle = child
            .raw_handle()
            .ok_or_else(|| execution_error("spawned process has no process handle"))?;
        let assigned =
            unsafe { AssignProcessToJobObject(job.as_raw_handle().cast(), child_handle.cast()) };
        if assigned == 0 {
            return Err(execution_error(std::io::Error::last_os_error().to_string()));
        }
        Ok(Self { job: Some(job) })
    }

    fn terminate(&mut self) -> Result<(), SandboxError> {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::System::JobObjects::TerminateJobObject;

        let Some(job) = &self.job else {
            return Ok(());
        };
        let terminated = unsafe { TerminateJobObject(job.as_raw_handle().cast(), 1) };
        if terminated == 0 {
            return Err(execution_error(std::io::Error::last_os_error().to_string()));
        }
        Ok(())
    }

    fn disarm(&mut self) {
        self.job.take();
    }
}

#[cfg(unix)]
struct ProcessSupervisor {
    process_group: Option<i32>,
}

#[cfg(unix)]
impl ProcessSupervisor {
    fn attach(child: &Child) -> Result<Self, SandboxError> {
        let process_group = child
            .id()
            .ok_or_else(|| execution_error("spawned process has no process identifier"))?;
        Ok(Self {
            process_group: Some(process_group as i32),
        })
    }

    fn terminate(&mut self) -> Result<(), SandboxError> {
        let Some(process_group) = self.process_group else {
            return Ok(());
        };
        let result = unsafe { libc::kill(-process_group, libc::SIGKILL) };
        if result == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH) {
            Ok(())
        } else {
            Err(execution_error(std::io::Error::last_os_error().to_string()))
        }
    }

    fn disarm(&mut self) {
        self.process_group.take();
    }
}

#[cfg(not(any(unix, windows)))]
struct ProcessSupervisor;

#[cfg(not(any(unix, windows)))]
impl ProcessSupervisor {
    fn attach(_child: &Child) -> Result<Self, SandboxError> {
        Ok(Self)
    }

    fn terminate(&mut self) -> Result<(), SandboxError> {
        Ok(())
    }

    fn disarm(&mut self) {}
}

impl Drop for ProcessSupervisor {
    fn drop(&mut self) {
        let _ = self.terminate();
    }
}
