//! The `distribution` plugin: provides the `admin` service, which mounts and
//! manages catalog plugins at runtime (for frontends such as `frontend-web`).

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, PoisonError},
};

use rah_api::{
    KernelHandle, MountOptions, Plugin, PluginError, PluginId, PluginManifest, PluginStatus, Scope,
    admin::{ADMIN, AdminError, MountRequest, PluginAdmin, PluginInstance, Workspace},
    async_trait,
};
use rah_plugin_host_process::ProcessPlugin;
use rah_plugin_host_wasm::WasmPlugin;
use serde_json::Value;

use crate::{
    PROCESS_PLUGIN, WASM_PLUGIN, WORKSPACE_PLUGINS, catalog, config_example, resolve_data_paths,
};

/// The id `rah` mounts the distribution plugin under.
pub const DISTRIBUTION_PLUGIN: &str = "distribution";

#[derive(Debug)]
pub(crate) enum BuildError {
    UnknownKind(String),
    Process(String),
}

/// Builds a plugin of `kind`. Process plugins are started once to read their
/// manifest.
pub(crate) async fn construct(
    kind: &str,
    id: PluginId,
    config: &Value,
) -> Result<Arc<dyn Plugin>, BuildError> {
    if kind == WASM_PLUGIN {
        let plugin = WasmPlugin::load(id, config)
            .await
            .map_err(BuildError::Process)?;
        return Ok(Arc::new(plugin));
    }
    if kind == PROCESS_PLUGIN {
        let plugin = ProcessPlugin::describe(id, config)
            .await
            .map_err(BuildError::Process)?;
        return Ok(Arc::new(plugin));
    }
    let constructor = *catalog()
        .get(kind)
        .ok_or_else(|| BuildError::UnknownKind(kind.to_owned()))?;
    Ok(constructor(id))
}

/// Kind and configuration of the plugins `rah` mounted.
#[derive(Default)]
pub struct Registry(Mutex<BTreeMap<PluginId, Record>>);

/// How a plugin was mounted, enough to mount it again.
#[derive(Clone)]
struct Record {
    kind: String,
    config: Value,
    bind: BTreeMap<String, String>,
    /// Unmounted by `stop`; `start` mounts it again.
    stopped: bool,
}

impl Registry {
    fn lock(&self) -> std::sync::MutexGuard<'_, BTreeMap<PluginId, Record>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub(crate) fn record(
        &self,
        id: PluginId,
        kind: &str,
        config: Value,
        bind: BTreeMap<String, String>,
    ) {
        let record = Record {
            kind: kind.to_owned(),
            config,
            bind,
            stopped: false,
        };
        self.lock().insert(id, record);
    }

    fn is_stopped(&self, id: &PluginId) -> bool {
        self.lock().get(id).is_some_and(|record| record.stopped)
    }
}

/// Kinds that guard tool calls. They cannot be stopped: tool calls would
/// run unguarded meanwhile.
const POLICY_KINDS: [&str; 1] = ["policy-authority"];

pub struct DistributionPlugin {
    manifest: PluginManifest,
    registry: Arc<Registry>,
    data_home: Option<PathBuf>,
}

impl DistributionPlugin {
    pub fn new(registry: Arc<Registry>) -> Self {
        Self {
            manifest: PluginManifest::new(DISTRIBUTION_PLUGIN, env!("CARGO_PKG_VERSION"))
                .provides(["admin"]),
            registry,
            data_home: None,
        }
    }

    /// Resolves the data paths of the plugins it mounts or configures
    /// against `data_home` (see [`crate::Distribution::data_home`]).
    pub fn with_data_home(mut self, data_home: Option<PathBuf>) -> Self {
        self.data_home = data_home;
        self
    }
}

#[async_trait]
impl Plugin for DistributionPlugin {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    async fn apply(&self, scope: &mut Scope) -> Result<(), PluginError> {
        let admin = Admin {
            kernel: scope.kernel(),
            registry: self.registry.clone(),
            own: scope.id().clone(),
            data_home: self.data_home.clone(),
        };
        scope.provide(ADMIN, Arc::new(admin) as Arc<dyn PluginAdmin>)?;
        Ok(())
    }
}

struct Admin {
    kernel: KernelHandle,
    registry: Arc<Registry>,
    own: PluginId,
    data_home: Option<PathBuf>,
}

fn failed(error: impl std::fmt::Display) -> AdminError {
    AdminError::Failed(error.to_string())
}

impl Admin {
    fn unlocked(&self, id: &PluginId) -> Result<(), AdminError> {
        if *id == self.own {
            return Err(AdminError::Locked(id.clone()));
        }
        if self.registry.is_stopped(id) {
            return Err(AdminError::Stopped(id.clone()));
        }
        if self.kernel.status(id).is_none() {
            return Err(AdminError::NotMounted(id.clone()));
        }
        Ok(())
    }

    fn resolve(&self, kind: &str, config: &mut Value) {
        if let Some(home) = &self.data_home {
            resolve_data_paths(kind, config, home);
        }
    }

    /// The requested id, or the kind, or the kind with the first free
    /// numeric suffix when `pick_free`. A stopped plugin keeps its id.
    fn new_id(&self, request: &MountRequest, pick_free: bool) -> Result<PluginId, AdminError> {
        let base = request.id.clone().unwrap_or_else(|| request.kind.clone());
        let free = |id: &str| {
            let id = PluginId::new(id);
            self.kernel.status(&id).is_none() && !self.registry.is_stopped(&id)
        };
        if free(&base) {
            return Ok(PluginId::new(base));
        }
        if !pick_free || request.id.is_some() {
            return Err(AdminError::AlreadyMounted(PluginId::new(base)));
        }
        (2..)
            .map(|n| format!("{base}-{n}"))
            .find(|id| free(id))
            .map(PluginId::new)
            .ok_or_else(|| failed("no free plugin id"))
    }

    async fn mount_as(
        &self,
        id: PluginId,
        mut request: MountRequest,
    ) -> Result<PluginStatus, AdminError> {
        self.resolve(&request.kind, &mut request.config);
        if (request.kind == PROCESS_PLUGIN || request.kind == WASM_PLUGIN) && request.id.is_none() {
            return Err(failed(format!("a `{}` plugin needs an `id`", request.kind)));
        }
        let plugin = construct(&request.kind, id.clone(), &request.config)
            .await
            .map_err(|error| match error {
                BuildError::UnknownKind(kind) => AdminError::UnknownKind(kind),
                BuildError::Process(message) => AdminError::Failed(message),
            })?;
        let mut options = MountOptions::with_config(request.config.clone());
        for (service, instance) in &request.bind {
            options = options.bind(service.as_str(), instance.clone());
        }
        let status = self.kernel.mount(plugin, options).await.map_err(failed)?;
        self.registry
            .record(id, &request.kind, request.config, request.bind);
        Ok(status)
    }
}

#[async_trait]
impl PluginAdmin for Admin {
    fn kinds(&self) -> Vec<String> {
        let mut kinds: Vec<String> = catalog().keys().map(|kind| (*kind).to_owned()).collect();
        kinds.push(PROCESS_PLUGIN.to_owned());
        kinds.push(WASM_PLUGIN.to_owned());
        kinds.sort();
        kinds
    }

    fn config_example(&self, kind: &str) -> Option<Value> {
        serde_json::from_str(config_example(kind)?).ok()
    }

    fn instance(&self, id: &PluginId) -> Option<PluginInstance> {
        if *id == self.own {
            return Some(PluginInstance {
                kind: DISTRIBUTION_PLUGIN.to_owned(),
                config: Value::Null,
                locked: true,
                stopped: false,
            });
        }
        self.registry.lock().get(id).map(|record| PluginInstance {
            kind: record.kind.clone(),
            config: record.config.clone(),
            locked: false,
            stopped: record.stopped,
        })
    }

    async fn mount(&self, request: MountRequest) -> Result<(PluginId, PluginStatus), AdminError> {
        let id = self.new_id(&request, false)?;
        let status = self.mount_as(id.clone(), request).await?;
        Ok((id, status))
    }

    async fn unmount(&self, id: &PluginId) -> Result<(), AdminError> {
        // A stopped plugin is no longer mounted; forget it.
        if self.registry.is_stopped(id) {
            self.registry.lock().remove(id);
            return Ok(());
        }
        self.unlocked(id)?;
        self.kernel.unmount(id).await.map_err(failed)?;
        self.registry.lock().remove(id);
        Ok(())
    }

    async fn configure(
        &self,
        id: &PluginId,
        mut config: Value,
    ) -> Result<PluginStatus, AdminError> {
        self.unlocked(id)?;
        let kind = self
            .registry
            .lock()
            .get(id)
            .map(|record| record.kind.clone());
        if let Some(kind) = kind {
            self.resolve(&kind, &mut config);
        }
        let status = self
            .kernel
            .update_config(id, config.clone())
            .await
            .map_err(failed)?;
        if let Some(record) = self.registry.lock().get_mut(id) {
            record.config = config;
        }
        Ok(status)
    }

    async fn restart(&self, id: &PluginId) -> Result<PluginStatus, AdminError> {
        self.unlocked(id)?;
        self.kernel.restart(id).await.map_err(failed)
    }

    async fn stop(&self, id: &PluginId) -> Result<(), AdminError> {
        self.unlocked(id)?;
        let kind = self
            .registry
            .lock()
            .get(id)
            .map(|record| record.kind.clone());
        let Some(kind) = kind else {
            // Mounted, but not through this distribution: it could not be
            // started again.
            return Err(failed(format!(
                "plugin `{id}` was not mounted by this distribution and cannot be stopped"
            )));
        };
        if POLICY_KINDS.contains(&kind.as_str()) {
            return Err(failed(format!(
                "plugin `{id}` guards tool calls and cannot be stopped"
            )));
        }
        self.kernel.unmount(id).await.map_err(failed)?;
        if let Some(record) = self.registry.lock().get_mut(id) {
            record.stopped = true;
        }
        Ok(())
    }

    async fn start(&self, id: &PluginId) -> Result<PluginStatus, AdminError> {
        let record = self.registry.lock().get(id).cloned();
        let record = match record {
            Some(record) if record.stopped => record,
            Some(_) => return Err(AdminError::NotStopped(id.clone())),
            None => return Err(AdminError::NotMounted(id.clone())),
        };
        let request = MountRequest {
            kind: record.kind,
            id: Some(id.as_str().to_owned()),
            config: record.config,
            bind: record.bind,
        };
        // On failure the record stays stopped, so the start can be retried.
        self.mount_as(id.clone(), request).await
    }

    fn stopped(&self) -> Vec<PluginId> {
        self.registry
            .lock()
            .iter()
            .filter(|(_, record)| record.stopped)
            .map(|(id, _)| id.clone())
            .collect()
    }

    async fn replace(
        &self,
        old: &PluginId,
        replacement: MountRequest,
    ) -> Result<(PluginId, PluginStatus), AdminError> {
        self.unlocked(old)?;
        let id = self.new_id(&replacement, true)?;
        let status = self.mount_as(id.clone(), replacement).await?;
        if status != PluginStatus::Active {
            let _ = self.kernel.unmount(&id).await;
            self.registry.lock().remove(&id);
            return Err(AdminError::ReplacementFailed {
                old: old.clone(),
                id,
                status: format!("{status:?}"),
            });
        }
        self.kernel.unmount(old).await.map_err(failed)?;
        self.registry.lock().remove(old);
        Ok((id, status))
    }

    async fn set_workspace(&self, root: &Path) -> Result<Workspace, AdminError> {
        let canonical = tokio::fs::canonicalize(root)
            .await
            .map_err(|error| failed(format!("`{}`: {error}", root.display())))?;
        if !canonical.is_dir() {
            return Err(failed(format!("`{}` is not a directory", root.display())));
        }
        let root = plain_path(canonical);
        let value = Value::String(root.display().to_string());
        // Every mounted instance of a workspace kind, sandbox first.
        let targets: Vec<(PluginId, Record)> = WORKSPACE_PLUGINS
            .iter()
            .flat_map(|kind| {
                self.registry
                    .lock()
                    .iter()
                    .filter(|(_, record)| record.kind == *kind)
                    .map(|(id, record)| (id.clone(), record.clone()))
                    .collect::<Vec<_>>()
            })
            .collect();
        if targets.iter().all(|(_, record)| record.stopped) {
            return Err(failed("no plugin that works in a workspace is mounted"));
        }
        let mut plugins = Vec::new();
        for (id, record) in targets {
            let mut config = match record.config {
                Value::Object(map) => map,
                _ => serde_json::Map::new(),
            };
            config.insert("root".to_owned(), value.clone());
            if record.stopped {
                // Takes the new root when it is started again.
                if let Some(record) = self.registry.lock().get_mut(&id) {
                    record.config = Value::Object(config);
                }
                continue;
            }
            let status = self.configure(&id, Value::Object(config)).await?;
            plugins.push((id, status));
        }
        Ok(Workspace { root, plugins })
    }
}

/// A canonical path without Windows' verbatim `\\?\` prefix, which other
/// programs (Git among them) and people do not expect.
fn plain_path(path: PathBuf) -> PathBuf {
    let text = path.display().to_string();
    match text.strip_prefix(r"\\?\") {
        Some(rest) if !rest.starts_with("UNC\\") => PathBuf::from(rest),
        _ => path,
    }
}
