//! The workspace file browser: read-only views of the workspace for the
//! local user, who already holds the access token.
//!
//! `GET /api/files?folder=&path=` lists one directory of a workspace folder
//! and `GET /api/files/content?folder=&path=` returns the start of one file
//! as text. `folder` is one of the workspace's folders (see
//! [`crate::workspace`]), the active one (the `sandbox` service's root) when
//! it is missing. Paths are relative to that folder and must stay inside it:
//! absolute paths, `..` and links leading outside are refused. Nothing here
//! writes, and no tool can reach these routes.

use std::path::{Component, Path, PathBuf};

use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
};
use rah_api::sandbox::SANDBOX;
use serde::Deserialize;
use serde_json::json;
use tokio::io::AsyncReadExt;

use crate::{ApiError, ApiResult, AppState};

/// The most entries one directory listing returns.
const MAX_ENTRIES: usize = 2000;
/// The most bytes of a file one read returns.
const MAX_BYTES: u64 = 512 * 1024;
/// Never listed: Git's own data is not workspace content.
const HIDDEN: &[&str] = &[".git"];

#[derive(Deserialize)]
pub(crate) struct FileQuery {
    folder: Option<String>,
    path: Option<String>,
}

/// `{root, path, entries: [{name, path, dir}], truncated}`, folders first.
pub(crate) async fn list(
    State(shared): State<AppState>,
    Query(query): Query<FileQuery>,
) -> ApiResult {
    let root = crate::workspace::readable_folder(&shared, query.folder.as_deref()).await?;
    let relative = query.path.unwrap_or_default();
    let path = resolve(&root, &relative).await?;
    let mut reader = tokio::fs::read_dir(&path)
        .await
        .map_err(|error| bad_request(&relative, error))?;
    let mut entries = Vec::new();
    let mut truncated = false;
    while let Ok(Some(entry)) = reader.next_entry().await {
        let name = entry.file_name().to_string_lossy().into_owned();
        if HIDDEN.contains(&name.as_str()) {
            continue;
        }
        if entries.len() >= MAX_ENTRIES {
            truncated = true;
            break;
        }
        // Follows links, so a linked folder can be opened like any other.
        let dir = tokio::fs::metadata(entry.path())
            .await
            .is_ok_and(|metadata| metadata.is_dir());
        entries.push((dir, name));
    }
    entries.sort_by(|(a_dir, a), (b_dir, b)| {
        b_dir
            .cmp(a_dir)
            .then_with(|| a.to_lowercase().cmp(&b.to_lowercase()))
    });
    let base = normalize(&relative);
    let entries: Vec<_> = entries
        .into_iter()
        .map(|(dir, name)| {
            let path = if base.is_empty() {
                name.clone()
            } else {
                format!("{base}/{name}")
            };
            json!({ "name": name, "path": path, "dir": dir })
        })
        .collect();
    Ok(Json(json!({
        "root": crate::workspace::plain(&root),
        "path": base,
        "entries": entries,
        "truncated": truncated,
    })))
}

/// `{path, size, text, truncated, binary}`. A file with a NUL byte in what
/// was read is binary and returns no text.
pub(crate) async fn content(
    State(shared): State<AppState>,
    Query(query): Query<FileQuery>,
) -> ApiResult {
    let root = crate::workspace::readable_folder(&shared, query.folder.as_deref()).await?;
    let relative = query.path.unwrap_or_default();
    let path = resolve(&root, &relative).await?;
    let file = tokio::fs::File::open(&path)
        .await
        .map_err(|error| bad_request(&relative, error))?;
    let metadata = file
        .metadata()
        .await
        .map_err(|error| bad_request(&relative, error))?;
    if !metadata.is_file() {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            format!("`{relative}` is not a file"),
        ));
    }
    let mut bytes = Vec::new();
    file.take(MAX_BYTES)
        .read_to_end(&mut bytes)
        .await
        .map_err(|error| bad_request(&relative, error))?;
    let binary = bytes.contains(&0);
    let text = if binary {
        String::new()
    } else {
        String::from_utf8_lossy(&bytes).into_owned()
    };
    Ok(Json(json!({
        "path": normalize(&relative),
        "size": metadata.len(),
        "text": text,
        "truncated": metadata.len() > MAX_BYTES,
        "binary": binary,
    })))
}

pub(crate) async fn workspace_root(shared: &AppState) -> Result<PathBuf, ApiError> {
    let root = shared
        .kernel
        .get(SANDBOX)
        .and_then(|sandbox| sandbox.root())
        .ok_or_else(|| {
            ApiError(
                StatusCode::SERVICE_UNAVAILABLE,
                "no workspace folder is set (no `sandbox` service)".into(),
            )
        })?;
    tokio::fs::canonicalize(&root).await.map_err(|error| {
        ApiError(
            StatusCode::SERVICE_UNAVAILABLE,
            format!("the workspace `{}`: {error}", root.display()),
        )
    })
}

/// The canonical path of `relative` inside `root` (itself canonical).
async fn resolve(root: &Path, relative: &str) -> Result<PathBuf, ApiError> {
    let joined = root.join(checked(relative)?);
    let path = tokio::fs::canonicalize(&joined)
        .await
        .map_err(|error| bad_request(relative, error))?;
    // A link may still lead outside the workspace.
    if !path.starts_with(root) {
        return Err(outside(relative));
    }
    Ok(path)
}

/// `relative` as a path of plain names: no root, drive, stream (`:`), `.`
/// or `..`. Both separators count on every platform, so a path means the
/// same everywhere.
pub(crate) fn checked(relative: &str) -> Result<PathBuf, ApiError> {
    let trimmed = relative.trim();
    if trimmed.starts_with(['/', '\\']) {
        return Err(outside(relative));
    }
    let mut path = PathBuf::new();
    for part in trimmed.split(['/', '\\']) {
        match part {
            "" | "." => {}
            ".." => return Err(outside(relative)),
            name if name.contains(':') => return Err(outside(relative)),
            name => {
                // A part the platform still reads as more than a name.
                if !matches!(
                    Path::new(name).components().collect::<Vec<_>>()[..],
                    [Component::Normal(_)]
                ) {
                    return Err(outside(relative));
                }
                path.push(name);
            }
        }
    }
    Ok(path)
}

/// A relative path as the UI shows it: `/`-separated, without empty parts.
fn normalize(relative: &str) -> String {
    relative
        .split(['/', '\\'])
        .filter(|part| !part.is_empty() && *part != ".")
        .collect::<Vec<_>>()
        .join("/")
}

fn outside(relative: &str) -> ApiError {
    ApiError(
        StatusCode::BAD_REQUEST,
        format!("`{relative}` is not inside the workspace"),
    )
}

fn bad_request(relative: &str, error: std::io::Error) -> ApiError {
    ApiError(StatusCode::BAD_REQUEST, format!("`{relative}`: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_plain_relative_names_are_accepted() {
        assert_eq!(checked("").ok(), Some(PathBuf::new()));
        assert_eq!(
            checked("src/./lib.rs").ok(),
            Some(Path::new("src").join("lib.rs"))
        );
        for refused in [
            "..",
            "src/../../x",
            "/etc/passwd",
            r"C:\Windows",
            r"\\server\share",
        ] {
            assert!(checked(refused).is_err(), "{refused}");
        }
    }

    #[test]
    fn paths_are_shown_with_forward_slashes() {
        assert_eq!(normalize(r"src\ui/./app.js"), "src/ui/app.js");
        assert_eq!(normalize("/"), "");
    }
}
