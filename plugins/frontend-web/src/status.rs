//! `GET /api/status`: what the status line shows.
//!
//! The workspace is the `sandbox` service's root. Git state comes from one
//! read-only `git status` run through that service (never a shell, with
//! `--no-optional-locks` so it does not write the index). Model and loop are
//! the providers the next turn will use.

use std::{path::Path, time::Duration};

use axum::{Json, extract::State};
use rah_api::{
    CancellationToken, KernelHandle, PluginId,
    admin::ADMIN,
    agent_loop::LOOP,
    model::MODEL,
    sandbox::{OutputLimits, ProgramSpec, SANDBOX},
    tools::ToolInvoker,
};
use serde_json::{Value, json};

use crate::{ApiResult, AppState};

const GIT_TIMEOUT: Duration = Duration::from_secs(5);

pub(crate) async fn status(State(shared): State<AppState>) -> ApiResult {
    let kernel = &shared.kernel;
    Ok(Json(json!({
        "workspace": workspace(kernel, &shared.stop).await,
        "model": provider(kernel, kernel.get_all(MODEL).first().map(|(id, _)| id.clone())),
        "loop": provider(kernel, kernel.get_all(LOOP).first().map(|(id, _)| id.clone())),
        "guarded": ToolInvoker::new(kernel.clone()).is_guarded(),
        "tools": ToolInvoker::new(kernel.clone()).definitions().len(),
        "version": env!("CARGO_PKG_VERSION"),
    })))
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
    let root = sandbox.root().map(|root| display_path(&root));
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
    spec.limits = OutputLimits {
        stdout_bytes: 1024 * 1024,
        stderr_bytes: 16 * 1024,
    };
    let git = match sandbox.run(spec, stop.child_token()).await {
        Ok(output) if output.exit_code == Some(0) => {
            parse_git_status(&String::from_utf8_lossy(&output.stdout))
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let reason = stderr.lines().next().unwrap_or("git status failed").trim();
            json!({ "error": reason })
        }
        Err(error) => json!({ "error": error.to_string() }),
    };
    json!({ "root": root, "git": git })
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
