//! Opt-in, process-local Desktop diagnostic log.
//!
//! The log records only presentation-safe Desktop events: the same Activity
//! and chat terminal data the UI already shows. It never records Tool input,
//! raw Tool output, model text, tickets, or authority material, and enabling
//! it grants nothing. It is off at startup and is never persisted as a
//! preference; the file lives in the system temporary directory.

use std::{
    fs::OpenOptions,
    io::Write,
    path::{Path, PathBuf},
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::Serialize;
use tauri::State;

use super::{DesktopAppState, FrontendError};

#[derive(Default)]
pub(crate) struct DiagnosticLog {
    path: Mutex<Option<PathBuf>>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DiagnosticLogStatus {
    enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<String>,
}

impl DiagnosticLog {
    pub(crate) fn status(&self) -> DiagnosticLogStatus {
        let path = self.lock();
        DiagnosticLogStatus {
            enabled: path.is_some(),
            path: path.as_ref().map(|path| path.display().to_string()),
        }
    }

    /// Enabling while already enabled keeps the current file.
    fn set_enabled(
        &self,
        enabled: bool,
        directory: &Path,
    ) -> Result<DiagnosticLogStatus, FrontendError> {
        {
            let mut current = self.lock();
            if !enabled {
                *current = None;
            } else if current.is_none() {
                let path = log_path(directory, unix_millis(), std::process::id());
                OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&path)
                    .map_err(|_| FrontendError::DiagnosticLogUnavailable)?;
                *current = Some(path);
            }
        }
        Ok(self.status())
    }

    /// Best effort: a write failure never affects the chat turn.
    pub(crate) fn append(&self, event: &str, record: &impl Serialize) {
        let current = self.lock();
        let Some(path) = current.as_ref() else {
            return;
        };
        let Ok(mut line) = serde_json::to_vec(&serde_json::json!({
            "timestamp_ms": unix_millis(),
            "event": event,
            "record": record,
        })) else {
            return;
        };
        line.push(b'\n');
        if let Ok(mut file) = OpenOptions::new().append(true).open(path) {
            let _ = file.write_all(&line);
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Option<PathBuf>> {
        self.path
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

fn log_path(directory: &Path, millis: u64, process_id: u32) -> PathBuf {
    directory.join(format!("rah-desktop-{millis}-{process_id}.log"))
}

fn unix_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| u64::try_from(duration.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or_default()
}

#[tauri::command]
pub(super) fn diagnostic_log_status(state: State<'_, DesktopAppState>) -> DiagnosticLogStatus {
    state.diagnostic_log.status()
}

#[tauri::command]
pub(super) fn set_diagnostic_log_enabled(
    state: State<'_, DesktopAppState>,
    enabled: bool,
) -> Result<DiagnosticLogStatus, FrontendError> {
    state
        .diagnostic_log
        .set_enabled(enabled, &std::env::temp_dir())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use serde_json::json;

    use super::{DiagnosticLog, log_path};

    fn test_directory(name: &str) -> std::path::PathBuf {
        let directory =
            std::env::temp_dir().join(format!("rah-diagnostic-log-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).expect("test directory should be created");
        directory
    }

    #[test]
    fn disabled_log_writes_nothing() {
        let directory = test_directory("disabled");
        let log = DiagnosticLog::default();
        log.append("tool_finished", &json!({"tool": "repo.search"}));
        assert!(!log.status().enabled);
        assert_eq!(fs::read_dir(&directory).unwrap().count(), 0);
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn enabled_log_appends_jsonl_records_in_the_given_directory() {
        let directory = test_directory("enabled");
        let log = DiagnosticLog::default();
        let status = log
            .set_enabled(true, &directory)
            .expect("log should enable");
        assert!(status.enabled);
        let path = std::path::PathBuf::from(status.path.clone().expect("path is reported"));
        assert_eq!(path.parent(), Some(directory.as_path()));
        assert_eq!(
            log.set_enabled(true, &directory)
                .expect("re-enable keeps file"),
            status
        );

        log.append(
            "tool_finished",
            &json!({"tool": "repo.search", "result": "failed"}),
        );
        log.append("chat_terminal", &json!({"kind": "completed"}));
        let records = fs::read_to_string(&path)
            .expect("log should be readable")
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0]["event"], "tool_finished");
        assert_eq!(records[0]["record"]["result"], "failed");
        assert!(records[0]["timestamp_ms"].is_u64());

        assert!(!log.set_enabled(false, &directory).unwrap().enabled);
        log.append("tool_finished", &json!({"tool": "after-disable"}));
        assert_eq!(fs::read_to_string(&path).unwrap().lines().count(), 2);
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn unwritable_directory_fails_closed_and_stays_disabled() {
        let directory = test_directory("missing").join("absent");
        let log = DiagnosticLog::default();
        assert!(log.set_enabled(true, &directory).is_err());
        assert!(!log.status().enabled);
    }

    #[test]
    fn log_file_name_is_per_process() {
        let path = log_path(std::path::Path::new("base"), 42, 7);
        assert_eq!(
            path,
            std::path::Path::new("base").join("rah-desktop-42-7.log")
        );
    }
}
