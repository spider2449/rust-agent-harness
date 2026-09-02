use std::{
    collections::{HashMap, HashSet},
    io::Write,
    sync::{Arc, Mutex},
};

use rah_protocol::{
    AgentErrorCode, AgentEvent, PermissionLevel, SessionId, ToolCall, ToolCallId, ToolContent,
    ToolDefinition, ToolInput, ToolName, ToolOutput,
};
use rah_tools::{ToolContext, ToolError, ToolRegistry};
use serde_json::{Value, json};
use tokio::{sync::mpsc, task::JoinHandle};

use crate::{
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

#[derive(Clone, Debug)]
pub(crate) struct ThreadToolSnapshot {
    pub(crate) dynamic_tools: Vec<Value>,
    pub(crate) by_alias: HashMap<String, ToolSnapshot>,
    pub(crate) by_name: HashMap<ToolName, String>,
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

pub(crate) fn snapshot_tools(registry: &ToolRegistry) -> ThreadToolSnapshot {
    let definitions = registry.definitions();
    let mut unavailable_aliases = definitions
        .iter()
        .filter(|definition| codex_name_is_usable(definition.name.as_str()))
        .map(|definition| definition.name.as_str().to_owned())
        .collect::<HashSet<_>>();
    let mut dynamic_tools = Vec::with_capacity(definitions.len());
    let mut by_alias = HashMap::with_capacity(definitions.len());
    let mut by_name = HashMap::with_capacity(definitions.len());

    for (index, definition) in definitions.into_iter().enumerate() {
        let alias = if codex_name_is_usable(definition.name.as_str()) {
            definition.name.as_str().to_owned()
        } else {
            private_alias(index, &mut unavailable_aliases)
        };
        let snapshot = ToolSnapshot { definition };
        dynamic_tools.push(dynamic_tool_spec(&alias, &snapshot));
        by_name.insert(snapshot.definition.name.clone(), alias.clone());
        if snapshot.definition.name == ToolName::new("repo.delete-file") {
            append_live_evidence(json!({
                "event": "tool_advertised",
                "public_tool": snapshot.definition.name.as_str(),
                "private_alias": alias.clone(),
                "dynamic_definition_emitted": true,
            }));
        }
        by_alias.insert(alias, snapshot);
    }

    ThreadToolSnapshot {
        dynamic_tools,
        by_alias,
        by_name,
    }
}

fn dynamic_tool_spec(alias: &str, snapshot: &ToolSnapshot) -> Value {
    let description = if alias == snapshot.definition.name.as_str() {
        snapshot.definition.description.clone()
    } else {
        format!(
            "RAH public tool `{}`. {}",
            snapshot.definition.name, snapshot.definition.description
        )
    };
    json!({
        "type": "function",
        "name": alias,
        "description": description,
        "inputSchema": snapshot.definition.input_schema,
        "deferLoading": false
    })
}

fn codex_name_is_usable(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && name != "mcp"
        && !name.starts_with("mcp__")
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

fn private_alias(index: usize, unavailable: &mut HashSet<String>) -> String {
    let mut suffix = index;
    loop {
        let alias = format!("rah_tool_{suffix}");
        if unavailable.insert(alias.clone()) {
            return alias;
        }
        suffix += 1;
    }
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
                let snapshot = record.bridge_tools.get(&params.tool).filter(|snapshot| {
                    record
                        .bridge_aliases
                        .get(&snapshot.definition.name)
                        .is_some_and(|alias| alias == &params.tool)
                });
                (session_id.clone(), snapshot.cloned())
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
    append_live_evidence(json!({
        "event": "tool_requested",
        "public_tool": call.name.as_str(),
        "private_alias": params.tool.clone(),
        "request": live_request_fields(&call.name, &call.input.0),
    }));

    let current = config
        .registry
        .get(&call.name)
        .map(|tool| tool.definition());
    let definition_matches_snapshot = current.as_ref().is_some_and(|definition| {
        definition.name == snapshot.definition.name
            && definition.description == snapshot.definition.description
            && definition.input_schema == snapshot.definition.input_schema
    });
    let permission_allowed = current
        .as_ref()
        .is_some_and(|definition| config.allowed_permissions.contains(&definition.permission));
    if !definition_matches_snapshot || !permission_allowed {
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
    append_live_evidence(json!({
        "event": "tool_started",
        "public_tool": call.name.as_str(),
    }));
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
            let live_error = output.is_error;
            let live_result = live_result_fields(&output);
            connection.publish_rah_event(
                completion.key.thread_id.clone(),
                completion.key.turn_id.clone(),
                AgentEvent::ToolFinished {
                    session_id: completion.session_id.clone(),
                    tool_call_id: completion.call.id,
                    output,
                },
            );
            append_live_evidence(json!({
                "event": "tool_finished",
                "public_tool": completion.call.name.as_str(),
                "is_error": live_error,
                "result": live_result,
            }));
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

fn live_request_fields(tool: &ToolName, input: &Value) -> Value {
    if tool.as_str() != "repo.delete-file" {
        return Value::Null;
    }
    json!({
        "path": input.get("path").cloned().unwrap_or(Value::Null),
        "expected_file_sha256": input
            .get("expected_file_sha256")
            .cloned()
            .unwrap_or(Value::Null),
        "expected_file_byte_length": input
            .get("expected_file_byte_length")
            .cloned()
            .unwrap_or(Value::Null),
    })
}

fn live_result_fields(output: &ToolOutput) -> Value {
    let [ToolContent::Text(text)] = output.content.as_slice() else {
        return Value::Null;
    };
    serde_json::from_str(text).unwrap_or(Value::Null)
}

fn append_live_evidence(record: Value) {
    let Some(path) = std::env::var_os("RAH_LIVE_EVIDENCE_PATH") else {
        return;
    };
    let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    else {
        return;
    };
    let Ok(line) = serde_json::to_string(&record) else {
        return;
    };
    let _ = writeln!(file, "{line}");
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

#[cfg(test)]
mod tests {
    use rah_protocol::ToolName;
    use serde_json::json;

    use super::live_request_fields;

    #[test]
    fn live_request_fields_records_repo_delete_file_schema_fields() {
        let input = json!({
            "path": "delete-target.txt",
            "expected_file_sha256": "a".repeat(64),
            "expected_file_byte_length": 26,
        });

        assert_eq!(
            live_request_fields(&ToolName::new("repo.delete-file"), &input),
            json!({
                "path": "delete-target.txt",
                "expected_file_sha256": "a".repeat(64),
                "expected_file_byte_length": 26,
            })
        );
    }
}
