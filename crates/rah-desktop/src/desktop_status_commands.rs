use super::{AppStatus, DesktopAppState};
use tauri::State;

#[cfg(target_os = "windows")]
#[tauri::command]
pub(super) fn app_status(state: State<'_, DesktopAppState>) -> AppStatus {
    state.status()
}
