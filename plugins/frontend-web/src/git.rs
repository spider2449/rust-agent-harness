//! Read-only Git views of the workspace for the file panel: the changed
//! files and one file's diff.
//!
//! Every view is one `git` run through the `sandbox` service in the workspace
//! root: a fixed argument vector, never a shell, with `--no-optional-locks`
//! (no index writes), `--literal-pathspecs` (a path is only a path),
//! `--no-ext-diff` and `--no-textconv` (no configured diff programs). Paths
//! are checked like the file browser's.

use std::time::Duration;

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
/// The most changed files one listing returns.
const MAX_CHANGES: usize = 2000;

/// One finished `git` run: its output, and whether it was cut at the limit.
struct GitOutput {
    stdout: String,
    truncated: bool,
}

async fn git(shared: &AppState, args: &[&str], stdout_bytes: usize) -> Result<GitOutput, ApiError> {
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

/// `{branch, changes: [{path, orig, status, staged}], truncated}`, or
/// `{error}` when Git cannot read the workspace (such as a folder that is no
/// repository): a state to show, not a failed request.
pub(crate) async fn changes(State(shared): State<AppState>) -> ApiResult {
    let output = match git(
        &shared,
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
            return Ok(Json(json!({ "error": reason })));
        }
        Err(error) => return Err(error),
    };
    let (branch, mut changes) = parse_status(&output.stdout);
    let truncated = output.truncated || changes.len() > MAX_CHANGES;
    changes.truncate(MAX_CHANGES);
    Ok(Json(json!({
        "branch": branch,
        "changes": changes,
        "truncated": truncated,
    })))
}

#[derive(Deserialize)]
pub(crate) struct DiffQuery {
    path: String,
}

/// `{path, text, truncated}`: the file's changes against `HEAD` (staged and
/// not), or against the index before the first commit.
pub(crate) async fn diff(
    State(shared): State<AppState>,
    Query(query): Query<DiffQuery>,
) -> ApiResult {
    let checked = files::checked(&query.path)?;
    let path = checked.to_string_lossy().replace('\\', "/");
    if path.is_empty() {
        return Err(ApiError(StatusCode::BAD_REQUEST, "no file named".into()));
    }
    let base = ["diff", "--no-color", "--no-ext-diff", "--no-textconv"];
    let with_head = [&base[..], &["HEAD", "--", &path]].concat();
    let output = match git(&shared, &with_head, MAX_PATCH).await {
        Ok(output) => output,
        // No commit yet: the changes against the index.
        Err(_) => git(&shared, &[&base[..], &["--", &path]].concat(), MAX_PATCH).await?,
    };
    Ok(Json(json!({
        "path": path,
        "text": output.stdout,
        "truncated": output.truncated,
    })))
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
