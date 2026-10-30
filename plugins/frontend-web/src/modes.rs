//! Modes: named working setups (such as daily work and coding) shared by
//! every frontend served by this plugin, the browser Web UI and the desktop
//! window alike (`GET /api/modes`, `PUT /api/mode {mode}`).
//!
//! A mode is defined in one place, this plugin's configuration (`modes`,
//! else [`builtin`]), and changes three things while it is active:
//!
//! - `plugins`: plugin kinds (or instance ids) to run (`true`) or stop (`false`), applied
//!   through the `admin` service. Running a kind starts a stopped instance
//!   or mounts one with its default configuration; stopping keeps its
//!   configuration, so running it again restores it. `tools-web-search` is
//!   switched as the web search switch does (the policy asks before `web.*`
//!   first). Policy and frontend kinds cannot be listed: a mode never
//!   changes what guards tool calls.
//! - `instructions`: added as a system item to every model request.
//! - `workspace`: whether the UI shows the workspace (files, Git, folder).
//! - `views`: which plugin views (`instance/view`, ADR 0062) the UI shows;
//!   without it, every view.
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
    PluginError, PluginId, PluginStatus,
    admin::{AdminError, MountRequest, PluginAdmin},
    storage::STORAGE,
    ui::{MAX_PANEL_ID_BYTES, ViewSpec},
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
    /// The mode this one refines (a sub-mode, such as an API group of daily
    /// work). It starts from the parent's plugins, instructions and
    /// workspace; its own plugins override them and its instructions follow
    /// the parent's. The parent must not extend another mode.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extends: Option<String>,
    /// Added to every model request while the mode is active.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    /// Plugin kinds or instance ids to run (`true`) or stop (`false`). An
    /// instance id switches that one plugin (for example one `tools-http`
    /// API group); an instance another mode runs and this one does not list
    /// is stopped when this mode is entered.
    #[serde(default)]
    pub plugins: BTreeMap<String, bool>,
    /// Whether the UI shows the workspace: files, Git and the folder
    /// (default: the parent's, else shown).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<bool>,
    /// Whether the UI offers the API groups (`tools-http` instances) here
    /// (default: the parent's, else not).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub groups: bool,
    /// The plugin views (`instance/view`) the UI shows; the others are
    /// hidden (default: the parent's, else every view). Presentation only:
    /// the plugins keep running.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub views: Option<Vec<String>>,
}

/// The modes used when the configuration names none.
pub(crate) fn builtin() -> Vec<Mode> {
    vec![
        Mode {
            id: "daily".into(),
            label: "Daily work".into(),
            description: "Writing, research and planning: web search on, no file changes.".into(),
            extends: None,
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
            workspace: Some(false),
            groups: true,
            views: None,
        },
        Mode {
            id: "coding".into(),
            label: "Coding".into(),
            description: "Software work in the workspace: file and Git tools, the file panel."
                .into(),
            extends: None,
            instructions: Some(
                "You are helping with software development in the workspace. Read the \
                 relevant files before changing them, keep changes small and focused, \
                 and say which files you changed."
                    .into(),
            ),
            plugins: BTreeMap::from([("tools-repo-write".into(), true)]),
            workspace: Some(true),
            groups: false,
            views: None,
        },
    ]
}

/// The most modes one configuration may define.
const MAX_MODES: usize = 32;
/// The most views one mode may list.
const MAX_VIEWS: usize = 64;

/// Whether `id` names a plugin view: `<instance>/<view>`.
fn view_id(id: &str) -> bool {
    id.len() <= MAX_PANEL_ID_BYTES
        && id
            .split_once('/')
            .is_some_and(|(instance, view)| !instance.is_empty() && ViewSpec::valid_id(view))
}

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
        if let Some(views) = &mode.views {
            if views.len() > MAX_VIEWS {
                return Err(format!("mode `{}` lists more than {MAX_VIEWS} views", mode.id).into());
            }
            if let Some(bad) = views.iter().find(|id| !view_id(id)) {
                return Err(format!(
                    "mode `{}`: `{bad}` is not a view; views are `instance/view`, such as \
                     `telemetry-log/usage`",
                    mode.id
                )
                .into());
            }
        }
        if let Some(parent) = &mode.extends {
            match modes.iter().find(|other| &other.id == parent) {
                None => {
                    return Err(
                        format!("mode `{}` extends unknown mode `{parent}`", mode.id).into(),
                    );
                }
                Some(found) if found.extends.is_some() => {
                    return Err(format!(
                        "mode `{}` extends `{parent}`, which extends another mode: only one level",
                        mode.id
                    )
                    .into());
                }
                Some(_) => {}
            }
        }
    }
    Ok(())
}

/// Folds each sub-mode into its parent's settings, so the rest of the plugin
/// and the UI see complete modes. `modes` passed [`check`].
fn resolve(modes: Vec<Mode>) -> Vec<Mode> {
    let parents = modes.clone();
    modes
        .into_iter()
        .map(|mut mode| {
            let Some(parent) = mode
                .extends
                .as_deref()
                .and_then(|id| parents.iter().find(|parent| parent.id == id))
            else {
                return mode;
            };
            let mut plugins = parent.plugins.clone();
            plugins.append(&mut mode.plugins);
            mode.plugins = plugins;
            mode.instructions = match (&parent.instructions, mode.instructions.take()) {
                (Some(parent), Some(own)) => Some(format!("{parent}\n\n{own}")),
                (parent, own) => own.or_else(|| parent.clone()),
            };
            mode.workspace = mode.workspace.or(parent.workspace);
            mode.groups |= parent.groups;
            if mode.views.is_none() {
                mode.views.clone_from(&parent.views);
            }
            mode
        })
        .collect()
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
            list: resolve(list),
            active: Mutex::new(None),
            switching: tokio::sync::Mutex::new(()),
        }
    }

    /// Every plugin key some mode lists.
    pub fn listed_plugins(&self) -> std::collections::BTreeSet<&str> {
        self.list
            .iter()
            .flat_map(|mode| mode.plugins.keys().map(String::as_str))
            .collect()
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
    let found = mode.as_deref().and_then(|id| shared.modes.find(id));
    // An instance another mode runs and this one does not list is stopped,
    // so leaving an API group removes its tools.
    let kinds = admin.kinds();
    let leaving: std::collections::BTreeSet<&String> = shared
        .modes
        .list
        .iter()
        .flat_map(|other| &other.plugins)
        .filter(|(key, run)| **run && !kinds.contains(*key))
        .map(|(key, _)| key)
        .filter(|key| !found.is_some_and(|found| found.plugins.contains_key(*key)))
        .collect();
    for key in leaving {
        if let Err(error) = switch_kind(shared, admin, key, false).await {
            problems.push(format!("{key}: {error}"));
        }
    }
    if let Some(found) = found {
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

pub(crate) async fn switch_kind(
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
    // A kind switches all its instances; any other key is one instance id.
    let by_id = !admin.kinds().iter().any(|known| known == kind);
    let instances = if by_id {
        let id = PluginId::new(kind);
        let all = shared
            .kernel
            .plugins()
            .into_iter()
            .map(|(id, status)| (id, Some(status)))
            .chain(admin.stopped().into_iter().map(|id| (id, None)));
        all.filter(|(found, _)| *found == id && admin.instance(found).is_some())
            .collect()
    } else {
        web_search::instances(shared, admin, kind)
    };
    if by_id && run && instances.is_empty() {
        return Err(AdminError::Failed("no plugin with this id".into()));
    }
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

    fn group(id: &str, parent: Option<&str>, instructions: &str, plugins: &[(&str, bool)]) -> Mode {
        Mode {
            id: id.into(),
            label: id.into(),
            description: String::new(),
            extends: parent.map(Into::into),
            instructions: Some(instructions.into()),
            plugins: plugins.iter().map(|(k, v)| ((*k).into(), *v)).collect(),
            workspace: None,
            groups: false,
            views: None,
        }
    }

    #[test]
    fn a_mode_lists_plugin_views_and_a_sub_mode_inherits_them() {
        let mut parent = group("daily", None, "x", &[]);
        parent.views = Some(vec!["todo/board".into()]);
        let own = group("ddt", Some("daily"), "y", &[]);
        let mut explicit = group("other", Some("daily"), "z", &[]);
        explicit.views = Some(Vec::new());
        let modes = vec![parent, own, explicit];
        assert!(check(&modes).is_ok());
        let resolved = resolve(modes);
        assert_eq!(resolved[1].views, Some(vec!["todo/board".to_owned()]));
        assert_eq!(resolved[2].views, Some(Vec::new()));

        for bad in ["sessions", "todo/Board", "/board", "todo/"] {
            let mut mode = group("daily", None, "x", &[]);
            mode.views = Some(vec![bad.into()]);
            let error = check(&[mode]).unwrap_err().to_string();
            assert!(error.contains("is not a view"), "{bad}: {error}");
        }
    }

    #[test]
    fn a_sub_mode_adds_to_its_parent() {
        let modes = vec![
            group("daily", None, "Be brief.", &[("tools-web-search", true)]),
            group(
                "ddt",
                Some("daily"),
                "Use ddt.* tools.",
                &[("http-ddt", true)],
            ),
        ];
        assert!(check(&modes).is_ok());
        let resolved = resolve(modes);
        assert_eq!(
            resolved[1].plugins,
            BTreeMap::from([("http-ddt".into(), true), ("tools-web-search".into(), true)])
        );
        assert_eq!(
            resolved[1].instructions.as_deref(),
            Some("Be brief.\n\nUse ddt.* tools.")
        );
        assert_eq!(resolved[1].extends.as_deref(), Some("daily"));
    }

    #[test]
    fn a_sub_mode_inherits_the_workspace_unless_it_sets_one() {
        let mut parent = group("daily", None, "x", &[]);
        parent.workspace = Some(false);
        let own = group("ddt", Some("daily"), "y", &[]);
        let mut explicit = group("other", Some("daily"), "z", &[]);
        explicit.workspace = Some(true);
        let resolved = resolve(vec![parent, own, explicit]);
        assert_eq!(resolved[1].workspace, Some(false));
        assert_eq!(resolved[2].workspace, Some(true));
    }

    #[test]
    fn a_sub_mode_needs_an_existing_parent_that_is_not_a_sub_mode() {
        let daily = group("daily", None, "x", &[]);
        let lost = group("ddt", Some("missing"), "y", &[]);
        assert!(check(&[daily.clone(), lost]).is_err());
        let first = group("ddt", Some("daily"), "y", &[]);
        let second = group("deep", Some("ddt"), "z", &[]);
        assert!(check(&[daily, first, second]).is_err());
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
