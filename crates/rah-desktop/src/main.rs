#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(target_os = "windows")]
use std::process::ExitCode;

#[cfg(target_os = "windows")]
fn main() -> ExitCode {
    match tauri::Builder::default().run(tauri::generate_context!()) {
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
