#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(target_os = "windows")]
use serde::Serialize;
#[cfg(target_os = "windows")]
use std::process::ExitCode;

#[cfg(target_os = "windows")]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AppStatus {
    app_name: &'static str,
    app_version: &'static str,
    platform: &'static str,
    desktop_shell: &'static str,
    runtime_status: &'static str,
    codex_status: &'static str,
    profile_status: &'static str,
    repository_status: &'static str,
}

#[cfg(target_os = "windows")]
fn current_app_status() -> AppStatus {
    AppStatus {
        app_name: "RAH",
        app_version: env!("CARGO_PKG_VERSION"),
        platform: "windows",
        desktop_shell: "ready",
        runtime_status: "not connected",
        codex_status: "not connected",
        profile_status: "not loaded",
        repository_status: "not selected",
    }
}

#[cfg(target_os = "windows")]
#[tauri::command]
fn app_status() -> AppStatus {
    current_app_status()
}

#[cfg(target_os = "windows")]
fn main() -> ExitCode {
    match tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![app_status])
        .run(tauri::generate_context!())
    {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("failed to run RAH desktop shell: {error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn main() {
    eprintln!("rah-desktop is available on Windows only");
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::current_app_status;

    #[test]
    fn status_contains_only_the_desktop_application_state() {
        let status = current_app_status();

        assert_eq!(status.app_name, "RAH");
        assert_eq!(status.app_version, env!("CARGO_PKG_VERSION"));
        assert_eq!(status.platform, "windows");
        assert_eq!(status.desktop_shell, "ready");
        assert_eq!(status.runtime_status, "not connected");
        assert_eq!(status.codex_status, "not connected");
        assert_eq!(status.profile_status, "not loaded");
        assert_eq!(status.repository_status, "not selected");

        let serialized = serde_json::to_string(&status).expect("status serializes");
        assert!(!serialized.contains('\\'));
        assert!(!serialized.contains('/'));
    }
}
