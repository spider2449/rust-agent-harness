//! What `rah config check` finds beyond the configuration file itself: that
//! the model plugins mount (a missing API key variable fails here) and
//! answer their `health` check, and which `rah` program runs. The model
//! check mounts only the model plugins, so no tool, loop or frontend starts,
//! and `health` sends no conversation and spends no tokens.

use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use rah_api::{
    CancellationToken, Kernel, PluginStatus,
    model::{MODEL, ModelHealth},
};

use crate::{Distribution, boot, is_model_kind};

/// One line of the report.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Finding {
    /// Whether this is a problem that stops RAH from working.
    pub problem: bool,
    pub text: String,
}

impl Finding {
    fn ok(text: impl Into<String>) -> Self {
        Self {
            problem: false,
            text: text.into(),
        }
    }

    fn problem(text: impl Into<String>) -> Self {
        Self {
            problem: true,
            text: text.into(),
        }
    }
}

/// Mounts the model plugins of `distribution` alone and asks the first one
/// for its health, waiting at most `timeout`.
pub async fn models(distribution: &Distribution, timeout: Duration) -> Vec<Finding> {
    let mut models = distribution.clone();
    models.plugin.retain(|spec| is_model_kind(&spec.kind));
    models.features.clear();
    let label = distribution
        .model_label()
        .unwrap_or_else(|| "the model".to_owned());
    if models.plugin.is_empty() {
        return vec![Finding::problem("no model plugin is configured")];
    }
    let kernel = Kernel::new();
    let findings = match boot(&kernel, &models).await {
        Err(error) => vec![Finding::problem(format!("{label} does not start: {error}"))],
        Ok(statuses) => {
            let mut findings: Vec<Finding> = statuses
                .into_iter()
                .filter_map(|(id, status)| match status {
                    PluginStatus::Active => None,
                    PluginStatus::Failed { message } => Some(Finding::problem(format!(
                        "model plugin `{id}` does not start: {message}"
                    ))),
                    other => Some(Finding::problem(format!(
                        "model plugin `{id}` is not active: {other:?}"
                    ))),
                })
                .collect();
            if findings.is_empty() {
                findings.push(health(&kernel, &label, timeout).await);
            }
            findings
        }
    };
    let _ = kernel.shutdown().await;
    findings
}

async fn health(kernel: &Kernel, label: &str, timeout: Duration) -> Finding {
    let Some((_, model)) = kernel.handle().get_all(MODEL).into_iter().next() else {
        return Finding::problem(format!("{label} provides no model service"));
    };
    let cancel = CancellationToken::new();
    let checked = tokio::time::timeout(timeout, model.health(cancel.child_token())).await;
    cancel.cancel();
    match checked {
        Ok(ModelHealth::Reachable) => Finding::ok(format!("{label}: the server answers")),
        Ok(ModelHealth::Unknown) => Finding::ok(format!(
            "{label}: starts (this provider cannot be checked without a request)"
        )),
        Ok(ModelHealth::Unavailable(reason)) => {
            Finding::problem(format!("{label} cannot be reached: {reason}"))
        }
        Err(_) => Finding::problem(format!(
            "{label} did not answer within {} s",
            timeout.as_secs()
        )),
    }
}

/// The program file name of `rah` on this platform.
const PROGRAM: &str = if cfg!(windows) { "rah.exe" } else { "rah" };

/// The first `rah` in `path` (a PATH value), if any.
pub fn first_on_path(path: &std::ffi::OsStr) -> Option<PathBuf> {
    std::env::split_paths(path)
        .map(|dir| dir.join(PROGRAM))
        .find(|candidate| candidate.is_file())
}

/// Which `rah` runs: this program's version and place, and a warning when
/// another `rah` comes first on PATH, so typing `rah` would run that one.
pub fn program(current: Option<&Path>, path: Option<&std::ffi::OsStr>) -> Vec<Finding> {
    let version = env!("CARGO_PKG_VERSION");
    let Some(current) = current else {
        return vec![Finding::ok(format!("rah {version}"))];
    };
    let mut findings = vec![Finding::ok(format!(
        "rah {version} at `{}`",
        current.display()
    ))];
    let same = |other: &Path| match (other.canonicalize(), current.canonicalize()) {
        (Ok(other), Ok(current)) => other == current,
        _ => other == current,
    };
    if let Some(first) = path.and_then(first_on_path)
        && !same(&first)
    {
        // Not a problem for this run, but `rah` typed elsewhere runs `first`.
        findings.push(Finding::ok(format!(
            "warning: `rah` on PATH is `{}`, not this program; remove it or move it later on PATH",
            first.display()
        )));
    }
    findings
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("rah-doctor-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn another_rah_first_on_path_is_named() {
        let dir = temp("path");
        let (early, late) = (dir.join("early"), dir.join("late"));
        for folder in [&early, &late] {
            std::fs::create_dir_all(folder).unwrap();
            std::fs::write(folder.join(PROGRAM), b"").unwrap();
        }
        let path = std::env::join_paths([&dir, &early, &late]).unwrap();
        assert_eq!(first_on_path(&path), Some(early.join(PROGRAM)));

        let current = late.join(PROGRAM);
        let findings = program(Some(&current), Some(&path));
        assert!(findings[0].text.contains(env!("CARGO_PKG_VERSION")));
        assert!(findings[1].text.contains("warning"), "{findings:?}");
        assert!(findings.iter().all(|finding| !finding.problem));

        let alone = std::env::join_paths([&late]).unwrap();
        assert_eq!(program(Some(&current), Some(&alone)).len(), 1);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[tokio::test]
    async fn a_mock_model_starts_and_a_missing_key_is_a_problem() {
        let mut mock = Distribution::default();
        mock.set_model("mock", &serde_json::Map::new()).unwrap();
        let findings = models(&mock, Duration::from_secs(5)).await;
        assert!(findings.iter().all(|f| !f.problem), "{findings:?}");

        let mut keyed = Distribution::default();
        let options = serde_json::Map::from_iter([(
            "api_key_env".to_owned(),
            serde_json::Value::from("RAH_TEST_UNSET_KEY_D0C7"),
        )]);
        keyed.set_model("anthropic/claude-x", &options).unwrap();
        let findings = models(&keyed, Duration::from_secs(5)).await;
        assert!(findings.iter().any(|f| f.problem), "{findings:?}");
        assert!(
            findings
                .iter()
                .any(|f| f.text.contains("RAH_TEST_UNSET_KEY_D0C7")),
            "{findings:?}"
        );

        assert!(models(&Distribution::default(), Duration::from_secs(1)).await[0].problem);
    }
}
