//! The WebSocket: turns, their events, cancellation and approvals. One turn
//! at a time per connection, with an independent side question. Closing
//! the connection cancels both.

use std::str::FromStr;

use axum::{
    extract::{
        State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    response::Response,
};
use futures::{StreamExt, future::BoxFuture};
use rah_api::{
    CancellationToken,
    agent_loop::{LOOP, TurnRequest, TurnSteering, TurnStream},
    model::answer_side_question,
};
use rah_protocol::{AgentEvent, SessionId};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::{broadcast::error::RecvError, oneshot};

use crate::{AppState, Shared, approvals::Answered, settings};

pub(crate) async fn upgrade(State(shared): State<AppState>, upgrade: WebSocketUpgrade) -> Response {
    upgrade
        .max_message_size(1024 * 1024)
        .on_upgrade(move |socket| serve(socket, shared))
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ClientMessage {
    Turn {
        session: String,
        input: String,
    },
    Cancel,
    Steer {
        session: String,
        input: String,
    },
    SideQuestion {
        id: u64,
        session: String,
        question: String,
    },
    CancelSideQuestion {
        id: u64,
    },
    Approval {
        id: String,
        approve: bool,
        /// Also allow the tool for the rest of the session (ADR 0056).
        #[serde(default)]
        always: bool,
        /// Also allow this exact `shell.exec` command from now on, as a
        /// policy `commands` rule (ADR 0066).
        #[serde(default)]
        command: bool,
    },
}

struct Running {
    session: SessionId,
    events: TurnStream,
    cancel: CancellationToken,
    steering: Option<TurnSteering>,
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

struct SideQuestion {
    id: u64,
    answer: BoxFuture<'static, Value>,
}

impl SideQuestion {
    fn new(shared: &Shared, id: u64, session: String, question: String) -> Self {
        let kernel = shared.kernel.clone();
        let cancel = shared.stop.child_token();
        let answer = Box::pin(async move {
            let result = match SessionId::from_str(&session) {
                Ok(session) => answer_side_question(&kernel, &session, &question, cancel)
                    .await
                    .map_err(|problem| problem.to_string()),
                Err(_) => Err(format!("`{session}` is not a session id")),
            };
            match result {
                Ok(answer) => json!({ "type": "side_question", "id": id, "answer": answer }),
                Err(error) => json!({ "type": "side_question", "id": id, "error": error }),
            }
        });
        Self { id, answer }
    }
}

async fn next_side_answer(side: &mut Option<SideQuestion>) -> Value {
    match side {
        Some(side) => side.answer.as_mut().await,
        None => std::future::pending().await,
    }
}

async fn next_steer_result(receipt: &mut Option<oneshot::Receiver<()>>) -> Value {
    match receipt {
        Some(receipt) => match receipt.await {
            Ok(()) => json!({ "type": "steer_result" }),
            Err(_) => steer_error("the turn ended before confirming the guidance"),
        },
        None => std::future::pending().await,
    }
}

fn steer_error(error: impl ToString) -> Value {
    json!({ "type": "steer_result", "error": error.to_string() })
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
    let mut side: Option<SideQuestion> = None;
    let mut steer_receipt = None;
    loop {
        let reply = tokio::select! {
            () = shared.stop.cancelled() => break,
            message = socket.recv() => match message {
                Some(Ok(Message::Text(text))) => match serde_json::from_str::<ClientMessage>(&text) {
                    Ok(message) => handle(&shared, &mut running, &mut side, &mut steer_receipt, message).await,
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
            answer = next_side_answer(&mut side) => {
                side = None;
                Some(answer)
            },
            result = next_steer_result(&mut steer_receipt) => {
                steer_receipt = None;
                Some(result)
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
        if let Some(mut reply) = reply {
            if reply["type"] == "error" {
                reply["running"] = json!(running.is_some());
            }
            if !send(&mut socket, &reply).await {
                break;
            }
        }
    }
}

fn steer(
    running: Option<&Running>,
    receipt: &mut Option<oneshot::Receiver<()>>,
    session: &str,
    input: &str,
) -> Option<Value> {
    let Some(turn) = running.filter(|turn| {
        !turn.cancel.is_cancelled()
            && SessionId::from_str(session).is_ok_and(|id| id == turn.session)
    }) else {
        return Some(steer_error(
            "there is no active turn for this session on this connection",
        ));
    };
    let Some(steering) = &turn.steering else {
        return Some(steer_error(
            "this loop does not accept guidance during a turn",
        ));
    };
    if receipt.is_some() {
        return Some(steer_error("another message is waiting to be recorded"));
    }
    match steering.send(input) {
        Ok(pending) => {
            *receipt = Some(pending);
            None
        }
        Err(error) => Some(steer_error(error)),
    }
}

async fn handle(
    shared: &AppState,
    running: &mut Option<Running>,
    side: &mut Option<SideQuestion>,
    steer_receipt: &mut Option<oneshot::Receiver<()>>,
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
            let (events, steering) = agent_loop.run_steerable_turn(TurnRequest {
                origin: rah_api::agent_loop::TurnOrigin::User,
                session: session.clone(),
                input,
                cancel: cancel.clone(),
            });
            *running = Some(Running {
                session,
                events,
                cancel,
                steering,
            });
            None
        }
        ClientMessage::Steer { session, input } => {
            steer(running.as_ref(), steer_receipt, &session, &input)
        }
        ClientMessage::Cancel => {
            if let Some(turn) = running {
                turn.cancel.cancel();
            }
            None
        }
        ClientMessage::SideQuestion {
            id,
            session,
            question,
        } => {
            *side = Some(SideQuestion::new(shared, id, session, question));
            None
        }
        ClientMessage::CancelSideQuestion { id } => {
            if side.as_ref().is_some_and(|side| side.id == id) {
                *side = None;
            }
            None
        }
        ClientMessage::Approval {
            id,
            approve,
            always,
            command,
        } => match shared.approvals.answer(&id, approve, always, command) {
            None => Some(error("that approval is no longer pending")),
            // The call already goes on; the rule is for the calls after it.
            Some(Answered {
                allow_command: Some(rule),
            }) => settings::allow_command(shared, rule)
                .await
                .err()
                .map(|problem| {
                    error(format!(
                        "the command was allowed once, not remembered: {problem}"
                    ))
                }),
            Some(_) => None,
        },
    }
}
