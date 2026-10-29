//! Undoing the file changes of a session (ADR 0060), through the `file-undo`
//! service: `GET /api/sessions/{id}/changes` lists them, newest first, and
//! `POST /api/sessions/{id}/undo` takes back the newest one. The next turn of
//! the session tells the model (see [`crate::ws`]).

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use rah_api::undo::{self, FILE_UNDO, UndoError};
use serde_json::{Value, json};

use crate::{ApiError, ApiResult, AppState, session_id};

pub(crate) async fn changes(State(shared): State<AppState>, Path(id): Path<String>) -> ApiResult {
    let session = session_id(&id)?;
    let changes: Vec<Value> = undo::changes(&shared.kernel, &session)
        .into_iter()
        .map(|change| json!({ "tool": change.tool, "files": change.files, "at_ms": change.at_ms }))
        .collect();
    Ok(Json(json!({
        "available": !shared.kernel.get_all(FILE_UNDO).is_empty(),
        "changes": changes,
    })))
}

pub(crate) async fn undo(State(shared): State<AppState>, Path(id): Path<String>) -> ApiResult {
    let session = session_id(&id)?;
    let report = match undo::undo_newest(&shared.kernel, &session).await {
        None => {
            return Err(ApiError(
                StatusCode::SERVICE_UNAVAILABLE,
                "undo is not available (no `file-undo` service)".into(),
            ));
        }
        Some(Ok(report)) => report,
        Some(Err(error)) => {
            let status = match error {
                UndoError::Nothing => StatusCode::NOT_FOUND,
                UndoError::Changed { .. } => StatusCode::CONFLICT,
                UndoError::Failed { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            };
            return Err(ApiError(status, error.to_string()));
        }
    };
    Ok(Json(json!({
        "tool": report.change.tool,
        "restored": report.restored,
        "removed": report.removed,
        "summary": report.summary(),
        "remaining": undo::changes(&shared.kernel, &session).len(),
    })))
}
