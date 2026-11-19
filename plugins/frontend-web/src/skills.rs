use std::{io, sync::Arc};

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use rah_api::skills::{SKILL_MANAGER, SkillManager};
use serde::Deserialize;
use serde_json::json;

use crate::{ApiError, ApiResult, AppState, to_json};

fn manager(shared: &AppState) -> Result<Arc<dyn SkillManager>, ApiError> {
    let mut managers = shared.kernel.get_all(SKILL_MANAGER).into_iter();
    let (_, manager) = managers.next().ok_or_else(|| {
        ApiError(
            StatusCode::SERVICE_UNAVAILABLE,
            "Enable Skills to manage installed skills.".into(),
        )
    })?;
    if managers.next().is_some() {
        return Err(ApiError(
            StatusCode::CONFLICT,
            "Multiple skill managers are active. Select one in Plugins.".into(),
        ));
    }
    Ok(manager)
}

fn failed(error: io::Error) -> ApiError {
    let status = match error.kind() {
        io::ErrorKind::AlreadyExists => StatusCode::CONFLICT,
        io::ErrorKind::NotFound => StatusCode::NOT_FOUND,
        io::ErrorKind::InvalidInput => StatusCode::BAD_REQUEST,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    };
    ApiError(status, error.to_string())
}

pub(crate) async fn list(State(shared): State<AppState>) -> ApiResult {
    to_json(manager(&shared)?.list().await.map_err(failed)?)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Add {
    name: String,
    text: String,
}

pub(crate) async fn add(State(shared): State<AppState>, Json(skill): Json<Add>) -> ApiResult {
    manager(&shared)?
        .add(&skill.name, &skill.text)
        .await
        .map_err(failed)?;
    Ok(Json(json!({ "name": skill.name })))
}

pub(crate) async fn delete(State(shared): State<AppState>, Path(name): Path<String>) -> ApiResult {
    manager(&shared)?.delete(&name).await.map_err(failed)?;
    Ok(Json(json!({ "name": name })))
}

#[derive(Deserialize)]
pub(crate) struct Source {
    repository: String,
}

pub(crate) async fn packages(State(shared): State<AppState>) -> ApiResult {
    to_json(manager(&shared)?.packages().await.map_err(failed)?)
}
pub(crate) async fn preview(
    State(shared): State<AppState>,
    Json(source): Json<Source>,
) -> ApiResult {
    to_json(
        manager(&shared)?
            .preview(&source.repository)
            .await
            .map_err(failed)?,
    )
}
pub(crate) async fn install(
    State(shared): State<AppState>,
    Json(selection): Json<rah_api::skills::InstallPackage>,
) -> ApiResult {
    to_json(manager(&shared)?.install(selection).await.map_err(failed)?)
}
pub(crate) async fn remove_package(
    State(shared): State<AppState>,
    Path((owner, repo)): Path<(String, String)>,
) -> ApiResult {
    manager(&shared)?
        .remove_package(&format!("{owner}/{repo}"))
        .await
        .map_err(failed)?;
    Ok(Json(json!({})))
}
