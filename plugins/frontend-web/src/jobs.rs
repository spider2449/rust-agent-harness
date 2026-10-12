//! Background jobs through the `scheduler` service.

use std::{str::FromStr, sync::Arc, time::Duration};

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use rah_api::scheduler::{JobInfo, JobSpec, JobState, SCHEDULER, Scheduler};
use rah_protocol::SessionId;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{ApiError, ApiResult, AppState};

fn scheduler(shared: &AppState) -> Result<Arc<dyn Scheduler>, ApiError> {
    shared.kernel.get(SCHEDULER).ok_or_else(|| {
        ApiError(
            StatusCode::SERVICE_UNAVAILABLE,
            "no scheduler service is mounted".into(),
        )
    })
}

fn job_json(job: &JobInfo) -> Value {
    let (state, detail) = match &job.state {
        JobState::Waiting => ("waiting", None),
        JobState::Running => ("running", None),
        JobState::Done { answer } => ("done", Some(answer.as_str())),
        JobState::Failed { message } => ("failed", Some(message.as_str())),
        JobState::Cancelled => ("cancelled", None),
    };
    json!({
        "id": job.id,
        "input": job.spec.input,
        "session": job.spec.session.as_ref().map(ToString::to_string),
        "every_secs": job.spec.every.map(|every| every.as_secs()),
        "state": state,
        "detail": detail,
        "runs": job.runs,
        "last_session": job.last_session.as_ref().map(ToString::to_string),
    })
}

/// `GET /api/jobs`.
pub(crate) async fn list(State(shared): State<AppState>) -> ApiResult {
    let jobs: Vec<Value> = scheduler(&shared)?.list().iter().map(job_json).collect();
    Ok(Json(json!(jobs)))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Submit {
    input: String,
    session: Option<String>,
    #[serde(default)]
    delay_secs: u64,
    every_secs: Option<u64>,
}

/// `POST /api/jobs`.
pub(crate) async fn submit(
    State(shared): State<AppState>,
    Json(submit): Json<Submit>,
) -> ApiResult {
    let session = submit
        .session
        .map(|id| {
            SessionId::from_str(&id).map_err(|_| {
                ApiError(
                    StatusCode::BAD_REQUEST,
                    format!("`{id}` is not a session id"),
                )
            })
        })
        .transpose()?;
    let job = scheduler(&shared)?
        .submit(JobSpec {
            input: submit.input,
            session,
            delay: Duration::from_secs(submit.delay_secs),
            every: submit.every_secs.map(Duration::from_secs),
        })
        .map_err(|error| ApiError(StatusCode::BAD_REQUEST, error.to_string()))?;
    Ok(Json(job_json(&job)))
}

/// `DELETE /api/jobs/{id}`.
pub(crate) async fn cancel(State(shared): State<AppState>, Path(id): Path<String>) -> ApiResult {
    if scheduler(&shared)?.cancel(&id) {
        Ok(Json(json!({ "id": id, "state": "cancelled" })))
    } else {
        Err(ApiError(
            StatusCode::NOT_FOUND,
            format!("job `{id}` is not pending"),
        ))
    }
}
