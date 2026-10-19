use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use rah_api::{
    Kernel, MountOptions, Plugin, PluginError, PluginId, PluginManifest, Scope, async_trait,
    hooks::{
        ToolAfterInvoke, ToolApproval, ToolBeforeInvoke, ToolDecision, ToolInputRepair,
        ToolRequestContext,
    },
    protocol::{
        PermissionLevel, SessionId, ToolCall, ToolCallId, ToolContent, ToolDefinition, ToolEffect,
        ToolInput, ToolName, ToolOutput,
    },
    tools::{
        Invocation, TOOL_ADMISSION, TOOL_ADMISSION_SERVICE, Tool, ToolAdmission, ToolContext,
        ToolError, ToolInvoker, ToolPermit, provide_tools,
    },
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
        other => panic!("unexpected content: {other:?}"),
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
async fn a_declined_call_tells_the_model_not_to_send_it_again() {
    let (kernel, calls) = kernel_with_tools(PermissionLevel::Write, None).await;
    mount_policy(
        &kernel,
        Some(ToolDecision::Ask {
            reason: "risky".into(),
        }),
        false,
    )
    .await;
    let approver = plugin(PluginManifest::new("approver", "1"), |scope| {
        scope.hook_bail::<ToolApproval, _, _>(0, |_request| async move { Some(false) });
        Ok(())
    });
    kernel
        .mount(approver, MountOptions::default())
        .await
        .unwrap();
    let invoker = ToolInvoker::new(kernel.handle());
    let outcome = invoker.invoke(invocation("count"), || {}).await;
    assert!(!outcome.authorized && outcome.output.is_error);
    let message = text(&outcome.output);
    assert!(
        message.starts_with("denied: rejected by the user"),
        "{message}"
    );
    assert!(
        message.contains("do not send the same call again"),
        "{message}"
    );
    assert_eq!(outcome.effect, ToolEffect::None);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
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

/// Admits at most `max` calls at once; `max == usize::MAX` panics instead.
struct Limit {
    max: usize,
    held: Arc<AtomicUsize>,
}

struct Held(Arc<AtomicUsize>);

impl Drop for Held {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

impl ToolAdmission for Limit {
    fn admit(&self, request: &ToolRequestContext) -> Result<ToolPermit, String> {
        assert!(self.max != usize::MAX, "admission bug");
        if self.held.load(Ordering::SeqCst) >= self.max {
            return Err(format!("`{}` waits for a free slot", request.call.name));
        }
        self.held.fetch_add(1, Ordering::SeqCst);
        Ok(ToolPermit::new(Held(self.held.clone())))
    }
}

async fn mount_limit(kernel: &Kernel, max: usize) -> Arc<AtomicUsize> {
    let held = Arc::new(AtomicUsize::new(0));
    let limit = {
        let held = held.clone();
        plugin(
            PluginManifest::new("limit", "1").provides([TOOL_ADMISSION_SERVICE]),
            move |scope| {
                let limit = Limit {
                    max,
                    held: held.clone(),
                };
                scope.provide(TOOL_ADMISSION, Arc::new(limit))?;
                Ok(())
            },
        )
    };
    kernel.mount(limit, MountOptions::default()).await.unwrap();
    held
}

/// A tool that signals when it starts and then never returns.
struct Hang(Arc<tokio::sync::Notify>);

#[async_trait]
impl Tool for Hang {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: ToolName::new("hang"),
            description: "never returns".into(),
            input_schema: json!({"type": "object"}),
            permission: PermissionLevel::Read,
        }
    }

    async fn call(&self, _input: Value, _cx: ToolContext) -> Result<ToolOutput, ToolError> {
        self.0.notify_one();
        std::future::pending().await
    }
}

#[tokio::test]
async fn admission_refuses_calls_over_the_limit_and_releases_on_every_ending() {
    let (kernel, calls) = kernel_with_tools(PermissionLevel::Read, None).await;
    let started = Arc::new(tokio::sync::Notify::new());
    let hang = {
        let started = started.clone();
        plugin(
            PluginManifest::new("hang", "1").provides(["tools"]),
            move |scope| {
                provide_tools(scope, vec![Arc::new(Hang(started.clone()))])?;
                Ok(())
            },
        )
    };
    kernel.mount(hang, MountOptions::default()).await.unwrap();
    let held = mount_limit(&kernel, 1).await;
    let invoker = ToolInvoker::new(kernel.handle());

    // A cancelled call releases its slot.
    let hanging = invocation("hang");
    let cancel = hanging.cancel.clone();
    let running = invoker.invoke(hanging, || {});
    let refuse_then_cancel = async {
        started.notified().await;
        assert_eq!(held.load(Ordering::SeqCst), 1);
        let refused = invoker
            .invoke(invocation("count"), || panic!("must not start"))
            .await;
        cancel.cancel();
        refused
    };
    let (cancelled, refused) = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        tokio::join!(running, refuse_then_cancel)
    })
    .await
    .expect("the call ends when cancelled");
    assert!(!refused.authorized && refused.output.is_error);
    assert_eq!(
        text(&refused.output),
        "not started: `count` waits for a free slot"
    );
    assert_eq!(refused.effect, ToolEffect::None);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert!(text(&cancelled.output).contains("cancelled"));
    assert_eq!(held.load(Ordering::SeqCst), 0);

    // A dropped invocation releases its slot.
    {
        let running = invoker.invoke(invocation("hang"), || {});
        tokio::pin!(running);
        tokio::select! {
            _ = &mut running => panic!("the tool never returns"),
            () = started.notified() => {}
        }
        assert_eq!(held.load(Ordering::SeqCst), 1);
    }
    assert_eq!(held.load(Ordering::SeqCst), 0);

    // A completed call releases its slot.
    let outcome = invoker.invoke(invocation("count"), || {}).await;
    assert!(outcome.authorized && !outcome.output.is_error);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(held.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn a_panicking_admission_refuses_the_call() {
    let (kernel, calls) = kernel_with_tools(PermissionLevel::Read, None).await;
    mount_limit(&kernel, usize::MAX).await;
    let outcome = ToolInvoker::new(kernel.handle())
        .invoke(invocation("count"), || panic!("must not start"))
        .await;
    assert!(!outcome.authorized);
    assert_eq!(
        text(&outcome.output),
        "not started: tool admission `limit` panicked"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn repaired_input_is_what_the_policy_judges_and_the_tool_receives() {
    let (kernel, calls) = kernel_with_tools(PermissionLevel::Read, None).await;
    let seen = Arc::new(std::sync::Mutex::new(Vec::<Value>::new()));
    let policy = {
        let seen = seen.clone();
        plugin(PluginManifest::new("policy", "1"), move |scope| {
            let seen = seen.clone();
            scope.hook_bail::<ToolBeforeInvoke, _, _>(0, move |request| {
                seen.lock().unwrap().push(request.call.input.0.clone());
                async { Some(ToolDecision::Allow) }
            });
            Ok(())
        })
    };
    kernel.mount(policy, MountOptions::default()).await.unwrap();
    let repair = plugin(PluginManifest::new("repair", "1"), |scope| {
        scope.hook_waterfall::<ToolInputRepair, _, _>(0, |mut request| async move {
            request.call.input = ToolInput(json!({"x": 2}));
            // Only the input counts: the tool and its definition are the invoker's.
            request.call.name = ToolName::new("other");
            request.definition.permission = PermissionLevel::Execute;
            request
        });
        Ok(())
    });
    kernel.mount(repair, MountOptions::default()).await.unwrap();
    let outcome = ToolInvoker::new(kernel.handle())
        .invoke(invocation("count"), || {})
        .await;
    assert!(
        outcome.authorized && !outcome.output.is_error,
        "{outcome:?}"
    );
    assert_eq!(text(&outcome.output), r#"{"x":2}"#);
    assert_eq!(outcome.effect, ToolEffect::None);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(*seen.lock().unwrap(), [json!({"x": 2})]);
}

#[tokio::test]
async fn a_failing_repair_leaves_the_input_as_sent() {
    let (kernel, calls) = kernel_with_tools(PermissionLevel::Read, None).await;
    let repair = plugin(PluginManifest::new("repair", "1"), |scope| {
        scope.hook_waterfall::<ToolInputRepair, _, _>(0, |_request| async move {
            panic!("repair bug");
        });
        Ok(())
    });
    kernel.mount(repair, MountOptions::default()).await.unwrap();
    let outcome = ToolInvoker::new(kernel.handle())
        .invoke(invocation("count"), || {})
        .await;
    assert!(
        outcome.authorized && !outcome.output.is_error,
        "{outcome:?}"
    );
    assert_eq!(text(&outcome.output), r#"{"x":1}"#);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

/// A provider with a tool called `count` whose description says who it is.
async fn mount_named_counter(kernel: &Kernel, id: &'static str, description: &'static str) {
    struct Named(&'static str, Counter);

    #[async_trait]
    impl Tool for Named {
        fn definition(&self) -> ToolDefinition {
            ToolDefinition {
                description: self.0.into(),
                ..self.1.definition()
            }
        }

        async fn call(&self, input: Value, cx: ToolContext) -> Result<ToolOutput, ToolError> {
            self.1.call(input, cx).await
        }
    }

    let provider = plugin(
        PluginManifest::new(id, "1").provides(["tools"]),
        move |scope| {
            let counter = Counter {
                name: "count",
                permission: PermissionLevel::Read,
                calls: Arc::new(AtomicUsize::new(0)),
                fail: None,
            };
            provide_tools(scope, vec![Arc::new(Named(description, counter))])?;
            Ok(())
        },
    );
    kernel
        .mount(provider, MountOptions::default())
        .await
        .unwrap();
}

#[tokio::test]
async fn on_a_name_clash_the_oldest_provider_wins_until_it_goes_away() {
    // Policy assigns permissions by tool name, so a later plugin must not be
    // able to take over a name an earlier one already serves.
    let kernel = Kernel::new();
    mount_named_counter(&kernel, "first", "first").await;
    mount_named_counter(&kernel, "second", "second").await;
    let invoker = ToolInvoker::new(kernel.handle());
    let definitions = invoker.definitions();
    assert_eq!(definitions.len(), 1);
    assert_eq!(definitions[0].description, "first");

    kernel.unmount(&PluginId::new("first")).await.unwrap();
    assert_eq!(invoker.definitions()[0].description, "second");
    let outcome = invoker.invoke(invocation("count"), || {}).await;
    assert!(outcome.authorized && !outcome.output.is_error);
}
