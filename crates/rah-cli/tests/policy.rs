//! P4 acceptance for `policy-authority`: guarded presets, fail-closed
//! assignments and a boot that refuses to continue without its policy.

mod common;

use common::{TempDir, headless, kernel, new_session, reply, turn};
use rah_api::{
    CancellationToken, Kernel, PluginId,
    tools::{Invocation, ToolInvoker},
};
use rah_cli::{BootError, Distribution, PRESETS, boot};
use rah_protocol::{ToolCall, ToolCallId, ToolContent, ToolInput, ToolName};
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
async fn the_presets_allow_the_built_in_file_tools_without_asking() {
    let dir = TempDir::new("policy-fs-tools");
    std::fs::create_dir_all(dir.workspace().join("src")).unwrap();
    std::fs::write(dir.workspace().join("src/lib.rs"), "fn found() {}\n").unwrap();
    let kernel = kernel(&headless(&dir)).await;
    let handle = kernel.handle();
    let session = new_session(&handle).await;
    let invoker = ToolInvoker::new(handle);
    for (tool, input, expected) in [
        ("fs.list", json!({}), "src/"),
        ("fs.glob", json!({ "pattern": "*.rs" }), "src/lib.rs"),
        (
            "fs.grep",
            json!({ "pattern": "found" }),
            "src/lib.rs:1: fn found() {}",
        ),
    ] {
        let call = ToolCall {
            id: ToolCallId::new(),
            name: ToolName::new(tool),
            input: ToolInput(input),
        };
        let invocation = Invocation {
            session: session.clone(),
            call,
            cancel: CancellationToken::new(),
        };
        let outcome = invoker.invoke(invocation, || {}).await;
        assert!(outcome.authorized, "{tool}: {outcome:?}");
        assert_eq!(
            outcome.output.content,
            vec![ToolContent::Text(expected.into())],
            "{tool}"
        );
    }
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
    // Only the distribution plugin and the failed policy were mounted.
    let mounted: Vec<String> = kernel
        .plugins()
        .into_iter()
        .map(|(id, _)| id.to_string())
        .collect();
    assert_eq!(mounted, ["distribution", "policy-authority"]);
}
