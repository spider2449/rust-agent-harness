use std::{collections::HashMap, sync::Mutex};

use serde_json::{Value, json};
use tokio::{
    sync::{broadcast, mpsc, oneshot},
    task::JoinHandle,
};

use crate::{
    CodexAdapterError,
    protocol::{self, Incoming},
    transport::AppServerTransport,
};

#[derive(Clone, Debug)]
pub(crate) enum ConnectionEvent {
    Notification {
        method: String,
        params: Value,
    },
    RahEvent {
        thread_id: String,
        turn_id: String,
        event: rah_protocol::AgentEvent,
    },
    UnsupportedRequest {
        method: String,
    },
    Fault {
        message: String,
    },
}

#[derive(Debug)]
pub(crate) struct ServerRequest {
    pub(crate) id: Value,
    pub(crate) method: String,
    pub(crate) params: Value,
}

enum Command {
    Request {
        method: String,
        params: Value,
        reply: oneshot::Sender<Result<Value, CodexAdapterError>>,
    },
    Notify {
        method: String,
        params: Value,
    },
    Respond {
        message: Value,
    },
    Shutdown {
        reply: oneshot::Sender<()>,
    },
}

pub(crate) struct AppServerConnection {
    commands: mpsc::UnboundedSender<Command>,
    events: broadcast::Sender<ConnectionEvent>,
    task: Mutex<Option<JoinHandle<()>>>,
    server_requests: Mutex<Option<mpsc::UnboundedReceiver<ServerRequest>>>,
}

impl AppServerConnection {
    pub(crate) async fn initialize(
        transport: impl AppServerTransport,
        experimental_api: bool,
    ) -> Result<Self, CodexAdapterError> {
        let (commands, receiver) = mpsc::unbounded_channel();
        let (events, _) = broadcast::channel(128);
        let (server_request_sender, server_requests) = mpsc::unbounded_channel();
        let task = tokio::spawn(run_connection(
            transport,
            receiver,
            events.clone(),
            server_request_sender,
            experimental_api,
        ));
        let connection = Self {
            commands,
            events,
            task: Mutex::new(Some(task)),
            server_requests: Mutex::new(Some(server_requests)),
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
                        "experimentalApi": experimental_api,
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
        let (reply, response) = oneshot::channel();
        self.commands
            .send(Command::Request {
                method: method.to_owned(),
                params,
                reply,
            })
            .map_err(|_| disconnected())?;
        response.await.map_err(|_| disconnected())?
    }

    pub(crate) async fn notify(
        &self,
        method: &str,
        params: Value,
    ) -> Result<(), CodexAdapterError> {
        self.commands
            .send(Command::Notify {
                method: method.to_owned(),
                params,
            })
            .map_err(|_| disconnected())
    }

    pub(crate) fn subscribe(&self) -> broadcast::Receiver<ConnectionEvent> {
        self.events.subscribe()
    }

    pub(crate) fn take_server_requests(&self) -> Option<mpsc::UnboundedReceiver<ServerRequest>> {
        self.server_requests
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
    }

    pub(crate) fn respond_result(&self, id: Value, result: Value) {
        let _ = self.commands.send(Command::Respond {
            message: protocol::success_response(id, result),
        });
    }

    pub(crate) fn respond_error(&self, id: Value, code: i64, message: &str) {
        let _ = self.commands.send(Command::Respond {
            message: protocol::error_response(id, code, message),
        });
    }

    pub(crate) fn publish_rah_event(
        &self,
        thread_id: String,
        turn_id: String,
        event: rah_protocol::AgentEvent,
    ) {
        let _ = self.events.send(ConnectionEvent::RahEvent {
            thread_id,
            turn_id,
            event,
        });
    }

    pub(crate) fn interrupt_now(&self, thread_id: String, turn_id: String) {
        let (reply, _response) = oneshot::channel();
        let _ = self.commands.send(Command::Request {
            method: "turn/interrupt".to_owned(),
            params: json!({ "threadId": thread_id, "turnId": turn_id }),
            reply,
        });
    }

    pub(crate) async fn shutdown(&self) -> Result<(), CodexAdapterError> {
        let (reply, response) = oneshot::channel();
        self.commands
            .send(Command::Shutdown { reply })
            .map_err(|_| disconnected())?;
        response.await.map_err(|_| disconnected())?;
        let task = self
            .task
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        if let Some(task) = task {
            task.await
                .map_err(|error| CodexAdapterError::ProtocolViolation {
                    message: format!("connection task failed: {error}"),
                })?;
        }
        Ok(())
    }
}

impl Drop for AppServerConnection {
    fn drop(&mut self) {
        if let Some(task) = self
            .task
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
        {
            task.abort();
        }
    }
}

async fn run_connection(
    mut transport: impl AppServerTransport,
    mut commands: mpsc::UnboundedReceiver<Command>,
    events: broadcast::Sender<ConnectionEvent>,
    server_requests: mpsc::UnboundedSender<ServerRequest>,
    experimental_api: bool,
) {
    let mut next_id = 1_u64;
    let mut pending = HashMap::new();
    loop {
        tokio::select! {
            command = commands.recv() => match command {
                Some(Command::Request { method, params, reply }) => {
                    let id = next_id;
                    next_id += 1;
                    if let Err(error) = transport.send(protocol::request(id, &method, params)).await {
                        let _ = reply.send(Err(error));
                        break;
                    }
                    pending.insert(id, reply);
                }
                Some(Command::Notify { method, params }) => {
                    if let Err(error) = transport.send(protocol::notification(&method, params)).await {
                        broadcast_fault(&events, &error);
                        break;
                    }
                }
                Some(Command::Respond { message }) => {
                    if let Err(error) = transport.send(message).await {
                        broadcast_fault(&events, &error);
                        break;
                    }
                }
                Some(Command::Shutdown { reply }) => {
                    let _ = transport.shutdown().await;
                    let _ = reply.send(());
                    return;
                }
                None => break,
            },
            message = transport.receive() => match message.and_then(protocol::parse) {
                Ok(Incoming::Response { id, result }) => {
                    if let Some(reply) = pending.remove(&id) {
                        let _ = reply.send(Ok(result));
                    } else {
                        let _ = events.send(ConnectionEvent::Fault {
                            message: format!("response ID {id} has no pending request"),
                        });
                        break;
                    }
                }
                Ok(Incoming::ErrorResponse { id, code, message }) => {
                    if let Some(reply) = pending.remove(&id) {
                        let _ = reply.send(Err(CodexAdapterError::JsonRpc { code, message }));
                    } else {
                        let _ = events.send(ConnectionEvent::Fault {
                            message: format!("error response ID {id} has no pending request"),
                        });
                        break;
                    }
                }
                Ok(Incoming::Notification { method, params }) => {
                    let _ = events.send(ConnectionEvent::Notification { method, params });
                }
                Ok(Incoming::Request { id, method, params }) => {
                    if experimental_api && method == "item/tool/call" {
                        if server_requests.send(ServerRequest { id: id.clone(), method, params }).is_err() {
                            let _ = transport.send(protocol::error_response(
                                id,
                                -32603,
                                "RAH dynamic tool responder is unavailable",
                            )).await;
                        }
                    } else {
                        let _ = transport.send(protocol::error_response(
                            id,
                            -32601,
                            "RAH restricted runtime does not support this Codex server request",
                        )).await;
                        let _ = events.send(ConnectionEvent::UnsupportedRequest { method });
                    }
                }
                Err(error) => {
                    broadcast_fault(&events, &error);
                    if let Some(id) = pending.keys().next().copied()
                        && let Some(reply) = pending.remove(&id)
                    {
                        let _ = reply.send(Err(error));
                    }
                    break;
                }
            }
        }
    }
    let _ = transport.shutdown().await;
    for (_, reply) in pending {
        let _ = reply.send(Err(disconnected()));
    }
}

fn broadcast_fault(events: &broadcast::Sender<ConnectionEvent>, error: &CodexAdapterError) {
    let _ = events.send(ConnectionEvent::Fault {
        message: error.to_string(),
    });
}

fn disconnected() -> CodexAdapterError {
    CodexAdapterError::ProtocolViolation {
        message: "app-server connection is not running".to_owned(),
    }
}
