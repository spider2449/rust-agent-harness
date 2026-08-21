use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use rah_protocol::{
    AgentErrorCode, AgentEvent, PermissionLevel, SessionId, ToolCall, ToolCallId, ToolContent,
    ToolDefinition, ToolInput, ToolOutput,
};
use rah_tools::{EchoTool, Tool, ToolContext, ToolError, ToolRegistry};
use serde_json::{Value, json};
use tokio::{sync::mpsc, task::JoinHandle};

use crate::{
    CodexAdapterError,
    connection::{AppServerConnection, ServerRequest},
    runtime::SessionRecord,
};

const MAX_TRACKED_CALLS: usize = 128;

#[derive(Clone)]
pub(crate) struct BridgeConfig {
    pub(crate) registry: Arc<ToolRegistry>,
    pub(crate) allowed_permissions: Arc<Vec<PermissionLevel>>,
}

#[derive(Clone, Debug)]
pub(crate) enum BridgeControl {
    Cancel { thread_id: String, turn_id: String },
    Terminal { thread_id: String, turn_id: String },
}

#[derive(Clone, Debug)]
pub(crate) struct ToolSnapshot {
    pub(crate) definition: ToolDefinition,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct CallKey {
    thread_id: String,
    turn_id: String,
    call_id: String,
}

struct CallEntry {
    tool: String,
    arguments: Value,
    request_ids: Vec<Value>,
    waiters: Vec<Value>,
    state: CallState,
}

enum CallState {
    InFlight,
    Completed(Value),
    Cancelled,
}

struct ExecutionResult {
    key: CallKey,
    session_id: SessionId,
    call: ToolCall,
    result: Result<ToolOutput, ToolError>,
}

struct DynamicCallParams {
    thread_id: String,
    turn_id: String,
    call_id: String,
    namespace: Option<String>,
    tool: String,
    arguments: Value,
}

pub(crate) fn echo_snapshot(registry: &ToolRegistry) -> Result<ToolSnapshot, CodexAdapterError> {
    let definitions = registry.definitions();
    let expected = EchoTool::new().definition();
    if definitions != [expected.clone()] {
        return Err(CodexAdapterError::ProtocolViolation {
            message:
                "echo bridge requires a registry containing only the exact RAH EchoTool definition"
                    .to_owned(),
        });
    }
    Ok(ToolSnapshot {
        definition: expected,
    })
}

pub(crate) fn dynamic_tool_spec(snapshot: &ToolSnapshot) -> Value {
    json!({
        "type": "function",
        "name": "echo",
        "description": snapshot.definition.description,
        "inputSchema": snapshot.definition.input_schema,
        "deferLoading": false
    })
}

pub(crate) async fn run_bridge(
    connection: Arc<AppServerConnection>,
    sessions: Arc<Mutex<HashMap<SessionId, SessionRecord>>>,
    config: BridgeConfig,
    mut requests: mpsc::UnboundedReceiver<ServerRequest>,
    mut controls: mpsc::UnboundedReceiver<BridgeControl>,
) {
    let (completed_tx, mut completed_rx) = mpsc::unbounded_channel();
    let mut calls: HashMap<CallKey, CallEntry> = HashMap::new();
    let mut tasks: HashMap<CallKey, JoinHandle<()>> = HashMap::new();

    loop {
        tokio::select! {
            request = requests.recv() => {
                let Some(request) = request else {
                    abort_all(&mut tasks).await;
                    return;
                };
                handle_request(
                    request,
                    &connection,
                    &sessions,
                    &config,
                    &mut calls,
                    &mut tasks,
                    &completed_tx,
                ).await;
            }
            completion = completed_rx.recv() => {
                let Some(completion) = completion else { continue };
                if let Some(task) = tasks.remove(&completion.key) {
                    let _ = task.await;
                }
                finish_execution(completion, &connection, &mut calls);
            }
            control = controls.recv() => {
                let Some(control) = control else { continue };
                reconcile(control, &connection, &mut calls, &mut tasks).await;
            }
        }
    }
}

async fn handle_request(
    request: ServerRequest,
    connection: &Arc<AppServerConnection>,
    sessions: &Mutex<HashMap<SessionId, SessionRecord>>,
    config: &BridgeConfig,
    calls: &mut HashMap<CallKey, CallEntry>,
    tasks: &mut HashMap<CallKey, JoinHandle<()>>,
    completed: &mpsc::UnboundedSender<ExecutionResult>,
) {
    debug_assert_eq!(request.method, "item/tool/call");
    tracing::debug!(
        target: "rah",
        method = %request.method,
        request_id = %request.id,
        params = %request.params,
        "received Codex dynamic tool request"
    );
    let params = match parse_params(&request.params) {
        Ok(params) => params,
        Err(message) => {
            connection.respond_error(request.id, -32602, &message);
            return;
        }
    };
    if params.namespace.is_some() {
        connection.respond_error(
            request.id,
            -32602,
            "dynamic tool namespaces are unsupported",
        );
        return;
    }

    let routed = {
        let sessions = sessions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        sessions.iter().find_map(|(session_id, record)| {
            (record.thread_id == params.thread_id
                && record.active_turn.as_deref() == Some(params.turn_id.as_str()))
            .then(|| {
                (
                    session_id.clone(),
                    record.bridge_tools.get(&params.tool).cloned(),
                )
            })
        })
    };
    let Some((session_id, snapshot)) = routed else {
        connection.respond_error(
            request.id,
            -32602,
            "dynamic tool request is not owned by the active turn",
        );
        return;
    };
    let Some(snapshot) = snapshot else {
        connection.respond_result(request.id, failure_response("RAH dynamic tool is unknown"));
        publish_failure(
            connection,
            &session_id,
            &params.thread_id,
            &params.turn_id,
            AgentErrorCode::Tool,
            "unadvertised dynamic tool request was denied",
        );
        return;
    };
    let key = CallKey {
        thread_id: params.thread_id.clone(),
        turn_id: params.turn_id.clone(),
        call_id: params.call_id,
    };

    if let Some(entry) = calls.get_mut(&key) {
        if entry.tool != params.tool || entry.arguments != params.arguments {
            connection.respond_error(
                request.id,
                -32602,
                "dynamic tool call ID was reused with different content",
            );
            publish_failure(
                connection,
                &session_id,
                &params.thread_id,
                &params.turn_id,
                AgentErrorCode::InvalidRequest,
                "dynamic tool call replay did not match the original request",
            );
            if let Some(task) = tasks.remove(&key) {
                task.abort();
                let _ = task.await;
            }
            for id in entry.waiters.drain(..) {
                connection.respond_error(
                    id,
                    -32602,
                    "dynamic tool call was invalidated by a conflicting replay",
                );
            }
            entry.state = CallState::Cancelled;
            return;
        }
        if entry.request_ids.contains(&request.id) {
            return;
        }
        match &entry.state {
            CallState::InFlight => {
                entry.request_ids.push(request.id.clone());
                entry.waiters.push(request.id);
            }
            CallState::Completed(response) => {
                entry.request_ids.push(request.id.clone());
                connection.respond_result(request.id, response.clone())
            }
            CallState::Cancelled => {
                entry.request_ids.push(request.id.clone());
                connection.respond_error(request.id, -32800, "dynamic tool call was cancelled")
            }
        }
        return;
    }
    if calls.len() >= MAX_TRACKED_CALLS && !remove_one_completed(calls) {
        connection.respond_error(request.id, -32000, "dynamic tool call limit reached");
        return;
    }

    let call = ToolCall {
        id: ToolCallId::new(),
        name: snapshot.definition.name.clone(),
        input: ToolInput(params.arguments.clone()),
    };
    connection.publish_rah_event(
        params.thread_id.clone(),
        params.turn_id.clone(),
        AgentEvent::ToolRequested {
            session_id: session_id.clone(),
            tool_call: call.clone(),
        },
    );

    let current = config
        .registry
        .get(&call.name)
        .map(|tool| tool.definition());
    if current.as_ref() != Some(&snapshot.definition)
        || !config
            .allowed_permissions
            .contains(&snapshot.definition.permission)
    {
        let response = failure_response("RAH permission policy denied the dynamic tool call");
        connection.respond_result(request.id.clone(), response.clone());
        calls.insert(
            key,
            completed_entry(params.tool, params.arguments, request.id, response),
        );
        publish_failure(
            connection,
            &session_id,
            &params.thread_id,
            &params.turn_id,
            AgentErrorCode::PermissionDenied,
            "RAH permission policy denied the dynamic tool call",
        );
        return;
    }

    connection.publish_rah_event(
        params.thread_id.clone(),
        params.turn_id.clone(),
        AgentEvent::ToolStarted {
            session_id: session_id.clone(),
            tool_call_id: call.id.clone(),
        },
    );
    calls.insert(
        key.clone(),
        CallEntry {
            tool: params.tool,
            arguments: params.arguments,
            request_ids: vec![request.id.clone()],
            waiters: vec![request.id],
            state: CallState::InFlight,
        },
    );
    let registry = Arc::clone(&config.registry);
    let completed = completed.clone();
    let task_session_id = session_id.clone();
    let task_key = key.clone();
    let task_call = call.clone();
    let task = tokio::spawn(async move {
        let result = registry
            .execute(task_call.clone(), ToolContext::default())
            .await;
        let _ = completed.send(ExecutionResult {
            key: task_key,
            session_id: task_session_id,
            call: task_call,
            result,
        });
    });
    tasks.insert(key, task);
}

fn finish_execution(
    completion: ExecutionResult,
    connection: &AppServerConnection,
    calls: &mut HashMap<CallKey, CallEntry>,
) {
    let Some(entry) = calls.get_mut(&completion.key) else {
        return;
    };
    if matches!(entry.state, CallState::Cancelled) {
        return;
    }
    let response = match completion.result {
        Ok(output) => {
            let response = output_response(&output);
            connection.publish_rah_event(
                completion.key.thread_id.clone(),
                completion.key.turn_id.clone(),
                AgentEvent::ToolFinished {
                    session_id: completion.session_id.clone(),
                    tool_call_id: completion.call.id,
                    output,
                },
            );
            response
        }
        Err(_) => {
            connection.publish_rah_event(
                completion.key.thread_id.clone(),
                completion.key.turn_id.clone(),
                AgentEvent::Failed {
                    session_id: completion.session_id,
                    code: AgentErrorCode::Tool,
                    message: "RAH dynamic tool execution failed".to_owned(),
                },
            );
            failure_response("RAH tool execution failed")
        }
    };
    tracing::debug!(
        target: "rah",
        thread_id = %completion.key.thread_id,
        turn_id = %completion.key.turn_id,
        call_id = %completion.key.call_id,
        response = %response,
        "sending Codex dynamic tool response"
    );
    for id in entry.waiters.drain(..) {
        connection.respond_result(id, response.clone());
    }
    entry.state = CallState::Completed(response);
}

async fn reconcile(
    control: BridgeControl,
    connection: &AppServerConnection,
    calls: &mut HashMap<CallKey, CallEntry>,
    tasks: &mut HashMap<CallKey, JoinHandle<()>>,
) {
    let (thread_id, turn_id, terminal) = match control {
        BridgeControl::Cancel { thread_id, turn_id } => (thread_id, turn_id, false),
        BridgeControl::Terminal { thread_id, turn_id } => (thread_id, turn_id, true),
    };
    let keys = calls
        .keys()
        .filter(|key| key.thread_id == thread_id && key.turn_id == turn_id)
        .cloned()
        .collect::<Vec<_>>();
    for key in keys {
        if let Some(task) = tasks.remove(&key) {
            task.abort();
            let _ = task.await;
        }
        if let Some(entry) = calls.get_mut(&key) {
            for id in entry.waiters.drain(..) {
                connection.respond_error(id, -32800, "dynamic tool call was cancelled");
            }
            entry.state = CallState::Cancelled;
        }
    }
    if terminal {
        calls.retain(|key, _| key.thread_id != thread_id || key.turn_id != turn_id);
    }
}

async fn abort_all(tasks: &mut HashMap<CallKey, JoinHandle<()>>) {
    for task in tasks.values() {
        task.abort();
    }
    for (_, task) in tasks.drain() {
        let _ = task.await;
    }
}

fn parse_params(value: &Value) -> Result<DynamicCallParams, String> {
    let object = value
        .as_object()
        .ok_or_else(|| "dynamic tool params must be an object".to_owned())?;
    let required = |name: &str| {
        object
            .get(name)
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| format!("dynamic tool params field `{name}` must be a string"))
    };
    let namespace = match object.get("namespace") {
        None | Some(Value::Null) => None,
        Some(Value::String(value)) => Some(value.clone()),
        Some(_) => {
            return Err(
                "dynamic tool params field `namespace` must be a string or null".to_owned(),
            );
        }
    };
    let arguments = object
        .get("arguments")
        .cloned()
        .ok_or_else(|| "dynamic tool params are missing `arguments`".to_owned())?;
    Ok(DynamicCallParams {
        thread_id: required("threadId")?,
        turn_id: required("turnId")?,
        call_id: required("callId")?,
        namespace,
        tool: required("tool")?,
        arguments,
    })
}

fn output_response(output: &ToolOutput) -> Value {
    let content_items = output
        .content
        .iter()
        .map(|content| match content {
            ToolContent::Text(text) => json!({ "type": "inputText", "text": text }),
            ToolContent::Json(value) => json!({ "type": "inputText", "text": value.to_string() }),
        })
        .collect::<Vec<_>>();
    json!({ "contentItems": content_items, "success": !output.is_error })
}

fn failure_response(message: &str) -> Value {
    json!({
        "contentItems": [{ "type": "inputText", "text": message }],
        "success": false
    })
}

fn completed_entry(
    tool: String,
    arguments: Value,
    request_id: Value,
    response: Value,
) -> CallEntry {
    CallEntry {
        tool,
        arguments,
        request_ids: vec![request_id],
        waiters: Vec::new(),
        state: CallState::Completed(response),
    }
}

fn remove_one_completed(calls: &mut HashMap<CallKey, CallEntry>) -> bool {
    let key = calls.iter().find_map(|(key, entry)| {
        matches!(entry.state, CallState::Completed(_)).then(|| key.clone())
    });
    key.is_some_and(|key| calls.remove(&key).is_some())
}

fn publish_failure(
    connection: &AppServerConnection,
    session_id: &SessionId,
    thread_id: &str,
    turn_id: &str,
    code: AgentErrorCode,
    message: &str,
) {
    connection.publish_rah_event(
        thread_id.to_owned(),
        turn_id.to_owned(),
        AgentEvent::Failed {
            session_id: session_id.clone(),
            code,
            message: message.to_owned(),
        },
    );
}
