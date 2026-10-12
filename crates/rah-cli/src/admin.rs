//! The `distribution` plugin: provides the `admin` service, which mounts and
//! manages catalog plugins at runtime (for frontends such as `frontend-web`).

use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, PoisonError},
};

use rah_api::{
    KernelHandle, MountOptions, Plugin, PluginError, PluginId, PluginManifest, PluginStatus, Scope,
    admin::{ADMIN, AdminError, MountRequest, PluginAdmin, PluginInstance},
    async_trait,
};
use rah_plugin_host_process::ProcessPlugin;
use serde_json::Value;

use crate::{PROCESS_PLUGIN, catalog};

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
pub struct Registry(Mutex<BTreeMap<PluginId, (String, Value)>>);

impl Registry {
    fn lock(&self) -> std::sync::MutexGuard<'_, BTreeMap<PluginId, (String, Value)>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub(crate) fn record(&self, id: PluginId, kind: &str, config: Value) {
        self.lock().insert(id, (kind.to_owned(), config));
    }
}

pub struct DistributionPlugin {
    manifest: PluginManifest,
    registry: Arc<Registry>,
}

impl DistributionPlugin {
    pub fn new(registry: Arc<Registry>) -> Self {
        Self {
            manifest: PluginManifest::new(DISTRIBUTION_PLUGIN, env!("CARGO_PKG_VERSION"))
                .provides(["admin"]),
            registry,
        }
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
        };
        scope.provide(ADMIN, Arc::new(admin) as Arc<dyn PluginAdmin>)?;
        Ok(())
    }
}

struct Admin {
    kernel: KernelHandle,
    registry: Arc<Registry>,
    own: PluginId,
}

fn failed(error: impl std::fmt::Display) -> AdminError {
    AdminError::Failed(error.to_string())
}

impl Admin {
    fn unlocked(&self, id: &PluginId) -> Result<(), AdminError> {
        if *id == self.own {
            return Err(AdminError::Locked(id.clone()));
        }
        if self.kernel.status(id).is_none() {
            return Err(AdminError::NotMounted(id.clone()));
        }
        Ok(())
    }

    /// The requested id, or the kind, or the kind with the first free
    /// numeric suffix when `pick_free`.
    fn new_id(&self, request: &MountRequest, pick_free: bool) -> Result<PluginId, AdminError> {
        let base = request.id.clone().unwrap_or_else(|| request.kind.clone());
        let free = |id: &str| self.kernel.status(&PluginId::new(id)).is_none();
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
        request: MountRequest,
    ) -> Result<PluginStatus, AdminError> {
        if request.kind == PROCESS_PLUGIN && request.id.is_none() {
            return Err(failed("a `process` plugin needs an `id`"));
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
        self.registry.record(id, &request.kind, request.config);
        Ok(status)
    }
}

#[async_trait]
impl PluginAdmin for Admin {
    fn kinds(&self) -> Vec<String> {
        let mut kinds: Vec<String> = catalog().keys().map(|kind| (*kind).to_owned()).collect();
        kinds.push(PROCESS_PLUGIN.to_owned());
        kinds.sort();
        kinds
    }

    fn instance(&self, id: &PluginId) -> Option<PluginInstance> {
        if *id == self.own {
            return Some(PluginInstance {
                kind: DISTRIBUTION_PLUGIN.to_owned(),
                config: Value::Null,
                locked: true,
            });
        }
        self.registry
            .lock()
            .get(id)
            .map(|(kind, config)| PluginInstance {
                kind: kind.clone(),
                config: config.clone(),
                locked: false,
            })
    }

    async fn mount(&self, request: MountRequest) -> Result<(PluginId, PluginStatus), AdminError> {
        let id = self.new_id(&request, false)?;
        let status = self.mount_as(id.clone(), request).await?;
        Ok((id, status))
    }

    async fn unmount(&self, id: &PluginId) -> Result<(), AdminError> {
        self.unlocked(id)?;
        self.kernel.unmount(id).await.map_err(failed)?;
        self.registry.lock().remove(id);
        Ok(())
    }

    async fn configure(&self, id: &PluginId, config: Value) -> Result<PluginStatus, AdminError> {
        self.unlocked(id)?;
        let status = self
            .kernel
            .update_config(id, config.clone())
            .await
            .map_err(failed)?;
        if let Some(entry) = self.registry.lock().get_mut(id) {
            entry.1 = config;
        }
        Ok(status)
    }

    async fn restart(&self, id: &PluginId) -> Result<PluginStatus, AdminError> {
        self.unlocked(id)?;
        self.kernel.restart(id).await.map_err(failed)
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
}
