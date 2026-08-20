use std::{path::Path, sync::Arc};

use crate::{
    CodexAdapterError, connection::AppServerConnection, process::ProcessTransport,
    transport::AppServerTransport,
};

/// Restricted Codex app-server runtime.
///
/// Construction verifies the exact CLI version and required schema contract,
/// starts an owned stdio child, and completes the app-server handshake.
pub struct CodexRuntime {
    pub(crate) connection: Arc<AppServerConnection>,
}

impl CodexRuntime {
    /// Starts and initializes a compatible Codex app-server executable.
    pub async fn connect(executable: impl AsRef<Path>) -> Result<Self, CodexAdapterError> {
        let transport = ProcessTransport::start(executable.as_ref()).await?;
        Self::from_transport(transport).await
    }

    pub(crate) async fn from_transport(
        transport: impl AppServerTransport,
    ) -> Result<Self, CodexAdapterError> {
        Ok(Self {
            connection: Arc::new(AppServerConnection::initialize(transport).await?),
        })
    }

    /// Stops the owned app-server transport.
    pub async fn shutdown(&self) -> Result<(), CodexAdapterError> {
        self.connection.shutdown().await
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::VecDeque,
        sync::{Arc, Mutex},
    };

    use async_trait::async_trait;
    use serde_json::{Value, json};

    use crate::{CodexAdapterError, transport::AppServerTransport};

    use super::CodexRuntime;

    struct FakeTransport {
        received: VecDeque<Value>,
        sent: Arc<Mutex<Vec<Value>>>,
    }

    #[async_trait]
    impl AppServerTransport for FakeTransport {
        async fn send(&mut self, message: Value) -> Result<(), CodexAdapterError> {
            self.sent.lock().expect("sent lock").push(message);
            Ok(())
        }

        async fn receive(&mut self) -> Result<Value, CodexAdapterError> {
            self.received
                .pop_front()
                .ok_or_else(|| CodexAdapterError::ProtocolViolation {
                    message: "fake transport exhausted".to_owned(),
                })
        }

        async fn shutdown(&mut self) -> Result<(), CodexAdapterError> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn handshake_sends_initialize_before_initialized() {
        let sent = Arc::new(Mutex::new(Vec::new()));
        let transport = FakeTransport {
            received: VecDeque::from([json!({"id": 1, "result": {}})]),
            sent: Arc::clone(&sent),
        };
        let runtime = CodexRuntime::from_transport(transport)
            .await
            .expect("handshake should succeed");
        {
            let messages = sent.lock().expect("sent lock");
            assert_eq!(messages.len(), 2);
            assert_eq!(messages[0]["method"], "initialize");
            assert_eq!(messages[1]["method"], "initialized");
        }
        runtime.shutdown().await.expect("shutdown should succeed");
    }
}
