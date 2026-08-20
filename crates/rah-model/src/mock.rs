use std::{collections::VecDeque, sync::Mutex};

use async_trait::async_trait;
use futures::stream;

use crate::{ModelBackend, ModelError, ModelEvent, ModelRequest, ModelStream};

struct MockState {
    turns: VecDeque<Vec<Result<ModelEvent, ModelError>>>,
    requests: Vec<ModelRequest>,
}

/// Deterministic model backend driven by queued event streams.
pub struct MockBackend {
    state: Mutex<MockState>,
}

impl MockBackend {
    /// Creates a backend from model turns consumed in queue order.
    #[must_use]
    pub fn new(turns: Vec<Vec<Result<ModelEvent, ModelError>>>) -> Self {
        Self {
            state: Mutex::new(MockState {
                turns: turns.into(),
                requests: Vec::new(),
            }),
        }
    }

    /// Returns the number of requests accepted by the backend.
    #[must_use]
    pub fn request_count(&self) -> usize {
        self.lock_state().requests.len()
    }

    /// Returns a snapshot of all requests accepted by the backend.
    #[must_use]
    pub fn requests(&self) -> Vec<ModelRequest> {
        self.lock_state().requests.clone()
    }

    fn lock_state(&self) -> std::sync::MutexGuard<'_, MockState> {
        self.state.lock().unwrap_or_else(|error| error.into_inner())
    }
}

#[async_trait]
impl ModelBackend for MockBackend {
    async fn complete(&self, request: ModelRequest) -> Result<ModelStream, ModelError> {
        let turn = {
            let mut state = self.lock_state();
            state.requests.push(request);
            state.turns.pop_front()
        };

        let events = turn.ok_or_else(|| ModelError::Unavailable {
            message: "mock backend script is exhausted".to_owned(),
        })?;

        Ok(Box::pin(stream::iter(events)))
    }
}

#[cfg(test)]
mod tests {
    use futures::{StreamExt, executor::block_on};
    use rah_protocol::{Message, MessageRole, ModelRequestId};

    use super::MockBackend;
    use crate::{GenerationOptions, ModelBackend, ModelError, ModelEvent, ModelRequest};

    fn request(content: &str) -> ModelRequest {
        ModelRequest {
            id: ModelRequestId::new(),
            messages: vec![Message {
                role: MessageRole::User,
                content: content.to_owned(),
            }],
            tools: Vec::new(),
            options: GenerationOptions::default(),
        }
    }

    #[test]
    fn scripted_turns_stream_in_queue_order() {
        block_on(async {
            let backend = MockBackend::new(vec![
                vec![
                    Ok(ModelEvent::TextDelta {
                        text: "first".to_owned(),
                    }),
                    Ok(ModelEvent::Completed),
                ],
                vec![
                    Ok(ModelEvent::TextDelta {
                        text: "second".to_owned(),
                    }),
                    Ok(ModelEvent::Completed),
                ],
            ]);

            let first = backend
                .complete(request("one"))
                .await
                .expect("first scripted turn should exist")
                .collect::<Vec<_>>()
                .await;
            let second = backend
                .complete(request("two"))
                .await
                .expect("second scripted turn should exist")
                .collect::<Vec<_>>()
                .await;

            assert_eq!(
                first,
                vec![
                    Ok(ModelEvent::TextDelta {
                        text: "first".to_owned()
                    }),
                    Ok(ModelEvent::Completed)
                ]
            );
            assert_eq!(
                second,
                vec![
                    Ok(ModelEvent::TextDelta {
                        text: "second".to_owned()
                    }),
                    Ok(ModelEvent::Completed)
                ]
            );
        });
    }

    #[test]
    fn requests_are_captured_for_assertions() {
        block_on(async {
            let backend = MockBackend::new(vec![vec![Ok(ModelEvent::Completed)]]);
            let expected = request("captured");

            let events = backend
                .complete(expected.clone())
                .await
                .expect("scripted turn should exist")
                .collect::<Vec<_>>()
                .await;

            assert_eq!(backend.request_count(), 1);
            assert_eq!(backend.requests(), vec![expected]);
            assert_eq!(events, vec![Ok(ModelEvent::Completed)]);
        });
    }

    #[test]
    fn empty_script_returns_defined_error() {
        block_on(async {
            let backend = MockBackend::new(Vec::new());

            let error = match backend.complete(request("missing")).await {
                Ok(_) => panic!("empty script should fail"),
                Err(error) => error,
            };

            assert_eq!(
                error,
                ModelError::Unavailable {
                    message: "mock backend script is exhausted".to_owned()
                }
            );
        });
    }
}
