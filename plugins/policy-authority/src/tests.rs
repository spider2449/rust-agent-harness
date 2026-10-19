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
async fn an_invalid_configuration_is_refused_and_a_running_policy_keeps_its_own() {
    let kernel = Kernel::new();
    let policy = PluginId::new("policy");
    for config in [
        json!({ "tools": { "a*b": "read" } }),
        json!({ "max_in_flight": 0 }),
        json!({ "allow": ["read"], "ask": ["read"] }),
        json!({ "tool": {} }),
    ] {
        let refused = kernel
            .mount(
                Arc::new(PolicyAuthorityPlugin::new("policy")),
                MountOptions::with_config(config.clone()),
            )
            .await;
        assert!(
            matches!(
                refused,
                Err(rah_api::kernel::KernelError::InvalidConfig { .. })
            ),
            "{config}: {refused:?}"
        );
        assert_eq!(kernel.status(&policy), None);
    }

    let calls = Arc::new(AtomicUsize::new(0));
    let tools = Tools(
        PluginManifest::new("tools", "1").provides(["tools"]),
        calls.clone(),
    );
    kernel
        .mount(Arc::new(tools), MountOptions::default())
        .await
        .unwrap();
    kernel
        .mount(
            Arc::new(PolicyAuthorityPlugin::new("policy")),
            MountOptions::with_config(json!({ "tools": { "echo": "none" } })),
        )
        .await
        .unwrap();
    // A typo in a new configuration must not leave tools unguarded.
    let refused = kernel
        .update_config(&policy, json!({ "tools": { "echo": "nothing" } }))
        .await;
    assert!(
        matches!(
            refused,
            Err(rah_api::kernel::KernelError::InvalidConfig { .. })
        ),
        "{refused:?}"
    );
    let invoker = ToolInvoker::new(kernel.handle());
    assert!(invoker.is_guarded());
    assert!(invoker.invoke(invocation("echo"), || {}).await.authorized);
    assert!(!invoker.invoke(invocation("write"), || {}).await.authorized);
}

#[test]
fn in_flight_admits_up_to_the_limit_and_releases_dropped_permits() {
    let in_flight = InFlight::new(2);
    let request = ToolRequestContext {
        session: SessionId::new(),
        call: invocation("echo").call,
        definition: definition("echo", PermissionLevel::None),
    };
    let first = in_flight.admit(&request).unwrap();
    let second = in_flight.admit(&request).unwrap();
    let refused = in_flight.admit(&request).unwrap_err();
    assert!(
        refused.contains("2 tool calls are already running"),
        "{refused}"
    );
    assert_eq!(in_flight.running(), 2);
    drop(first);
    assert_eq!(in_flight.running(), 1);
    let third = in_flight.admit(&request).unwrap();
    drop((second, third));
    assert_eq!(in_flight.running(), 0);
}

#[tokio::test]
async fn the_plugin_limits_tool_calls_in_flight() {
    let kernel = Kernel::new();
    let tools = Tools(
        PluginManifest::new("tools", "1").provides(["tools"]),
        Arc::new(AtomicUsize::new(0)),
    );
    kernel
        .mount(Arc::new(tools), MountOptions::default())
        .await
        .unwrap();
    kernel
        .mount(
            Arc::new(PolicyAuthorityPlugin::new("policy")),
            MountOptions::with_config(json!({ "tools": { "echo": "none" }, "max_in_flight": 1 })),
        )
        .await
        .unwrap();
    let invoker = ToolInvoker::new(kernel.handle());
    // Calls one after another each get the single slot.
    for _ in 0..2 {
        assert!(invoker.invoke(invocation("echo"), || {}).await.authorized);
    }
    // While the slot is taken, a call is refused before it starts.
    let admission = kernel
        .handle()
        .get(TOOL_ADMISSION)
        .expect("the policy admits calls");
    let request = ToolRequestContext {
        session: SessionId::new(),
        call: invocation("echo").call,
        definition: definition("echo", PermissionLevel::None),
    };
    let slot = admission.admit(&request).unwrap();
    let refused = invoker
        .invoke(invocation("echo"), || panic!("must not start"))
        .await;
    assert!(!refused.authorized);
    assert_eq!(refused.effect, rah_protocol::ToolEffect::None);
    drop(slot);
    assert!(invoker.invoke(invocation("echo"), || {}).await.authorized);
}

fn shell_call(config: Value, input: Value) -> ToolDecision {
    let policy = parse(config);
    policy.decide_call(&ToolRequestContext {
        session: SessionId::new(),
        call: ToolCall {
            id: ToolCallId::new(),
            name: ToolName::new("shell.exec"),
            input: ToolInput(input),
        },
        definition: definition("shell.exec", PermissionLevel::Execute),
    })
}

fn shell_config(commands: Value) -> Value {
    json!({ "tools": { "shell.exec": "execute" }, "commands": commands })
}

fn asks(decision: &ToolDecision) -> bool {
    matches!(decision, ToolDecision::Ask { .. })
}

#[test]
fn a_command_rule_allows_exactly_that_command() {
    let config = shell_config(json!([["git", "status"], ["git", "diff", "*"], ["ls"]]));
    let run = |input| shell_call(config.clone(), input);
    assert_eq!(
        run(json!({ "program": "git", "args": ["status"] })),
        ToolDecision::Allow
    );
    // Other options and arguments, or another program, are asked about.
    assert!(asks(&run(
        json!({ "program": "git", "args": ["status", "-s"] })
    )));
    assert!(asks(&run(json!({ "program": "git", "args": [] }))));
    assert!(asks(&run(json!({ "program": "git", "args": ["push"] }))));
    assert!(asks(&run(
        json!({ "program": "./git", "args": ["status"] })
    )));
    assert!(asks(&run(json!({ "program": "Git", "args": ["status"] }))));
    // A trailing `*` accepts any further arguments, none included.
    assert_eq!(
        run(json!({ "program": "git", "args": ["diff"] })),
        ToolDecision::Allow
    );
    assert_eq!(
        run(json!({ "program": "git", "args": ["diff", "--stat", "HEAD"] })),
        ToolDecision::Allow
    );
    assert!(asks(&run(
        json!({ "program": "git", "args": ["difftool"] })
    )));
    // No arguments is a missing `args`, too.
    assert_eq!(run(json!({ "program": "ls" })), ToolDecision::Allow);
    assert!(asks(&run(json!({ "program": "ls", "args": ["-l"] }))));
}

#[test]
fn malformed_input_matches_no_rule() {
    let config = shell_config(json!([["git", "status"], ["ls"]]));
    for input in [
        json!(null),
        json!("git status"),
        json!({ "program": ["git"], "args": ["status"] }),
        json!({ "program": "git", "args": "status" }),
        json!({ "program": "git", "args": ["status", 1] }),
        json!({ "args": ["status"] }),
        json!({ "command": "git status" }),
    ] {
        assert!(asks(&shell_call(config.clone(), input.clone())), "{input}");
    }
}

#[test]
fn a_rule_never_overrides_a_decision_a_denial_or_another_tool() {
    let input = json!({ "program": "git", "args": ["status"] });
    let mut config = shell_config(json!([["git", "status"]]));
    config["decisions"] = json!({ "shell.exec": "ask" });
    assert!(asks(&shell_call(config.clone(), input.clone())));
    config["decisions"] = json!({ "shell.exec": "deny" });
    assert!(matches!(
        shell_call(config, input.clone()),
        ToolDecision::Deny { .. }
    ));
    // Without an assignment the tool is denied, rule or not.
    let unassigned = json!({ "commands": [["git", "status"]] });
    assert!(matches!(
        shell_call(unassigned, input.clone()),
        ToolDecision::Deny { .. }
    ));
    // A tool that takes the same arguments under another name is not covered.
    let policy = parse(json!({
        "tools": { "other.exec": "execute" },
        "commands": [["git", "status"]],
    }));
    let decision = policy.decide_call(&ToolRequestContext {
        session: SessionId::new(),
        call: ToolCall {
            id: ToolCallId::new(),
            name: ToolName::new("other.exec"),
            input: ToolInput(input),
        },
        definition: definition("other.exec", PermissionLevel::Execute),
    });
    assert!(asks(&decision));
}

#[test]
fn a_command_rule_does_not_lower_a_denied_level() {
    // `execute` is neither allowed nor asked: the tool is denied.
    let config = json!({
        "ask": [],
        "tools": { "shell.exec": "execute" },
        "commands": [["git", "status"]],
    });
    assert!(matches!(
        shell_call(config, json!({ "program": "git", "args": ["status"] })),
        ToolDecision::Deny { .. }
    ));
}

fn tool_call(config: Value, name: &str, permission: PermissionLevel) -> ToolDecision {
    parse(config).decide_call(&ToolRequestContext {
        session: SessionId::new(),
        call: ToolCall {
            id: ToolCallId::new(),
            name: ToolName::new(name),
            input: ToolInput(json!({ "path": "a.txt" })),
        },
        definition: definition(name, permission),
    })
}

#[test]
fn an_approved_tool_runs_without_asking_and_others_are_still_asked() {
    let config = json!({
        "tools": { "repo.create-file": "write", "repo.patch": "write" },
        "approve": ["repo.create-file"],
    });
    let write = PermissionLevel::Write;
    assert_eq!(
        tool_call(config.clone(), "repo.create-file", write),
        ToolDecision::Allow
    );
    assert!(asks(&tool_call(config, "repo.patch", write)));
}

#[test]
fn approve_never_overrides_a_decision_a_denial_or_a_missing_assignment() {
    let write = PermissionLevel::Write;
    // The host's explicit decision wins, `ask` and `deny` alike.
    for (decision, asked) in [("ask", true), ("deny", false)] {
        let config = json!({
            "tools": { "repo.create-file": "write" },
            "decisions": { "repo.create-file": decision },
            "approve": ["repo.create-file"],
        });
        let outcome = tool_call(config, "repo.create-file", write);
        assert_eq!(asks(&outcome), asked, "{decision}");
        assert_ne!(outcome, ToolDecision::Allow, "{decision}");
    }
    // Without an assignment the tool is denied, listed or not (a decision of
    // `allow` would let it through; `approve` must not).
    let unassigned = json!({ "approve": ["repo.create-file"] });
    assert!(matches!(
        tool_call(unassigned, "repo.create-file", write),
        ToolDecision::Deny { .. }
    ));
    // A level that is neither allowed nor asked stays denied.
    let denied = json!({
        "ask": [],
        "tools": { "repo.create-file": "write" },
        "approve": ["repo.create-file"],
    });
    assert!(matches!(
        tool_call(denied, "repo.create-file", write),
        ToolDecision::Deny { .. }
    ));
}

#[test]
fn approve_covers_the_tool_named_and_no_other() {
    let config = json!({
        "tools": { "repo.*": "write" },
        "approve": ["repo.create-file"],
    });
    let write = PermissionLevel::Write;
    // Another name, and a name that merely starts the same way, are asked.
    assert!(asks(&tool_call(config.clone(), "repo.create-files", write)));
    assert!(asks(&tool_call(config.clone(), "repo.create", write)));
    // Names are case sensitive: this one has no assignment, so it is denied.
    let other_case = tool_call(config, "REPO.CREATE-FILE", write);
    assert!(
        matches!(other_case, ToolDecision::Deny { .. }),
        "{other_case:?}"
    );
}

#[test]
fn approve_takes_exact_names_only() {
    for name in ["*", "repo.*", "", "  "] {
        let config: PolicyConfig = serde_json::from_value(json!({ "approve": [name] })).unwrap();
        let error = Policy::new(config).unwrap_err();
        assert!(
            error.contains("`approve` takes exact tool names"),
            "{error}"
        );
    }
}

#[test]
fn approve_leaves_command_rules_alone() {
    // Approving `shell.exec` runs every command; a command rule still works
    // for a policy that does not list it.
    let input = json!({ "program": "git", "args": ["push"] });
    let mut config = shell_config(json!([["git", "status"]]));
    assert!(asks(&shell_call(config.clone(), input.clone())));
    config["approve"] = json!(["shell.exec"]);
    assert_eq!(shell_call(config, input), ToolDecision::Allow);
}

#[test]
fn invalid_command_rules_are_refused() {
    for (commands, expected) in [
        (json!([[]]), "needs a program"),
        (json!([[""]]), "needs a program"),
        (json!([["*"]]), "`*` may only be the last"),
        (json!([["git", "*", "status"]]), "`*` may only be the last"),
        (json!([["git", "*", "*"]]), "`*` may only be the last"),
    ] {
        let config: PolicyConfig = serde_json::from_value(json!({ "commands": commands })).unwrap();
        let error = Policy::new(config).unwrap_err();
        assert!(error.contains(expected), "{error}");
    }
}

#[tokio::test]
async fn the_plugin_applies_command_rules_to_the_invoker() {
    let kernel = Kernel::new();
    let calls = Arc::new(AtomicUsize::new(0));
    let shell = Counter {
        name: "shell.exec",
        permission: PermissionLevel::Execute,
        calls: calls.clone(),
    };
    struct Shell(PluginManifest, Arc<dyn Tool>);
    #[async_trait]
    impl Plugin for Shell {
        fn manifest(&self) -> &PluginManifest {
            &self.0
        }
        async fn apply(&self, scope: &mut Scope) -> Result<(), PluginError> {
            provide_tools(scope, vec![self.1.clone()])?;
            Ok(())
        }
    }
    kernel
        .mount(
            Arc::new(Shell(
                PluginManifest::new("shell", "1").provides(["tools"]),
                Arc::new(shell),
            )),
            MountOptions::default(),
        )
        .await
        .unwrap();
    kernel
        .mount(
            Arc::new(PolicyAuthorityPlugin::new("policy")),
            MountOptions::with_config(shell_config(json!([["git", "status"]]))),
        )
        .await
        .unwrap();
    let invoker = ToolInvoker::new(kernel.handle());
    let call = |input: Value| Invocation {
        session: SessionId::new(),
        call: ToolCall {
            id: ToolCallId::new(),
            name: ToolName::new("shell.exec"),
            input: ToolInput(input),
        },
        cancel: CancellationToken::new(),
    };
    // No approver is mounted, so an Ask is a denial and a rule is the only
    // way through.
    let allowed = invoker
        .invoke(call(json!({ "program": "git", "args": ["status"] })), || {})
        .await;
    assert!(
        allowed.authorized && !allowed.output.is_error,
        "{allowed:?}"
    );
    let asked = invoker
        .invoke(call(json!({ "program": "git", "args": ["push"] })), || {})
        .await;
    assert!(!asked.authorized);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}
