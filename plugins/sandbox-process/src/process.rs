//! A started program as supervision sees it: its pipes, its exit, and how to
//! kill it. Programs are started by tokio, or on Windows by `sandbox-os`
//! itself (std cannot start a process with another token).

use std::io;

use tokio::io::{AsyncRead, AsyncWrite};

pub(crate) type Reader = Box<dyn AsyncRead + Send + Unpin>;
pub(crate) type Writer = Box<dyn AsyncWrite + Send + Unpin>;

pub(crate) struct Process {
    pub(crate) stdin: Option<Writer>,
    pub(crate) stdout: Option<Reader>,
    pub(crate) stderr: Option<Reader>,
    child: Child,
}

enum Child {
    Tokio(Box<tokio::process::Child>),
    #[cfg(windows)]
    Native(crate::os::windows::NativeChild),
}

impl Process {
    pub(crate) fn from_tokio(mut child: tokio::process::Child) -> Self {
        Self {
            stdin: child.stdin.take().map(|pipe| Box::new(pipe) as Writer),
            stdout: child.stdout.take().map(|pipe| Box::new(pipe) as Reader),
            stderr: child.stderr.take().map(|pipe| Box::new(pipe) as Reader),
            child: Child::Tokio(Box::new(child)),
        }
    }

    #[cfg(windows)]
    pub(crate) fn native(
        child: crate::os::windows::NativeChild,
        stdin: Option<Writer>,
        stdout: Reader,
        stderr: Reader,
    ) -> Self {
        Self {
            stdin,
            stdout: Some(stdout),
            stderr: Some(stderr),
            child: Child::Native(child),
        }
    }

    /// Waits for the program to exit; its exit code, if it has one.
    pub(crate) async fn wait(&mut self) -> io::Result<Option<i32>> {
        match &mut self.child {
            Child::Tokio(child) => Ok(child.wait().await?.code()),
            #[cfg(windows)]
            Child::Native(child) => child.wait().await.map(Some),
        }
    }

    pub(crate) fn start_kill(&mut self) -> io::Result<()> {
        match &mut self.child {
            Child::Tokio(child) => child.start_kill(),
            #[cfg(windows)]
            Child::Native(child) => child.kill(),
        }
    }

    /// Lets a program started suspended run, once its process tree is owned.
    pub(crate) fn resume(&mut self) -> io::Result<()> {
        match &mut self.child {
            Child::Tokio(_) => Ok(()),
            #[cfg(windows)]
            Child::Native(child) => child.resume(),
        }
    }

    #[cfg(windows)]
    pub(crate) fn raw_handle(&self) -> Option<std::os::windows::io::RawHandle> {
        match &self.child {
            Child::Tokio(child) => child.raw_handle(),
            Child::Native(child) => Some(child.raw_handle()),
        }
    }

    #[cfg(unix)]
    pub(crate) fn id(&self) -> Option<u32> {
        match &self.child {
            Child::Tokio(child) => child.id(),
        }
    }
}
