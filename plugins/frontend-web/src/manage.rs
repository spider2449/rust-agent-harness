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

use crate::{ApiError, ApiResult, AppState, status_json};

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
/// one, an example configuration for each.
pub(crate) async fn catalog(State(shared): State<AppState>) -> ApiResult {
    let admin = admin(&shared)?;
    let kinds = admin.kinds();
    let examples: serde_json::Map<String, Value> = kinds
        .iter()
        .filter_map(|kind| Some((kind.clone(), admin.config_example(kind)?)))
        .collect();
    Ok(Json(json!({ "kinds": kinds, "examples": examples })))
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

/// `POST /api/plugins`.
pub(crate) async fn mount(State(shared): State<AppState>, Json(mount): Json<Mount>) -> ApiResult {
    let (id, status) = admin(&shared)?.mount(mount.request()).await?;
    Ok(status_reply(id.as_str(), &status))
}

/// `DELETE /api/plugins/{id}`.
pub(crate) async fn unmount(State(shared): State<AppState>, Path(id): Path<String>) -> ApiResult {
    admin(&shared)?.unmount(&PluginId::new(id.clone())).await?;
    Ok(Json(json!({ "id": id, "state": "unmounted" })))
}

/// `POST /api/plugins/{id}/restart`.
pub(crate) async fn restart(State(shared): State<AppState>, Path(id): Path<String>) -> ApiResult {
    let status = admin(&shared)?.restart(&PluginId::new(id.clone())).await?;
    Ok(status_reply(&id, &status))
}

/// `POST /api/plugins/{id}/stop`.
pub(crate) async fn stop(State(shared): State<AppState>, Path(id): Path<String>) -> ApiResult {
    admin(&shared)?.stop(&PluginId::new(id.clone())).await?;
    Ok(Json(json!({ "id": id, "state": "stopped", "detail": "" })))
}

/// `POST /api/plugins/{id}/start`.
pub(crate) async fn start(State(shared): State<AppState>, Path(id): Path<String>) -> ApiResult {
    let status = admin(&shared)?.start(&PluginId::new(id.clone())).await?;
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
    let status = admin(&shared)?
        .configure(&PluginId::new(id.clone()), configure.config)
        .await?;
    Ok(status_reply(&id, &status))
}

/// `POST /api/plugins/{id}/replace`: mounts the replacement and unmounts
/// `{id}` once the replacement is active.
pub(crate) async fn replace(
    State(shared): State<AppState>,
    Path(id): Path<String>,
    Json(mount): Json<Mount>,
) -> ApiResult {
    let (id, status) = admin(&shared)?
        .replace(&PluginId::new(id), mount.request())
        .await?;
    Ok(status_reply(id.as_str(), &status))
}
