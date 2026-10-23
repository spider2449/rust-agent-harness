//! The workspace, and the folder browser the UI picks its folders with.
//!
//! Like a multi-root workspace in VS Code, a workspace is a list of folders
//! with an optional name. One folder is active: the `sandbox` service's root,
//! where tools work. The file browser shows every folder, and source control
//! reads the active folder through the sandbox (the others only show their
//! checked-out branch, read from Git's `HEAD` file). The folders are kept in
//! `storage` under the key `workspace`; when the active root is none of them
//! (`rah --root` named another folder), the workspace is that folder alone.
//!
//! `GET /api/workspace` describes the workspace. `PUT /api/workspace {root,
//! folders, name}` makes `root` the active folder (asking the distribution's
//! `admin` service to point the workspace plugins at it when it moves) and,
//! with `folders`, sets the workspace's folders and name. `GET
//! /api/fs/dirs?path=` lists the subdirectories of a host directory (names
//! only, never file contents) for the local user, who already holds the
//! access token.

use std::{
    path::{Path, PathBuf},
    sync::PoisonError,
};

use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
};
use rah_api::{admin::ADMIN, sandbox::SANDBOX, storage::STORAGE};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{ApiError, ApiResult, AppState, Shared, git, status_json};

/// The most directories one listing returns.
const MAX_ENTRIES: usize = 1000;
/// The most folders one workspace holds, and its longest name.
pub(crate) const MAX_FOLDERS: usize = 32;
pub(crate) const MAX_NAME_CHARS: usize = 100;
/// The `storage` key of the workspace's folders.
const STORED: &str = "workspace";

/// The folders of the workspace, as plain canonical paths, and its name.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Folders {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default)]
    pub folders: Vec<String>,
}

impl Folders {
    fn contains(&self, path: &str) -> bool {
        self.folders.iter().any(|folder| same(folder, path))
    }
}

/// The workspace around the active root: the stored one when the root is one
/// of its folders, otherwise the root alone (no folders without a root).
pub(crate) fn effective(stored: &Folders, root: Option<&str>) -> Folders {
    match root {
        Some(root) if stored.contains(root) => stored.clone(),
        Some(root) => Folders {
            name: None,
            folders: vec![root.to_owned()],
        },
        None => Folders::default(),
    }
}

/// Whether two plain canonical paths name the same folder (ignoring case on
/// Windows, whose file systems usually do).
pub(crate) fn same(a: &str, b: &str) -> bool {
    if cfg!(windows) {
        a.eq_ignore_ascii_case(b)
    } else {
        a == b
    }
}

/// The active folder: the `sandbox` service's root, canonical and plain.
pub(crate) async fn active_root(shared: &Shared) -> Option<String> {
    let root = shared.kernel.get(SANDBOX)?.root()?;
    let canonical = tokio::fs::canonicalize(&root).await.ok()?;
    Some(plain(&canonical))
}

/// The stored folders, read from `storage` on first use.
async fn stored(shared: &Shared) -> Folders {
    if let Some(folders) = shared
        .folders
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
    {
        return folders;
    }
    let loaded = match shared.kernel.get(STORAGE) {
        Some(storage) => storage
            .get(shared.id.as_str(), STORED)
            .await
            .ok()
            .flatten()
            .and_then(|bytes| serde_json::from_slice::<Folders>(&bytes).ok())
            .unwrap_or_default(),
        None => Folders::default(),
    };
    shared
        .folders
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get_or_insert(loaded)
        .clone()
}

/// The workspace as it is now, and the active root.
pub(crate) async fn current(shared: &Shared) -> (Option<String>, Folders) {
    let root = active_root(shared).await;
    let folders = effective(&stored(shared).await, root.as_deref());
    (root, folders)
}

/// Keeps `folders` for this process and, with a `storage` service, for the
/// next start. Returns whether they were stored.
async fn remember(shared: &Shared, folders: &Folders) -> bool {
    *shared
        .folders
        .lock()
        .unwrap_or_else(PoisonError::into_inner) = Some(folders.clone());
    match (shared.kernel.get(STORAGE), serde_json::to_vec(folders)) {
        (Some(storage), Ok(bytes)) => storage
            .put(shared.id.as_str(), STORED, &bytes)
            .await
            .is_ok(),
        _ => false,
    }
}

/// `{name, root, folders: [{path, name, active, repos: [{path, branch}]}]}`.
/// `repos` are the folder's own repository (`path` empty) and those in its
/// folders below, with the branch from Git's `HEAD` file.
async fn describe(shared: &Shared) -> Value {
    let (root, workspace) = current(shared).await;
    let mut folders = Vec::new();
    for folder in &workspace.folders {
        let path = Path::new(folder);
        folders.push(json!({
            "path": folder,
            "name": path.file_name().map_or_else(|| folder.clone(), |name| name.to_string_lossy().into_owned()),
            "active": root.as_deref().is_some_and(|root| same(root, folder)),
            "repos": repo_heads(path).await,
        }));
    }
    json!({ "name": workspace.name, "root": root, "folders": folders })
}

/// The repositories of a folder with their checked-out branch, without
/// running Git.
async fn repo_heads(folder: &Path) -> Vec<Value> {
    let mut repos = Vec::new();
    if let Some(branch) = git::head_branch(folder).await {
        repos.push(json!({ "path": "", "branch": branch }));
    }
    for repo in git::nested_repos(folder).await {
        if let Some(branch) = git::head_branch(&folder.join(&repo)).await {
            repos.push(json!({ "path": repo, "branch": branch }));
        }
    }
    repos
}

/// `GET /api/workspace`.
pub(crate) async fn get(State(shared): State<AppState>) -> ApiResult {
    Ok(Json(describe(&shared).await))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SetWorkspace {
    /// The folder to make active.
    root: String,
    /// The workspace's folders (`root` is added first when missing). Without
    /// them, `root` stays in the workspace when it is one of its folders;
    /// otherwise the workspace becomes `root` alone, without a name.
    #[serde(default)]
    folders: Option<Vec<String>>,
    /// The workspace's name, with `folders`.
    #[serde(default)]
    name: Option<String>,
}

/// `{root, plugins, workspace}`: the active root, the plugins restarted in
/// it (none when it did not move) and the workspace as `GET` describes it.
pub(crate) async fn set(
    State(shared): State<AppState>,
    Json(request): Json<SetWorkspace>,
) -> ApiResult {
    let root = request.root.trim();
    if root.is_empty() {
        return Err(bad_request("the folder is empty".into()));
    }
    let root = folder(root).await?;
    let (active, workspace) = current(&shared).await;
    let mut next = match request.folders {
        Some(paths) => {
            if paths.len() > MAX_FOLDERS {
                return Err(bad_request(format!(
                    "a workspace holds at most {MAX_FOLDERS} folders"
                )));
            }
            let mut next = Folders {
                name: request
                    .name
                    .map(|name| name.trim().to_owned())
                    .filter(|name| !name.is_empty()),
                folders: Vec::new(),
            };
            if next
                .name
                .as_ref()
                .is_some_and(|name| name.chars().count() > MAX_NAME_CHARS)
            {
                return Err(bad_request("the workspace name is too long".into()));
            }
            for path in paths {
                let path = folder(path.trim()).await?;
                if !next.contains(&path) {
                    next.folders.push(path);
                }
            }
            if !next.contains(&root) {
                next.folders.insert(0, root.clone());
            }
            next
        }
        None => effective(&workspace, Some(&root)),
    };
    let moved = !active.as_deref().is_some_and(|active| same(active, &root));
    let (root, plugins) = if moved {
        let admin = shared.kernel.get(ADMIN).ok_or_else(|| {
            ApiError(
                StatusCode::SERVICE_UNAVAILABLE,
                "plugin management is not available (no `admin` service)".into(),
            )
        })?;
        let switched = admin.set_workspace(Path::new(&root)).await?;
        let plugins: Vec<Value> = switched
            .plugins
            .iter()
            .map(|(id, status)| {
                let mut entry = status_json(status);
                entry["id"] = json!(id.as_str());
                entry
            })
            .collect();
        (switched.root.display().to_string(), plugins)
    } else {
        (root, Vec::new())
    };
    // The distribution may spell the root differently (case on Windows).
    if let Some(entry) = next.folders.iter_mut().find(|folder| same(folder, &root)) {
        entry.clone_from(&root);
    }
    let remembered = remember(&shared, &next).await;
    Ok(Json(json!({
        "root": root,
        "plugins": plugins,
        "workspace": describe(&shared).await,
        "remembered": remembered,
    })))
}

/// An existing directory as a plain canonical path.
async fn folder(path: &str) -> Result<String, ApiError> {
    let canonical = tokio::fs::canonicalize(path)
        .await
        .map_err(|error| bad_request(format!("`{path}`: {error}")))?;
    if !canonical.is_dir() {
        return Err(bad_request(format!("`{path}` is not a directory")));
    }
    Ok(plain(&canonical))
}

/// The canonical path of a workspace folder that the file browser may read:
/// the active root without `folder`, otherwise one of the workspace's folders.
pub(crate) async fn readable_folder(
    shared: &Shared,
    folder: Option<&str>,
) -> Result<PathBuf, ApiError> {
    let unavailable = |message: String| ApiError(StatusCode::SERVICE_UNAVAILABLE, message);
    let (root, workspace) = current(shared).await;
    let wanted = match folder.map(str::trim) {
        Some(folder) if !folder.is_empty() => folder.to_owned(),
        _ => root.ok_or_else(|| {
            unavailable("no workspace folder is set (no `sandbox` service)".into())
        })?,
    };
    let canonical = tokio::fs::canonicalize(&wanted)
        .await
        .map_err(|error| unavailable(format!("the workspace folder `{wanted}`: {error}")))?;
    if !workspace.contains(&plain(&canonical)) {
        return Err(bad_request(format!(
            "`{wanted}` is not a folder of the workspace"
        )));
    }
    Ok(canonical)
}

fn bad_request(message: String) -> ApiError {
    ApiError(StatusCode::BAD_REQUEST, message)
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
pub(crate) fn plain(path: &Path) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn folders(name: Option<&str>, paths: &[&str]) -> Folders {
        Folders {
            name: name.map(str::to_owned),
            folders: paths.iter().map(|path| (*path).to_owned()).collect(),
        }
    }

    #[test]
    fn the_stored_workspace_applies_while_the_root_is_one_of_its_folders() {
        let stored = folders(Some("product"), &["/work/api", "/work/web"]);
        assert_eq!(effective(&stored, Some("/work/web")), stored);
        // `rah --root` named another folder: that folder alone.
        assert_eq!(
            effective(&stored, Some("/elsewhere")),
            folders(None, &["/elsewhere"])
        );
        assert_eq!(effective(&stored, None), Folders::default());
    }

    #[test]
    fn folders_compare_like_the_platform() {
        assert!(same("/work/api", "/work/api"));
        assert_eq!(same(r"D:\Work\API", r"d:\work\api"), cfg!(windows));
    }
}
