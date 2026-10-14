//! Web UI settings that survive a restart: `PUT /api/settings/{slot}
//! {id, kind, config}` for the slots `model`, `assistant`, `tools` and
//! `loop`.
//!
//! A save applies the change through the `admin` service (configure when
//! the slot's plugin keeps its kind, replace when the kind changes, mount
//! when the slot is empty) and, once the plugin is active, stores the kind
//! and the whole configuration in the `storage` service under this plugin's
//! id (key `settings-<slot>`). A configuration that does not start is rolled
//! back, so a bad policy never leaves tool calls unguarded. At start the
//! stored settings are applied again (the policy first), then the web search
//! switch. A stored setting replaces the distribution's configuration of
//! that plugin as a whole. Changes made in the Plugins panel are applied
//! after them (see [`crate::panel`]).

use std::{collections::BTreeMap, sync::Arc, time::Duration};

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use rah_api::{
    PluginId, PluginStatus,
    admin::{ADMIN, AdminError, MountRequest, PluginAdmin},
    storage::STORAGE,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{ApiError, ApiResult, AppState, Shared, manage::admin, panel, status_json, web_search};

/// Slots and the plugin kinds each may hold, in the order they are restored.
const SLOTS: [(&str, &[&str]); 4] = [
    ("tools", &["policy-authority"]),
    (
        "model",
        &[
            "model-llamacpp",
            "model-openai",
            "model-anthropic",
            "model-chat-completions",
            "model-mock",
        ],
    ),
    ("assistant", &["context-default"]),
    ("loop", &["loop-react", "loop-plan"]),
];

#[derive(Deserialize, Serialize, PartialEq)]
struct Saved {
    kind: String,
    config: Value,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Save {
    /// The plugin the form showed, when there was one.
    id: Option<String>,
    kind: String,
    #[serde(default)]
    config: Value,
}

/// `PUT /api/settings/{slot}`.
pub(crate) async fn save(
    State(shared): State<AppState>,
    Path(slot): Path<String>,
    Json(save): Json<Save>,
) -> ApiResult {
    let bad = |message: String| ApiError(StatusCode::BAD_REQUEST, message);
    let kinds = SLOTS
        .iter()
        .find(|(name, _)| *name == slot)
        .map(|(_, kinds)| *kinds)
        .ok_or_else(|| bad(format!("unknown settings `{slot}`")))?;
    if !kinds.contains(&save.kind.as_str()) {
        return Err(bad(format!(
            "`{}` cannot hold the {slot} settings",
            save.kind
        )));
    }
    let admin = admin(&shared)?;
    let current = save
        .id
        .map(PluginId::new)
        .filter(|id| {
            admin
                .instance(id)
                .is_some_and(|instance| kinds.contains(&instance.kind.as_str()))
        })
        .or_else(|| current(&shared, admin.as_ref(), kinds));
    let saved = Saved {
        kind: save.kind,
        config: save.config,
    };
    let (id, status) = apply(admin.as_ref(), current.clone(), &saved).await?;
    let remembered = status == PluginStatus::Active && remember(&shared, &slot, &saved).await;
    if remembered {
        // The form now decides this plugin, not an earlier panel change.
        for id in current.iter().chain([&id]) {
            panel::forget(&shared, id).await;
        }
    }
    let mut reply = status_json(&status);
    reply["id"] = json!(id.as_str());
    reply["remembered"] = json!(remembered);
    Ok(Json(reply))
}

/// Applies the stored settings once this plugin is active.
pub(crate) async fn restore(shared: Arc<Shared>) {
    // Mounting from inside this plugin's own start would race the boot.
    let mut active = false;
    for _ in 0..200 {
        active = shared
            .kernel
            .plugins()
            .into_iter()
            .any(|(plugin, status)| plugin == shared.id && status == PluginStatus::Active);
        if active {
            break;
        }
        tokio::select! {
            () = shared.stop.cancelled() => return,
            () = tokio::time::sleep(Duration::from_millis(50)) => {}
        }
    }
    let (true, Some(admin)) = (active, shared.kernel.get(ADMIN)) else {
        return;
    };
    if let Some(storage) = shared.kernel.get(STORAGE) {
        for (slot, kinds) in SLOTS {
            let stored = storage.get(shared.id.as_str(), &key(slot)).await;
            let Some(saved) = stored
                .ok()
                .flatten()
                .and_then(|bytes| serde_json::from_slice::<Saved>(&bytes).ok())
                .filter(|saved| kinds.contains(&saved.kind.as_str()))
            else {
                continue;
            };
            let current = current(&shared, admin.as_ref(), kinds);
            let unchanged = current
                .as_ref()
                .and_then(|id| admin.instance(id))
                .is_some_and(|instance| {
                    instance.kind == saved.kind && instance.config == saved.config
                });
            if !unchanged {
                let _ = apply(admin.as_ref(), current, &saved).await;
            }
        }
    }
    panel::restore(&shared, admin.as_ref()).await;
    web_search::restore(&shared, admin.as_ref()).await;
}

fn key(slot: &str) -> String {
    format!("settings-{slot}")
}

/// The active plugin holding a slot, else any mounted one.
fn current(shared: &Shared, admin: &dyn PluginAdmin, kinds: &[&str]) -> Option<PluginId> {
    let mut candidates: Vec<(PluginId, Option<PluginStatus>)> = kinds
        .iter()
        .flat_map(|kind| web_search::instances(shared, admin, kind))
        .filter(|(_, status)| status.is_some())
        .collect();
    candidates.sort_by_key(|(_, status)| *status != Some(PluginStatus::Active));
    candidates.into_iter().next().map(|(id, _)| id)
}

/// Configures, replaces or mounts. A configuration that does not start is
/// rolled back to the previous one.
async fn apply(
    admin: &dyn PluginAdmin,
    current: Option<PluginId>,
    saved: &Saved,
) -> Result<(PluginId, PluginStatus), AdminError> {
    let request = MountRequest {
        kind: saved.kind.clone(),
        config: saved.config.clone(),
        ..MountRequest::default()
    };
    let Some(id) = current else {
        return admin.mount(request).await;
    };
    let Some(instance) = admin.instance(&id) else {
        return Err(AdminError::NotMounted(id));
    };
    if instance.kind != saved.kind {
        return admin.replace(&id, request).await;
    }
    let status = admin.configure(&id, saved.config.clone()).await?;
    if status == PluginStatus::Active {
        return Ok((id, status));
    }
    let detail = status_json(&status)["detail"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    let _ = admin.configure(&id, instance.config).await;
    Err(AdminError::Failed(format!(
        "`{id}` did not start with the new settings ({detail}); the previous settings were kept"
    )))
}

/// Stores a slot's settings; returns whether they were stored.
async fn remember(shared: &Shared, slot: &str, saved: &Saved) -> bool {
    let (Some(storage), Ok(bytes)) = (shared.kernel.get(STORAGE), serde_json::to_vec(saved)) else {
        return false;
    };
    storage
        .put(shared.id.as_str(), &key(slot), &bytes)
        .await
        .is_ok()
}

/// Preferences of the Web UI, shared by every browser on this `rah`
/// (`GET|PUT /api/preferences`), stored under the key `preferences`: kept
/// with the rest of the user's data instead of one browser's storage.
#[derive(Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Preferences {
    /// Show the model's replies as Markdown source instead of rendered.
    #[serde(default)]
    raw_markdown: bool,
    /// Folders of the session list the user opened (`true`) or closed, by
    /// lowercase path (`""`: sessions without a folder).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    folders: BTreeMap<String, bool>,
    /// Workspaces picked recently, the latest first.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    recent_workspaces: Vec<String>,
    /// Layout of the side panel.
    #[serde(default, skip_serializing_if = "SidePanel::is_empty")]
    side: SidePanel,
}

#[derive(Default, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct SidePanel {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    open: Option<bool>,
    /// `files` or `plugins`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    view: Option<String>,
    /// Width in CSS pixels.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    width: Option<f64>,
    /// Share of the file tree in the files view, between 0 and 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    tree_ratio: Option<f64>,
}

impl SidePanel {
    fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

const PREFERENCES: &str = "preferences";
/// The most folders remembered, and the longest path kept.
const MAX_FOLDERS: usize = 1000;
const MAX_PATH_CHARS: usize = 4096;
/// The most recent workspaces kept.
const MAX_RECENT: usize = 8;

impl Preferences {
    /// Refuses what no UI sends, so a client cannot grow the stored entry
    /// without bound; trims the recent workspaces to the most recent.
    fn checked(mut self) -> Result<Self, ApiError> {
        let bad = |message: &str| Err(ApiError(StatusCode::BAD_REQUEST, message.to_owned()));
        if self.folders.len() > MAX_FOLDERS {
            return bad("too many folders in `folders`");
        }
        let long = |path: &String| path.chars().count() > MAX_PATH_CHARS;
        if self.folders.keys().any(long) || self.recent_workspaces.iter().any(long) {
            return bad("a path in the preferences is too long");
        }
        if matches!(&self.side.view, Some(view) if view != "files" && view != "plugins") {
            return bad("`side.view` must be `files` or `plugins`");
        }
        let finite = |value: Option<f64>| value.is_none_or(f64::is_finite);
        if !finite(self.side.width) || !finite(self.side.tree_ratio) {
            return bad("`side` sizes must be numbers");
        }
        self.recent_workspaces.truncate(MAX_RECENT);
        Ok(self)
    }
}

/// `GET /api/preferences`; the defaults when nothing is stored.
pub(crate) async fn preferences(State(shared): State<AppState>) -> ApiResult {
    let stored = match shared.kernel.get(STORAGE) {
        Some(storage) => storage
            .get(shared.id.as_str(), PREFERENCES)
            .await
            .ok()
            .flatten()
            .and_then(|bytes| serde_json::from_slice::<Preferences>(&bytes).ok()),
        None => None,
    };
    Ok(Json(json!(stored.unwrap_or_default())))
}

/// `PUT /api/preferences`.
pub(crate) async fn set_preferences(
    State(shared): State<AppState>,
    Json(preferences): Json<Preferences>,
) -> ApiResult {
    let preferences = preferences.checked()?;
    let remembered = match (shared.kernel.get(STORAGE), serde_json::to_vec(&preferences)) {
        (Some(storage), Ok(bytes)) => storage
            .put(shared.id.as_str(), PREFERENCES, &bytes)
            .await
            .is_ok(),
        _ => false,
    };
    let mut reply = json!(preferences);
    reply["remembered"] = json!(remembered);
    Ok(Json(reply))
}
