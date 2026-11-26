//! `loop-react`: the default agent loop.
//!
//! Per round: apply `model.before_request`, stream one model request, record
//! text and tool calls, run every tool call through the
//! [`ToolInvoker`](rah_api::tools::ToolInvoker), then ask the model again,
//! until it answers without tool calls. Every step is appended to the session
//! log first, so a resumed session never re-executes a tool call.
//!
//! The model is resolved on every round (not injected), so model plugins can
//! be swapped while this loop keeps running. Each turn runs as a task owned by
//! this plugin's scope.
//!
//! `max_rounds` bounds the model requests that may request tools. When they
//! are used up, one more request without tools asks the model to answer with
//! what it has, so a long search ends with an answer instead of an error.
//!
//! A model request the provider cannot take (rate limit, overload, a lost
//! connection before the response starts) or an interrupted llama.cpp
//! stream is retried after a wait
//! (ADR 0051): `retries` times (default 10), the first after `retry_delay_ms`
//! (default 2000), doubling up to five times that. `AgentEvent::ModelRetrying`
//! reports each wait; `retries = 0` turns retrying off.
//!
//! Configuration: `{ "max_rounds": <n>, "retries": 10, "retry_delay_ms": 2000 }`
//! (`max_rounds` defaults to 128).

use std::sync::Arc;

use futures::channel::mpsc;
use rah_api::{
    CancellationToken, KernelHandle, Plugin, PluginError, PluginId, PluginManifest, Scope,
    agent_loop::{
        AgentLoop, LOOP, RunningTurn, RunningTurns, SteeringInbox, TurnEvent, TurnRequest,
        TurnSteering, TurnStream,
    },
    async_trait,
    hooks::ModelBeforeRequest,
    kernel::Spawner,
    model::MODEL,
    retry::RetryPolicy,
    session::{SESSION, SessionLog, rebuild_context},
    tools::{Invocation, ToolInvoker},
};
use rah_protocol::{
    AgentErrorCode, AgentEvent, AgentOutput, ContextItem, Message, MessageRole, ModelRequest,
    ModelRequestId, RequestId, SessionEntry, SessionId, ToolCall, ToolCallId, ToolDefinition,
    ToolInput, ToolName,
};
use serde::Deserialize;

const DEFAULT_MAX_ROUNDS: u32 = 128;

pub struct ReactLoopPlugin {
    manifest: PluginManifest,
}

impl ReactLoopPlugin {
    pub fn new(id: impl Into<PluginId>) -> Self {
        Self {
            manifest: PluginManifest::new(id, env!("CARGO_PKG_VERSION"))
                .provides(["loop"])
                .requires(["session"])
                .optional([
                    "model",
                    "workspace-turns",
                    "sandbox",
                    "running-turns",
                    "file-undo",
                ]),
        }
    }
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    retries: Option<u32>,
    retry_delay_ms: Option<u64>,
    max_rounds: Option<u32>,
}

#[async_trait]
impl Plugin for ReactLoopPlugin {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    fn check_config(&self, config: &serde_json::Value) -> Result<(), PluginError> {
        let config: Config = rah_api::config::parse(config)?;
        RetryPolicy::from_settings(config.retries, config.retry_delay_ms)?;
        Ok(())
    }

    async fn apply(&self, scope: &mut Scope) -> Result<(), PluginError> {
        let config: Config = rah_api::config::parse(scope.config())?;
        let react = ReactLoop {
            kernel: scope.kernel(),
            session: scope.inject(SESSION)?,
            spawner: scope.spawner(),
            running: scope
                .inject_optional(rah_api::agent_loop::RUNNING_TURNS)?
                .unwrap_or_default(),
            max_rounds: config.max_rounds.unwrap_or(DEFAULT_MAX_ROUNDS).max(1),
            retry: RetryPolicy::from_settings(config.retries, config.retry_delay_ms)?,
        };
        scope.provide(LOOP, Arc::new(react))?;
        Ok(())
    }
}

struct ReactLoop {
    kernel: KernelHandle,
    session: Arc<dyn SessionLog>,
    spawner: Spawner,
    running: Arc<RunningTurns>,
    max_rounds: u32,
    retry: RetryPolicy,
}

impl AgentLoop for ReactLoop {
    fn run_turn(&self, request: TurnRequest) -> TurnStream {
        self.run_steerable_turn(request).0
    }

    fn run_steerable_turn(&self, request: TurnRequest) -> (TurnStream, Option<TurnSteering>) {
        let (sender, receiver) = mpsc::unbounded();
        // One turn per session: a second one records nothing.
        let running = match self.running.admit(&request.session) {
            Ok(guard) => guard,
            Err(reason) => {
                let _ = sender.unbounded_send(AgentEvent::Failed {
                    session_id: request.session,
                    code: AgentErrorCode::InvalidRequest,
                    message: reason.into(),
                });
                return (Box::pin(receiver), None);
            }
        };
        let (steering, inbox) = TurnSteering::channel();
        let turn = Turn {
            steering: inbox,
            origin: request.origin,
            kernel: self.kernel.clone(),
            session: self.session.clone(),
            max_rounds: self.max_rounds,
            retry: self.retry,
            id: RequestId::new(),
            sid: request.session.clone(),
            cancel: request.cancel.clone(),
            stop: self.spawner.cancellation(),
            events: sender.clone(),
        };
        let input = request.input;
        let spawned = self.spawner.spawn(async move {
            turn.run(input, running).await;
        });
        if !spawned {
            let _ = sender.unbounded_send(AgentEvent::Failed {
                session_id: request.session,
                code: AgentErrorCode::Internal,
                message: "the agent loop is shutting down".into(),
            });
        }
        drop(sender);
        (Box::pin(receiver), Some(steering))
    }
}

struct Failure {
    code: AgentErrorCode,
    message: String,
}

fn failure(code: AgentErrorCode, message: impl Into<String>) -> Failure {
    Failure {
        code,
        message: message.into(),
    }
}

/// Adds the text of one model round to the answer of the turn. Rounds are
/// separate messages, so they are kept apart by a blank line: text before a
/// tool call does not run into the text after it.
fn add_round(answer: &mut String, text: &str) {
    if !answer.is_empty() {
        answer.push_str(
            "

",
        );
    }
    answer.push_str(text);
}

enum Ended {
    Completed(String),
    Cancelled,
}

struct Turn {
    steering: SteeringInbox,
    origin: rah_api::agent_loop::TurnOrigin,
    kernel: KernelHandle,
    session: Arc<dyn SessionLog>,
    max_rounds: u32,
    retry: RetryPolicy,
    id: RequestId,
    sid: SessionId,
    cancel: CancellationToken,
    stop: CancellationToken,
    events: mpsc::UnboundedSender<AgentEvent>,
}

impl Turn {
    async fn record_steering(
        &self,
        items: &mut Vec<ContextItem>,
        completing: bool,
    ) -> Result<bool, Failure> {
        self.steering
            .record_pending(
                self.session.as_ref(),
                &self.sid,
                &self.id,
                items,
                completing,
            )
            .await
            .map_err(|error| failure(AgentErrorCode::Session, error.to_string()))
    }

    fn emit(&self, event: AgentEvent) {
        self.kernel.emit(&TurnEvent {
            event: event.clone(),
        });
        let _ = self.events.unbounded_send(event);
    }

    fn cancelled(&self) -> bool {
        self.cancel.is_cancelled() || self.stop.is_cancelled() || self.events.is_closed()
    }

    async fn append(&self, entry: SessionEntry) -> Result<u64, Failure> {
        self.session
            .append(&self.sid, entry)
            .await
            .map_err(|error| failure(AgentErrorCode::Session, error.to_string()))
    }

    async fn run(self, input: String, running: RunningTurn) {
        let history = match rah_api::agent_loop::open_turn_history(
            &self.kernel,
            self.session.as_ref(),
            &self.sid,
        )
        .await
        {
            Ok(opened) => opened,
            Err(message) => {
                self.steering.close();
                drop(running);
                self.emit(AgentEvent::Failed {
                    session_id: self.sid.clone(),
                    code: AgentErrorCode::Session,
                    message: message.to_string(),
                });
                return;
            }
        };
        self.emit(AgentEvent::Started {
            session_id: self.sid.clone(),
            request_id: self.id.clone(),
        });
        let outcome = self.drive(input, &history.records).await;
        let sid = self.sid.clone();
        let terminal = match outcome {
            Ok(Ended::Completed(text)) => {
                match self
                    .append(SessionEntry::TurnCompleted {
                        turn: self.id.clone(),
                    })
                    .await
                {
                    Ok(_) => AgentEvent::Completed {
                        session_id: sid,
                        output: AgentOutput {
                            message: Message {
                                role: MessageRole::Assistant,
                                content: text,
                            },
                        },
                    },
                    Err(error) => AgentEvent::Failed {
                        session_id: sid,
                        code: error.code,
                        message: error.message,
                    },
                }
            }
            Ok(Ended::Cancelled) => {
                let _ = self
                    .append(SessionEntry::TurnFailed {
                        turn: self.id.clone(),
                        message: "cancelled".into(),
                    })
                    .await;
                AgentEvent::Cancelled { session_id: sid }
            }
            Err(error) => {
                let _ = self
                    .append(SessionEntry::TurnFailed {
                        turn: self.id.clone(),
                        message: error.message.clone(),
                    })
                    .await;
                AgentEvent::Failed {
                    session_id: sid,
                    code: error.code,
                    message: error.message,
                }
            }
        };
        self.steering.close();
        drop(running);
        self.emit(terminal);
    }

    async fn drive(
        &self,
        input: String,
        history: &[rah_protocol::SessionRecord],
    ) -> Result<Ended, Failure> {
        let mut items = rebuild_context(history);
        let input = rah_api::undo::record_turn_start(
            &self.kernel,
            self.session.as_ref(),
            &self.sid,
            &self.id,
            self.origin,
            input,
            history,
        )
        .await
        .map_err(|error| failure(AgentErrorCode::Session, error.to_string()))?;
        items.push(ContextItem::User { text: input });

        let invoker = ToolInvoker::new(self.kernel.clone());
        let mut answer = String::new();
        for _ in 0..self.max_rounds {
            if self.cancelled() {
                return Ok(Ended::Cancelled);
            }
            self.record_steering(&mut items, false).await?;
            let (text, calls) = match self.model_round(&items, invoker.definitions()).await? {
                Some(round) => round,
                None => return Ok(Ended::Cancelled),
            };
            if !text.is_empty() {
                self.append(SessionEntry::AssistantMessage {
                    turn: self.id.clone(),
                    text: text.clone(),
                })
                .await?;
                items.push(ContextItem::Assistant { text: text.clone() });
                add_round(&mut answer, &text);
            }
            if calls.is_empty() {
                if self.record_steering(&mut items, true).await? {
                    continue;
                }
                return Ok(Ended::Completed(answer));
            }
            for (call_id, name, arguments) in calls {
                if self.cancelled() {
                    return Ok(Ended::Cancelled);
                }
                if self.steering.has_pending() {
                    break;
                }
                self.tool_round(&invoker, &mut items, call_id, name, arguments)
                    .await?;
            }
        }
        self.final_round(items, answer).await
    }

    /// The tool rounds are used up: one more request, without tools, for an
    /// answer from what the turn has gathered. The note is not recorded, so a
    /// resumed session does not see it.
    async fn final_round(
        &self,
        mut items: Vec<ContextItem>,
        mut answer: String,
    ) -> Result<Ended, Failure> {
        if self.cancelled() {
            return Ok(Ended::Cancelled);
        }
        items.push(ContextItem::System {
            text: format!(
                "This message has used all {} model rounds that may call tools. Do not \
                 request tools. Answer now from what you have gathered, and say briefly \
                 what remains unfinished.",
                self.max_rounds
            ),
        });
        loop {
            self.record_steering(&mut items, false).await?;
            let Some((text, _)) = self.model_round(&items, Vec::new()).await? else {
                return Ok(Ended::Cancelled);
            };
            // Tool calls a provider sends anyway are not run.
            if !text.is_empty() {
                self.append(SessionEntry::AssistantMessage {
                    turn: self.id.clone(),
                    text: text.clone(),
                })
                .await?;
                items.push(ContextItem::Assistant { text: text.clone() });
                add_round(&mut answer, &text);
            }
            if self.record_steering(&mut items, true).await? {
                continue;
            }
            return if text.is_empty() {
                Err(failure(
                    AgentErrorCode::Model,
                    format!(
                        "the turn exceeded {} model rounds and the model gave no answer",
                        self.max_rounds
                    ),
                ))
            } else {
                Ok(Ended::Completed(answer))
            };
        }
    }

    /// One model request offering `tools`. `None` means the turn was
    /// cancelled.
    async fn model_round(
        &self,
        items: &[ContextItem],
        tools: Vec<ToolDefinition>,
    ) -> Result<Option<(String, rah_api::retry::ModelCalls)>, Failure> {
        let model = self
            .kernel
            .get(MODEL)
            .ok_or_else(|| failure(AgentErrorCode::Model, "no model provider is mounted"))?;
        let request = ModelRequest {
            model: None,
            items: items.to_vec(),
            tools,
        };
        let request = self
            .kernel
            .waterfall::<ModelBeforeRequest>(request)
            .await
            .map_err(|error| failure(AgentErrorCode::Internal, error.to_string()))?;
        let model_request_id = ModelRequestId::new();
        self.emit(AgentEvent::ModelRequestStarted {
            session_id: self.sid.clone(),
            model_request_id: model_request_id.clone(),
        });
        let response = rah_api::retry::complete(
            model.as_ref(),
            &self.kernel,
            request,
            &self.cancel,
            &self.retry,
            (&self.sid, &model_request_id),
            |event| self.emit(event),
        );
        tokio::select! {
            biased;
            () = self.cancel.cancelled() => Ok(None),
            () = self.stop.cancelled() => Ok(None),
            result = response => result
                .map(Some)
                .map_err(|error| failure(AgentErrorCode::Model, error.to_string())),
        }
    }

    async fn tool_round(
        &self,
        invoker: &ToolInvoker,
        items: &mut Vec<ContextItem>,
        call_id: String,
        name: ToolName,
        arguments: serde_json::Value,
    ) -> Result<(), Failure> {
        let tool_call = ToolCall {
            id: ToolCallId::new(),
            name: name.clone(),
            input: ToolInput(arguments.clone()),
        };
        self.append(SessionEntry::ToolCall {
            turn: self.id.clone(),
            call_id: call_id.clone(),
            tool_call_id: tool_call.id.clone(),
            name: name.clone(),
            arguments: arguments.clone(),
        })
        .await?;
        items.push(ContextItem::ToolCall {
            call_id: call_id.clone(),
            name,
            arguments,
        });
        self.emit(AgentEvent::ToolRequested {
            session_id: self.sid.clone(),
            tool_call: tool_call.clone(),
        });
        let invocation = Invocation {
            session: self.sid.clone(),
            call: tool_call.clone(),
            cancel: self.cancel.clone(),
        };
        // Sent when the policy has allowed the call and the tool starts, so a
        // frontend can show it running (and not while it waits for approval).
        let outcome = invoker
            .invoke(invocation, || {
                self.emit(AgentEvent::ToolStarted {
                    session_id: self.sid.clone(),
                    tool_call_id: tool_call.id.clone(),
                });
            })
            .await;
        self.append(SessionEntry::ToolResult {
            turn: self.id.clone(),
            call_id: call_id.clone(),
            output: outcome.output.clone(),
            effect: outcome.effect,
        })
        .await?;
        items.push(ContextItem::ToolResult {
            call_id,
            output: outcome.output.clone(),
        });
        self.emit(AgentEvent::ToolFinished {
            session_id: self.sid.clone(),
            tool_call_id: tool_call.id,
            output: outcome.output,
        });
        Ok(())
    }
}

impl ReactLoopPlugin {
    /// An example configuration, mostly the defaults, for reference (the Web
    /// UI shows it in an empty configuration editor). It is never applied.
    pub const CONFIG_EXAMPLE: &str = r#"{
  "max_rounds": 128,
  "retries": 10,
  "retry_delay_ms": 2000
}"#;
}

#[cfg(test)]
#[test]
fn the_config_example_is_a_valid_configuration() {
    let parsed: Result<Config, _> = serde_json::from_str(ReactLoopPlugin::CONFIG_EXAMPLE);
    assert!(parsed.is_ok(), "{:?}", parsed.err());
}

#[cfg(test)]
#[test]
fn the_rounds_of_an_answer_are_kept_apart() {
    let mut answer = String::new();
    add_round(&mut answer, "Let me check.");
    assert_eq!(answer, "Let me check.");
    add_round(&mut answer, "The file has 3 lines.");
    assert_eq!(
        answer,
        "Let me check.

The file has 3 lines."
    );
}
