//! `GET /api/status`: what the status line shows.
//!
//! The workspace is the `sandbox` service's root. Git state comes from one
//! read-only `git status` run through that service (never a shell, with
//! `--no-optional-locks` so it does not write the index); when the root is no
//! repository, from one run in each repository below it, summed. Model and loop are
//! the providers the next turn will use.

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

use crate::{ApiResult, AppState};

const GIT_TIMEOUT: Duration = Duration::from_secs(5);

pub(crate) async fn status(State(shared): State<AppState>) -> ApiResult {
    let kernel = &shared.kernel;
    let models = kernel.get_all(MODEL);
    let (workspace, health) = tokio::join!(
        workspace(kernel, &shared.stop),
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

async fn workspace(kernel: &KernelHandle, stop: &CancellationToken) -> Value {
    let Some(sandbox) = kernel.get(SANDBOX) else {
        return Value::Null;
    };
    let root = sandbox.root();
    let git = match git_status(sandbox.as_ref(), None, stop).await {
        Ok(text) => parse_git_status(&text),
        // The root is no repository: the ones below it, summed.
        Err(reason) => {
            let nested = match &root {
                Some(root) => match tokio::fs::canonicalize(root).await {
                    Ok(root) => crate::git::nested_repos(&root).await,
                    Err(_) => Vec::new(),
                },
                None => Vec::new(),
            };
            let statuses = futures::future::join_all(
                nested
                    .iter()
                    .map(|repo| git_status(sandbox.as_ref(), Some(PathBuf::from(repo)), stop)),
            )
            .await;
            let summaries: Vec<_> = statuses
                .iter()
                .filter_map(|status| status.as_deref().ok())
                .map(parse_git_status)
                .collect();
            if summaries.is_empty() {
                json!({ "error": reason })
            } else {
                sum_git_status(&summaries)
            }
        }
    };
    json!({ "root": root.map(|root| display_path(&root)), "git": git })
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

/// Several repositories' summaries as one: `{repos, ahead, behind, changed,
/// untracked, conflicts}`, each count summed.
fn sum_git_status(summaries: &[Value]) -> Value {
    let sum = |key: &str| {
        summaries
            .iter()
            .map(|summary| summary[key].as_u64().unwrap_or(0))
            .sum::<u64>()
    };
    json!({
        "repos": summaries.len(),
        "ahead": sum("ahead"),
        "behind": sum("behind"),
        "changed": sum("changed"),
        "untracked": sum("untracked"),
        "conflicts": sum("conflicts"),
    })
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
    fn nested_repository_summaries_are_summed() {
        let api = parse_git_status("# branch.head main\n# branch.ab +1 -0\n? a\n");
        let web = parse_git_status(
            "# branch.head dev\n1 .M N... 1 2 3 a b x\nu UU N... 1 2 3 4 a b c y\n",
        );
        assert_eq!(
            sum_git_status(&[api, web]),
            json!({
                "repos": 2, "ahead": 1, "behind": 0, "changed": 1, "untracked": 1, "conflicts": 1,
            })
        );
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
