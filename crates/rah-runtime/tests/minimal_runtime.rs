use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use async_trait::async_trait;
use futures::{StreamExt, executor::block_on, future};
use rah_model::{MockBackend, ModelBackend, ModelError, ModelEvent, ModelRequest, ModelStream};
use rah_protocol::{
    AgentErrorCode, AgentEvent, AgentInput, AgentOptions, AgentRequest, Message, MessageRole,
    PermissionLevel, RequestId, ToolCall, ToolCallId, ToolContent, ToolDefinition, ToolInput,
    ToolName, ToolOutput,
};
use rah_runtime::{AgentRuntime, MinimalTestRuntime};
use rah_tools::{EchoTool, Tool, ToolContext, ToolError, ToolRegistry};
use serde_json::json;
use tokio::sync::Notify;

#[test]
fn mock_model_drives_echo_tool_loop() {
    block_on(async {
        let backend = Arc::new(MockBackend::new(vec![
            vec![
                Ok(ModelEvent::ToolCall {
                    call: ToolCall {
                        id: ToolCallId::new(),
                        name: ToolName::new("echo"),
                        input: ToolInput(json!({"text": "echoed"})),
                    },
                }),
                Ok(ModelEvent::Completed),
            ],
            vec![
                Ok(ModelEvent::TextDelta {
                    text: "done".to_owned(),
                }),
                Ok(ModelEvent::Completed),
            ],
        ]));
        let mut registry = ToolRegistry::new();
        registry
            .register(Arc::new(EchoTool::new()))
            .expect("echo should register");
        let runtime = MinimalTestRuntime::new(backend.clone(), Arc::new(registry));
        let handle = runtime
            .start(AgentRequest {
                request_id: RequestId::new(),
                input: AgentInput {
                    messages: vec![Message {
                        role: MessageRole::User,
                        content: "echo something".to_owned(),
                    }],
                },
                options: AgentOptions::default(),
            })
            .await
            .expect("minimal runtime should complete");
        let events = handle.into_events().collect::<Vec<_>>().await;

        assert!(matches!(events[0], AgentEvent::Started { .. }));
        assert!(matches!(events[1], AgentEvent::ModelRequestStarted { .. }));
        assert!(matches!(events[2], AgentEvent::ToolRequested { .. }));
        assert!(matches!(events[3], AgentEvent::ToolStarted { .. }));
        assert!(matches!(events[4], AgentEvent::ToolFinished { .. }));
        assert!(matches!(events[5], AgentEvent::ModelRequestStarted { .. }));
        assert!(matches!(events[6], AgentEvent::ModelDelta { .. }));
        assert!(matches!(events[7], AgentEvent::Completed { .. }));
        assert_eq!(events.len(), 8);
        assert_eq!(backend.request_count(), 2);
        assert_eq!(
            backend.requests()[1].messages.last(),
            Some(&Message {
                role: MessageRole::Tool,
                content: "echoed".to_owned()
            })
        );
    });
}

struct BlockingBackend {
    started: Arc<Notify>,
}

#[async_trait]
impl ModelBackend for BlockingBackend {
    async fn complete(&self, _request: ModelRequest) -> Result<ModelStream, ModelError> {
        self.started.notify_one();
        future::pending().await
    }
}

struct BlockingTool {
    started: Arc<Notify>,
    dropped: Arc<AtomicBool>,
}

struct ExecuteProbe {
    executed: Arc<AtomicBool>,
}

#[async_trait]
impl Tool for ExecuteProbe {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: ToolName::new("process.test.probe"),
            description: "Records one deterministic Execute dispatch.".to_owned(),
            input_schema: json!({"type": "object", "additionalProperties": false}),
            permission: PermissionLevel::Execute,
        }
    }

    async fn execute(
        &self,
        _input: ToolInput,
        _context: ToolContext,
    ) -> Result<ToolOutput, ToolError> {
        self.executed.store(true, Ordering::SeqCst);
        Ok(ToolOutput {
            content: vec![ToolContent::Text("executed".to_owned())],
            is_error: false,
        })
    }
}

#[async_trait]
impl Tool for BlockingTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: ToolName::new("test.block"),
            description: "Waits until its execution future is cancelled.".to_owned(),
            input_schema: json!({"type": "object"}),
            permission: rah_protocol::PermissionLevel::None,
        }
    }

    async fn execute(
        &self,
        _input: ToolInput,
        _context: ToolContext,
    ) -> Result<ToolOutput, ToolError> {
        let _drop_marker = DropMarker(Arc::clone(&self.dropped));
        self.started.notify_one();
        future::pending().await
    }
}

struct DropMarker(Arc<AtomicBool>);

impl Drop for DropMarker {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

fn empty_request() -> AgentRequest {
    AgentRequest {
        request_id: RequestId::new(),
        input: AgentInput {
            messages: Vec::new(),
        },
        options: AgentOptions::default(),
    }
}

fn assert_cancelled_without_completion(events: &[AgentEvent]) {
    assert!(
        events
            .iter()
            .any(|event| matches!(event, AgentEvent::Cancelled { .. }))
    );
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, AgentEvent::Completed { .. }))
    );
}

#[tokio::test(flavor = "current_thread")]
async fn dropping_unpolled_handle_releases_session() {
    let backend = Arc::new(MockBackend::new(Vec::new()));
    let runtime = MinimalTestRuntime::new(backend, Arc::new(ToolRegistry::new()));
    let handle = runtime
        .start(empty_request())
        .await
        .expect("runtime should start");
    let session_id = handle.session_id().clone();

    drop(handle);

    let error = runtime
        .cancel(session_id.clone())
        .await
        .expect_err("dropped session should not remain active");
    assert_eq!(
        error,
        rah_runtime::AgentError::SessionNotFound { session_id }
    );
}

#[tokio::test(flavor = "current_thread")]
async fn cancellation_stops_running_model_operation() {
    let started = Arc::new(Notify::new());
    let runtime = MinimalTestRuntime::new(
        Arc::new(BlockingBackend {
            started: Arc::clone(&started),
        }),
        Arc::new(ToolRegistry::new()),
    );
    let handle = runtime
        .start(empty_request())
        .await
        .expect("runtime should start");
    let session_id = handle.session_id().clone();
    let collector = tokio::spawn(handle.into_events().collect::<Vec<_>>());
    started.notified().await;

    runtime
        .cancel(session_id)
        .await
        .expect("running session should cancel");
    let events = collector.await.expect("event collector should finish");

    assert_cancelled_without_completion(&events);
}

#[tokio::test(flavor = "current_thread")]
async fn cancellation_drops_running_tool_future() {
    let started = Arc::new(Notify::new());
    let dropped = Arc::new(AtomicBool::new(false));
    let call = ToolCall {
        id: ToolCallId::new(),
        name: ToolName::new("test.block"),
        input: ToolInput(json!({})),
    };
    let backend = Arc::new(MockBackend::new(vec![vec![Ok(ModelEvent::ToolCall {
        call,
    })]]));
    let mut registry = ToolRegistry::new();
    registry
        .register(Arc::new(BlockingTool {
            started: Arc::clone(&started),
            dropped: Arc::clone(&dropped),
        }))
        .expect("blocking tool should register");
    let runtime = MinimalTestRuntime::new(backend, Arc::new(registry));
    let handle = runtime
        .start(empty_request())
        .await
        .expect("runtime should start");
    let session_id = handle.session_id().clone();
    let collector = tokio::spawn(handle.into_events().collect::<Vec<_>>());
    started.notified().await;

    runtime
        .cancel(session_id)
        .await
        .expect("running session should cancel");
    let events = collector.await.expect("event collector should finish");

    assert!(dropped.load(Ordering::SeqCst));
    assert_cancelled_without_completion(&events);
    assert!(
        events
            .iter()
            .any(|event| matches!(event, AgentEvent::ToolStarted { .. }))
    );
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, AgentEvent::ToolFinished { .. }))
    );
}

#[tokio::test(flavor = "current_thread")]
async fn execute_permission_is_required_independently_of_registration() {
    let executed = Arc::new(AtomicBool::new(false));
    let call = ToolCall {
        id: ToolCallId::new(),
        name: ToolName::new("process.test.probe"),
        input: ToolInput(json!({})),
    };
    let backend = Arc::new(MockBackend::new(vec![vec![Ok(ModelEvent::ToolCall {
        call,
    })]]));
    let mut registry = ToolRegistry::new();
    registry
        .register(Arc::new(ExecuteProbe {
            executed: Arc::clone(&executed),
        }))
        .expect("Execute probe should register");
    let runtime = MinimalTestRuntime::new(backend, Arc::new(registry));

    let events = runtime
        .start(empty_request())
        .await
        .unwrap()
        .into_events()
        .collect::<Vec<_>>()
        .await;

    assert!(!executed.load(Ordering::SeqCst));
    assert!(events.iter().any(|event| matches!(
        event,
        AgentEvent::Failed {
            code: AgentErrorCode::PermissionDenied,
            ..
        }
    )));
    assert!(!events.iter().any(|event| matches!(
        event,
        AgentEvent::ToolStarted { .. } | AgentEvent::ToolFinished { .. }
    )));
}

#[tokio::test(flavor = "current_thread")]
async fn host_must_explicitly_enable_execute_permission() {
    let executed = Arc::new(AtomicBool::new(false));
    let call = ToolCall {
        id: ToolCallId::new(),
        name: ToolName::new("process.test.probe"),
        input: ToolInput(json!({})),
    };
    let backend = Arc::new(MockBackend::new(vec![
        vec![Ok(ModelEvent::ToolCall { call })],
        vec![
            Ok(ModelEvent::TextDelta {
                text: "done".to_owned(),
            }),
            Ok(ModelEvent::Completed),
        ],
    ]));
    let mut registry = ToolRegistry::new();
    registry
        .register(Arc::new(ExecuteProbe {
            executed: Arc::clone(&executed),
        }))
        .expect("Execute probe should register");
    let runtime = MinimalTestRuntime::new(backend, Arc::new(registry))
        .with_permission(PermissionLevel::Execute);

    let events = runtime
        .start(empty_request())
        .await
        .unwrap()
        .into_events()
        .collect::<Vec<_>>()
        .await;

    assert!(executed.load(Ordering::SeqCst));
    assert!(
        events
            .iter()
            .any(|event| matches!(event, AgentEvent::ToolStarted { .. }))
    );
    assert!(
        events
            .iter()
            .any(|event| matches!(event, AgentEvent::ToolFinished { .. }))
    );
    assert!(
        events
            .iter()
            .any(|event| matches!(event, AgentEvent::Completed { .. }))
    );
}
