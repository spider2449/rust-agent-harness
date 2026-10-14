//! `sandbox-os`: the `sandbox` service with OS-level confinement.
//!
//! Programs run under the same supervision as `sandbox-process` (fixed
//! argument vector, bounded working directory, cleared environment, bounded
//! output, timeout, process-tree termination). On top of that, every program
//! and its descendants are confined by the kernel before `exec`:
//!
//! - **Files (landlock).** Paths in `readable` (default `/`) can be read and
//!   executed; the root, a private temporary directory (`TMPDIR`) and the
//!   paths in `writable` can also be written. Everything else is read-only or
//!   inaccessible. `/dev/null`, `/dev/zero` and `/dev/full` stay writable and
//!   `/dev/random` and `/dev/urandom` readable.
//! - **Network (seccomp).** Unless `network` is `true`, creating any socket
//!   other than a Unix-domain socket fails, `io_uring` is unavailable, and
//!   on landlock ABI 6 or later abstract Unix sockets outside the sandbox
//!   cannot be reached. This also blocks loopback.
//! - **Signals.** On landlock ABI 6 or later a program cannot signal
//!   processes outside its sandbox.
//!
//! Requirements: Linux with landlock ABI 3 or later (Linux 6.2) and seccomp
//! filters; with `network = false`, an x86-64 or AArch64 host. When a
//! requirement is missing the plugin fails to start; it never falls back to
//! plain supervision. Other platforms are not supported yet, and the plugin
//! fails to start there too.
//!
//! Configuration: the `sandbox-process` keys (`root`, `inherit_env`,
//! `max_timeout_ms`) plus `readable` and `writable` (paths; relative ones are
//! relative to `root`; each must exist) and `network` (default `false`).

use rah_api::{Plugin, PluginError, PluginId, PluginManifest, Scope, async_trait};

#[cfg(target_os = "linux")]
pub(crate) mod linux;

#[cfg(target_os = "linux")]
pub(crate) use linux::Confinement;

/// No confinement exists on this platform; `sandbox-os` refuses to start.
#[cfg(not(target_os = "linux"))]
pub(crate) enum Confinement {}

#[cfg(not(target_os = "linux"))]
impl Confinement {
    pub(crate) fn configure(&self, _command: &mut tokio::process::Command) {
        match *self {}
    }
}

pub struct SandboxOsPlugin {
    manifest: PluginManifest,
}

impl SandboxOsPlugin {
    pub fn new(id: impl Into<PluginId>) -> Self {
        Self {
            manifest: PluginManifest::new(id, env!("CARGO_PKG_VERSION")).provides(["sandbox"]),
        }
    }
}

#[async_trait]
impl Plugin for SandboxOsPlugin {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    #[cfg(not(target_os = "linux"))]
    async fn apply(&self, _scope: &mut Scope) -> Result<(), PluginError> {
        Err(
            "sandbox-os supports only Linux (landlock and seccomp) so far; \
             `sandbox-process` runs programs without isolation"
                .into(),
        )
    }

    #[cfg(target_os = "linux")]
    async fn apply(&self, scope: &mut Scope) -> Result<(), PluginError> {
        linux::apply(scope).await
    }
}
