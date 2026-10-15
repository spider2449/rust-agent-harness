//! The `admin` service: plugin management for frontends.
//!
//! The distribution provides it, because only the distribution knows which
//! plugin kinds it can build. A frontend that exposes it must restrict it to
//! the local user; it is never reachable from model output.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use async_trait::async_trait;
use rah_kernel::{PluginId, PluginStatus, ServiceKey};
use serde_json::Value;

pub const ADMIN: ServiceKey<dyn PluginAdmin> = ServiceKey::new("admin");

/// A plugin to mount.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MountRequest {
    /// Catalog kind, for example `model-mock` or `process`.
    pub kind: String,
    /// Instance id; defaults to the kind.
    pub id: Option<String>,
    pub config: Value,
    /// Provider instances for dependencies: `{ service = "instance" }`.
    pub bind: BTreeMap<String, String>,
}

/// What one configuration key of a plugin kind does, for a frontend to show
/// next to its configuration editor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigKeyHelp {
    pub key: String,
    pub help: String,
}

/// What the distribution knows about a mounted plugin.
#[derive(Clone, Debug, PartialEq)]
pub struct PluginInstance {
    pub kind: String,
    pub config: Value,
    /// Whether the admin service refuses to change it (itself).
    pub locked: bool,
    /// Whether it is stopped (see [`PluginAdmin::stop`]).
    pub stopped: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum AdminError {
    #[error("unknown plugin kind `{0}`")]
    UnknownKind(String),
    #[error("plugin `{0}` is not mounted")]
    NotMounted(PluginId),
    #[error("plugin `{0}` is already mounted")]
    AlreadyMounted(PluginId),
    #[error("plugin `{0}` cannot be changed through the admin service")]
    Locked(PluginId),
    #[error("plugin `{0}` is stopped; start it first")]
    Stopped(PluginId),
    #[error("plugin `{0}` is not stopped")]
    NotStopped(PluginId),
    #[error("the replacement `{id}` did not start ({status}); `{old}` was kept")]
    ReplacementFailed {
        old: PluginId,
        id: PluginId,
        status: String,
    },
    #[error("{0}")]
    Failed(String),
}

#[async_trait]
pub trait PluginAdmin: Send + Sync {
    /// Kinds this distribution can mount, sorted.
    fn kinds(&self) -> Vec<String>;

    /// An example configuration for `kind`, mostly its defaults, for a
    /// frontend to show as a reference. It is never applied.
    fn config_example(&self, kind: &str) -> Option<Value> {
        let _ = kind;
        None
    }

    /// Help for the configuration keys of `kind`, in display order; empty
    /// when the distribution has none.
    fn config_help(&self, kind: &str) -> Vec<ConfigKeyHelp> {
        let _ = kind;
        Vec::new()
    }

    /// Kind and configuration of a mounted plugin, if this distribution
    /// mounted it.
    fn instance(&self, id: &PluginId) -> Option<PluginInstance>;

    /// Returns the id the plugin was mounted under and its status.
    async fn mount(&self, request: MountRequest) -> Result<(PluginId, PluginStatus), AdminError>;

    async fn unmount(&self, id: &PluginId) -> Result<(), AdminError>;

    /// Replaces the configuration and restarts the plugin and its
    /// dependents.
    async fn configure(&self, id: &PluginId, config: Value) -> Result<PluginStatus, AdminError>;

    async fn restart(&self, id: &PluginId) -> Result<PluginStatus, AdminError>;

    /// Stops a plugin and keeps its kind, configuration and bindings, so
    /// [`start`](Self::start) can mount it again under the same id. Its
    /// dependents wait as `Pending` meanwhile. A stopped plugin is not
    /// mounted: the kernel does not list it, but [`stopped`](Self::stopped)
    /// and [`instance`](Self::instance) do, and its id stays taken.
    /// Policy plugins cannot be stopped; stopping one would leave tool calls
    /// unguarded.
    async fn stop(&self, id: &PluginId) -> Result<(), AdminError>;

    /// Mounts a stopped plugin again. Returns its status.
    async fn start(&self, id: &PluginId) -> Result<PluginStatus, AdminError>;

    /// Ids of the stopped plugins, sorted.
    fn stopped(&self) -> Vec<PluginId>;

    /// Mounts `replacement` (under the first free id when its id is taken),
    /// and unmounts `old` only if the replacement became active. Consumers
    /// that resolve services per use (the standard loop and frontends)
    /// switch to it without restarting. Returns the replacement's id.
    async fn replace(
        &self,
        old: &PluginId,
        replacement: MountRequest,
    ) -> Result<(PluginId, PluginStatus), AdminError>;

    /// Points the plugins that work in a workspace directory at `root` (an
    /// existing directory) and restarts them with their dependents. Returns
    /// the canonical root and the plugins changed with their status. Only
    /// the distribution knows which plugins take a workspace root.
    async fn set_workspace(&self, root: &Path) -> Result<Workspace, AdminError> {
        let _ = root;
        Err(AdminError::Failed(
            "this distribution cannot switch workspaces".into(),
        ))
    }
}

/// The result of [`PluginAdmin::set_workspace`].
#[derive(Clone, Debug, PartialEq)]
pub struct Workspace {
    pub root: PathBuf,
    pub plugins: Vec<(PluginId, PluginStatus)>,
}
