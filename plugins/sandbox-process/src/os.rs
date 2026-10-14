//! `sandbox-os`: the `sandbox` service with OS-level confinement.
//!
//! Programs run under the same supervision as `sandbox-process` (fixed
//! argument vector, bounded working directory, cleared environment, bounded
//! output, timeout, process-tree termination). On top of that, every program
//! and its descendants are confined by the operating system.
//!
//! **Linux** (landlock and seccomp, applied before `exec`):
//!
//! - **Files.** Paths in `readable` (default `/`) can be read and executed;
//!   the root, a private temporary directory (`TMPDIR`) and the paths in
//!   `writable` can also be written. Everything else is read-only or
//!   inaccessible. `/dev/null`, `/dev/zero` and `/dev/full` stay writable and
//!   `/dev/random` and `/dev/urandom` readable.
//! - **Network.** Unless `network` is `true`, creating any socket other than
//!   a Unix-domain socket fails, `io_uring` is unavailable, and on landlock
//!   ABI 6 or later abstract Unix sockets outside the sandbox cannot be
//!   reached. This also blocks loopback.
//! - **Signals.** On landlock ABI 6 or later a program cannot signal
//!   processes outside its sandbox.
//!
//! Requirements: landlock ABI 3 or later (Linux 6.2) and seccomp filters;
//! with `network = false`, an x86-64 or AArch64 host.
//!
//! **Windows** (a Low integrity token, ADR 0043): programs run at Low
//! integrity without privileges, with Administrators as a deny-only group,
//! so they cannot write anything that is not labeled Low. The root, a
//! private temporary directory (`TEMP` and `TMP`) and the paths in
//! `writable` are labeled Low. **The label stays on these directories** (it
//! is inherited by everything in them, and new files inherit it), so a
//! later mount does not have to label the tree again. Reads and the network
//! are not restricted: `readable` is refused, and `network` must be set to
//! `true` to acknowledge that.
//!
//! When a requirement is missing the plugin fails to start; it never falls
//! back to plain supervision. Other platforms are not supported, and the
//! plugin fails to start there too.
//!
//! Configuration: the `sandbox-process` keys (`root`, `inherit_env`,
//! `max_timeout_ms`) plus `readable` and `writable` (paths; relative ones are
//! relative to `root`; each must exist) and `network` (default `false`).

#[cfg(any(target_os = "linux", windows))]
use std::path::{Path, PathBuf};

use rah_api::{Plugin, PluginError, PluginId, PluginManifest, Scope, async_trait};

#[cfg(target_os = "linux")]
pub(crate) mod linux;

#[cfg(target_os = "linux")]
pub(crate) use linux::Confinement;

#[cfg(windows)]
pub(crate) mod windows;

#[cfg(windows)]
pub(crate) use windows::Confinement;

/// No confinement exists on this platform; `sandbox-os` refuses to start.
#[cfg(not(any(target_os = "linux", windows)))]
pub(crate) enum Confinement {}

#[cfg(not(any(target_os = "linux", windows)))]
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

    #[cfg(not(any(target_os = "linux", windows)))]
    async fn apply(&self, _scope: &mut Scope) -> Result<(), PluginError> {
        Err("sandbox-os supports only Linux and Windows; \
             `sandbox-process` runs programs without isolation"
            .into())
    }

    #[cfg(target_os = "linux")]
    async fn apply(&self, scope: &mut Scope) -> Result<(), PluginError> {
        linux::apply(scope).await
    }

    #[cfg(windows)]
    async fn apply(&self, scope: &mut Scope) -> Result<(), PluginError> {
        windows::apply(scope).await
    }
}

/// Canonical paths; relative ones are relative to `root`. Every path must
/// exist, so a typo cannot silently grant nothing.
#[cfg(any(target_os = "linux", windows))]
async fn resolve(root: &Path, paths: Vec<PathBuf>, key: &str) -> Result<Vec<PathBuf>, String> {
    let mut resolved = Vec::with_capacity(paths.len());
    for path in paths {
        let canonical = tokio::fs::canonicalize(root.join(&path))
            .await
            .map_err(|error| format!("{key}: `{}`: {error}", path.display()))?;
        resolved.push(canonical);
    }
    Ok(resolved)
}

impl SandboxOsPlugin {
    /// An example configuration, mostly the defaults, for reference (the Web
    /// UI shows it in an empty configuration editor). It is never applied.
    #[cfg(windows)]
    pub const CONFIG_EXAMPLE: &str = r#"{
  "root": ".",
  "writable": [],
  "network": true,
  "max_timeout_ms": 600000
}"#;
    /// An example configuration, mostly the defaults, for reference (the Web
    /// UI shows it in an empty configuration editor). It is never applied.
    #[cfg(not(windows))]
    pub const CONFIG_EXAMPLE: &str = r#"{
  "root": ".",
  "readable": [
    "/"
  ],
  "writable": [],
  "network": false,
  "max_timeout_ms": 600000
}"#;
}
