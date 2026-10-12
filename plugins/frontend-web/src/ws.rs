//! The WebSocket: turns, their events, cancellation and approvals. One turn
//! at a time per connection; closing the connection cancels it.

use std::str::FromStr;

use axum::{
    extract::{
        State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    response::Response,
};
use futures::StreamExt;
use rah_api::{
    CancellationToken,
    agent_loop::{LOOP, TurnRequest, TurnStream},
};
use rah_protocol::{AgentEvent, SessionId};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::broadcast::error::RecvError;

use crate::AppState;

pub(crate) async fn upgrade(State(shared): State<AppState>, upgrade: WebSocketUpgrade) -> Response {
    upgrade
        .max_message_size(1024 * 1024)
        .on_upgrade(move |socket| serve(socket, shared))
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ClientMessage {
    Turn { session: String, input: String },
    Cancel,
    Approval { id: String, approve: bool },
}

struct Running {
    session: SessionId,
    events: TurnStream,
    cancel: CancellationToken,
}

impl Drop for Running {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

async fn next_event(running: &mut Option<Running>) -> Option<AgentEvent> {
    match running {
        Some(turn) => turn.events.next().await,
        None => std::future::pending().await,
    }
}

async fn send(socket: &mut WebSocket, message: &Value) -> bool {
    socket
        .send(Message::Text(message.to_string().into()))
        .await
        .is_ok()
}

fn error(message: impl Into<String>) -> Value {
    json!({ "type": "error", "message": message.into() })
}

async fn serve(mut socket: WebSocket, shared: AppState) {
    let (_client, mut notices, waiting) = shared.approvals.connect();
    for notice in waiting {
        if !send(&mut socket, &notice).await {
            return;
        }
    }
    let mut running: Option<Running> = None;
    loop {
        let reply = tokio::select! {
            () = shared.stop.cancelled() => break,
            message = socket.recv() => match message {
                Some(Ok(Message::Text(text))) => match serde_json::from_str::<ClientMessage>(&text) {
                    Ok(message) => handle(&shared, &mut running, message),
                    Err(problem) => Some(error(format!("invalid message: {problem}"))),
                },
                Some(Ok(Message::Close(_))) | Some(Err(_)) | None => break,
                Some(Ok(_)) => None,
            },
            notice = notices.recv() => match notice {
                Ok(notice) => Some(notice),
                Err(RecvError::Lagged(_)) => None,
                Err(RecvError::Closed) => break,
            },
            event = next_event(&mut running) => match event {
                Some(event) => {
                    let session = running.as_ref().map(|turn| turn.session.to_string());
                    let terminal = matches!(
                        event,
                        AgentEvent::Completed { .. } | AgentEvent::Failed { .. } | AgentEvent::Cancelled { .. }
                    );
                    if terminal {
                        running = None;
                    }
                    Some(json!({ "type": "event", "session": session, "event": event }))
                }
                None => {
                    running = None;
                    Some(error("the agent loop stopped before the turn ended"))
                }
            },
        };
        if let Some(reply) = reply
            && !send(&mut socket, &reply).await
        {
            break;
        }
    }
}

fn handle(
    shared: &AppState,
    running: &mut Option<Running>,
    message: ClientMessage,
) -> Option<Value> {
    match message {
        ClientMessage::Turn { session, input } => {
            if running.is_some() {
                return Some(error("a turn is already running on this connection"));
            }
            let Ok(session) = SessionId::from_str(&session) else {
                return Some(error(format!("`{session}` is not a session id")));
            };
            let Some(agent_loop) = shared.kernel.get(LOOP) else {
                return Some(error("no agent loop is mounted"));
            };
            let cancel = shared.stop.child_token();
            let events = agent_loop.run_turn(TurnRequest {
                session: session.clone(),
                input,
                cancel: cancel.clone(),
            });
            *running = Some(Running {
                session,
                events,
                cancel,
            });
            None
        }
        ClientMessage::Cancel => {
            if let Some(turn) = running {
                turn.cancel.cancel();
            }
            None
        }
        ClientMessage::Approval { id, approve } => (!shared.approvals.answer(&id, approve))
            .then(|| error("that approval is no longer pending")),
    }
}
