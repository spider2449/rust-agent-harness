//! Plugins panel changes that survive a restart.
//!
//! Every change made through `/api/plugins` that succeeds (mount, configure,
//! stop, start, unmount, replace) records the desired state of the plugin
//! ids it touched in the `storage` service under this plugin's id (key
//! `plugins`). At start, after the Settings forms and before the web search
//! switch (see [`crate::settings::restore`]), the recorded states are applied
//! again: a plugin is mounted or configured (a configuration that does not
//! start is rolled back), stopped, or unmounted.
//!
//! A policy plugin is never recorded as stopped or removed, and never
//! stopped or removed at start, so a restart does not come up unguarded.
//! Saving a Settings form for a plugin drops its recorded panel change, so
//! the later of the two wins.

use std::collections::BTreeMap;

use rah_api::{
    PluginId, PluginStatus,
    admin::{MountRequest, PluginAdmin},
    storage::STORAGE,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::Shared;

const KEY: &str = "plugins";
/// The most plugin ids recorded; changes to further ids are not kept.
const MAX_ENTRIES: usize = 256;
const POLICY_KINDS: [&str; 1] = ["policy-authority"];

/// The desired state of one plugin id.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(tag = "state", rename_all = "lowercase", deny_unknown_fields)]
pub(crate) enum Desired {
    Mounted {
        kind: String,
        #[serde(default)]
        config: Value,
        #[serde(default)]
        bind: BTreeMap<String, String>,
    },
    Stopped {
        kind: String,
        #[serde(default)]
        config: Value,
        #[serde(default)]
        bind: BTreeMap<String, String>,
    },
    Removed,
}

type Entries = BTreeMap<String, Desired>;

async fn load(shared: &Shared) -> Option<Entries> {
    let storage = shared.kernel.get(STORAGE)?;
    match storage.get(shared.id.as_str(), KEY).await {
        Ok(Some(bytes)) => serde_json::from_slice(&bytes).ok(),
        Ok(None) => Some(Entries::new()),
        Err(_) => None,
    }
}

async fn store(shared: &Shared, entries: &Entries) {
    let (Some(storage), Ok(bytes)) = (shared.kernel.get(STORAGE), serde_json::to_vec(entries))
    else {
        return;
    };
    let _ = storage.put(shared.id.as_str(), KEY, &bytes).await;
}

/// Changes the recorded entries under the panel lock, so concurrent
/// requests do not lose each other's changes.
async fn change(shared: &Shared, edit: impl FnOnce(&mut Entries)) {
    let _guard = shared.panel.lock().await;
    let Some(mut entries) = load(shared).await else {
        return;
    };
    let before = entries.clone();
    edit(&mut entries);
    if entries.len() > MAX_ENTRIES {
        return;
    }
    if entries != before {
        store(shared, &entries).await;
    }
}

/// The bind a recorded entry has for `id`, kept across configure, stop and
/// start, which do not carry one.
fn bind_of(entries: &Entries, id: &str) -> BTreeMap<String, String> {
    match entries.get(id) {
        Some(Desired::Mounted { bind, .. } | Desired::Stopped { bind, .. }) => bind.clone(),
        _ => BTreeMap::new(),
    }
}

/// Records `id` as mounted with this kind, configuration and bind.
pub(crate) async fn mounted(shared: &Shared, id: &PluginId, request: &MountRequest) {
    let desired = Desired::Mounted {
        kind: request.kind.clone(),
        config: request.config.clone(),
        bind: request.bind.clone(),
    };
    change(shared, |entries| {
        entries.insert(id.as_str().to_owned(), desired);
    })
    .await;
}

/// Records a new configuration (`stopped`: the plugin was stopped, not
/// started) for `id` of `kind`.
pub(crate) async fn configured(
    shared: &Shared,
    id: &PluginId,
    kind: &str,
    config: Value,
    stopped: bool,
) {
    if stopped && POLICY_KINDS.contains(&kind) {
        return;
    }
    change(shared, |entries| {
        let bind = bind_of(entries, id.as_str());
        let kind = kind.to_owned();
        let desired = if stopped {
            Desired::Stopped { kind, config, bind }
        } else {
            Desired::Mounted { kind, config, bind }
        };
        entries.insert(id.as_str().to_owned(), desired);
    })
    .await;
}

/// Records `id` (of `kind`) as removed.
pub(crate) async fn removed(shared: &Shared, id: &PluginId, kind: Option<&str>) {
    if kind.is_some_and(|kind| POLICY_KINDS.contains(&kind)) {
        // Forgotten instead: the distribution's policy comes back at start.
        forget(shared, id).await;
        return;
    }
    change(shared, |entries| {
        entries.insert(id.as_str().to_owned(), Desired::Removed);
    })
    .await;
}

/// Drops the recorded change of `id`.
pub(crate) async fn forget(shared: &Shared, id: &PluginId) {
    change(shared, |entries| {
        entries.remove(id.as_str());
    })
    .await;
}

/// Applies the recorded changes again, at start.
pub(crate) async fn restore(shared: &Shared, admin: &dyn PluginAdmin) {
    let Some(entries) = load(shared).await else {
        return;
    };
    for (id, desired) in entries {
        let id = PluginId::new(id);
        if id == shared.id {
            continue;
        }
        let current = admin.instance(&id);
        let policy = current
            .as_ref()
            .is_some_and(|instance| POLICY_KINDS.contains(&instance.kind.as_str()));
        match desired {
            Desired::Removed => {
                if current.is_some() && !policy {
                    let _ = admin.unmount(&id).await;
                }
            }
            Desired::Mounted { kind, config, bind } => {
                ensure(admin, &id, kind, config, bind).await;
            }
            Desired::Stopped { kind, config, bind } => {
                if POLICY_KINDS.contains(&kind.as_str()) || policy {
                    continue;
                }
                if ensure(admin, &id, kind, config, bind).await {
                    let _ = admin.stop(&id).await;
                }
            }
        }
    }
}

/// Mounts `id`, or configures it when it is mounted with the same kind.
/// A configuration that does not start is rolled back. Returns whether the
/// plugin is mounted with the recorded kind afterwards.
async fn ensure(
    admin: &dyn PluginAdmin,
    id: &PluginId,
    kind: String,
    config: Value,
    bind: BTreeMap<String, String>,
) -> bool {
    let request = MountRequest {
        kind: kind.clone(),
        id: Some(id.as_str().to_owned()),
        config: config.clone(),
        bind,
    };
    let Some(instance) = admin.instance(id) else {
        return admin.mount(request).await.is_ok();
    };
    if instance.kind != kind {
        // Another kind under this id now: the distribution changed. Left as
        // it is, and never for a policy.
        return false;
    }
    if instance.stopped && admin.start(id).await.is_err() {
        return false;
    }
    let Some(previous) = admin.instance(id).map(|instance| instance.config) else {
        return false;
    };
    if previous == config {
        return true;
    }
    match admin.configure(id, config).await {
        Ok(PluginStatus::Active) => true,
        _ => {
            let _ = admin.configure(id, previous).await;
            true
        }
    }
}
