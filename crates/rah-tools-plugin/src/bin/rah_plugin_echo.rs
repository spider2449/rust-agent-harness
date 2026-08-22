use std::{
    collections::{BTreeMap, HashMap},
    env,
    io::{self, BufRead, Write},
    process,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use serde_json::{Map, Value, json};

const PROTOCOL_VERSION: &str = "1";

#[derive(Clone)]
struct PendingCall {
    request_id: u64,
    value: Value,
}

fn main() {
    let arguments = env::args().skip(1).collect::<Vec<_>>();
    let wrong_protocol = arguments.iter().any(|arg| arg == "--wrong-protocol");
    let wrong_id = arguments.iter().any(|arg| arg == "--wrong-id");
    let wrong_version = arguments.iter().any(|arg| arg == "--wrong-version");
    let contradictory_metadata = arguments
        .iter()
        .any(|arg| arg == "--contradictory-metadata");
    let live_text_audit = arguments.iter().any(|arg| arg == "--live-text-audit");
    let discovery_mode = arguments
        .iter()
        .find_map(|arg| arg.strip_prefix("--discovery="))
        .unwrap_or("echo");
    let stdout = Arc::new(Mutex::new(io::stdout()));
    let mut initialized = false;
    let mut lifecycle = Vec::new();
    let mut execution_count = 0_u64;
    let mut cancellation_count = 0_u64;
    let mut received_arguments = Vec::new();
    let mut call_counts = BTreeMap::<String, u64>::new();
    let mut pending = HashMap::<String, PendingCall>::new();

    for line in io::stdin().lock().lines() {
        let Ok(line) = line else { break };
        let Ok(message) = serde_json::from_str::<Value>(&line) else {
            break;
        };
        match message
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or_default()
        {
            "initialize" => {
                lifecycle.push("initialize");
                let id = message["id"].clone();
                let configured_id = message["params"]["configured_plugin_id"]
                    .as_str()
                    .unwrap_or_default();
                let offered = message["params"]["protocol_versions"]
                    .as_array()
                    .is_some_and(|versions| {
                        versions.iter().any(|version| version == PROTOCOL_VERSION)
                    });
                if !offered {
                    write_error(&stdout, id, -32602, "unsupported protocol version");
                    continue;
                }
                write_result(
                    &stdout,
                    id,
                    json!({
                        "protocol_version": if wrong_protocol { "2" } else { PROTOCOL_VERSION },
                        "plugin": {
                            "id": if wrong_id { "other" } else { configured_id },
                            "version": if wrong_version { "9.9.9" } else { "0.1.0" }
                        },
                        "capabilities": {"cancellation": true}
                    }),
                );
            }
            "initialized" => {
                lifecycle.push("initialized");
                initialized = true;
            }
            "tools/list" => {
                lifecycle.push("tools/list");
                let id = message["id"].clone();
                if discovery_mode == "hang" {
                    thread::sleep(Duration::from_secs(3));
                }
                if discovery_mode == "exit" {
                    process::exit(18);
                }
                if !initialized {
                    write_error(&stdout, id, -32002, "plugin was not initialized");
                    continue;
                }
                let metadata = if contradictory_metadata {
                    json!({"permission": "execute", "safe": false, "name": "read_file"})
                } else {
                    json!({"fixture": true})
                };
                let (description, input_schema) = if live_text_audit {
                    (
                        "Returns the supplied text.",
                        json!({
                            "type": "object",
                            "properties": {"text": {"type": "string"}},
                            "required": ["text"],
                            "additionalProperties": false
                        }),
                    )
                } else {
                    (
                        "Returns the supplied value.",
                        json!({
                            "type": "object",
                            "properties": {"value": {}},
                            "required": ["value"],
                            "additionalProperties": false
                        }),
                    )
                };
                let echo = json!({
                    "name": "echo",
                    "description": description,
                    "input_schema": input_schema,
                    "metadata": metadata
                });
                let tools = match discovery_mode {
                    "missing" => json!([]),
                    "extra" => json!([echo, {
                        "name": "extra", "description": "extra",
                        "input_schema": {"type": "object"}, "metadata": {}
                    }]),
                    "duplicate" => json!([echo.clone(), echo]),
                    "invalid" => json!([{
                        "name": "INVALID", "description": "invalid",
                        "input_schema": {"type": "object"}, "metadata": {}
                    }]),
                    "malformed" => Value::String("not a tool array".to_owned()),
                    "property-added" => json!([{ "name": "echo", "description": "echo",
                        "input_schema": {"type":"object","properties":{"value":{},"other":{}},"required":["value"],"additionalProperties":false}, "metadata": {} }]),
                    "property-removed" => json!([{ "name": "echo", "description": "echo",
                        "input_schema": {"type":"object","properties":{},"required":["value"],"additionalProperties":false}, "metadata": {} }]),
                    "type-drift" => json!([{ "name": "echo", "description": "echo",
                        "input_schema": {"type":"object","properties":{"value":{"type":"string"}},"required":["value"],"additionalProperties":false}, "metadata": {} }]),
                    "required-drift" => json!([{ "name": "echo", "description": "echo",
                        "input_schema": {"type":"object","properties":{"value":{}},"additionalProperties":false}, "metadata": {} }]),
                    "additional-properties-drift" => {
                        json!([{ "name": "echo", "description": "echo",
                        "input_schema": {"type":"object","properties":{"value":{}},"required":["value"],"additionalProperties":true}, "metadata": {} }])
                    }
                    "nested-drift" => json!([{ "name": "echo", "description": "echo",
                        "input_schema": {"type":"object","properties":{"value":{"type":"object","properties":{"nested":{"type":"number"}}}},"required":["value"],"additionalProperties":false}, "metadata": {} }]),
                    _ => json!([echo]),
                };
                write_result(&stdout, id, json!({"tools": tools}));
            }
            "tools/call" => {
                let id = message["id"].as_u64().unwrap_or_default();
                let execution_id = message["params"]["execution_id"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned();
                if message["params"]["name"] != "echo" {
                    write_error(&stdout, json!(id), -32602, "unknown tool");
                    continue;
                }
                let arguments = message["params"]["arguments"].clone();
                let input_name = if live_text_audit { "text" } else { "value" };
                let Some(value) = arguments.get(input_name).cloned() else {
                    write_error(
                        &stdout,
                        json!(id),
                        -32602,
                        &format!("{input_name} is required"),
                    );
                    continue;
                };
                let marker = value.as_str().unwrap_or_default().to_owned();
                if marker != "__audit__" && marker != "__lifecycle__" {
                    execution_count += 1;
                    received_arguments.push(arguments.clone());
                    *call_counts.entry(marker.clone()).or_default() += 1;
                }
                if live_text_audit {
                    eprintln!(
                        "RAH_PLUGIN_AUDIT {}",
                        json!({
                            "execution_count": execution_count,
                            "received_arguments": received_arguments,
                            "tools_call": message,
                            "cwd": env::current_dir().expect("fixture cwd").display().to_string(),
                            "environment": environment_map()
                        })
                    );
                    write_result(&stdout, json!(id), echo_result(value, false));
                    continue;
                }
                match marker.as_str() {
                    "__lifecycle__" => write_text(&stdout, id, &lifecycle.join(","), false),
                    "__audit__" => write_result(
                        &stdout,
                        json!(id),
                        json!({
                            "content": [{"type": "json", "value": {
                                "execution_count": execution_count,
                                "cancellation_count": cancellation_count,
                                "received_arguments": received_arguments,
                                "call_counts": call_counts,
                                "cwd": env::current_dir().expect("fixture cwd").display().to_string(),
                                "environment": environment_map()
                            }}],
                            "is_error": false
                        }),
                    ),
                    "__tool_error__" => write_text(&stdout, id, "deterministic plugin error", true),
                    "__remote_error__" => {
                        write_error(&stdout, json!(id), -32001, "untrusted remote detail")
                    }
                    "__malformed_result__" => write_result(
                        &stdout,
                        json!(id),
                        json!({"content": [{"type": "text"}], "is_error": false}),
                    ),
                    "__malformed_message__" => write_raw(&stdout, b"{malformed\n"),
                    "__oversized_message__" => {
                        let payload = vec![b'x'; 1024 * 1024 + 1];
                        write_raw(&stdout, &payload);
                    }
                    "__oversized_result__" => {
                        write_text(&stdout, id, &"x".repeat(1024 * 1024), false)
                    }
                    "__unknown_response__" => write_result(
                        &stdout,
                        json!(999_999_u64),
                        echo_result(json!("unknown"), false),
                    ),
                    "__duplicate_response__" => {
                        let result = echo_result(json!("duplicate"), false);
                        write_result(&stdout, json!(id), result.clone());
                        write_result(&stdout, json!(id), result);
                    }
                    "__timeout_late__" | "__cancel_late__" | "__hang__" => {
                        pending.insert(
                            execution_id,
                            PendingCall {
                                request_id: id,
                                value,
                            },
                        );
                    }
                    "__crash__" => process::exit(17),
                    "__disconnect__" => process::exit(0),
                    "__stderr_flood__" => {
                        let mut stderr = io::stderr().lock();
                        stderr
                            .write_all(&vec![b'd'; 128 * 1024])
                            .expect("fixture stderr flood");
                        stderr.flush().expect("fixture stderr flush");
                        write_text(&stdout, id, "stderr drained", false);
                    }
                    _ => write_result(&stdout, json!(id), echo_result(value, false)),
                }
            }
            "tools/cancel" => {
                let execution_id = message["params"]["execution_id"]
                    .as_str()
                    .unwrap_or_default();
                if let Some(call) = pending.remove(execution_id) {
                    cancellation_count += 1;
                    if call.value == "__timeout_late__" || call.value == "__cancel_late__" {
                        let stdout = Arc::clone(&stdout);
                        thread::spawn(move || {
                            thread::sleep(Duration::from_millis(10));
                            write_result(
                                &stdout,
                                json!(call.request_id),
                                echo_result(json!("late"), false),
                            );
                        });
                    }
                }
            }
            "shutdown" => {
                write_result(&stdout, message["id"].clone(), json!({}));
                break;
            }
            _ => {
                if let Some(id) = message.get("id") {
                    write_error(&stdout, id.clone(), -32601, "method not found");
                }
            }
        }
    }
}

fn echo_result(value: Value, is_error: bool) -> Value {
    let content = if value.is_string() {
        json!([{"type": "text", "value": value}])
    } else {
        json!([{"type": "json", "value": value}])
    };
    json!({"content": content, "is_error": is_error})
}

fn write_text(stdout: &Arc<Mutex<io::Stdout>>, id: u64, text: &str, is_error: bool) {
    write_result(stdout, json!(id), echo_result(json!(text), is_error));
}

fn write_result(stdout: &Arc<Mutex<io::Stdout>>, id: Value, result: Value) {
    write_json(
        stdout,
        json!({"jsonrpc": "2.0", "id": id, "result": result}),
    );
}

fn write_error(stdout: &Arc<Mutex<io::Stdout>>, id: Value, code: i64, message: &str) {
    write_json(
        stdout,
        json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}}),
    );
}

fn write_json(stdout: &Arc<Mutex<io::Stdout>>, message: Value) {
    let mut stdout = stdout.lock().expect("fixture stdout lock");
    serde_json::to_writer(&mut *stdout, &message).expect("fixture response serialization");
    stdout.write_all(b"\n").expect("fixture response write");
    stdout.flush().expect("fixture response flush");
}

fn write_raw(stdout: &Arc<Mutex<io::Stdout>>, bytes: &[u8]) {
    let mut stdout = stdout.lock().expect("fixture stdout lock");
    stdout.write_all(bytes).expect("fixture raw write");
    stdout.flush().expect("fixture raw flush");
}

fn environment_map() -> Value {
    let entries = env::vars()
        .map(|(name, value)| (name, Value::String(value)))
        .collect::<Map<_, _>>();
    Value::Object(entries)
}
