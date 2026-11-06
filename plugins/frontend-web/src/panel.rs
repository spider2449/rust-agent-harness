//! Plugins panel changes that survive a restart.
//!
//! Every change made through `/api/plugins` that succeeds is kept in the
//! distribution's configuration file through the `admin` service's
//! `persist` (ADR 0067): a mount, a configuration or a replacement as the
//! plugin's configuration, an unmount as its removal. A change to the only
//! model plugin is a change of the model the distribution runs. Stopping and
//! starting a plugin is a runtime switch (modes do it too) and is not kept.
//!
//! The distribution refuses to remove a policy plugin or the model from the
//! file, so a restart never comes up unguarded or without a model. When it
//! cannot keep a change (no configuration file, or one that lists its
//! plugins), the change still holds until the next start, and the user is
//! told once per reason.

use rah_api::{
    PluginId,
    admin::{ADMIN, ConfigChange, MountRequest, PluginAdmin},
};
use serde_json::Value;

use crate::{Shared, settings::MODEL_KINDS};

const POLICY_KINDS: [&str; 1] = ["policy-authority"];

/// Keeps `change`; a refusal becomes a notice.
async fn keep(shared: &Shared, change: ConfigChange) {
    let Some(admin) = shared.kernel.get(ADMIN) else {
        return;
    };
    if let Err(error) = admin.persist(change).await {
        let reason = error.to_string();
        shared.notices.emit(
            &reason,
            &format!("a Plugins panel change holds until rah restarts, but is not kept: {reason}"),
        );
    }
}

/// Whether `id` is the only model plugin mounted, so the model the
/// distribution runs.
fn sole_model(shared: &Shared, admin: &dyn PluginAdmin, id: &PluginId) -> bool {
    let models: Vec<PluginId> = shared
        .kernel
        .plugins()
        .into_iter()
        .map(|(plugin, _)| plugin)
        .filter(|plugin| {
            admin
                .instance(plugin)
                .is_some_and(|instance| MODEL_KINDS.contains(&instance.kind.as_str()))
        })
        .collect();
    models.len() == 1 && models[0] == *id
}

/// The change that keeps `id` running as `kind` with `config`.
fn running(shared: &Shared, id: &PluginId, kind: &str, config: Value) -> ConfigChange {
    let model = MODEL_KINDS.contains(&kind)
        && shared
            .kernel
            .get(ADMIN)
            .is_some_and(|admin| sole_model(shared, admin.as_ref(), id));
    if model {
        ConfigChange::Model {
            kind: kind.to_owned(),
            config,
        }
    } else {
        ConfigChange::Plugin {
            id: id.clone(),
            kind: kind.to_owned(),
            config,
        }
    }
}

/// Keeps `id` as mounted with this kind and configuration.
pub(crate) async fn mounted(shared: &Shared, id: &PluginId, request: &MountRequest) {
    let change = running(shared, id, &request.kind, request.config.clone());
    keep(shared, change).await;
}

/// Keeps a new configuration for `id` of `kind`; a stopped plugin's is a
/// runtime state and is not kept.
pub(crate) async fn configured(
    shared: &Shared,
    id: &PluginId,
    kind: &str,
    config: Value,
    stopped: bool,
) {
    if stopped {
        return;
    }
    let change = running(shared, id, kind, config);
    keep(shared, change).await;
}

/// Keeps `id` (of `kind`) as removed; never a policy.
pub(crate) async fn removed(shared: &Shared, id: &PluginId, kind: Option<&str>) {
    if kind.is_some_and(|kind| POLICY_KINDS.contains(&kind)) {
        return;
    }
    keep(shared, ConfigChange::Removed { id: id.clone() }).await;
}

/// Keeps `old` (of `old_kind`) replaced by `id`. A model replacing the model
/// is one change of the model; anything else is a removal and a mount.
pub(crate) async fn replaced(
    shared: &Shared,
    old: &PluginId,
    old_kind: Option<&str>,
    id: &PluginId,
    request: &MountRequest,
) {
    let models = old_kind.is_some_and(|kind| MODEL_KINDS.contains(&kind))
        && MODEL_KINDS.contains(&request.kind.as_str());
    if !models {
        removed(shared, old, old_kind).await;
    }
    mounted(shared, id, request).await;
}
