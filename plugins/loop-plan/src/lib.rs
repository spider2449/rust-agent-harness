//! `loop-plan`: a plan-and-execute agent loop, interchangeable with
//! `loop-react`.
//!
//! 1. Plan: one model request without tools asks for a short numbered plan.
//!    The plan is recorded as an assistant message.
//! 2. Execute: each plan step (at most `max_steps`) runs as a bounded model
//!    and tool loop (at most `max_rounds_per_step` model requests that may
//!    call tools). A system item gives the plan and the current step; results
//!    of earlier steps stay in the context. A step ends when the model answers
//!    without tool calls. When its tool rounds are used up, one more request
//!    without tools asks for the step's result from what it has.
//!
//! The answer of the turn is the text of the last step. Like every loop, it
//! applies `model.before_request`, routes every tool call through the
//! [`ToolInvoker`](rah_api::tools::ToolInvoker), appends every step to the
//! session log before continuing (a resumed session never re-executes a tool
//! call), resolves the model per request, and runs each turn as a task of
//! this plugin's scope.
//!
//! Configuration: `{ "max_steps": 8, "max_rounds_per_step": 16 }`.

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

const PLAN_INSTRUCTION: &str = "Before acting, write a short numbered plan (at most a few \
steps) for the user's next request. Reply with the plan only; do not carry it out yet.";

pub struct PlanLoopPlugin {
    manifest: PluginManifest,
}

impl PlanLoopPlugin {
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
    max_steps: Option<usize>,
    max_rounds_per_step: Option<u32>,
}

#[async_trait]
impl Plugin for PlanLoopPlugin {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    async fn apply(&self, scope: &mut Scope) -> Result<(), PluginError> {
        let config: Config = if scope.config().is_null() {
            Config::default()
        } else {
            scope.config_as()?
        };
        let plan = PlanLoop {
            kernel: scope.kernel(),
            session: scope.inject(SESSION)?,
            spawner: scope.spawner(),
            max_steps: config.max_steps.unwrap_or(8).max(1),
            max_rounds: config.max_rounds_per_step.unwrap_or(16).max(1),
        };
        scope.provide(LOOP, Arc::new(plan))?;
        Ok(())
    }
}

/// The steps of a plan: numbered or bulleted lines, or the whole text.
pub fn steps(plan: &str, max: usize) -> Vec<String> {
    let listed: Vec<String> = plan
        .lines()
        .map(str::trim)
        .filter_map(|line| {
            let rest = line.trim_start_matches(|c: char| c.is_ascii_digit());
            let numbered =
                rest.len() < line.len() && (rest.starts_with('.') || rest.starts_with(')'));
            if numbered {
                Some(rest[1..].trim())
            } else {
                line.strip_prefix("- ")
                    .or_else(|| line.strip_prefix("* "))
                    .map(str::trim)
            }
        })
        .filter(|step| !step.is_empty())
        .map(str::to_owned)
        .collect();
    let mut steps = if listed.is_empty() {
        vec![plan.trim().to_owned()]
    } else {
        listed
    };
    steps.truncate(max);
    steps
}

struct PlanLoop {
    kernel: KernelHandle,
    session: Arc<dyn SessionLog>,
    spawner: Spawner,
    max_steps: usize,
    max_rounds: u32,
}

impl AgentLoop for PlanLoop {
    fn run_turn(&self, request: TurnRequest) -> TurnStream {
        let (sender, receiver) = mpsc::unbounded();
        let turn = Turn {
            kernel: self.kernel.clone(),
            session: self.session.clone(),
            max_steps: self.max_steps,
            max_rounds: self.max_rounds,
            id: RequestId::new(),
            sid: request.session.clone(),
            cancel: request.cancel.clone(),
            stop: self.spawner.cancellation(),
            events: sender.clone(),
        };
        if !self.spawner.spawn(turn.run(request.input)) {
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

type Calls = Vec<(String, ToolName, serde_json::Value)>;

struct Turn {
    kernel: KernelHandle,
    session: Arc<dyn SessionLog>,
    max_steps: usize,
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
            Ok(Ended::Completed(text)) => match self
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
            },
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

    async fn assistant(&self, text: &str) -> Result<(), Failure> {
        if text.is_empty() {
            return Ok(());
        }
        self.append(SessionEntry::AssistantMessage {
            turn: self.id.clone(),
            text: text.to_owned(),
        })
        .await
        .map(|_| ())
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
        let user = ContextItem::User { text: input };

        // Plan: no tools, the instruction just before the request.
        let mut planning = items.clone();
        planning.push(ContextItem::System {
            text: PLAN_INSTRUCTION.into(),
        });
        planning.push(user.clone());
        let Some((plan, _)) = self.model_round(planning, Vec::new()).await? else {
            return Ok(Ended::Cancelled);
        };
        self.assistant(&plan).await?;
        let steps = steps(&plan, self.max_steps);

        items.push(user);
        let invoker = ToolInvoker::new(self.kernel.clone());
        let mut answer = String::new();
        for (index, step) in steps.iter().enumerate() {
            let instruction = ContextItem::System {
                text: format!(
                    "Plan:\n{plan}\n\nYou are on step {} of {}: {step}\nUse tools as needed. \
                     When this step is done, reply with its result and no tool calls.",
                    index + 1,
                    steps.len()
                ),
            };
            let mut rounds = 0;
            loop {
                if self.cancelled() {
                    return Ok(Ended::Cancelled);
                }
                // With the tool rounds used up, one more request without
                // tools asks for the step's result from what it has.
                let last = rounds == self.max_rounds;
                rounds += 1;
                // The step instruction goes just before the turn's user message.
                let mut context = items.clone();
                let at = context
                    .iter()
                    .rposition(|item| matches!(item, ContextItem::User { .. }))
                    .unwrap_or(context.len());
                context.insert(at, instruction.clone());
                let tools = if last {
                    context.push(ContextItem::System {
                        text: format!(
                            "This step has used all {} model rounds that may call tools. Do \
                             not request tools. Reply now with the step's result from what you \
                             have gathered, and say briefly what remains unfinished.",
                            self.max_rounds
                        ),
                    });
                    Vec::new()
                } else {
                    invoker.definitions()
                };
                let Some((text, calls)) = self.model_round(context, tools).await? else {
                    return Ok(Ended::Cancelled);
                };
                if last && text.is_empty() {
                    return Err(failure(
                        AgentErrorCode::Model,
                        format!(
                            "step {} exceeded {} model rounds and the model gave no result",
                            index + 1,
                            self.max_rounds
                        ),
                    ));
                }
                self.assistant(&text).await?;
                if !text.is_empty() {
                    items.push(ContextItem::Assistant { text: text.clone() });
                }
                // Tool calls a provider sends in the last round anyway are not run.
                if calls.is_empty() || last {
                    answer = text;
                    break;
                }
                for (call_id, name, arguments) in calls {
                    if self.cancelled() {
                        return Ok(Ended::Cancelled);
                    }
                    self.tool_round(&invoker, &mut items, call_id, name, arguments)
                        .await?;
                }
            }
        }
        Ok(Ended::Completed(answer))
    }

    /// One model request. `None` means the turn was cancelled.
    async fn model_round(
        &self,
        items: Vec<ContextItem>,
        tools: Vec<ToolDefinition>,
    ) -> Result<Option<(String, Calls)>, Failure> {
        let model = self
            .kernel
            .get(MODEL)
            .ok_or_else(|| failure(AgentErrorCode::Model, "no model provider is mounted"))?;
        let request = ModelRequest {
            model: None,
            items,
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

#[cfg(test)]
mod tests {
    use super::steps;

    #[test]
    fn plans_split_into_numbered_or_bulleted_steps() {
        assert_eq!(
            steps("1. Read the file\n2) Fix it\nThen done", 8),
            ["Read the file", "Fix it"]
        );
        assert_eq!(steps("Plan:\n- one\n* two\n", 8), ["one", "two"]);
        assert_eq!(steps("Just answer.", 8), ["Just answer."]);
        assert_eq!(steps("1. a\n2. b\n3. c", 2), ["a", "b"]);
        assert_eq!(steps("2024 was a year", 8), ["2024 was a year"]);
    }
}
