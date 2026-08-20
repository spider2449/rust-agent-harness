use std::{
    collections::VecDeque,
    sync::atomic::{AtomicU64, Ordering},
};

use serde_json::{Value, json};
use tokio::sync::Mutex;

use crate::{
    CodexAdapterError,
    protocol::{self, Incoming},
    transport::AppServerTransport,
};

pub(crate) struct AppServerConnection {
    transport: Mutex<Box<dyn AppServerTransport>>,
    queued: Mutex<VecDeque<Incoming>>,
    next_id: AtomicU64,
}

impl AppServerConnection {
    pub(crate) async fn initialize(
        transport: impl AppServerTransport,
    ) -> Result<Self, CodexAdapterError> {
        let connection = Self {
            transport: Mutex::new(Box::new(transport)),
            queued: Mutex::new(VecDeque::new()),
            next_id: AtomicU64::new(1),
        };
        connection
            .request(
                protocol::INITIALIZE,
                json!({
                    "clientInfo": {
                        "name": "rah-runtime-codex",
                        "version": env!("CARGO_PKG_VERSION")
                    },
                    "capabilities": {
                        "experimentalApi": false,
                        "requestAttestation": false
                    }
                }),
            )
            .await?;
        connection.notify(protocol::INITIALIZED, json!({})).await?;
        Ok(connection)
    }

    pub(crate) async fn request(
        &self,
        method: &str,
        params: Value,
    ) -> Result<Value, CodexAdapterError> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let mut transport = self.transport.lock().await;
        transport
            .send(protocol::request(id, method, params))
            .await?;
        loop {
            match protocol::parse(transport.receive().await?)? {
                Incoming::Response {
                    id: response_id,
                    result,
                } if response_id == id => {
                    return Ok(result);
                }
                Incoming::Response {
                    id: response_id, ..
                } => {
                    return Err(CodexAdapterError::ProtocolViolation {
                        message: format!(
                            "received response ID {response_id} while waiting for {id}"
                        ),
                    });
                }
                Incoming::Notification { method, params } => self
                    .queued
                    .lock()
                    .await
                    .push_back(Incoming::Notification { method, params }),
                Incoming::Request { id, method } => self
                    .queued
                    .lock()
                    .await
                    .push_back(Incoming::Request { id, method }),
            }
        }
    }

    pub(crate) async fn notify(
        &self,
        method: &str,
        params: Value,
    ) -> Result<(), CodexAdapterError> {
        self.transport
            .lock()
            .await
            .send(protocol::notification(method, params))
            .await
    }

    pub(crate) async fn shutdown(&self) -> Result<(), CodexAdapterError> {
        self.transport.lock().await.shutdown().await
    }
}
