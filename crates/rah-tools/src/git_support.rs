use std::{
    collections::BTreeMap,
    ffi::{OsStr, OsString},
    path::Path,
};

use crate::ToolError;

pub(crate) fn git_environment() -> BTreeMap<OsString, OsString> {
    let mut environment = BTreeMap::new();
    insert_environment(&mut environment, "GIT_CONFIG_NOSYSTEM", "1");
    insert_environment(
        &mut environment,
        "GIT_CONFIG_GLOBAL",
        platform_null_device(),
    );
    insert_environment(&mut environment, "GIT_CONFIG_COUNT", "2");
    insert_environment(&mut environment, "GIT_CONFIG_KEY_0", "core.fsmonitor");
    insert_environment(&mut environment, "GIT_CONFIG_VALUE_0", "false");
    insert_environment(&mut environment, "GIT_CONFIG_KEY_1", "core.untrackedCache");
    insert_environment(&mut environment, "GIT_CONFIG_VALUE_1", "false");
    insert_environment(&mut environment, "GIT_OPTIONAL_LOCKS", "0");
    insert_environment(&mut environment, "GIT_TERMINAL_PROMPT", "0");
    environment
}

/// Returns the fixed Git child environment for one repository observer.
///
/// The sole additional config value is the exact canonical root already
/// captured by the host-owned `RepositoryIdentity`. It lets Git inspect that
/// one selected repository when protected-ownership checks would otherwise
/// reject it; it is neither user configuration nor general repository trust.
pub(crate) fn repository_observer_environment(
    canonical_repository_root: &Path,
) -> Result<BTreeMap<OsString, OsString>, ToolError> {
    let root = canonical_repository_root.as_os_str();
    if root.is_empty() || root == OsStr::new("*") || root.to_string_lossy().ends_with("/*") {
        return Err(git_error("repository safe-directory authority is invalid"));
    }

    let mut environment = git_environment();
    insert_environment(&mut environment, "GIT_CONFIG_COUNT", "3");
    insert_environment(&mut environment, "GIT_CONFIG_KEY_2", "safe.directory");
    insert_environment(&mut environment, "GIT_CONFIG_VALUE_2", root);
    Ok(environment)
}

pub(crate) fn git_error(message: impl Into<String>) -> ToolError {
    ToolError::Execution {
        message: format!(
            "Git repository policy rejected capability: {}",
            message.into()
        ),
    }
}

fn insert_environment(
    environment: &mut BTreeMap<OsString, OsString>,
    name: impl AsRef<OsStr>,
    value: impl AsRef<OsStr>,
) {
    environment.insert(name.as_ref().to_owned(), value.as_ref().to_owned());
}

#[cfg(windows)]
fn platform_null_device() -> &'static str {
    "NUL"
}
#[cfg(not(windows))]
fn platform_null_device() -> &'static str {
    "/dev/null"
}

#[cfg(test)]
mod tests {
    use std::{
        ffi::OsStr,
        fs,
        path::{Path, PathBuf},
        process::Command,
        sync::atomic::{AtomicU64, Ordering},
    };

    use super::{git_environment, repository_observer_environment};

    static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn observer_environment_adds_only_one_exact_safe_directory_after_hardening_entries() {
        let root = if cfg!(windows) {
            Path::new(r"C:\\host-selected\\repository")
        } else {
            Path::new("/host-selected/repository")
        };
        let environment = repository_observer_environment(root).unwrap();
        let entries = environment
            .iter()
            .map(|(name, value)| format!("{}={}", name.to_string_lossy(), value.to_string_lossy()))
            .collect::<Vec<_>>();
        assert_eq!(
            entries,
            vec![
                "GIT_CONFIG_COUNT=3".to_owned(),
                "GIT_CONFIG_GLOBAL=NUL".replace("NUL", super::platform_null_device()),
                "GIT_CONFIG_KEY_0=core.fsmonitor".to_owned(),
                "GIT_CONFIG_KEY_1=core.untrackedCache".to_owned(),
                "GIT_CONFIG_KEY_2=safe.directory".to_owned(),
                "GIT_CONFIG_NOSYSTEM=1".to_owned(),
                "GIT_CONFIG_VALUE_0=false".to_owned(),
                "GIT_CONFIG_VALUE_1=false".to_owned(),
                format!("GIT_CONFIG_VALUE_2={}", root.display()),
                "GIT_OPTIONAL_LOCKS=0".to_owned(),
                "GIT_TERMINAL_PROMPT=0".to_owned(),
            ]
        );
    }

    #[test]
    fn observer_environment_rejects_wildcard_and_prefix_safe_directory_values() {
        assert!(repository_observer_environment(Path::new("*")).is_err());
        assert!(repository_observer_environment(Path::new("/host-selected/*")).is_err());
    }

    #[test]
    fn each_observer_environment_contains_only_its_own_repository_authority() {
        let root_a = Path::new(if cfg!(windows) {
            r"C:\\host-selected\\repository-a"
        } else {
            "/host-selected/repository-a"
        });
        let root_b = Path::new(if cfg!(windows) {
            r"C:\\host-selected\\repository-b"
        } else {
            "/host-selected/repository-b"
        });
        let environment_a = repository_observer_environment(root_a).unwrap();
        let environment_b = repository_observer_environment(root_b).unwrap();
        let value_a = environment_a.get(OsStr::new("GIT_CONFIG_VALUE_2")).unwrap();
        let value_b = environment_b.get(OsStr::new("GIT_CONFIG_VALUE_2")).unwrap();
        assert_eq!(value_a, root_a.as_os_str());
        assert_eq!(value_b, root_b.as_os_str());
        assert_ne!(value_a, value_b);
    }

    #[test]
    fn exact_host_safe_directory_is_required_for_foreign_owner_git_status() {
        let root = temporary_repository();
        let git = native_git();
        let status = [
            "--no-pager",
            "status",
            "--porcelain=v2",
            "-z",
            "--untracked-files=normal",
            "--ignored=no",
            "--no-renames",
            "--ignore-submodules=all",
        ];
        let rejected = run_status(&git, &root, &status, git_environment());
        assert!(!rejected.status.success());

        let accepted = run_status(
            &git,
            &root,
            &status,
            repository_observer_environment(&root).unwrap(),
        );
        assert!(accepted.status.success());

        let other_root = temporary_repository();
        let wrong = run_status(
            &git,
            &root,
            &status,
            repository_observer_environment(&other_root).unwrap(),
        );
        assert!(!wrong.status.success());
        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(other_root).unwrap();
    }

    fn native_git() -> PathBuf {
        #[cfg(windows)]
        let output = Command::new("where.exe").arg("git.exe").output().unwrap();
        #[cfg(not(windows))]
        let output = Command::new("which").arg("git").output().unwrap();
        assert!(output.status.success());
        let path = String::from_utf8(output.stdout).unwrap();
        fs::canonicalize(path.lines().next().unwrap()).unwrap()
    }

    fn temporary_repository() -> PathBuf {
        let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "rah-observer-safe-directory-{}-{id}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let git = native_git();
        for arguments in [
            ["init", "--quiet"].as_slice(),
            ["config", "user.name", "RAH Test"].as_slice(),
            ["config", "user.email", "rah@example.invalid"].as_slice(),
            ["commit", "--allow-empty", "--quiet", "-m", "fixture"].as_slice(),
        ] {
            assert!(
                Command::new(&git)
                    .args(arguments)
                    .current_dir(&root)
                    .status()
                    .unwrap()
                    .success()
            );
        }
        fs::canonicalize(root).unwrap()
    }

    fn run_status(
        git: &Path,
        root: &Path,
        status: &[&str],
        environment: std::collections::BTreeMap<std::ffi::OsString, std::ffi::OsString>,
    ) -> std::process::Output {
        let mut command = Command::new(git);
        command
            .args(status)
            .current_dir(root)
            .env_clear()
            .envs(environment)
            .env("GIT_TEST_ASSUME_DIFFERENT_OWNER", "1");
        command.output().unwrap()
    }
}
