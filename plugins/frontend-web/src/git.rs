//! Git views of the workspace for the file panel: the changed files and one
//! file's diff (read-only), and the branches with switching to one.
//!
//! The workspace may hold several repositories: the root's own (when the root
//! is in one) and the folders below it, a few levels deep, that hold a `.git`
//! entry. Every view takes the repository as `repo`, its folder relative to
//! the workspace root (empty for the root).
//!
//! Every view is one `git` run through the `sandbox` service in the
//! repository's folder: a fixed argument vector, never a shell, with
//! `--no-optional-locks` (no index writes), `--literal-pathspecs` (a path is
//! only a path), `--no-ext-diff` and `--no-textconv` (no configured diff
//! programs). Paths are checked like the file browser's.

use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
    time::Duration,
};

use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
};
use rah_api::sandbox::{OutputLimits, ProgramSpec, SANDBOX, Termination};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{ApiError, ApiResult, AppState, files};

const GIT_TIMEOUT: Duration = Duration::from_secs(10);
/// The most bytes of a diff one view returns.
const MAX_PATCH: usize = 512 * 1024;
/// The most changed files one listing returns, per repository.
const MAX_CHANGES: usize = 2000;
/// How many folder levels below the root are searched for repositories.
const MAX_DEPTH: usize = 3;
/// The most nested repositories one search returns.
const MAX_REPOS: usize = 32;
/// Folders never searched for repositories: dependencies and build output.
/// Hidden folders (a leading `.`) are skipped as well.
const SKIPPED: &[&str] = &["node_modules", "target"];

/// One finished `git` run: its output, and whether it was cut at the limit.
struct GitOutput {
    stdout: String,
    truncated: bool,
}

/// One `git` run in the repository folder `repo` (empty for the root).
async fn git(
    shared: &AppState,
    repo: &str,
    args: &[&str],
    stdout_bytes: usize,
) -> Result<GitOutput, ApiError> {
    let sandbox = shared.kernel.get(SANDBOX).ok_or_else(|| {
        ApiError(
            StatusCode::SERVICE_UNAVAILABLE,
            "no workspace folder is set (no `sandbox` service)".into(),
        )
    })?;
    let mut argv = vec!["--no-optional-locks", "--literal-pathspecs"];
    argv.extend_from_slice(args);
    let mut spec = ProgramSpec::new(
        "git",
        argv.into_iter().map(String::from).collect(),
        GIT_TIMEOUT,
    );
    spec.cwd = repo_dir(repo)?;
    spec.limits = OutputLimits {
        stdout_bytes,
        stderr_bytes: 16 * 1024,
    };
    let output = sandbox
        .run(spec, shared.stop.child_token())
        .await
        .map_err(|error| ApiError(StatusCode::BAD_GATEWAY, error.to_string()))?;
    let truncated = output.termination == Some(Termination::OutputLimit);
    if output.exit_code != Some(0) && !truncated {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let reason = match output.termination {
            Some(Termination::TimedOut) => "git timed out",
            Some(Termination::Cancelled) => "git was cancelled",
            _ => stderr.lines().next().unwrap_or("git failed").trim(),
        };
        return Err(ApiError(StatusCode::BAD_REQUEST, reason.to_owned()));
    }
    Ok(GitOutput {
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        truncated,
    })
}

/// The working directory for a repository folder: `None` for the root.
fn repo_dir(repo: &str) -> Result<Option<PathBuf>, ApiError> {
    let path = files::checked(repo)?;
    Ok((!path.as_os_str().is_empty()).then_some(path))
}

/// The branch checked out in the repository folder `dir`, read from Git's
/// `HEAD` file without running Git (for workspace folders outside the
/// sandbox root): `(detached)` for a detached HEAD, `None` when `dir` holds no
/// repository. A `.git` file (a linked work tree or a submodule) names the
/// Git folder with `gitdir:`.
pub(crate) async fn head_branch(dir: &Path) -> Option<String> {
    let dot_git = dir.join(".git");
    let git_dir = match tokio::fs::metadata(&dot_git).await.ok()? {
        metadata if metadata.is_dir() => dot_git,
        _ => {
            let text = tokio::fs::read_to_string(&dot_git).await.ok()?;
            dir.join(text.trim().strip_prefix("gitdir:")?.trim())
        }
    };
    let head = tokio::fs::read_to_string(git_dir.join("HEAD")).await.ok()?;
    Some(parse_head(&head))
}

/// The branch a `HEAD` file names: `ref: refs/heads/<branch>`, or a commit
/// for a detached HEAD.
fn parse_head(text: &str) -> String {
    match text.trim().strip_prefix("ref:") {
        Some(target) => {
            let target = target.trim();
            target
                .strip_prefix("refs/heads/")
                .unwrap_or(target)
                .to_owned()
        }
        None => "(detached)".to_owned(),
    }
}

/// The folders below `root` (canonical) that hold a `.git` entry (a folder,
/// or a file for a submodule or linked work tree), `/`-separated and sorted,
/// at most [`MAX_DEPTH`] levels deep. Links are not followed, so the search
/// stays inside the workspace and cannot loop.
pub(crate) async fn nested_repos(root: &Path) -> Vec<String> {
    let mut found = Vec::new();
    let mut queue = VecDeque::from([(root.to_path_buf(), String::new(), 0)]);
    while let Some((dir, name, depth)) = queue.pop_front() {
        let Ok(mut reader) = tokio::fs::read_dir(&dir).await else {
            continue;
        };
        while let Ok(Some(entry)) = reader.next_entry().await {
            let Ok(kind) = entry.file_type().await else {
                continue;
            };
            let child = entry.file_name().to_string_lossy().into_owned();
            if !kind.is_dir() || child.starts_with('.') || SKIPPED.contains(&child.as_str()) {
                continue;
            }
            let child_name = if name.is_empty() {
                child
            } else {
                format!("{name}/{child}")
            };
            if tokio::fs::symlink_metadata(entry.path().join(".git"))
                .await
                .is_ok()
            {
                found.push(child_name.clone());
                if found.len() >= MAX_REPOS {
                    found.sort();
                    return found;
                }
            }
            if depth + 1 < MAX_DEPTH {
                queue.push_back((entry.path(), child_name, depth + 1));
            }
        }
    }
    found.sort();
    found
}

/// `{repos: [{path, branch, changes: [{path, orig, status, staged}],
/// truncated} | {path, error}]}`: the root's repository first, then the
/// nested ones. Change paths are relative to the workspace root. Without any
/// repository, `{error}`: a state to show, not a failed request.
pub(crate) async fn changes(State(shared): State<AppState>) -> ApiResult {
    let root = repo_changes(&shared, String::new()).await?;
    let nested = match files::workspace_root(&shared).await {
        Ok(path) => nested_repos(&path).await,
        Err(_) => Vec::new(),
    };
    if nested.is_empty()
        && let Some(error) = root.get("error")
    {
        return Ok(Json(json!({ "error": error })));
    }
    let mut repos = Vec::new();
    if root.get("error").is_none() {
        repos.push(root);
    }
    let listings =
        futures::future::join_all(nested.into_iter().map(|repo| repo_changes(&shared, repo))).await;
    for listing in listings {
        repos.push(listing?);
    }
    Ok(Json(json!({ "repos": repos })))
}

/// One repository's changes, or `{path, error}` when Git cannot read it.
async fn repo_changes(shared: &AppState, repo: String) -> Result<Value, ApiError> {
    let output = match git(
        shared,
        &repo,
        &[
            "status",
            "--porcelain=v2",
            "--branch",
            "-z",
            "--untracked-files=all",
        ],
        1024 * 1024,
    )
    .await
    {
        Ok(output) => output,
        Err(ApiError(StatusCode::BAD_REQUEST, reason)) => {
            return Ok(json!({ "path": repo, "error": reason }));
        }
        Err(error) => return Err(error),
    };
    let (branch, mut changes) = parse_status(&output.stdout);
    let truncated = output.truncated || changes.len() > MAX_CHANGES;
    changes.truncate(MAX_CHANGES);
    if !repo.is_empty() {
        for change in &mut changes {
            for key in ["path", "orig"] {
                if let Some(path) = change[key].as_str() {
                    change[key] = json!(format!("{repo}/{path}"));
                }
            }
        }
    }
    Ok(json!({
        "path": repo,
        "branch": branch,
        "changes": changes,
        "truncated": truncated,
    }))
}

#[derive(Deserialize)]
pub(crate) struct RepoQuery {
    #[serde(default)]
    repo: String,
}

/// `{current, branches: [{name, remote}]}`: the local branches, then the
/// remote ones that have no local branch of the same name.
pub(crate) async fn branches(
    State(shared): State<AppState>,
    Query(query): Query<RepoQuery>,
) -> ApiResult {
    let (current, branches) = list_branches(&shared, &query.repo).await?;
    Ok(Json(json!({
        "current": current,
        "branches": branches.iter().map(|branch| json!({
            "name": branch.name,
            "remote": branch.remote,
        })).collect::<Vec<_>>(),
    })))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Checkout {
    branch: String,
    #[serde(default)]
    repo: String,
}

/// `POST /api/git/checkout {branch, repo}`: switches to a branch that
/// `GET /api/git/branches?repo=` lists (never an arbitrary revision or
/// option). A remote-only branch gets a local tracking branch. Git refuses
/// when local changes would be overwritten, and that reason is returned as
/// the error.
pub(crate) async fn checkout(
    State(shared): State<AppState>,
    Json(request): Json<Checkout>,
) -> ApiResult {
    let (_, branches) = list_branches(&shared, &request.repo).await?;
    let Some(branch) = branches.iter().find(|branch| branch.name == request.branch) else {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            format!("`{}` is not a branch of this repository", request.branch),
        ));
    };
    let args: &[&str] = if branch.remote {
        &["switch", "--track", &branch.name]
    } else {
        &["switch", &branch.name]
    };
    git(&shared, &request.repo, args, 16 * 1024).await?;
    Ok(Json(json!({ "branch": branch.name })))
}

struct Branch {
    name: String,
    remote: bool,
}

/// The current branch (none when detached) and every branch to switch to.
async fn list_branches(
    shared: &AppState,
    repo: &str,
) -> Result<(Option<String>, Vec<Branch>), ApiError> {
    let output = git(
        shared,
        repo,
        &[
            "for-each-ref",
            "--format=%(HEAD)%09%(refname)",
            "refs/heads",
            "refs/remotes",
        ],
        1024 * 1024,
    )
    .await?;
    Ok(parse_branches(&output.stdout))
}

/// `for-each-ref` lines of `HEAD-mark TAB refname` as the current branch and
/// the branches: local ones first, then remote ones without a local twin.
fn parse_branches(text: &str) -> (Option<String>, Vec<Branch>) {
    let (mut current, mut local, mut remote) = (None, Vec::new(), Vec::new());
    for line in text.lines() {
        let Some((mark, refname)) = line.split_once('\t') else {
            continue;
        };
        if let Some(name) = refname.strip_prefix("refs/heads/") {
            if mark == "*" {
                current = Some(name.to_owned());
            }
            local.push(name.to_owned());
        } else if let Some(name) = refname.strip_prefix("refs/remotes/") {
            // `origin/HEAD` is a pointer, not a branch.
            if !name.ends_with("/HEAD") && name.contains('/') {
                remote.push(name.to_owned());
            }
        }
    }
    local.sort();
    remote.sort();
    let remote = remote.into_iter().filter(|name| {
        let short = name
            .split_once('/')
            .map_or(name.as_str(), |(_, short)| short);
        !local.iter().any(|existing| existing == short)
    });
    let branches = local
        .iter()
        .map(|name| Branch {
            name: name.clone(),
            remote: false,
        })
        .chain(remote.map(|name| Branch { name, remote: true }))
        .collect();
    (current, branches)
}

#[derive(Deserialize)]
pub(crate) struct DiffQuery {
    path: String,
    #[serde(default)]
    repo: String,
}

/// `{path, text, truncated}`: the file's changes against `HEAD` (staged and
/// not), or against the index before the first commit. `path` is relative to
/// the workspace root and must lie inside `repo`.
pub(crate) async fn diff(
    State(shared): State<AppState>,
    Query(query): Query<DiffQuery>,
) -> ApiResult {
    let repo = slash_path(&query.repo)?;
    let path = slash_path(&query.path)?;
    let Some(inner) = inside_repo(&repo, &path) else {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            format!("`{path}` is not inside the repository `{repo}`"),
        ));
    };
    if inner.is_empty() {
        return Err(ApiError(StatusCode::BAD_REQUEST, "no file named".into()));
    }
    let base = ["diff", "--no-color", "--no-ext-diff", "--no-textconv"];
    let with_head = [&base[..], &["HEAD", "--", inner]].concat();
    let output = match git(&shared, &repo, &with_head, MAX_PATCH).await {
        Ok(output) => output,
        // No commit yet: the changes against the index.
        Err(_) => {
            git(
                &shared,
                &repo,
                &[&base[..], &["--", inner]].concat(),
                MAX_PATCH,
            )
            .await?
        }
    };
    Ok(Json(json!({
        "path": path,
        "text": output.stdout,
        "truncated": output.truncated,
    })))
}

/// A checked relative path, `/`-separated.
fn slash_path(relative: &str) -> Result<String, ApiError> {
    Ok(files::checked(relative)?
        .to_string_lossy()
        .replace('\\', "/"))
}

/// `path` relative to the repository folder `repo` (both `/`-separated and
/// relative to the workspace root), or `None` when it lies outside.
fn inside_repo<'a>(repo: &str, path: &'a str) -> Option<&'a str> {
    if repo.is_empty() {
        return Some(path);
    }
    path.strip_prefix(repo)?.strip_prefix('/')
}

/// `git status --porcelain=v2 --branch -z` as the branch name and the changed
/// files. A file's `status` is one letter: `U` untracked, `!` conflicted,
/// otherwise the work tree's change, or the staged one when the work tree
/// has none (`M`, `A`, `D`, `R`, `C`, `T`).
fn parse_status(text: &str) -> (Option<String>, Vec<Value>) {
    let mut branch = None;
    let mut changes = Vec::new();
    let mut records = text.split('\0');
    while let Some(record) = records.next() {
        if let Some(head) = record.strip_prefix("# branch.head ") {
            branch = Some(head.to_owned());
            continue;
        }
        let mut fields = record.splitn(2, ' ');
        let (Some(kind), Some(rest)) = (fields.next(), fields.next()) else {
            continue;
        };
        let (xy, path, orig) = match kind {
            "?" => ("??", rest, None),
            // `1 XY sub mH mI mW hH hI path`
            "1" => match rest.splitn(8, ' ').collect::<Vec<_>>()[..] {
                [xy, _, _, _, _, _, _, path] => (xy, path, None),
                _ => continue,
            },
            // `2 XY sub mH mI mW hH hI Xscore path`, then the original path.
            "2" => match rest.splitn(9, ' ').collect::<Vec<_>>()[..] {
                [xy, _, _, _, _, _, _, _, path] => (xy, path, records.next()),
                _ => continue,
            },
            // `u XY sub m1 m2 m3 mW h1 h2 h3 path`
            "u" => match rest.splitn(10, ' ').collect::<Vec<_>>()[..] {
                [_, _, _, _, _, _, _, _, _, path] => ("UU", path, None),
                _ => continue,
            },
            _ => continue,
        };
        let mut letters = xy.chars();
        let (staged, worktree) = (letters.next().unwrap_or('.'), letters.next().unwrap_or('.'));
        let status = match (kind, staged, worktree) {
            ("?", _, _) => 'U',
            ("u", _, _) => '!',
            (_, _, letter) if letter != '.' => letter,
            (_, letter, _) => letter,
        };
        changes.push(json!({
            "path": path,
            "orig": orig,
            "status": status.to_string(),
            "staged": kind != "?" && kind != "u" && staged != '.',
        }));
    }
    (branch, changes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn head_files_name_the_branch() {
        assert_eq!(parse_head("ref: refs/heads/main\n"), "main");
        assert_eq!(parse_head("ref: refs/heads/feature/x\n"), "feature/x");
        assert_eq!(parse_head("4ccfb59a2e1f0c3d\n"), "(detached)");
    }

    #[test]
    fn diff_paths_must_lie_inside_the_repository() {
        assert_eq!(inside_repo("", "src/main.rs"), Some("src/main.rs"));
        assert_eq!(inside_repo("app", "app/src/main.rs"), Some("src/main.rs"));
        assert_eq!(inside_repo("app", "app"), None);
        assert_eq!(inside_repo("app", "apps/main.rs"), None);
        assert_eq!(inside_repo("app", "lib/main.rs"), None);
    }

    #[tokio::test]
    async fn nested_repositories_are_found_below_the_root() {
        let root = std::env::temp_dir().join(format!("rah-git-repos-{}", uuid::Uuid::new_v4()));
        for dir in [
            "api/.git",
            "web/.git",
            "libs/core/.git",
            "a/b/c/deep/.git",
            "node_modules/pkg/.git",
            ".hidden/repo/.git",
            "plain/src",
        ] {
            std::fs::create_dir_all(root.join(dir)).unwrap();
        }
        // A submodule or linked work tree has a `.git` file.
        std::fs::create_dir_all(root.join("module")).unwrap();
        std::fs::write(root.join("module/.git"), "gitdir: ../.git/modules/module").unwrap();
        let canonical = std::fs::canonicalize(&root).unwrap();
        let found = nested_repos(&canonical).await;
        std::fs::remove_dir_all(&root).unwrap();
        assert_eq!(found, ["api", "libs/core", "module", "web"]);
    }

    #[test]
    fn branches_list_local_then_remote_only() {
        let text = " \trefs/heads/dev\n*\trefs/heads/master\n\
                    \trefs/remotes/origin/HEAD\n \trefs/remotes/origin/master\n\
                    \trefs/remotes/origin/feature/x\n";
        let (current, branches) = parse_branches(text);
        assert_eq!(current.as_deref(), Some("master"));
        let brief: Vec<_> = branches
            .iter()
            .map(|b| (b.name.as_str(), b.remote))
            .collect();
        assert_eq!(
            brief,
            [
                ("dev", false),
                ("master", false),
                ("origin/feature/x", true)
            ]
        );
    }

    #[test]
    fn status_lists_each_change_with_one_letter() {
        let text = "# branch.oid 4ccfb59\0# branch.head master\0\
                    1 .M N... 100644 100644 100644 a b src/main.rs\0\
                    1 A. N... 000000 100644 100644 a b new file.rs\0\
                    2 R. N... 100644 100644 100644 a b R100 docs/new.md\0docs/old.md\0\
                    u UU N... 1 2 3 4 a b c both.rs\0\
                    ? notes.txt\0";
        let (branch, changes) = parse_status(text);
        assert_eq!(branch.as_deref(), Some("master"));
        let brief: Vec<_> = changes
            .iter()
            .map(|change| {
                (
                    change["path"].as_str().unwrap_or_default(),
                    change["status"].as_str().unwrap_or_default(),
                    change["staged"].as_bool().unwrap_or_default(),
                )
            })
            .collect();
        assert_eq!(
            brief,
            [
                ("src/main.rs", "M", false),
                ("new file.rs", "A", true),
                ("docs/new.md", "R", true),
                ("both.rs", "!", false),
                ("notes.txt", "U", false),
            ]
        );
        assert_eq!(changes[2]["orig"], "docs/old.md");
        assert_eq!(changes[0]["orig"], Value::Null);
    }
}
