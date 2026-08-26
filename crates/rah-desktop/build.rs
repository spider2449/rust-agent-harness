#[cfg(target_os = "windows")]
fn main() {
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "app_status",
            "connect_codex",
            "disconnect_codex",
            "send_chat",
        ]),
    ))
    .expect("failed to build Tauri desktop command permissions");
}

#[cfg(not(target_os = "windows"))]
fn main() {}
