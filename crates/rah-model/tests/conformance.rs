use futures::{TryStreamExt, executor::block_on};
use rah_model::{GenerationOptions, MockBackend, ModelBackend, ModelEvent, ModelRequest};
use rah_protocol::{Message, MessageRole, ModelRequestId};

async fn assert_model_backend_conformance(backend: &dyn ModelBackend) {
    let request = ModelRequest {
        id: ModelRequestId::new(),
        messages: vec![Message {
            role: MessageRole::User,
            content: "contract input".to_owned(),
        }],
        tools: Vec::new(),
        options: GenerationOptions::default(),
    };
    let events = backend
        .complete(request)
        .await
        .expect("conforming backend accepts a valid request")
        .try_collect::<Vec<_>>()
        .await
        .expect("conforming backend streams valid events");
    assert!(matches!(events.last(), Some(ModelEvent::Completed)));
}

#[test]
fn mock_backend_satisfies_model_backend_contract() {
    block_on(assert_model_backend_conformance(&MockBackend::new(vec![
        vec![
            Ok(ModelEvent::TextDelta {
                text: "contract output".to_owned(),
            }),
            Ok(ModelEvent::Completed),
        ],
    ])));
}
