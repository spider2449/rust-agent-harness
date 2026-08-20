use std::sync::Arc;

use futures::{StreamExt, executor::block_on};
use rah_model::{MockBackend, ModelEvent};
use rah_protocol::{
    AgentEvent, AgentInput, AgentOptions, AgentRequest, Message, MessageRole, RequestId, ToolCall,
    ToolCallId, ToolInput, ToolName,
};
use rah_runtime::{AgentRuntime, MinimalTestRuntime};
use rah_tools::{EchoTool, ToolRegistry};
use serde_json::json;

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
