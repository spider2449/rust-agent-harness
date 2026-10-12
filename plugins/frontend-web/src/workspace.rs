//! Switching the workspace, and the folder browser the UI picks it with.
//!
//! `PUT /api/workspace {root}` asks the distribution's `admin` service to
//! point the workspace plugins at `root`. `GET /api/fs/dirs?path=` lists the
//! subdirectories of a host directory (names only, never file contents) for
//! the local user, who already holds the access token.

use std::path::{Path, PathBuf};

use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
};
use rah_api::{admin::ADMIN, sandbox::SANDBOX};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{ApiError, ApiResult, AppState, status_json};

/// The most directories one listing returns.
const MAX_ENTRIES: usize = 1000;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SetWorkspace {
    root: String,
}

pub(crate) async fn set(
    State(shared): State<AppState>,
    Json(request): Json<SetWorkspace>,
) -> ApiResult {
    let admin = shared.kernel.get(ADMIN).ok_or_else(|| {
        ApiError(
            StatusCode::SERVICE_UNAVAILABLE,
            "plugin management is not available (no `admin` service)".into(),
        )
    })?;
    let root = request.root.trim();
    if root.is_empty() {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "the folder is empty".into(),
        ));
    }
    let workspace = admin.set_workspace(Path::new(root)).await?;
    let plugins: Vec<Value> = workspace
        .plugins
        .iter()
        .map(|(id, status)| {
            let mut entry = status_json(status);
            entry["id"] = json!(id.as_str());
            entry
        })
        .collect();
    Ok(Json(json!({
        "root": workspace.root.display().to_string(),
        "plugins": plugins,
    })))
}

#[derive(Deserialize)]
pub(crate) struct ListDirs {
    path: Option<String>,
}

/// `{path, parent, dirs: [{name, path, git}], roots}`. Without `path`, the
/// current workspace root (or the process directory).
pub(crate) async fn list(
    State(shared): State<AppState>,
    Query(query): Query<ListDirs>,
) -> ApiResult {
    let requested = match query.path.as_deref().map(str::trim) {
        Some(path) if !path.is_empty() => PathBuf::from(path),
        _ => shared
            .kernel
            .get(SANDBOX)
            .and_then(|sandbox| sandbox.root())
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(|| PathBuf::from(".")),
    };
    let path = tokio::fs::canonicalize(&requested).await.map_err(|error| {
        ApiError(
            StatusCode::BAD_REQUEST,
            format!("`{}`: {error}", requested.display()),
        )
    })?;
    let mut reader = tokio::fs::read_dir(&path).await.map_err(|error| {
        ApiError(
            StatusCode::BAD_REQUEST,
            format!("`{}`: {error}", requested.display()),
        )
    })?;
    let mut dirs = Vec::new();
    let mut truncated = false;
    while let Ok(Some(entry)) = reader.next_entry().await {
        let Ok(kind) = entry.file_type().await else {
            continue;
        };
        if !kind.is_dir() {
            continue;
        }
        if dirs.len() >= MAX_ENTRIES {
            truncated = true;
            break;
        }
        let child = entry.path();
        let git = tokio::fs::try_exists(child.join(".git"))
            .await
            .unwrap_or(false);
        dirs.push(json!({
            "name": entry.file_name().to_string_lossy(),
            "path": plain(&child),
            "git": git,
        }));
    }
    dirs.sort_by_key(|dir| dir["name"].as_str().unwrap_or_default().to_lowercase());
    let git = tokio::fs::try_exists(path.join(".git"))
        .await
        .unwrap_or(false);
    Ok(Json(json!({
        "path": plain(&path),
        "parent": path.parent().map(plain),
        "git": git,
        "dirs": dirs,
        "truncated": truncated,
        "roots": roots(),
    })))
}

/// A path for people: without Windows' verbatim `\\?\` prefix.
fn plain(path: &Path) -> String {
    let text = path.display().to_string();
    match text.strip_prefix(r"\\?\") {
        Some(rest) if !rest.starts_with("UNC\\") => rest.to_owned(),
        _ => text,
    }
}

/// Where browsing can start: the drives on Windows, `/` elsewhere.
fn roots() -> Vec<String> {
    if cfg!(windows) {
        ('A'..='Z')
            .map(|letter| format!("{letter}:\\"))
            .filter(|drive| Path::new(drive).is_dir())
            .collect()
    } else {
        vec!["/".to_owned()]
    }
}
