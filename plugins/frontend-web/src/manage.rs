//! Plugin management through the distribution's `admin` service.

use std::{collections::BTreeMap, sync::Arc};

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use rah_api::{
    PluginId, PluginStatus,
    admin::{ADMIN, AdminError, MountRequest, PluginAdmin},
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{ApiError, ApiResult, AppState, panel, status_json};

pub(crate) fn admin(shared: &AppState) -> Result<Arc<dyn PluginAdmin>, ApiError> {
    shared.kernel.get(ADMIN).ok_or_else(|| {
        ApiError(
            StatusCode::SERVICE_UNAVAILABLE,
            "plugin management is not available (no `admin` service)".into(),
        )
    })
}

impl From<AdminError> for ApiError {
    fn from(error: AdminError) -> Self {
        let status = match error {
            AdminError::UnknownKind(_) | AdminError::Failed(_) => StatusCode::BAD_REQUEST,
            AdminError::NotMounted(_) => StatusCode::NOT_FOUND,
            AdminError::AlreadyMounted(_)
            | AdminError::ReplacementFailed { .. }
            | AdminError::Stopped(_)
            | AdminError::NotStopped(_) => StatusCode::CONFLICT,
            AdminError::Locked(_) => StatusCode::FORBIDDEN,
        };
        Self(status, error.to_string())
    }
}

fn status_reply(id: &str, status: &PluginStatus) -> Json<Value> {
    let mut reply = status_json(status);
    reply["id"] = json!(id);
    Json(reply)
}

/// `GET /api/plugins`: status, plus kind and configuration when known.
/// Stopped plugins are listed with the state `stopped`, sorted in by id.
pub(crate) async fn list(State(shared): State<AppState>) -> ApiResult {
    let admin = shared.kernel.get(ADMIN);
    let mounted = shared
        .kernel
        .plugins()
        .into_iter()
        .map(|(id, status)| (id, status_json(&status)));
    let stopped = admin
        .as_ref()
        .map(|admin| admin.stopped())
        .unwrap_or_default()
        .into_iter()
        .map(|id| (id, json!({ "state": "stopped", "detail": "" })));
    let mut plugins: Vec<(PluginId, Value)> = mounted.chain(stopped).collect();
    plugins.sort_by(|a, b| a.0.cmp(&b.0));
    let plugins: Vec<Value> = plugins
        .into_iter()
        .map(|(id, mut entry)| {
            entry["id"] = json!(id.as_str());
            if let Some(instance) = admin.as_ref().and_then(|admin| admin.instance(&id)) {
                entry["kind"] = json!(instance.kind);
                entry["config"] = instance.config;
                entry["locked"] = json!(instance.locked);
            }
            entry
        })
        .collect();
    Ok(Json(json!(plugins)))
}

/// `GET /api/catalog`: the mountable kinds and, where the distribution has
/// them, an example configuration and help for the configuration keys of
/// each (`help`: `{kind: [{key, help}]}`).
pub(crate) async fn catalog(State(shared): State<AppState>) -> ApiResult {
    let admin = admin(&shared)?;
    let kinds = admin.kinds();
    let examples: serde_json::Map<String, Value> = kinds
        .iter()
        .filter_map(|kind| Some((kind.clone(), admin.config_example(kind)?)))
        .collect();
    let help: serde_json::Map<String, Value> = kinds
        .iter()
        .filter_map(|kind| {
            let keys = admin.config_help(kind);
            if keys.is_empty() {
                return None;
            }
            let keys: Vec<Value> = keys
                .into_iter()
                .map(|entry| json!({ "key": entry.key, "help": entry.help }))
                .collect();
            Some((kind.clone(), json!(keys)))
        })
        .collect();
    Ok(Json(
        json!({ "kinds": kinds, "examples": examples, "help": help }),
    ))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Mount {
    kind: String,
    id: Option<String>,
    #[serde(default)]
    config: Value,
    #[serde(default)]
    bind: BTreeMap<String, String>,
}

impl Mount {
    fn request(self) -> MountRequest {
        MountRequest {
            kind: self.kind,
            id: self.id.filter(|id| !id.trim().is_empty()),
            config: self.config,
            bind: self.bind,
        }
    }
}

/// `POST /api/plugins`. Changes that succeed are remembered for the next
/// start (see [`crate::panel`]).
pub(crate) async fn mount(State(shared): State<AppState>, Json(mount): Json<Mount>) -> ApiResult {
    let request = mount.request();
    let (id, status) = admin(&shared)?.mount(request.clone()).await?;
    panel::mounted(&shared, &id, &request).await;
    Ok(status_reply(id.as_str(), &status))
}

/// `DELETE /api/plugins/{id}`.
pub(crate) async fn unmount(State(shared): State<AppState>, Path(id): Path<String>) -> ApiResult {
    let admin = admin(&shared)?;
    let plugin = PluginId::new(id.clone());
    let kind = admin.instance(&plugin).map(|instance| instance.kind);
    admin.unmount(&plugin).await?;
    panel::removed(&shared, &plugin, kind.as_deref()).await;
    Ok(Json(json!({ "id": id, "state": "unmounted" })))
}

/// Records the current configuration of `id` as started or stopped.
async fn record(shared: &AppState, admin: &dyn PluginAdmin, id: &PluginId, stopped: bool) {
    if let Some(instance) = admin.instance(id) {
        panel::configured(shared, id, &instance.kind, instance.config, stopped).await;
    }
}

/// `POST /api/plugins/{id}/restart`.
pub(crate) async fn restart(State(shared): State<AppState>, Path(id): Path<String>) -> ApiResult {
    let status = admin(&shared)?.restart(&PluginId::new(id.clone())).await?;
    Ok(status_reply(&id, &status))
}

/// `POST /api/plugins/{id}/stop`.
pub(crate) async fn stop(State(shared): State<AppState>, Path(id): Path<String>) -> ApiResult {
    let admin = admin(&shared)?;
    let plugin = PluginId::new(id.clone());
    admin.stop(&plugin).await?;
    record(&shared, admin.as_ref(), &plugin, true).await;
    Ok(Json(json!({ "id": id, "state": "stopped", "detail": "" })))
}

/// `POST /api/plugins/{id}/start`.
pub(crate) async fn start(State(shared): State<AppState>, Path(id): Path<String>) -> ApiResult {
    let admin = admin(&shared)?;
    let plugin = PluginId::new(id.clone());
    let status = admin.start(&plugin).await?;
    record(&shared, admin.as_ref(), &plugin, false).await;
    Ok(status_reply(&id, &status))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Configure {
    config: Value,
}

/// `PUT /api/plugins/{id}/config`.
pub(crate) async fn configure(
    State(shared): State<AppState>,
    Path(id): Path<String>,
    Json(configure): Json<Configure>,
) -> ApiResult {
    let admin = admin(&shared)?;
    let plugin = PluginId::new(id.clone());
    let status = admin.configure(&plugin, configure.config.clone()).await?;
    if let Some(instance) = admin.instance(&plugin) {
        panel::configured(&shared, &plugin, &instance.kind, configure.config, false).await;
    }
    Ok(status_reply(&id, &status))
}

/// `POST /api/plugins/{id}/replace`: mounts the replacement and unmounts
/// `{id}` once the replacement is active.
pub(crate) async fn replace(
    State(shared): State<AppState>,
    Path(id): Path<String>,
    Json(mount): Json<Mount>,
) -> ApiResult {
    let admin = admin(&shared)?;
    let old = PluginId::new(id);
    let kind = admin.instance(&old).map(|instance| instance.kind);
    let request = mount.request();
    let (id, status) = admin.replace(&old, request.clone()).await?;
    panel::removed(&shared, &old, kind.as_deref()).await;
    panel::mounted(&shared, &id, &request).await;
    Ok(status_reply(id.as_str(), &status))
}
