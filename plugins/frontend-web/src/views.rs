//! Views plugins contribute through the `ui-views` service (ADR 0062).
//!
//! - `GET /api/ui`: `{views: [{id, instance, view, title}]}`, every mounted
//!   instance's views, oldest instance first. `id` is `<instance>/<view>`,
//!   the panel id a layout uses.
//! - `GET /api/ui/{instance}/{view}?session=`: the view's content, refused
//!   when it is over the size bounds.
//! - `POST /api/ui/{instance}/{view}/actions/{action} {input, session}`:
//!   the user pressed a button or submitted a form.
//!
//! Views are data the page renders as text; actions run only on the user's
//! request, through this token-guarded API. Neither is a tool, so the model
//! cannot reach them. A provider gets [`ANSWER_TIMEOUT`] to answer.

use std::{sync::Arc, time::Duration};

use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use rah_api::{
    KernelHandle,
    ui::{UI_VIEWS, UiError, UiViews, ViewContext, ViewSpec},
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{ApiError, ApiResult, AppState, session_id};

/// How long a provider may take to render or act.
const ANSWER_TIMEOUT: Duration = Duration::from_secs(10);
/// The longest view title kept, in characters.
const MAX_TITLE_CHARS: usize = 40;

/// One contributed view, by its panel id.
pub(crate) struct Listed {
    pub id: String,
    pub instance: String,
    pub spec: ViewSpec,
}

/// Every valid view of every mounted `ui-views` instance; a view id used
/// twice by one instance is listed once.
pub(crate) fn listed(kernel: &KernelHandle) -> Vec<Listed> {
    let mut all: Vec<Listed> = Vec::new();
    for (instance, provider) in kernel.get_all(UI_VIEWS) {
        for mut spec in provider.views() {
            let id = format!("{instance}/{}", spec.id);
            if !ViewSpec::valid_id(&spec.id) || all.iter().any(|seen| seen.id == id) {
                continue;
            }
            spec.title = spec.title.chars().take(MAX_TITLE_CHARS).collect();
            if spec.title.trim().is_empty() {
                spec.title.clone_from(&spec.id);
            }
            all.push(Listed {
                id,
                instance: instance.clone(),
                spec,
            });
        }
    }
    all
}

pub(crate) async fn list(State(shared): State<AppState>) -> ApiResult {
    let views: Vec<Value> = listed(&shared.kernel)
        .into_iter()
        .map(|view| {
            json!({
                "id": view.id,
                "instance": view.instance,
                "view": view.spec.id,
                "title": view.spec.title,
            })
        })
        .collect();
    Ok(Json(json!({ "views": views })))
}

fn provider(shared: &AppState, instance: &str, view: &str) -> Result<Arc<dyn UiViews>, ApiError> {
    shared
        .kernel
        .get_all(UI_VIEWS)
        .into_iter()
        .find(|(name, provider)| {
            name == instance && provider.views().iter().any(|spec| spec.id == view)
        })
        .map(|(_, provider)| provider)
        .ok_or_else(|| {
            ApiError(
                StatusCode::NOT_FOUND,
                format!("no view `{instance}/{view}`"),
            )
        })
}

fn context(session: Option<&str>) -> Result<ViewContext, ApiError> {
    Ok(ViewContext {
        session: session
            .filter(|id| !id.is_empty())
            .map(session_id)
            .transpose()?,
    })
}

fn refused(error: UiError) -> ApiError {
    let status = match error {
        UiError::UnknownView(_) => StatusCode::NOT_FOUND,
        UiError::UnknownAction(_) | UiError::InvalidInput(_) => StatusCode::BAD_REQUEST,
        UiError::TooLarge(_) | UiError::Failed(_) => StatusCode::BAD_GATEWAY,
    };
    ApiError(status, error.to_string())
}

fn late() -> ApiError {
    ApiError(
        StatusCode::GATEWAY_TIMEOUT,
        format!(
            "the view did not answer within {} seconds",
            ANSWER_TIMEOUT.as_secs()
        ),
    )
}

#[derive(Deserialize)]
pub(crate) struct RenderQuery {
    session: Option<String>,
}

pub(crate) async fn render(
    State(shared): State<AppState>,
    Path((instance, view)): Path<(String, String)>,
    Query(query): Query<RenderQuery>,
) -> ApiResult {
    let provider = provider(&shared, &instance, &view)?;
    let cx = context(query.session.as_deref())?;
    let content = tokio::time::timeout(ANSWER_TIMEOUT, provider.render(&view, &cx))
        .await
        .map_err(|_| late())?
        .map_err(refused)?;
    content.check().map_err(refused)?;
    Ok(Json(json!(content)))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ActRequest {
    #[serde(default)]
    input: Value,
    #[serde(default)]
    session: Option<String>,
}

pub(crate) async fn act(
    State(shared): State<AppState>,
    Path((instance, view, action)): Path<(String, String, String)>,
    Json(request): Json<ActRequest>,
) -> ApiResult {
    let provider = provider(&shared, &instance, &view)?;
    let cx = context(request.session.as_deref())?;
    let outcome = tokio::time::timeout(
        ANSWER_TIMEOUT,
        provider.act(&view, &action, request.input, &cx),
    )
    .await
    .map_err(|_| late())?
    .map_err(refused)?;
    Ok(Json(json!(outcome)))
}
