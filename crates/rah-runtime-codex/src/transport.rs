use async_trait::async_trait;
use serde_json::Value;

use crate::CodexAdapterError;

#[async_trait]
pub(crate) trait AppServerTransport: Send + 'static {
    async fn send(&mut self, message: Value) -> Result<(), CodexAdapterError>;
    async fn receive(&mut self) -> Result<Value, CodexAdapterError>;
    async fn shutdown(&mut self) -> Result<(), CodexAdapterError>;
}
