use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use rah_api::{
    CancellationToken, Kernel, MountOptions, PluginId,
    tools::{Invocation, Tool, ToolContext, ToolError, ToolInvoker, provide_tools},
};
use rah_protocol::{SessionId, ToolCall, ToolCallId, ToolContent, ToolInput, ToolName, ToolOutput};
use serde_json::{Value, json};

use super::*;

fn definition(name: &str, permission: PermissionLevel) -> ToolDefinition {
    ToolDefinition {
        name: ToolName::new(name),
        description: "test tool".into(),
        input_schema: json!({"type": "object"}),
        permission,
    }
}

fn parse(config: Value) -> Policy {
    Policy::new(serde_json::from_value(config).unwrap()).unwrap()
}

fn deny_reason(decision: ToolDecision) -> String {
    match decision {
        ToolDecision::Deny { reason } => reason,
        other => panic!("expected a denial, got {other:?}"),
    }
}

#[test]
fn unassigned_tools_are_denied_even_when_they_declare_no_permission() {
    let policy = Policy::new(PolicyConfig::default()).unwrap();
    let reason = deny_reason(policy.decide(&definition("echo", PermissionLevel::None)));
    assert!(reason.contains("no permission assignment"), "{reason}");
}

#[test]
fn assigned_levels_map_to_allow_ask_and_deny() {
    let policy = parse(json!({
        "allow": ["none", "read"],
        "ask": ["write"],
        "tools": { "a": "none", "b": "read", "c": "write", "d": "execute" }
    }));
    let decide = |name| policy.decide(&definition(name, PermissionLevel::None));
    assert_eq!(decide("a"), ToolDecision::Allow);
    assert_eq!(decide("b"), ToolDecision::Allow);
    assert!(matches!(decide("c"), ToolDecision::Ask { reason } if reason.contains("`write`")));
    assert!(deny_reason(decide("d")).contains("`execute` permission, which is not granted"));
}

#[test]
fn declared_permission_can_tighten_but_never_grant() {
    let policy = parse(json!({ "tools": { "tool": "read" } }));
    // Declaring less than the assignment does not lower it.
    assert_eq!(
        policy.decide(&definition("tool", PermissionLevel::None)),
        ToolDecision::Allow
    );
    // Declaring more than the assignment raises it.
    assert!(matches!(
        policy.decide(&definition("tool", PermissionLevel::Execute)),
        ToolDecision::Ask { .. }
    ));
    let strict = parse(json!({ "ask": [], "tools": { "tool": "read" } }));
    deny_reason(strict.decide(&definition("tool", PermissionLevel::Write)));
}

#[test]
fn exact_names_win_over_prefixes_and_longer_prefixes_win() {
    let policy = parse(json!({
        "tools": { "repo.*": "read", "repo.git.*": "execute", "repo.list": "none", "*": "write" }
    }));
    assert_eq!(policy.assignment("repo.list"), Some(PermissionLevel::None));
    assert_eq!(
        policy.assignment("repo.search"),
        Some(PermissionLevel::Read)
    );
    assert_eq!(
        policy.assignment("repo.git.log"),
        Some(PermissionLevel::Execute)
    );
    assert_eq!(policy.assignment("other"), Some(PermissionLevel::Write));
    assert_eq!(
        Policy::new(PolicyConfig::default())
            .unwrap()
            .assignment("x"),
        None
    );
}

#[test]
fn invalid_configurations_are_rejected() {
    for config in [
        json!({ "allow": ["read"], "ask": ["read"] }),
        json!({ "tools": { "": "read" } }),
        json!({ "tools": { "a*b": "read" } }),
        json!({ "tools": { "a**": "read" } }),
        json!({ "decisions": { "": "deny" } }),
        json!({ "decisions": { "a*b": "allow" } }),
    ] {
        let parsed: PolicyConfig = serde_json::from_value(config.clone()).unwrap();
        assert!(Policy::new(parsed).is_err(), "{config}");
    }
    for config in [
        json!({ "tools": { "a": "admin" } }),
        json!({ "deny": ["read"] }),
        json!({ "decisions": { "a": "maybe" } }),
    ] {
        assert!(serde_json::from_value::<PolicyConfig>(config).is_err());
    }
}

#[test]
fn host_decisions_replace_the_level_rules_for_their_tools() {
    let policy = parse(json!({
        "tools": { "repo.*": "read", "fs.read": "read" },
        "decisions": {
            "repo.patch": "allow",
            "repo.status": "deny",
            "fs.read": "ask",
            "mcp.*": "deny",
            "mcp.files.read": "allow",
        }
    }));
    // A host decision can allow a tool that declares `write`, and needs no
    // level assignment.
    assert_eq!(
        policy.decide(&definition("repo.patch", PermissionLevel::Write)),
        ToolDecision::Allow
    );
    assert_eq!(
        policy.decide(&definition("mcp.files.read", PermissionLevel::Read)),
        ToolDecision::Allow
    );
    // A pattern assignment can be turned off for one tool.
    let reason = deny_reason(policy.decide(&definition("repo.status", PermissionLevel::Read)));
    assert!(reason.contains("turned off"), "{reason}");
    assert_eq!(
        policy.decide(&definition("repo.diff", PermissionLevel::Read)),
        ToolDecision::Allow
    );
    deny_reason(policy.decide(&definition("mcp.web.fetch", PermissionLevel::None)));
    assert!(matches!(
        policy.decide(&definition("fs.read", PermissionLevel::Read)),
        ToolDecision::Ask { .. }
    ));
    // Tools without a decision follow the levels; unassigned ones stay denied.
    deny_reason(policy.decide(&definition("shell.exec", PermissionLevel::Execute)));
}

struct Counter {
    name: &'static str,
    permission: PermissionLevel,
    calls: Arc<AtomicUsize>,
}

#[async_trait]
impl Tool for Counter {
    fn definition(&self) -> ToolDefinition {
        definition(self.name, self.permission)
    }

    async fn call(&self, _input: Value, _cx: ToolContext) -> Result<ToolOutput, ToolError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(ToolOutput {
            content: vec![ToolContent::Text("ran".into())],
            is_error: false,
        })
    }
}

struct Tools(PluginManifest, Arc<AtomicUsize>);

#[async_trait]
impl Plugin for Tools {
    fn manifest(&self) -> &PluginManifest {
        &self.0
    }

    async fn apply(&self, scope: &mut Scope) -> Result<(), PluginError> {
        let tools: Vec<Arc<dyn Tool>> = [
            ("echo", PermissionLevel::None),
            ("write", PermissionLevel::Write),
            ("unassigned", PermissionLevel::None),
        ]
        .into_iter()
        .map(|(name, permission)| {
            Arc::new(Counter {
                name,
                permission,
                calls: self.1.clone(),
            }) as Arc<dyn Tool>
        })
        .collect();
        provide_tools(scope, tools)?;
        Ok(())
    }
}

fn invocation(name: &str) -> Invocation {
    Invocation {
        session: SessionId::new(),
        call: ToolCall {
            id: ToolCallId::new(),
            name: ToolName::new(name),
            input: ToolInput(json!({})),
        },
        cancel: CancellationToken::new(),
    }
}

#[tokio::test]
async fn the_plugin_guards_the_invoker_and_fails_closed() {
    let kernel = Kernel::new();
    let calls = Arc::new(AtomicUsize::new(0));
    let tools = Tools(
        PluginManifest::new("tools", "1").provides(["tools"]),
        calls.clone(),
    );
    kernel
        .mount(Arc::new(tools), MountOptions::default())
        .await
        .unwrap();
    let invoker = ToolInvoker::new(kernel.handle());
    assert!(!invoker.is_guarded());

    let config = json!({ "tools": { "echo": "none", "write": "write" } });
    kernel
        .mount(
            Arc::new(PolicyAuthorityPlugin::new("policy")),
            MountOptions::with_config(config),
        )
        .await
        .unwrap();
    assert!(invoker.is_guarded());

    let outcome = invoker.invoke(invocation("echo"), || {}).await;
    assert!(outcome.authorized && !outcome.output.is_error);
    for name in ["unassigned", "write"] {
        let outcome = invoker.invoke(invocation(name), || {}).await;
        assert!(!outcome.authorized, "{name}");
    }
    assert_eq!(calls.load(Ordering::SeqCst), 1);

    // Unmounting the policy leaves the invoker unguarded again.
    kernel.unmount(&PluginId::new("policy")).await.unwrap();
    assert!(!invoker.is_guarded());
}

#[tokio::test]
async fn an_invalid_configuration_fails_the_plugin() {
    let kernel = Kernel::new();
    let status = kernel
        .mount(
            Arc::new(PolicyAuthorityPlugin::new("policy")),
            MountOptions::with_config(json!({ "tools": { "a*b": "read" } })),
        )
        .await
        .unwrap();
    assert!(
        matches!(status, rah_api::PluginStatus::Failed { ref message } if message.contains("`*`")),
        "{status:?}"
    );
    assert!(!ToolInvoker::new(kernel.handle()).is_guarded());
}
