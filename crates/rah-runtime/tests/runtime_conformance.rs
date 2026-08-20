use std::sync::Arc;

use futures::{StreamExt, executor::block_on};
use rah_model::{MockBackend, ModelEvent};
use rah_protocol::{
    AgentEvent, AgentInput, AgentOptions, AgentRequest, Message, MessageRole, RequestId,
};
use rah_runtime::{AgentRuntime, MinimalTestRuntime};
use rah_tools::ToolRegistry;

async fn assert_runtime_conformance(runtime: &dyn AgentRuntime) {
    let handle = runtime
        .start(AgentRequest {
            request_id: RequestId::new(),
            input: AgentInput {
                messages: vec![Message {
                    role: MessageRole::User,
                    content: "contract input".to_owned(),
                }],
            },
            options: AgentOptions::default(),
        })
        .await
        .expect("conforming runtime accepts valid input");
    let session_id = handle.session_id().clone();
    let events = handle.into_events().collect::<Vec<_>>().await;
    assert!(matches!(
        events.first(),
        Some(AgentEvent::Started { session_id: started, .. }) if started == &session_id
    ));
    let terminal_count = events
        .iter()
        .filter(|event| {
            matches!(
                event,
                AgentEvent::Completed { .. }
                    | AgentEvent::Failed { .. }
                    | AgentEvent::Cancelled { .. }
            )
        })
        .count();
    assert_eq!(terminal_count, 1);
}

#[test]
fn minimal_runtime_satisfies_runtime_contract() {
    block_on(assert_runtime_conformance(&MinimalTestRuntime::new(
        Arc::new(MockBackend::new(vec![vec![
            Ok(ModelEvent::TextDelta {
                text: "contract output".to_owned(),
            }),
            Ok(ModelEvent::Completed),
        ]])),
        Arc::new(ToolRegistry::new()),
    )));
}
