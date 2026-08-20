use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use async_trait::async_trait;
use serde_json::{Value, json};
use tokio::sync::mpsc;

use crate::{CodexAdapterError, transport::AppServerTransport};

pub(crate) struct FakeTransport {
    incoming: mpsc::UnboundedReceiver<Result<Value, CodexAdapterError>>,
    sent: mpsc::UnboundedSender<Value>,
    stopped: Arc<AtomicBool>,
}

pub(crate) struct FakePeer {
    incoming: mpsc::UnboundedSender<Result<Value, CodexAdapterError>>,
    sent: mpsc::UnboundedReceiver<Value>,
    pub(crate) stopped: Arc<AtomicBool>,
}

pub(crate) fn fake_transport() -> (FakeTransport, FakePeer) {
    let (incoming_sender, incoming) = mpsc::unbounded_channel();
    let (sent, sent_receiver) = mpsc::unbounded_channel();
    let stopped = Arc::new(AtomicBool::new(false));
    (
        FakeTransport {
            incoming,
            sent,
            stopped: Arc::clone(&stopped),
        },
        FakePeer {
            incoming: incoming_sender,
            sent: sent_receiver,
            stopped,
        },
    )
}

impl FakePeer {
    pub(crate) async fn respond(&mut self, method: &str, result: Value) -> Value {
        let request = self.sent.recv().await.expect("outbound request");
        assert_eq!(request["method"], method);
        let id = request["id"].clone();
        self.incoming
            .send(Ok(json!({ "id": id, "result": result })))
            .expect("fake incoming channel");
        request
    }

    pub(crate) async fn expect_notification(&mut self, method: &str) -> Value {
        let notification = self.sent.recv().await.expect("outbound notification");
        assert_eq!(notification["method"], method);
        assert!(notification.get("id").is_none());
        notification
    }

    pub(crate) fn notify(&self, method: &str, params: Value) {
        self.incoming
            .send(Ok(json!({ "method": method, "params": params })))
            .expect("fake incoming channel");
    }
}

#[async_trait]
impl AppServerTransport for FakeTransport {
    async fn send(&mut self, message: Value) -> Result<(), CodexAdapterError> {
        self.sent.send(message).map_err(|_| disconnected())
    }

    async fn receive(&mut self) -> Result<Value, CodexAdapterError> {
        self.incoming
            .recv()
            .await
            .unwrap_or_else(|| Err(disconnected()))
    }

    async fn shutdown(&mut self) -> Result<(), CodexAdapterError> {
        self.stopped.store(true, Ordering::SeqCst);
        Ok(())
    }
}

fn disconnected() -> CodexAdapterError {
    CodexAdapterError::ProtocolViolation {
        message: "fake transport disconnected".to_owned(),
    }
}
