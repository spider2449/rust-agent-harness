//! `GET /api/status`: what the status line shows.
//!
//! The workspace is the `sandbox` service's root (the active folder), with
//! the workspace's name and number of folders. Git state comes from
//! read-only `git status` runs through that service (never a shell, with
//! `--no-optional-locks` so they do not write the index): one in the root and
//! one in each repository below it, listed as `repos` so the UI can show the
//! branch of the repository the user works in. When the root is a
//! repository, its own summary is also the top level. Model and loop are the
//! providers the next turn will use.

use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use axum::{Json, extract::State};
use rah_api::{
    CancellationToken, KernelHandle, PluginId,
    admin::ADMIN,
    agent_loop::LOOP,
    model::{MODEL, ModelHealth, ModelProvider},
    sandbox::{ExecEnv, OutputLimits, ProgramSpec, SANDBOX},
    tools::ToolInvoker,
};
use serde_json::{Value, json};

use crate::{ApiResult, AppState, Shared};

const GIT_TIMEOUT: Duration = Duration::from_secs(5);

pub(crate) async fn status(State(shared): State<AppState>) -> ApiResult {
    let kernel = &shared.kernel;
    let models = kernel.get_all(MODEL);
    let (workspace, health) = tokio::join!(
        workspace(&shared),
        model_health(models.first().map(|(_, model)| model.clone()), &shared.stop),
    );
    let mut model = provider(kernel, models.first().map(|(id, _)| id.clone()));
    if let Some(model) = model.as_object_mut() {
        model.insert("health".into(), health);
    }
    Ok(Json(json!({
        "workspace": workspace,
        "model": model,
        "loop": provider(kernel, kernel.get_all(LOOP).first().map(|(id, _)| id.clone())),
        "guarded": ToolInvoker::new(kernel.clone()).is_guarded(),
        "tools": ToolInvoker::new(kernel.clone()).definitions().len(),
        "version": env!("CARGO_PKG_VERSION"),
    })))
}

/// `{state: "unknown" | "reachable" | "unavailable", reason}` of the model.
async fn model_health(model: Option<Arc<dyn ModelProvider>>, stop: &CancellationToken) -> Value {
    let Some(model) = model else {
        return Value::Null;
    };
    match model.health(stop.child_token()).await {
        ModelHealth::Unknown => json!({ "state": "unknown" }),
        ModelHealth::Reachable => json!({ "state": "reachable" }),
        ModelHealth::Unavailable(reason) => json!({ "state": "unavailable", "reason": reason }),
    }
}

/// `{plugin, kind, model}` of a service provider, or `null`.
fn provider(kernel: &KernelHandle, instance: Option<String>) -> Value {
    let Some(instance) = instance else {
        return Value::Null;
    };
    let known = kernel
        .get(ADMIN)
        .and_then(|admin| admin.instance(&PluginId::new(instance.clone())));
    let kind = known.as_ref().map(|known| known.kind.clone());
    let model = known
        .as_ref()
        .and_then(|known| known.config.get("model"))
        .and_then(Value::as_str)
        .map(str::to_owned);
    json!({ "plugin": instance, "kind": kind, "model": model })
}

async fn workspace(shared: &Shared) -> Value {
    let (kernel, stop) = (&shared.kernel, &shared.stop);
    let Some(sandbox) = kernel.get(SANDBOX) else {
        return Value::Null;
    };
    let root = sandbox.root();
    let nested = match &root {
        Some(root) => match tokio::fs::canonicalize(root).await {
            Ok(root) => crate::git::nested_repos(&root).await,
            Err(_) => Vec::new(),
        },
        None => Vec::new(),
    };
    let (own, statuses) = tokio::join!(
        git_status(sandbox.as_ref(), None, stop),
        futures::future::join_all(nested.iter().map(|repo| git_status(
            sandbox.as_ref(),
            Some(PathBuf::from(repo)),
            stop
        )),),
    );
    let repos = repo_summaries(
        own.as_deref().ok(),
        nested
            .iter()
            .zip(&statuses)
            .filter_map(|(path, status)| Some((path.as_str(), status.as_deref().ok()?))),
    );
    let git = match own {
        Ok(text) => {
            let mut git = parse_git_status(&text);
            git["repos"] = json!(repos);
            git
        }
        Err(reason) if repos.is_empty() => json!({ "error": reason }),
        // The root is no repository, the folders below it hold some.
        Err(_) => json!({ "repos": repos }),
    };
    let (_, folders) = crate::workspace::current(shared).await;
    json!({
        "root": root.map(|root| display_path(&root)),
        "name": folders.name,
        "folders": folders.folders.len(),
        "git": git,
    })
}

/// Each repository's summary with its folder as `path` (empty for the
/// root's): the root's first, then the nested ones.
fn repo_summaries<'a>(
    own: Option<&'a str>,
    nested: impl Iterator<Item = (&'a str, &'a str)>,
) -> Vec<Value> {
    own.map(|text| ("", text))
        .into_iter()
        .chain(nested)
        .map(|(path, text)| {
            let mut summary = parse_git_status(text);
            summary["path"] = json!(path);
            summary
        })
        .collect()
}

/// `git status --porcelain=v2 --branch` in `cwd` (the root when `None`), or
/// the first line of why it failed.
async fn git_status(
    sandbox: &dyn ExecEnv,
    cwd: Option<PathBuf>,
    stop: &CancellationToken,
) -> Result<String, String> {
    let mut spec = ProgramSpec::new(
        "git",
        [
            "--no-optional-locks",
            "status",
            "--porcelain=v2",
            "--branch",
        ]
        .map(String::from)
        .to_vec(),
        GIT_TIMEOUT,
    );
    spec.cwd = cwd;
    spec.limits = OutputLimits {
        stdout_bytes: 1024 * 1024,
        stderr_bytes: 16 * 1024,
    };
    match sandbox.run(spec, stop.child_token()).await {
        Ok(output) if output.exit_code == Some(0) => {
            Ok(String::from_utf8_lossy(&output.stdout).into_owned())
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(stderr
                .lines()
                .next()
                .unwrap_or("git status failed")
                .trim()
                .to_owned())
        }
        Err(error) => Err(error.to_string()),
    }
}

/// A path for people: without Windows' verbatim `\\?\` prefix.
fn display_path(path: &Path) -> String {
    let text = path.display().to_string();
    match text.strip_prefix(r"\\?\") {
        Some(rest) if !rest.starts_with("UNC\\") => rest.to_owned(),
        _ => text,
    }
}

/// `git status --porcelain=v2 --branch` as `{branch, detached, upstream,
/// ahead, behind, changed, untracked, conflicts}`.
fn parse_git_status(text: &str) -> Value {
    let (mut branch, mut upstream) = (None, None);
    let (mut ahead, mut behind) = (0_u64, 0_u64);
    let (mut changed, mut untracked, mut conflicts) = (0_u64, 0_u64, 0_u64);
    let mut detached = false;
    for line in text.lines() {
        if let Some(head) = line.strip_prefix("# branch.head ") {
            detached = head == "(detached)";
            branch = Some(head.to_owned());
        } else if let Some(name) = line.strip_prefix("# branch.upstream ") {
            upstream = Some(name.to_owned());
        } else if let Some(counts) = line.strip_prefix("# branch.ab ") {
            for count in counts.split_whitespace() {
                if let Some(n) = count.strip_prefix('+') {
                    ahead = n.parse().unwrap_or(0);
                } else if let Some(n) = count.strip_prefix('-') {
                    behind = n.parse().unwrap_or(0);
                }
            }
        } else if line.starts_with("? ") {
            untracked += 1;
        } else if line.starts_with("u ") {
            conflicts += 1;
        } else if line.starts_with("1 ") || line.starts_with("2 ") {
            changed += 1;
        }
    }
    json!({
        "branch": branch,
        "detached": detached,
        "upstream": upstream,
        "ahead": ahead,
        "behind": behind,
        "changed": changed,
        "untracked": untracked,
        "conflicts": conflicts,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn git_status_is_summarized() {
        let text = "# branch.oid 4ccfb59\n# branch.head master\n# branch.upstream origin/master\n\
                    # branch.ab +2 -1\n1 .M N... 100644 100644 100644 a b README.md\n\
                    2 R. N... 100644 100644 100644 a b R100 new\told\nu UU N... 1 2 3 4 a b c x\n\
                    ? notes.txt\n? tmp/\n";
        assert_eq!(
            parse_git_status(text),
            json!({
                "branch": "master", "detached": false, "upstream": "origin/master",
                "ahead": 2, "behind": 1, "changed": 2, "untracked": 2, "conflicts": 1,
            })
        );
        let detached = parse_git_status("# branch.oid 1\n# branch.head (detached)\n");
        assert_eq!(detached["detached"], true);
        assert_eq!(detached["upstream"], Value::Null);
    }

    #[test]
    fn every_repository_is_summarized_with_its_folder() {
        let nested = [
            ("api", "# branch.head main\n# branch.ab +1 -0\n? a\n"),
            ("web", "# branch.head dev\n1 .M N... 1 2 3 a b x\n"),
        ];
        let repos = repo_summaries(Some("# branch.head trunk\n"), nested.into_iter());
        let brief: Vec<_> = repos
            .iter()
            .map(|repo| (repo["path"].clone(), repo["branch"].clone()))
            .collect();
        assert_eq!(
            brief,
            [
                (json!(""), json!("trunk")),
                (json!("api"), json!("main")),
                (json!("web"), json!("dev")),
            ]
        );
        assert_eq!(repos[1]["ahead"], 1);
        assert_eq!(repos[2]["changed"], 1);
        // The root is no repository: only the nested ones.
        assert_eq!(repo_summaries(None, nested.into_iter()).len(), 2);
    }

    #[test]
    fn windows_verbatim_prefixes_are_hidden() {
        assert_eq!(
            display_path(Path::new(r"\\?\D:\work\repo")),
            r"D:\work\repo"
        );
        assert_eq!(
            display_path(Path::new(r"\\?\UNC\server\share")),
            r"\\?\UNC\server\share"
        );
        assert_eq!(display_path(Path::new("/home/me/repo")), "/home/me/repo");
    }
}
