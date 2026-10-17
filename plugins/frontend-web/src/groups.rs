//! API groups: every `tools-http` instance that no mode lists is one group,
//! a choice of the "Internal API" selector in a mode with `groups = true` (daily work). Pressing it
//! starts the instance, so the model can query that API; pressing it again
//! stops it (`GET /api/groups`, `PUT /api/group {id, on}`).
//!
//! At most one group is connected at a time, so the model never has to pick
//! between several APIs: connecting one disconnects the others. Groups are off
//! until pressed: at start every group is stopped except the one that was on
//! when the last run ended (kept in `storage`). A group is independent of the
//! active mode, so it stays connected when the mode is switched. An instance a mode lists in `plugins` is that mode's own group
//! (ADR 0047) and is not offered here.

use std::collections::BTreeSet;

use axum::{Json, extract::State, http::StatusCode};
use rah_api::{
    PluginId, PluginStatus,
    admin::{AdminError, PluginAdmin},
    storage::STORAGE,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{ApiError, ApiResult, AppState, Shared, manage::admin, modes};

const KIND: &str = "tools-http";
const KEY: &str = "groups";

#[derive(Serialize, PartialEq, Debug)]
struct Group {
    id: String,
    label: String,
    active: bool,
}

/// Mounted and stopped `tools-http` instances no mode lists, sorted by id.
fn groups(shared: &Shared, admin: &dyn PluginAdmin) -> Vec<Group> {
    let listed = shared.modes.listed_plugins();
    let mounted = shared
        .kernel
        .plugins()
        .into_iter()
        .map(|(id, status)| (id, status == PluginStatus::Active));
    let stopped = admin.stopped().into_iter().map(|id| (id, false));
    let mut found: Vec<Group> = mounted
        .chain(stopped)
        .filter(|(id, _)| {
            admin
                .instance(id)
                .is_some_and(|instance| instance.kind == KIND)
        })
        .filter(|(id, _)| !listed.contains(id.as_str()))
        .map(|(id, active)| Group {
            label: label(id.as_str()),
            id: id.as_str().to_owned(),
            active,
        })
        .collect();
    found.sort_by(|a, b| a.id.cmp(&b.id));
    found
}

/// `http-ddt` is shown as `ddt`.
fn label(id: &str) -> String {
    match id.strip_prefix("http-") {
        Some(rest) if !rest.is_empty() => rest.to_owned(),
        _ => id.to_owned(),
    }
}

/// `GET /api/groups`.
pub(crate) async fn list(State(shared): State<AppState>) -> ApiResult {
    let admin = admin(&shared)?;
    Ok(Json(json!({ "groups": groups(&shared, admin.as_ref()) })))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Switch {
    id: String,
    on: bool,
}

/// `PUT /api/group {id, on}`: connects or disconnects one group.
pub(crate) async fn set(State(shared): State<AppState>, Json(switch): Json<Switch>) -> ApiResult {
    let admin = admin(&shared)?;
    if !groups(&shared, admin.as_ref())
        .iter()
        .any(|group| group.id == switch.id)
    {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            format!("unknown API group `{}`", switch.id),
        ));
    }
    if switch.on {
        // Disconnect the others first, so two APIs are never offered at once.
        for other in groups(&shared, admin.as_ref())
            .iter()
            .filter(|group| group.active && group.id != switch.id)
        {
            modes::switch_kind(&shared, admin.as_ref(), &other.id, false)
                .await
                .map_err(|error: AdminError| ApiError::from(error))?;
        }
    }
    modes::switch_kind(&shared, admin.as_ref(), &switch.id, switch.on)
        .await
        .map_err(|error: AdminError| ApiError::from(error))?;
    let now = groups(&shared, admin.as_ref());
    remember(&shared, &now).await;
    shared.approvals.notify(json!({ "type": "groups" }));
    Ok(Json(json!({ "groups": now })))
}

/// What the Plugins panel did to a plugin.
pub(crate) enum Change {
    /// Mounted just now.
    Added,
    /// Configured or restarted; `was_active` before.
    Edited { was_active: bool },
    /// Started by hand.
    Started,
}

/// Keeps the panel in step with the selector, so a group never needs to be
/// started by hand: a group added or edited while disconnected stays
/// disconnected until chosen, and one started by hand replaces the connected
/// group.
pub(crate) async fn changed(
    shared: &Shared,
    admin: &dyn PluginAdmin,
    id: &PluginId,
    change: Change,
) {
    let all = groups(shared, admin);
    if !all.iter().any(|group| group.id == id.as_str()) {
        return;
    }
    let keep = match change {
        Change::Added | Change::Edited { was_active: false } => false,
        Change::Edited { was_active: true } | Change::Started => true,
    };
    if keep {
        for other in all
            .iter()
            .filter(|group| group.active && group.id != id.as_str())
        {
            let _ = modes::switch_kind(shared, admin, &other.id, false).await;
        }
    } else {
        let _ = modes::switch_kind(shared, admin, id.as_str(), false).await;
    }
    remember(shared, &groups(shared, admin)).await;
    shared.approvals.notify(json!({ "type": "groups" }));
}

/// Applies the stored choice at start: every group is stopped except the one
/// that was on (the first, should several be stored).
pub(crate) async fn restore(shared: &Shared, admin: &dyn PluginAdmin) {
    let stored: BTreeSet<String> = match shared.kernel.get(STORAGE) {
        Some(storage) => storage
            .get(shared.id.as_str(), KEY)
            .await
            .ok()
            .flatten()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default(),
        None => BTreeSet::new(),
    };
    let keep = groups(shared, admin)
        .into_iter()
        .find(|group| stored.contains(&group.id))
        .map(|group| group.id);
    for group in groups(shared, admin) {
        let on = keep.as_deref() == Some(group.id.as_str());
        if on != group.active {
            let _ = modes::switch_kind(shared, admin, &group.id, on).await;
        }
    }
}

async fn remember(shared: &Shared, groups: &[Group]) {
    let Some(storage) = shared.kernel.get(STORAGE) else {
        return;
    };
    let on: BTreeSet<&str> = groups
        .iter()
        .filter(|group| group.active)
        .map(|group| group.id.as_str())
        .collect();
    if let Ok(bytes) = serde_json::to_vec(&on) {
        let _ = storage.put(shared.id.as_str(), KEY, &bytes).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_http_prefix_is_not_shown() {
        assert_eq!(label("http-ddt"), "ddt");
        assert_eq!(label("tools-http"), "tools-http");
        assert_eq!(label("http-"), "http-");
    }
}
