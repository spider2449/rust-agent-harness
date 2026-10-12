//! Supervision of long-running programs started with `ExecEnv::spawn`.

use std::{
    collections::VecDeque,
    sync::{Arc, Mutex, PoisonError},
};

use rah_api::{
    CancellationToken, async_trait,
    kernel::Spawner,
    sandbox::{ExecError, ProcessHandle, Spawned},
};
use tokio::{
    io::{AsyncRead, AsyncReadExt},
    process::Child,
    sync::watch,
    time,
};

use crate::supervise::{GRACE, ProcessTree};

/// `None` while running; `Some(exit code)` once ended.
type Exit = Option<Option<i32>>;

/// Hands the child to scope-owned supervision tasks and returns its handle.
pub(crate) fn start(
    mut child: Child,
    stderr_bytes: usize,
    spawner: &Spawner,
    stop: CancellationToken,
) -> Result<Spawned, ExecError> {
    let tree = match ProcessTree::attach(&child) {
        Ok(tree) => tree,
        Err(error) => {
            let _ = child.start_kill();
            return Err(ExecError::Supervision(error));
        }
    };
    let missing = || ExecError::Supervision("a pipe of the spawned program is missing".into());
    let stdin = child.stdin.take().ok_or_else(missing)?;
    let stdout = child.stdout.take().ok_or_else(missing)?;
    let stderr = child.stderr.take().ok_or_else(missing)?;
    let tail = Arc::new(Mutex::new(Tail::new(stderr_bytes)));
    let (exit, exited) = watch::channel::<Exit>(None);
    // If the scope is already disposed, the futures are dropped here, which
    // kills the child (`kill_on_drop`) and its tree (`ProcessTree::drop`).
    if !spawner.spawn(supervise(child, tree, stop.clone(), exit))
        || !spawner.spawn(drain(stderr, tail.clone()))
    {
        stop.cancel();
        return Err(ExecError::Supervision(
            "the sandbox is shutting down".into(),
        ));
    }
    Ok(Spawned {
        stdin: Box::new(stdin),
        stdout: Box::new(stdout),
        process: Box::new(SpawnedProcess { stop, exited, tail }),
    })
}

async fn supervise(
    mut child: Child,
    mut tree: ProcessTree,
    stop: CancellationToken,
    exit: watch::Sender<Exit>,
) {
    let code = tokio::select! {
        status = child.wait() => status.ok().and_then(|status| status.code()),
        () = stop.cancelled() => {
            let _ = tree.terminate();
            let _ = child.start_kill();
            let _ = time::timeout(GRACE, child.wait()).await;
            None
        }
    };
    // On Windows, closing the job also ends descendants left behind.
    tree.disarm();
    exit.send_replace(Some(code));
}

async fn drain(mut stderr: impl AsyncRead + Unpin, tail: Arc<Mutex<Tail>>) {
    let mut chunk = vec![0_u8; 8 * 1024];
    while let Ok(count @ 1..) = stderr.read(&mut chunk).await {
        tail.lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(&chunk[..count]);
    }
}

/// The most recent `limit` bytes of a stream.
struct Tail {
    bytes: VecDeque<u8>,
    limit: usize,
}

impl Tail {
    fn new(limit: usize) -> Self {
        Self {
            bytes: VecDeque::new(),
            limit,
        }
    }

    fn push(&mut self, chunk: &[u8]) {
        self.bytes.extend(chunk);
        let excess = self.bytes.len().saturating_sub(self.limit);
        self.bytes.drain(..excess);
    }
}

struct SpawnedProcess {
    stop: CancellationToken,
    exited: watch::Receiver<Exit>,
    tail: Arc<Mutex<Tail>>,
}

#[async_trait]
impl ProcessHandle for SpawnedProcess {
    async fn wait(&self) -> Option<i32> {
        let mut exited = self.exited.clone();
        match exited.wait_for(Option::is_some).await {
            Ok(exit) => exit.flatten(),
            // Supervision was dropped with the scope; the program is gone.
            Err(_) => None,
        }
    }

    fn terminate(&self) {
        self.stop.cancel();
    }

    fn stderr(&self) -> String {
        let tail = self.tail.lock().unwrap_or_else(PoisonError::into_inner);
        let bytes: Vec<u8> = tail.bytes.iter().copied().collect();
        String::from_utf8_lossy(&bytes).into_owned()
    }
}

impl Drop for SpawnedProcess {
    fn drop(&mut self) {
        self.stop.cancel();
    }
}
