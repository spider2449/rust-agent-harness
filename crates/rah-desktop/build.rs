#[cfg(target_os = "windows")]
fn main() {
    validate_task120_capability();
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "app_status",
            "trusted_profile_selection",
            "choose_trusted_profile",
            "clear_trusted_profile",
            "model_configuration",
            "set_model_configuration",
            "reset_model_preferences",
            "commit_identity",
            "set_commit_identity",
            "desktop_preferences_warning",
            "test_llama_cpp_endpoint",
            "choose_repository",
            "connect_codex",
            "disconnect_codex",
            "repository_snapshot",
            "repository_authorize_commit_review",
            "repository_stage_action",
            "repository_unstage_action",
            "send_chat",
            "cancel_chat",
            "new_conversation",
            "clear_conversation_history",
            "resume_previous_conversation",
            "conversation_transcript",
            "get_effective_authority_snapshot",
        ]),
    ))
    .expect("failed to build Tauri desktop command permissions");
}

/// Keeps the Task 120 frontend commands on the closed default IPC surface.
///
/// Tauri generates their command permissions from the manifest below, but the
/// generated permissions are inert until the Desktop capability opts into them.
#[cfg(target_os = "windows")]
fn validate_task120_capability() {
    const TASK120_FRONTEND_COMMANDS: &[&str] = &[
        "test_llama_cpp_endpoint",
        "cancel_chat",
        "reset_model_preferences",
        "desktop_preferences_warning",
    ];
    let capability_path = "capabilities/default.json";
    println!("cargo:rerun-if-changed={capability_path}");
    let capability = std::fs::read_to_string(capability_path)
        .expect("failed to read the default Desktop capability");

    for command in TASK120_FRONTEND_COMMANDS {
        let permission = format!(r#""allow-{}""#, command.replace('_', "-"));
        assert!(
            capability.contains(&permission),
            "Task 120 command `{command}` is registered but not authorized by {capability_path}: missing {permission}",
        );
    }
}

#[cfg(not(target_os = "windows"))]
fn main() {}
