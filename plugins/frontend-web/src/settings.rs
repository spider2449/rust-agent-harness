//! Web UI settings forms: `PUT /api/settings/{slot} {id, kind, config}` for
//! the slots `model`, `assistant`, `tools` and `loop`.
//!
//! A save applies the change through the `admin` service (configure when
//! the slot's plugin keeps its kind, replace when the kind changes, mount
//! when the slot is empty) and, once the plugin is active, asks the
//! distribution to keep it in its configuration file (`persist`, ADR 0067),
//! so the next start runs it. A configuration that does not start is rolled
//! back, so a bad policy never leaves tool calls unguarded. Nothing is
//! applied from `storage` at start any more: the configuration file is the
//! one source. Settings an earlier version stored there are reported once
//! (`rah config migrate` moves them into the file).

use std::{collections::BTreeMap, sync::Arc, time::Duration};

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use rah_api::{
    PluginId, PluginStatus,
    admin::{ADMIN, AdminError, ConfigChange, MountRequest, PluginAdmin},
    host::HostNotice,
    storage::STORAGE,
    ui::{MAX_PANEL_ID_BYTES, ViewSpec, panels},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{
    ApiError, ApiResult, AppState, Shared, groups, manage::admin, modes, status_json, web_search,
    workspace,
};

/// The model plugin kinds.
pub(crate) const MODEL_KINDS: &[&str] = &[
    "model-llamacpp",
    "model-openai",
    "model-anthropic",
    "model-chat-completions",
    "model-mock",
];

/// Slots and the plugin kinds each may hold.
const SLOTS: [(&str, &[&str]); 4] = [
    ("tools", &["policy-authority"]),
    ("model", MODEL_KINDS),
    ("assistant", &["context-default"]),
    ("loop", &["loop-react", "loop-plan"]),
];

/// Keys under which an earlier version kept settings in `storage`; they
/// are no longer applied (see [`restore`]).
pub(crate) const LEGACY_KEYS: [&str; 6] = [
    "settings-tools",
    "settings-model",
    "settings-assistant",
    "settings-loop",
    "plugins",
    "web-search",
];

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
    let previous_kind = current
        .as_ref()
        .and_then(|id| admin.instance(id))
        .map(|instance| instance.kind);
    let saved = Saved {
        kind: save.kind,
        config: save.config,
    };
    let (id, status) = apply(admin.as_ref(), current.clone(), &saved).await?;
    let mut reply = status_json(&status);
    reply["id"] = json!(id.as_str());
    if status == PluginStatus::Active {
        let mut changes = Vec::new();
        // Another kind took over the slot (but the model is one key in the
        // file, whatever its kind).
        if let (Some(old), Some(old_kind)) = (&current, &previous_kind)
            && *old_kind != saved.kind
            && slot != "model"
        {
            changes.push(ConfigChange::Removed { id: old.clone() });
        }
        changes.push(if slot == "model" {
            ConfigChange::Model {
                kind: saved.kind.clone(),
                config: saved.config.clone(),
            }
        } else {
            ConfigChange::Plugin {
                id: id.clone(),
                kind: saved.kind.clone(),
                config: saved.config.clone(),
            }
        });
        remembered(&mut reply, persist(admin.as_ref(), changes).await);
    } else {
        remembered(&mut reply, Err("the plugin is not active".into()));
    }
    Ok(Json(reply))
}

/// Asks the distribution to keep `changes`, in order; the first refusal
/// stops it.
pub(crate) async fn persist(
    admin: &dyn PluginAdmin,
    changes: Vec<ConfigChange>,
) -> Result<(), String> {
    for change in changes {
        admin
            .persist(change)
            .await
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

/// Says in `reply` whether the change is kept for the next start, and why
/// not.
pub(crate) fn remembered(reply: &mut Value, result: Result<(), String>) {
    match result {
        Ok(()) => reply["remembered"] = json!(true),
        Err(reason) => {
            reply["remembered"] = json!(false);
            reply["remember_error"] = json!(reason);
        }
    }
}

/// Runs once this plugin is active: reports settings an earlier version
/// stored, then restores the UI state that is not configuration (the mode
/// and the connected API group).
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
    report_legacy(&shared).await;
    modes::restore(&shared, admin.as_ref(), shared.start_mode.clone()).await;
    groups::restore(&shared, admin.as_ref()).await;
}

/// Tells the user, once per start, that settings an earlier version kept in
/// `storage` no longer apply.
async fn report_legacy(shared: &Shared) {
    let Some(storage) = shared.kernel.get(STORAGE) else {
        return;
    };
    let mut found = Vec::new();
    for key in LEGACY_KEYS {
        if matches!(storage.get(shared.id.as_str(), key).await, Ok(Some(_))) {
            found.push(key);
        }
    }
    if !found.is_empty() {
        shared.kernel.emit(&HostNotice {
            plugin: shared.id.clone(),
            message: format!(
                "settings saved by an earlier version ({}) are no longer applied; \
                 `rah config migrate` moves them into the configuration file",
                found.join(", ")
            ),
        });
    }
}

/// The policy configuration with `rule` added to its `commands` (ADR 0050);
/// `None` when it is already there. A malformed `commands` is replaced only
/// when it is not a list.
fn with_command(config: &Value, rule: &[String]) -> Option<Value> {
    let mut config = match config {
        Value::Object(_) => config.clone(),
        _ => Value::Object(serde_json::Map::new()),
    };
    let object = config.as_object_mut()?;
    let commands = object
        .entry("commands")
        .or_insert_with(|| Value::Array(Vec::new()));
    if !commands.is_array() {
        *commands = Value::Array(Vec::new());
    }
    let list = commands.as_array_mut()?;
    let rule = json!(rule);
    if list.contains(&rule) {
        return None;
    }
    list.push(rule);
    Some(config)
}

/// Adds `rule` to the mounted policy's `commands` and keeps the policy in
/// the configuration file like a saved **Permissions** form, so the exact command
/// runs without asking from now on, also after a restart (ADR 0066). The
/// user chose it at the approval prompt; the rule comes from the call the
/// user saw.
pub(crate) async fn allow_command(shared: &Shared, rule: Vec<String>) -> Result<(), String> {
    let kinds = SLOTS[0].1;
    let admin = shared
        .kernel
        .get(ADMIN)
        .ok_or("no `admin` service is mounted")?;
    let id = current(shared, admin.as_ref(), kinds).ok_or("no policy is mounted")?;
    let instance = admin
        .instance(&id)
        .ok_or_else(|| format!("`{id}` is not mounted"))?;
    let Some(config) = with_command(&instance.config, &rule) else {
        return Ok(());
    };
    let saved = Saved {
        kind: instance.kind,
        config,
    };
    let (id, status) = apply(admin.as_ref(), Some(id), &saved)
        .await
        .map_err(|error| error.to_string())?;
    if status != PluginStatus::Active {
        return Err(format!("`{id}` is not active"));
    }
    let change = ConfigChange::Plugin {
        id,
        kind: saved.kind,
        config: saved.config,
    };
    persist(admin.as_ref(), vec![change])
        .await
        .map_err(|reason| {
            format!("the policy was changed until the next start, but not kept: {reason}")
        })
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
    // A refused configuration changed nothing.
    let status = admin
        .configure(&id, saved.config.clone())
        .await
        .map_err(|error| AdminError::Failed(format!("{error}; the previous settings were kept")))?;
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
    /// Folders and workspaces opened recently, the latest first.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    recent_workspaces: Vec<Recent>,
    /// Layout of the side panel.
    #[serde(default, skip_serializing_if = "SidePanel::is_empty")]
    side: SidePanel,
    /// Wrap long lines in the file viewer and in the activity log: one
    /// switch in the UI, which also writes `wrap_activity` (kept so older
    /// pages and stored preferences still read).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    wrap_files: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    wrap_activity: bool,
    /// The colour theme by name; the UI knows the names and falls back to
    /// following the system for one it does not know.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    theme: Option<String>,
    /// Sizes of the other draggable dividers by name (pixels, or a share
    /// between 0 and 1), so a new divider needs no change here.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    layout: BTreeMap<String, f64>,
}

#[derive(Default, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct SidePanel {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    open: Option<bool>,
    /// The panel shown in the right column: a built-in panel or a view id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    view: Option<String>,
    /// Width in CSS pixels.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    width: Option<f64>,
    /// Share of the file tree in the files view, between 0 and 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    tree_ratio: Option<f64>,
    /// Whether the activity log is unfolded; folded when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    activity_open: Option<bool>,
}

impl SidePanel {
    fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// A recently opened folder (its path), or a workspace of several folders.
#[derive(Deserialize, Serialize)]
#[serde(untagged)]
enum Recent {
    Folder(String),
    Workspace(RecentWorkspace),
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RecentWorkspace {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    folders: Vec<String>,
    /// The folder that was active.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    root: Option<String>,
}

impl Recent {
    fn paths(&self) -> impl Iterator<Item = &String> {
        let (folders, root): (&[String], _) = match self {
            Self::Folder(path) => (std::slice::from_ref(path), None),
            Self::Workspace(workspace) => (&workspace.folders, workspace.root.as_ref()),
        };
        folders.iter().chain(root)
    }
}

const PREFERENCES: &str = "preferences";
/// The most folders remembered, and the longest path kept.
const MAX_FOLDERS: usize = 1000;
const MAX_PATH_CHARS: usize = 4096;
/// The most recent workspaces kept.
const MAX_RECENT: usize = 8;
/// The most layout entries kept, the longest name and the largest size.
const MAX_LAYOUT: usize = 32;
const MAX_LAYOUT_NAME: usize = 64;
const MAX_LAYOUT_SIZE: f64 = 100_000.0;
const MAX_THEME_NAME: usize = 32;

impl Preferences {
    /// Refuses what no UI sends, so a client cannot grow the stored entry
    /// without bound; trims the recent workspaces to the most recent.
    fn checked(mut self) -> Result<Self, ApiError> {
        let bad = |message: &str| Err(ApiError(StatusCode::BAD_REQUEST, message.to_owned()));
        if self.folders.len() > MAX_FOLDERS {
            return bad("too many folders in `folders`");
        }
        let long = |path: &String| path.chars().count() > MAX_PATH_CHARS;
        if self.folders.keys().any(long)
            || self
                .recent_workspaces
                .iter()
                .any(|recent| recent.paths().any(long))
        {
            return bad("a path in the preferences is too long");
        }
        if self.recent_workspaces.iter().any(|recent| match recent {
            Recent::Folder(_) => false,
            Recent::Workspace(workspace) => {
                workspace.folders.is_empty()
                    || workspace.folders.len() > workspace::MAX_FOLDERS
                    || workspace
                        .name
                        .as_ref()
                        .is_some_and(|name| name.chars().count() > workspace::MAX_NAME_CHARS)
            }
        }) {
            return bad("a recent workspace has no folders, too many or a long name");
        }
        let panel = |view: &str| {
            panels::BUILT_IN.contains(&view)
                || view
                    .split_once('/')
                    .is_some_and(|(instance, id)| !instance.is_empty() && ViewSpec::valid_id(id))
                    && view.len() <= MAX_PANEL_ID_BYTES
        };
        if matches!(&self.side.view, Some(view) if !panel(view)) {
            return bad(
                "`side.view` must be a panel: `sessions`, `files`, `plugins` or `instance/view`",
            );
        }
        let finite = |value: Option<f64>| value.is_none_or(f64::is_finite);
        if !finite(self.side.width) || !finite(self.side.tree_ratio) {
            return bad("`side` sizes must be numbers");
        }
        if let Some(theme) = &self.theme
            && (theme.is_empty()
                || theme.len() > MAX_THEME_NAME
                || !theme.bytes().all(|b| b.is_ascii_lowercase() || b == b'-'))
        {
            return bad("`theme` must be a short lowercase name");
        }
        if self.layout.len() > MAX_LAYOUT
            || self.layout.keys().any(|name| name.len() > MAX_LAYOUT_NAME)
            || !self
                .layout
                .values()
                .all(|size| (0.0..=MAX_LAYOUT_SIZE).contains(size))
        {
            return bad("`layout` has too many entries, a long name or a size out of range");
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

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{remembered, with_command};

    #[test]
    fn a_command_rule_is_added_once() {
        let rule = ["netstat".to_owned(), "-ano".to_owned()];
        let added = with_command(&json!({ "tools": { "shell.exec": "execute" } }), &rule).unwrap();
        assert_eq!(
            added,
            json!({ "tools": { "shell.exec": "execute" }, "commands": [["netstat", "-ano"]] })
        );
        assert_eq!(with_command(&added, &rule), None);
        let other = ["git".to_owned()];
        assert_eq!(
            with_command(&added, &other).unwrap()["commands"],
            json!([["netstat", "-ano"], ["git"]])
        );
        assert_eq!(
            with_command(&json!({ "commands": 3 }), &other).unwrap(),
            json!({ "commands": [["git"]] })
        );
    }

    #[test]
    fn a_reply_says_whether_and_why_not_a_change_is_kept() {
        let mut reply = json!({});
        remembered(&mut reply, Ok(()));
        assert_eq!(reply, json!({ "remembered": true }));
        let mut reply = json!({});
        remembered(&mut reply, Err("no file".into()));
        assert_eq!(
            reply,
            json!({ "remembered": false, "remember_error": "no file" })
        );
    }
}
