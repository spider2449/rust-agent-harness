use super::{DesktopAppState, PreferencesWarning};
use tauri::State;

#[tauri::command]
pub(super) fn desktop_preferences_warning(
    state: State<'_, DesktopAppState>,
) -> Option<&'static str> {
    state
        .preferences
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .take_warning()
        .map(|warning| match warning {
            PreferencesWarning::RestoreFailed => "preferences_restore_failed",
            PreferencesWarning::SaveFailed => "preferences_save_failed",
        })
}
