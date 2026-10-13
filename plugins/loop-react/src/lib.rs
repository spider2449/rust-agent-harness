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
//! Configuration: `{ "max_rounds": <n> }` (default 32).

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use futures::{StreamExt, channel::mpsc};
use rah_api::{
    CancellationToken, KernelHandle, Plugin, PluginError, PluginId, PluginManifest, Scope,
    agent_loop::{AgentLoop, LOOP, TurnEvent, TurnRequest, TurnStream},
    async_trait,
    hooks::ModelBeforeRequest,
    kernel::Spawner,
    model::MODEL,
    session::{SESSION, SessionLog, rebuild_context},
    tools::{Invocation, ToolInvoker},
};
use rah_protocol::{
    AgentErrorCode, AgentEvent, AgentOutput, ContextItem, Message, MessageRole, ModelEvent,
    ModelRequest, ModelRequestId, RequestId, SessionEntry, SessionId, ToolCall, ToolCallId,
    ToolDefinition, ToolInput, ToolName,
};
use serde::Deserialize;

const DEFAULT_MAX_ROUNDS: u32 = 32;

pub struct ReactLoopPlugin {
    manifest: PluginManifest,
}

impl ReactLoopPlugin {
    pub fn new(id: impl Into<PluginId>) -> Self {
        Self {
            manifest: PluginManifest::new(id, env!("CARGO_PKG_VERSION"))
                .provides(["loop"])
                .requires(["session"])
                .optional(["model"]),
        }
    }
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    max_rounds: Option<u32>,
}

#[async_trait]
impl Plugin for ReactLoopPlugin {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    async fn apply(&self, scope: &mut Scope) -> Result<(), PluginError> {
        let config: Config = if scope.config().is_null() {
            Config::default()
        } else {
            scope.config_as()?
        };
        let react = ReactLoop {
            kernel: scope.kernel(),
            session: scope.inject(SESSION)?,
            spawner: scope.spawner(),
            max_rounds: config.max_rounds.unwrap_or(DEFAULT_MAX_ROUNDS).max(1),
        };
        scope.provide(LOOP, Arc::new(react))?;
        Ok(())
    }
}

struct ReactLoop {
    kernel: KernelHandle,
    session: Arc<dyn SessionLog>,
    spawner: Spawner,
    max_rounds: u32,
}

impl AgentLoop for ReactLoop {
    fn run_turn(&self, request: TurnRequest) -> TurnStream {
        let (sender, receiver) = mpsc::unbounded();
        let turn = Turn {
            kernel: self.kernel.clone(),
            session: self.session.clone(),
            max_rounds: self.max_rounds,
            id: RequestId::new(),
            sid: request.session.clone(),
            cancel: request.cancel.clone(),
            stop: self.spawner.cancellation(),
            events: sender.clone(),
        };
        let spawned = self.spawner.spawn(turn.run(request.input));
        if !spawned {
            let _ = sender.unbounded_send(AgentEvent::Failed {
                session_id: request.session,
                code: AgentErrorCode::Internal,
                message: "the agent loop is shutting down".into(),
            });
        }
        drop(sender);
        Box::pin(receiver)
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

enum Ended {
    Completed(String),
    Cancelled,
}

struct Turn {
    kernel: KernelHandle,
    session: Arc<dyn SessionLog>,
    max_rounds: u32,
    id: RequestId,
    sid: SessionId,
    cancel: CancellationToken,
    stop: CancellationToken,
    events: mpsc::UnboundedSender<AgentEvent>,
}

impl Turn {
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

    async fn run(self, input: String) {
        self.emit(AgentEvent::Started {
            session_id: self.sid.clone(),
            request_id: self.id.clone(),
        });
        let outcome = self.drive(input).await;
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
        self.emit(terminal);
    }

    async fn drive(&self, input: String) -> Result<Ended, Failure> {
        let history = self
            .session
            .read(&self.sid)
            .await
            .map_err(|error| failure(AgentErrorCode::Session, error.to_string()))?;
        let mut items = rebuild_context(&history);
        self.append(SessionEntry::TurnStarted {
            turn: self.id.clone(),
            input: input.clone(),
        })
        .await?;
        items.push(ContextItem::User { text: input });

        let invoker = ToolInvoker::new(self.kernel.clone());
        let mut answer = String::new();
        for _ in 0..self.max_rounds {
            if self.cancelled() {
                return Ok(Ended::Cancelled);
            }
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
                answer.push_str(&text);
            }
            if calls.is_empty() {
                return Ok(Ended::Completed(answer));
            }
            for (call_id, name, arguments) in calls {
                if self.cancelled() {
                    return Ok(Ended::Cancelled);
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
        let Some((text, _)) = self.model_round(&items, Vec::new()).await? else {
            return Ok(Ended::Cancelled);
        };
        // Tool calls a provider sends anyway are not run.
        if text.is_empty() {
            return Err(failure(
                AgentErrorCode::Model,
                format!(
                    "the turn exceeded {} model rounds and the model gave no answer",
                    self.max_rounds
                ),
            ));
        }
        self.append(SessionEntry::AssistantMessage {
            turn: self.id.clone(),
            text: text.clone(),
        })
        .await?;
        answer.push_str(&text);
        Ok(Ended::Completed(answer))
    }

    /// One model request offering `tools`. `None` means the turn was
    /// cancelled.
    async fn model_round(
        &self,
        items: &[ContextItem],
        tools: Vec<ToolDefinition>,
    ) -> Result<Option<(String, Vec<(String, ToolName, serde_json::Value)>)>, Failure> {
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
        let mut stream = model
            .stream(request, self.cancel.clone())
            .await
            .map_err(|error| failure(AgentErrorCode::Model, error.to_string()))?;

        let (mut text, mut calls, mut reasoning) = (String::new(), Vec::new(), false);
        loop {
            let event = tokio::select! {
                biased;
                () = self.cancel.cancelled() => return Ok(None),
                () = self.stop.cancelled() => return Ok(None),
                event = stream.next() => event,
            };
            match event {
                None => {
                    return Err(failure(
                        AgentErrorCode::Model,
                        "the model stream ended without completing",
                    ));
                }
                Some(Err(error)) => return Err(failure(AgentErrorCode::Model, error.to_string())),
                Some(Ok(ModelEvent::TextDelta { text: delta })) => {
                    text.push_str(&delta);
                    self.emit(AgentEvent::ModelDelta {
                        session_id: self.sid.clone(),
                        model_request_id: model_request_id.clone(),
                        delta,
                    });
                }
                Some(Ok(ModelEvent::ReasoningStarted)) if !reasoning => {
                    reasoning = true;
                    self.emit(AgentEvent::ModelReasoningStarted {
                        session_id: self.sid.clone(),
                        model_request_id: model_request_id.clone(),
                    });
                }
                Some(Ok(ModelEvent::ReasoningStarted)) => {}
                Some(Ok(ModelEvent::Usage { usage })) => {
                    self.emit(AgentEvent::ModelUsage {
                        session_id: self.sid.clone(),
                        model_request_id: model_request_id.clone(),
                        usage,
                    });
                }
                Some(Ok(ModelEvent::ToolCall {
                    call_id,
                    name,
                    arguments,
                })) => calls.push((call_id, name, arguments)),
                Some(Ok(ModelEvent::Completed)) => return Ok(Some((text, calls))),
            }
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
        let started = AtomicBool::new(false);
        let invocation = Invocation {
            session: self.sid.clone(),
            call: tool_call.clone(),
            cancel: self.cancel.clone(),
        };
        let outcome = invoker
            .invoke(invocation, || started.store(true, Ordering::SeqCst))
            .await;
        if started.load(Ordering::SeqCst) {
            self.emit(AgentEvent::ToolStarted {
                session_id: self.sid.clone(),
                tool_call_id: tool_call.id.clone(),
            });
        }
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
