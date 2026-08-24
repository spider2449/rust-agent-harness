#[path = "../examples/support/live_gate_contract.rs"]
mod live_gate_contract;

use live_gate_contract::{require_completed, require_exactly_once};

fn completed_echo_sequence() -> Vec<String> {
    [
        "Started",
        "ModelRequestStarted",
        "ToolRequested",
        "ToolStarted",
        "ToolFinished",
        "ModelDelta",
        "Completed",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

fn record_diagnostic(_final_text: &str) {}

#[test]
fn echo_structural_success_ignores_final_assistant_text() {
    for final_text in [
        "",
        "RAH_ECHO_BRIDGE_OK",
        "Done. RAH_ECHO_BRIDGE_OK",
        "Done. The echo tool returned the expected value.",
    ] {
        require_exactly_once("echo", 1, 1, 1).expect("lifecycle should pass");
        require_completed(&completed_echo_sequence()).expect("turn should complete");
        record_diagnostic(final_text);
    }
}

#[test]
fn model_marker_without_echo_lifecycle_fails() {
    assert!(require_exactly_once("echo", 0, 0, 0).is_err());
    require_completed(&completed_echo_sequence()).expect("marker text is irrelevant");
}

#[test]
fn failed_echo_lifecycle_with_model_marker_fails() {
    assert!(require_exactly_once("echo", 1, 1, 0).is_err());
    require_completed(&completed_echo_sequence()).expect("marker text is irrelevant");
}

#[test]
fn missing_repo_patch_lifecycle_fails_despite_model_claim() {
    assert!(require_exactly_once("repo.patch", 0, 0, 0).is_err());
    require_completed(&completed_echo_sequence()).expect("model claim is non-authoritative");
}
