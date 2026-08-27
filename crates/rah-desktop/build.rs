#[cfg(target_os = "windows")]
fn main() {
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "app_status",
            "model_configuration",
            "set_model_configuration",
            "choose_repository",
            "connect_codex",
            "disconnect_codex",
            "repository_snapshot",
            "send_chat",
            "new_conversation",
            "clear_conversation_history",
            "conversation_transcript",
        ]),
    ))
    .expect("failed to build Tauri desktop command permissions");
}

#[cfg(not(target_os = "windows"))]
fn main() {}
