//! The Web UI's web search switch (`GET|PUT /api/web-search {enabled}`).
//!
//! Turning it on first makes every policy plugin ask before `web.*` calls
//! (`decisions = { "web.*" = "ask" }`, unless the policy already decides
//! `web.*`), then mounts `tools-web-search`, so its tools are never
//! reachable without that rule. Turning it off unmounts `tools-web-search`
//! and leaves the policy rule, which then matches no tool. The choice is
//! kept in the `storage` service, when one is mounted, under this plugin's
//! id, and applied again when the plugin starts.

use std::{sync::Arc, time::Duration};

use axum::{Json, extract::State};
use rah_api::{
    PluginId, PluginStatus,
    admin::{ADMIN, AdminError, MountRequest, PluginAdmin},
    storage::STORAGE,
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{ApiResult, AppState, Shared, manage::admin};

const KIND: &str = "tools-web-search";
const POLICY: &str = "policy-authority";
const PATTERN: &str = "web.*";
const KEY: &str = "web-search";

/// `GET /api/web-search`.
pub(crate) async fn get(State(shared): State<AppState>) -> ApiResult {
    Ok(Json(state(&shared)))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Switch {
    enabled: bool,
}

/// `PUT /api/web-search {enabled}`.
pub(crate) async fn set(State(shared): State<AppState>, Json(switch): Json<Switch>) -> ApiResult {
    let admin = admin(&shared)?;
    if switch.enabled {
        enable(&shared, admin.as_ref()).await?;
    } else {
        disable(&shared, admin.as_ref()).await?;
    }
    let mut reply = state(&shared);
    reply["remembered"] = json!(remember(&shared, switch.enabled).await);
    Ok(Json(reply))
}

/// Applies a stored "on" once this plugin is active, so a choice made in
/// the Web UI survives a restart of `rah`.
pub(crate) async fn restore(shared: Arc<Shared>) {
    let id = shared.id.clone();
    let Some(storage) = shared.kernel.get(STORAGE) else {
        return;
    };
    if !matches!(storage.get(id.as_str(), KEY).await, Ok(Some(value)) if value == b"on") {
        return;
    }
    // Mounting from inside this plugin's own start would race the boot.
    for _ in 0..200 {
        let active = shared
            .kernel
            .plugins()
            .into_iter()
            .any(|(plugin, status)| plugin == id && status == PluginStatus::Active);
        if active {
            if let Some(admin) = shared.kernel.get(ADMIN) {
                let _ = enable(&shared, admin.as_ref()).await;
            }
            return;
        }
        tokio::select! {
            () = shared.stop.cancelled() => return,
            () = tokio::time::sleep(Duration::from_millis(50)) => {}
        }
    }
}

/// Mounted (or stopped) plugins of `kind`, with their status; `None` for a
/// stopped one.
fn instances(
    shared: &Shared,
    admin: &dyn PluginAdmin,
    kind: &str,
) -> Vec<(PluginId, Option<PluginStatus>)> {
    let mounted = shared
        .kernel
        .plugins()
        .into_iter()
        .map(|(id, status)| (id, Some(status)));
    let stopped = admin.stopped().into_iter().map(|id| (id, None));
    mounted
        .chain(stopped)
        .filter(|(id, _)| admin.instance(id).is_some_and(|i| i.kind == kind))
        .collect()
}

fn state(shared: &Shared) -> Value {
    let Some(admin) = shared.kernel.get(ADMIN) else {
        return json!({ "available": false, "enabled": false });
    };
    let available = admin.kinds().iter().any(|kind| kind == KIND);
    let enabled = instances(shared, admin.as_ref(), KIND)
        .iter()
        .any(|(_, status)| *status == Some(PluginStatus::Active));
    json!({ "available": available, "enabled": enabled })
}

async fn enable(shared: &Shared, admin: &dyn PluginAdmin) -> Result<(), AdminError> {
    for (id, status) in instances(shared, admin, POLICY) {
        let Some(instance) = admin.instance(&id).filter(|_| status.is_some()) else {
            continue;
        };
        let mut config = match instance.config {
            Value::Object(map) => map,
            _ => serde_json::Map::new(),
        };
        let decisions = config.entry("decisions").or_insert_with(|| json!({}));
        if !decisions.is_object() {
            *decisions = json!({});
        }
        if let Some(decisions) = decisions.as_object_mut()
            && !decisions.contains_key(PATTERN)
        {
            decisions.insert(PATTERN.to_owned(), json!("ask"));
            let status = admin.configure(&id, Value::Object(config)).await?;
            if status != PluginStatus::Active {
                return Err(AdminError::Failed(format!(
                    "the policy `{id}` did not restart ({status:?}); web search stays off"
                )));
            }
        }
    }
    let existing = instances(shared, admin, KIND);
    if existing
        .iter()
        .any(|(_, status)| *status == Some(PluginStatus::Active))
    {
        return Ok(());
    }
    let status = match existing.iter().find(|(_, status)| status.is_none()) {
        Some((id, _)) => admin.start(id).await?,
        None => {
            let request = MountRequest {
                kind: KIND.to_owned(),
                ..MountRequest::default()
            };
            admin.mount(request).await?.1
        }
    };
    if status != PluginStatus::Active {
        return Err(AdminError::Failed(format!(
            "`{KIND}` did not start: {status:?}"
        )));
    }
    Ok(())
}

async fn disable(shared: &Shared, admin: &dyn PluginAdmin) -> Result<(), AdminError> {
    for (id, status) in instances(shared, admin, KIND) {
        if status.is_some() {
            admin.unmount(&id).await?;
        }
    }
    Ok(())
}

/// Stores the choice; returns whether it was stored.
async fn remember(shared: &Shared, enabled: bool) -> bool {
    let Some(storage) = shared.kernel.get(STORAGE) else {
        return false;
    };
    let value: &[u8] = if enabled { b"on" } else { b"off" };
    storage.put(shared.id.as_str(), KEY, value).await.is_ok()
}
