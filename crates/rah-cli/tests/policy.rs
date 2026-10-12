//! P4 acceptance for `policy-authority`: guarded presets, fail-closed
//! assignments and a boot that refuses to continue without its policy.

mod common;

use common::{TempDir, headless, kernel, new_session, reply, turn};
use rah_api::{Kernel, PluginId, tools::ToolInvoker};
use rah_cli::{BootError, Distribution, PRESETS, boot};
use serde_json::json;

#[test]
fn every_preset_except_minimal_mounts_the_policy() {
    for preset in PRESETS {
        let distribution = Distribution::preset(preset.name).unwrap();
        let kinds: Vec<&str> = distribution
            .plugin
            .iter()
            .map(|spec| spec.kind.as_str())
            .collect();
        assert_eq!(
            kinds.first() == Some(&"policy-authority"),
            preset.name != "minimal",
            "{}: {kinds:?}",
            preset.name
        );
        assert_eq!(preset.guarded, preset.name != "minimal");
    }
}

#[tokio::test]
async fn the_dev_preset_is_guarded_and_allows_assigned_built_in_tools() {
    let dir = TempDir::new("policy-guarded");
    let kernel = kernel(&headless(&dir)).await;
    let handle = kernel.handle();
    assert!(ToolInvoker::new(handle.clone()).is_guarded());
    let session = new_session(&handle).await;
    let events = turn(&handle, &session, "hello").await;
    assert_eq!(reply(&events), "[model-mock] echo returned: hello");
}

#[tokio::test]
async fn unassigned_tools_are_denied_and_never_run() {
    let dir = TempDir::new("policy-unassigned");
    let mut distribution = headless(&dir);
    // Keep only the `fs.read` assignment: `echo` is no longer assigned.
    distribution.set_config("policy-authority", "tools", json!({ "fs.read": "read" }));
    let kernel = kernel(&distribution).await;
    let handle = kernel.handle();
    let session = new_session(&handle).await;
    let events = turn(&handle, &session, "hello").await;
    assert_eq!(
        reply(&events),
        "[model-mock] echo returned: denied: tool `echo` has no permission assignment"
    );
}

#[tokio::test]
async fn an_inactive_policy_aborts_the_boot_before_later_plugins_mount() {
    let dir = TempDir::new("policy-inactive");
    let mut distribution = headless(&dir);
    distribution.set_config("policy-authority", "allow", json!(["read"]));
    distribution.set_config("policy-authority", "ask", json!(["read"]));
    let kernel = Kernel::new();
    let error = boot(&kernel, &distribution).await.unwrap_err();
    assert!(
        matches!(&error, BootError::PolicyInactive(id, _) if *id == PluginId::new("policy-authority")),
        "{error}"
    );
    assert_eq!(kernel.plugins().len(), 1, "{:?}", kernel.plugins());
}
