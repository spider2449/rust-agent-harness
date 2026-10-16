//! Modes: named working setups (such as daily work and coding) shared by
//! every frontend served by this plugin, the browser Web UI and the desktop
//! window alike (`GET /api/modes`, `PUT /api/mode {mode}`).
//!
//! A mode is defined in one place, this plugin's configuration (`modes`,
//! else [`builtin`]), and changes three things while it is active:
//!
//! - `plugins`: plugin kinds to run (`true`) or stop (`false`), applied
//!   through the `admin` service. Running a kind starts a stopped instance
//!   or mounts one with its default configuration; stopping keeps its
//!   configuration, so running it again restores it. `tools-web-search` is
//!   switched as the web search switch does (the policy asks before `web.*`
//!   first). Policy and frontend kinds cannot be listed: a mode never
//!   changes what guards tool calls.
//! - `instructions`: added as a system item to every model request.
//! - `workspace`: whether the UI shows the workspace (files, Git, folder).
//!
//! One mode is active for the whole `rah`, not per window: plugins are
//! shared, so every connected UI is told about a switch
//! (`{type: "mode", active}` on the WebSocket). The active mode is kept in
//! `storage` and applied again at start, after the stored settings; the
//! `mode` configuration key picks the mode at start instead.

use std::{
    collections::BTreeMap,
    sync::{Mutex, PoisonError},
};

use axum::{Json, extract::State, http::StatusCode};
use rah_api::{
    PluginError, PluginStatus,
    admin::{AdminError, MountRequest, PluginAdmin},
    storage::STORAGE,
};
use rah_protocol::{ContextItem, ModelRequest};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{ApiError, ApiResult, AppState, Shared, manage::admin, web_search};

/// One mode, as configured.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Mode {
    pub id: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// Added to every model request while the mode is active.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    /// Plugin kinds to run (`true`) or stop (`false`), in this order.
    #[serde(default)]
    pub plugins: BTreeMap<String, bool>,
    /// Whether the UI shows the workspace: files, Git and the folder.
    #[serde(default = "yes")]
    pub workspace: bool,
}

fn yes() -> bool {
    true
}

/// The modes used when the configuration names none.
pub(crate) fn builtin() -> Vec<Mode> {
    vec![
        Mode {
            id: "daily".into(),
            label: "Daily work".into(),
            description: "Writing, research and planning: web search on, no file changes.".into(),
            instructions: Some(
                "You are helping with everyday work: writing, research, planning and \
                 questions. Answer directly and concisely. Use web search for facts that \
                 may have changed and say where they come from."
                    .into(),
            ),
            plugins: BTreeMap::from([
                ("tools-repo-write".into(), false),
                (web_search::KIND.into(), true),
            ]),
            workspace: false,
        },
        Mode {
            id: "coding".into(),
            label: "Coding".into(),
            description: "Software work in the workspace: file and Git tools, the file panel."
                .into(),
            instructions: Some(
                "You are helping with software development in the workspace. Read the \
                 relevant files before changing them, keep changes small and focused, \
                 and say which files you changed."
                    .into(),
            ),
            plugins: BTreeMap::from([("tools-repo-write".into(), true)]),
            workspace: true,
        },
    ]
}

/// The most modes one configuration may define.
const MAX_MODES: usize = 32;

/// Checks configured modes: unique ids, and no kind a mode may not switch.
pub(crate) fn check(modes: &[Mode]) -> Result<(), PluginError> {
    if modes.len() > MAX_MODES {
        return Err(format!("at most {MAX_MODES} modes").into());
    }
    let mut ids = std::collections::HashSet::new();
    for mode in modes {
        if mode.id.is_empty() || mode.label.is_empty() {
            return Err("every mode needs an `id` and a `label`".into());
        }
        if !ids.insert(mode.id.as_str()) {
            return Err(format!("mode `{}` is defined twice", mode.id).into());
        }
        if let Some(kind) = mode.plugins.keys().find(|kind| !switchable(kind)) {
            return Err(format!(
                "mode `{}` cannot switch `{kind}`: policy and frontend plugins stay as configured",
                mode.id
            )
            .into());
        }
    }
    Ok(())
}

fn switchable(kind: &str) -> bool {
    !kind.starts_with("policy-") && !kind.starts_with("frontend-")
}

/// The modes and which one is active.
pub(crate) struct Modes {
    list: Vec<Mode>,
    active: Mutex<Option<String>>,
    /// Serializes switches.
    switching: tokio::sync::Mutex<()>,
}

impl Modes {
    pub fn new(list: Vec<Mode>) -> Self {
        Self {
            list,
            active: Mutex::new(None),
            switching: tokio::sync::Mutex::new(()),
        }
    }

    fn find(&self, id: &str) -> Option<&Mode> {
        self.list.iter().find(|mode| mode.id == id)
    }

    fn active_id(&self) -> Option<String> {
        self.active
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn active(&self) -> Option<&Mode> {
        self.active_id().and_then(|id| self.find(&id))
    }

    fn set_active(&self, id: Option<String>) {
        *self.active.lock().unwrap_or_else(PoisonError::into_inner) = id;
    }

    /// Adds the active mode's instructions after the leading system items.
    pub fn shape(&self, mut request: ModelRequest) -> ModelRequest {
        if let Some(text) = self.active().and_then(|mode| mode.instructions.clone()) {
            let at = request
                .items
                .iter()
                .position(|item| !matches!(item, ContextItem::System { .. }))
                .unwrap_or(request.items.len());
            request.items.insert(at, ContextItem::System { text });
        }
        request
    }

    fn state(&self) -> Value {
        json!({ "modes": self.list, "active": self.active_id() })
    }
}

/// `GET /api/modes`.
pub(crate) async fn list(State(shared): State<AppState>) -> ApiResult {
    Ok(Json(shared.modes.state()))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Switch {
    /// `null` leaves every mode.
    mode: Option<String>,
}

/// `PUT /api/mode {mode}`: replies with the active mode, the plugins that
/// could not be switched (`problems`) and whether the choice was stored.
pub(crate) async fn set(State(shared): State<AppState>, Json(switch): Json<Switch>) -> ApiResult {
    if let Some(id) = &switch.mode
        && shared.modes.find(id).is_none()
    {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            format!("unknown mode `{id}`"),
        ));
    }
    let admin = admin(&shared)?;
    let problems = enter(&shared, admin.as_ref(), switch.mode.clone()).await;
    let remembered = remember(&shared, switch.mode.as_deref()).await;
    let mut reply = shared.modes.state();
    reply["problems"] = json!(problems);
    reply["remembered"] = json!(remembered);
    Ok(Json(reply))
}

/// Enters the mode chosen at start: `configured` (the `mode` key), else the
/// stored one (see [`crate::settings::restore`]).
pub(crate) async fn restore(shared: &Shared, admin: &dyn PluginAdmin, configured: Option<String>) {
    let stored = match (&configured, shared.kernel.get(STORAGE)) {
        (None, Some(storage)) => storage
            .get(shared.id.as_str(), KEY)
            .await
            .ok()
            .flatten()
            .and_then(|bytes| String::from_utf8(bytes).ok()),
        _ => None,
    };
    let Some(id) = configured.or(stored) else {
        return;
    };
    if shared.modes.find(&id).is_some() {
        let _ = enter(shared, admin, Some(id)).await;
    }
}

const KEY: &str = "mode";

/// Makes `mode` active, switches its plugins and tells every connected UI.
/// Returns the plugins that could not be switched; the mode is active
/// regardless, so the user sees what is missing.
async fn enter(shared: &Shared, admin: &dyn PluginAdmin, mode: Option<String>) -> Vec<String> {
    let _switching = shared.modes.switching.lock().await;
    let mut problems = Vec::new();
    if let Some(found) = mode.as_deref().and_then(|id| shared.modes.find(id)) {
        for (kind, run) in &found.plugins {
            if let Err(error) = switch_kind(shared, admin, kind, *run).await {
                problems.push(format!("{kind}: {error}"));
            }
        }
    }
    shared.modes.set_active(mode);
    shared.approvals.notify(json!({
        "type": "mode",
        "active": shared.modes.active_id(),
        "problems": problems,
    }));
    problems
}

async fn switch_kind(
    shared: &Shared,
    admin: &dyn PluginAdmin,
    kind: &str,
    run: bool,
) -> Result<(), AdminError> {
    if kind == web_search::KIND {
        return if run {
            web_search::enable(shared, admin).await
        } else {
            web_search::disable(shared, admin).await
        };
    }
    let instances = web_search::instances(shared, admin, kind);
    if !run {
        for (id, status) in instances {
            if status.is_some() {
                admin.stop(&id).await?;
            }
        }
        return Ok(());
    }
    if instances
        .iter()
        .any(|(_, status)| *status == Some(PluginStatus::Active))
    {
        return Ok(());
    }
    let status = match instances.iter().find(|(_, status)| status.is_none()) {
        Some((id, _)) => admin.start(id).await?,
        None => {
            let request = MountRequest {
                kind: kind.to_owned(),
                ..MountRequest::default()
            };
            admin.mount(request).await?.1
        }
    };
    if status != PluginStatus::Active {
        return Err(AdminError::Failed(format!("did not start: {status:?}")));
    }
    Ok(())
}

/// Stores the choice; returns whether it was stored.
async fn remember(shared: &Shared, mode: Option<&str>) -> bool {
    let Some(storage) = shared.kernel.get(STORAGE) else {
        return false;
    };
    match mode {
        Some(id) => storage
            .put(shared.id.as_str(), KEY, id.as_bytes())
            .await
            .is_ok(),
        None => storage.delete(shared.id.as_str(), KEY).await.is_ok(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(items: Vec<ContextItem>) -> ModelRequest {
        ModelRequest {
            items,
            ..ModelRequest::default()
        }
    }

    #[test]
    fn the_builtin_modes_are_valid() {
        assert!(check(&builtin()).is_ok());
    }

    #[test]
    fn modes_cannot_switch_the_policy_or_a_frontend() {
        for kind in ["policy-authority", "frontend-web"] {
            let mut modes = builtin();
            modes[0].plugins.insert(kind.into(), false);
            assert!(check(&modes).is_err(), "{kind}");
        }
        let mut twice = builtin();
        twice[1].id = twice[0].id.clone();
        assert!(check(&twice).is_err());
    }

    #[test]
    fn instructions_follow_the_system_prompt_while_a_mode_is_active() {
        let modes = Modes::new(builtin());
        let items = vec![
            ContextItem::System {
                text: "system".into(),
            },
            ContextItem::User { text: "hi".into() },
        ];
        assert_eq!(modes.shape(request(items.clone())).items, items);

        modes.set_active(Some("coding".into()));
        let shaped = modes.shape(request(items)).items;
        assert_eq!(shaped.len(), 3);
        assert!(matches!(&shaped[0], ContextItem::System { text } if text == "system"));
        assert!(
            matches!(&shaped[1], ContextItem::System { text } if text.contains("software development"))
        );
        assert!(matches!(&shaped[2], ContextItem::User { .. }));
    }
}
