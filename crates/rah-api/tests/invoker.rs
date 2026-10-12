use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use rah_api::{
    Kernel, MountOptions, Plugin, PluginError, PluginId, PluginManifest, Scope, async_trait,
    hooks::{ToolAfterInvoke, ToolApproval, ToolBeforeInvoke, ToolDecision},
    protocol::{
        PermissionLevel, SessionId, ToolCall, ToolCallId, ToolContent, ToolDefinition, ToolEffect,
        ToolInput, ToolName, ToolOutput,
    },
    tools::{Invocation, Tool, ToolContext, ToolError, ToolInvoker, provide_tools},
};
use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;

struct Counter {
    name: &'static str,
    permission: PermissionLevel,
    calls: Arc<AtomicUsize>,
    fail: Option<ToolError>,
}

#[async_trait]
impl Tool for Counter {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: ToolName::new(self.name),
            description: "test tool".into(),
            input_schema: json!({"type": "object"}),
            permission: self.permission,
        }
    }

    async fn call(&self, input: Value, _cx: ToolContext) -> Result<ToolOutput, ToolError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if let Some(error) = &self.fail {
            return Err(error.clone());
        }
        Ok(ToolOutput {
            content: vec![ToolContent::Json(input)],
            is_error: false,
        })
    }
}

type Apply = dyn Fn(&mut Scope) -> Result<(), PluginError> + Send + Sync;

struct TestPlugin(PluginManifest, Box<Apply>);

#[async_trait]
impl Plugin for TestPlugin {
    fn manifest(&self) -> &PluginManifest {
        &self.0
    }

    async fn apply(&self, scope: &mut Scope) -> Result<(), PluginError> {
        (self.1)(scope)
    }
}

fn plugin(
    manifest: PluginManifest,
    apply: impl Fn(&mut Scope) -> Result<(), PluginError> + Send + Sync + 'static,
) -> Arc<dyn Plugin> {
    Arc::new(TestPlugin(manifest, Box::new(apply)))
}

async fn kernel_with_tools(
    permission: PermissionLevel,
    fail: Option<ToolError>,
) -> (Kernel, Arc<AtomicUsize>) {
    let kernel = Kernel::new();
    let calls = Arc::new(AtomicUsize::new(0));
    let tools = {
        let calls = calls.clone();
        plugin(
            PluginManifest::new("tools", "1").provides(["tools"]),
            move |scope| {
                let tool = Counter {
                    name: "count",
                    permission,
                    calls: calls.clone(),
                    fail: fail.clone(),
                };
                provide_tools(scope, vec![Arc::new(tool)])?;
                Ok(())
            },
        )
    };
    kernel.mount(tools, MountOptions::default()).await.unwrap();
    (kernel, calls)
}

fn invocation(name: &str) -> Invocation {
    Invocation {
        session: SessionId::new(),
        call: ToolCall {
            id: ToolCallId::new(),
            name: ToolName::new(name),
            input: ToolInput(json!({"x": 1})),
        },
        cancel: CancellationToken::new(),
    }
}

async fn mount_policy(kernel: &Kernel, decision: Option<ToolDecision>, panics: bool) {
    let policy = plugin(PluginManifest::new("policy", "1"), move |scope| {
        let decision = decision.clone();
        scope.hook_bail::<ToolBeforeInvoke, _, _>(0, move |_request| {
            let decision = decision.clone();
            async move {
                assert!(!panics, "policy bug");
                decision
            }
        });
        Ok(())
    });
    kernel.mount(policy, MountOptions::default()).await.unwrap();
}

fn text(output: &ToolOutput) -> String {
    match &output.content[0] {
        ToolContent::Text(text) => text.clone(),
        ToolContent::Json(value) => value.to_string(),
    }
}

#[tokio::test]
async fn unguarded_invocation_runs_the_tool_and_reports_no_policy() {
    let (kernel, calls) = kernel_with_tools(PermissionLevel::Read, None).await;
    let invoker = ToolInvoker::new(kernel.handle());
    assert!(!invoker.is_guarded());
    assert_eq!(invoker.definitions().len(), 1);
    let started = AtomicUsize::new(0);
    let outcome = invoker
        .invoke(invocation("count"), || {
            started.fetch_add(1, Ordering::SeqCst);
        })
        .await;
    assert!(outcome.authorized && !outcome.output.is_error);
    assert_eq!(outcome.effect, ToolEffect::None);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(started.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn denial_and_policy_panic_fail_closed_without_calling_the_tool() {
    for (decision, panics) in [
        (
            Some(ToolDecision::Deny {
                reason: "nope".into(),
            }),
            false,
        ),
        (Some(ToolDecision::Allow), true),
    ] {
        let (kernel, calls) = kernel_with_tools(PermissionLevel::Write, None).await;
        mount_policy(&kernel, decision, panics).await;
        let invoker = ToolInvoker::new(kernel.handle());
        assert!(invoker.is_guarded());
        let outcome = invoker.invoke(invocation("count"), || {}).await;
        assert!(!outcome.authorized && outcome.output.is_error);
        assert!(text(&outcome.output).starts_with("denied: "));
        assert_eq!(outcome.effect, ToolEffect::None);
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }
}

#[tokio::test]
async fn ask_requires_an_approver() {
    let (kernel, calls) = kernel_with_tools(PermissionLevel::Execute, None).await;
    mount_policy(
        &kernel,
        Some(ToolDecision::Ask {
            reason: "risky".into(),
        }),
        false,
    )
    .await;
    let invoker = ToolInvoker::new(kernel.handle());
    let outcome = invoker.invoke(invocation("count"), || {}).await;
    assert!(text(&outcome.output).contains("no approver"));
    assert_eq!(calls.load(Ordering::SeqCst), 0);

    let approver = plugin(PluginManifest::new("approver", "1"), |scope| {
        scope.hook_bail::<ToolApproval, _, _>(0, |request| async move {
            Some(request.reason == "risky")
        });
        Ok(())
    });
    kernel
        .mount(approver, MountOptions::default())
        .await
        .unwrap();
    let outcome = invoker.invoke(invocation("count"), || {}).await;
    assert!(outcome.authorized && !outcome.output.is_error);
    assert_eq!(outcome.effect, ToolEffect::Completed);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn cancellation_ends_a_call_waiting_for_approval() {
    let (kernel, calls) = kernel_with_tools(PermissionLevel::Write, None).await;
    mount_policy(
        &kernel,
        Some(ToolDecision::Ask {
            reason: "risky".into(),
        }),
        false,
    )
    .await;
    let asked = Arc::new(tokio::sync::Notify::new());
    let approver = {
        let asked = asked.clone();
        plugin(PluginManifest::new("approver", "1"), move |scope| {
            let asked = asked.clone();
            scope.hook_bail::<ToolApproval, _, _>(0, move |_request| {
                let asked = asked.clone();
                async move {
                    asked.notify_one();
                    std::future::pending::<Option<bool>>().await
                }
            });
            Ok(())
        })
    };
    kernel
        .mount(approver, MountOptions::default())
        .await
        .unwrap();
    let invocation = invocation("count");
    let cancel = invocation.cancel.clone();
    let invoker = ToolInvoker::new(kernel.handle());
    let call = invoker.invoke(invocation, || panic!("must not start"));
    let cancel_once_asked = async {
        asked.notified().await;
        cancel.cancel();
    };
    let (outcome, ()) = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        tokio::join!(call, cancel_once_asked)
    })
    .await
    .expect("cancellation ends the approval wait");
    assert!(!outcome.authorized && outcome.output.is_error);
    assert!(text(&outcome.output).contains("cancelled before the tool started"));
    assert_eq!(outcome.effect, ToolEffect::None);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn effects_are_classified_by_permission_and_failure() {
    let cases = [
        (
            PermissionLevel::Read,
            Some(ToolError::Execution("x".into())),
            ToolEffect::None,
        ),
        (
            PermissionLevel::Write,
            Some(ToolError::Execution("x".into())),
            ToolEffect::Uncertain,
        ),
        (
            PermissionLevel::Execute,
            Some(ToolError::InvalidInput("x".into())),
            ToolEffect::None,
        ),
        (
            PermissionLevel::Write,
            Some(ToolError::NotStarted("x".into())),
            ToolEffect::None,
        ),
        (PermissionLevel::Write, None, ToolEffect::Completed),
    ];
    for (permission, fail, expected) in cases {
        let (kernel, _) = kernel_with_tools(permission, fail).await;
        let outcome = ToolInvoker::new(kernel.handle())
            .invoke(invocation("count"), || {})
            .await;
        assert_eq!(outcome.effect, expected, "{permission:?}");
    }
}

#[tokio::test]
async fn unknown_tools_and_pre_cancelled_calls_do_not_run() {
    let (kernel, calls) = kernel_with_tools(PermissionLevel::Write, None).await;
    let invoker = ToolInvoker::new(kernel.handle());
    let outcome = invoker.invoke(invocation("missing"), || {}).await;
    assert!(!outcome.authorized && text(&outcome.output).contains("unknown tool"));

    let cancelled = invocation("count");
    cancelled.cancel.cancel();
    let outcome = invoker.invoke(cancelled, || panic!("must not start")).await;
    assert_eq!(outcome.effect, ToolEffect::None);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn after_invoke_rewrites_output_and_tools_follow_provider_lifecycle() {
    let (kernel, _) = kernel_with_tools(PermissionLevel::Read, None).await;
    let redactor = plugin(PluginManifest::new("redactor", "1"), |scope| {
        scope.hook_waterfall::<ToolAfterInvoke, _, _>(0, |mut completion| async move {
            completion.output.content = vec![ToolContent::Text("[redacted]".into())];
            completion
        });
        Ok(())
    });
    kernel
        .mount(redactor, MountOptions::default())
        .await
        .unwrap();
    let invoker = ToolInvoker::new(kernel.handle());
    let outcome = invoker.invoke(invocation("count"), || {}).await;
    assert_eq!(text(&outcome.output), "[redacted]");

    kernel.unmount(&PluginId::new("tools")).await.unwrap();
    assert!(invoker.definitions().is_empty());
}
