use std::{
    collections::HashSet,
    io::{self, BufRead, Write},
};

use serde_json::{Value, json};

const PROTOCOL_VERSION: &str = "2025-06-18";

fn main() {
    let stdin = io::stdin();
    let mut stdout = io::stdout().lock();
    let mut initialized = false;
    let mut lifecycle = Vec::new();
    let mut calls = 0_u64;
    let mut bridge_echo_calls = 0_u64;
    let mut cancellations = 0_u64;
    let mut pending = HashSet::new();

    for line in stdin.lock().lines() {
        let Ok(line) = line else {
            break;
        };
        let Ok(message) = serde_json::from_str::<Value>(&line) else {
            break;
        };
        let method = message["method"].as_str().unwrap_or_default();
        match method {
            "initialize" => {
                lifecycle.push("initialize");
                let id = message["id"].clone();
                if message["params"]["protocolVersion"] != PROTOCOL_VERSION {
                    write_error(&mut stdout, id, -32602, "unsupported protocol version");
                    continue;
                }
                write_result(
                    &mut stdout,
                    id,
                    json!({
                        "protocolVersion": PROTOCOL_VERSION,
                        "capabilities": {"tools": {"listChanged": false}},
                        "serverInfo": {"name": "untrusted-test-server", "version": "0.1.0"}
                    }),
                );
            }
            "notifications/initialized" => {
                lifecycle.push("initialized");
                initialized = true;
            }
            "tools/list" => {
                lifecycle.push("tools/list");
                let id = message["id"].clone();
                if !initialized {
                    write_error(&mut stdout, id, -32002, "server was not initialized");
                    continue;
                }
                write_result(
                    &mut stdout,
                    id,
                    json!({
                        "tools": [{
                            "name": "echo",
                            "description": "Returns the supplied text unchanged.",
                            "inputSchema": {
                                "type": "object",
                                "properties": {"text": {"type": "string"}},
                                "required": ["text"],
                                "additionalProperties": false
                            },
                            "annotations": {
                                "destructiveHint": true,
                                "readOnlyHint": false
                            }
                        }]
                    }),
                );
            }
            "tools/call" => {
                let id = message["id"].clone();
                if message["params"]["name"] != "echo" {
                    write_error(&mut stdout, id, -32602, "unknown tool");
                    continue;
                }
                let Some(text) = message["params"]["arguments"]["text"].as_str() else {
                    write_error(&mut stdout, id, -32602, "text must be a string");
                    continue;
                };
                match text {
                    "RAH_MCP_BRIDGE_OK" => {
                        bridge_echo_calls += 1;
                        write_result(
                            &mut stdout,
                            id,
                            json!({
                                "content": [{"type": "text", "text": text}],
                                "structuredContent": {
                                    "bridgeEchoCalls": bridge_echo_calls,
                                    "receivedArguments": message["params"]["arguments"]
                                },
                                "isError": false
                            }),
                        );
                    }
                    "__lifecycle__" => write_text(&mut stdout, id, &lifecycle.join(","), false),
                    "__structured__" => write_result(
                        &mut stdout,
                        id,
                        json!({
                            "content": [{"type": "text", "text": "structured"}],
                            "structuredContent": {"echo": "structured"},
                            "isError": false
                        }),
                    ),
                    "__tool_error__" => {
                        write_text(&mut stdout, id, "deterministic echo error", true)
                    }
                    "__malformed_result__" => write_result(
                        &mut stdout,
                        id,
                        json!({"content": [{"type": "text"}], "isError": false}),
                    ),
                    "__protocol_error__" => {
                        write_error(&mut stdout, id, -32603, "deterministic protocol error")
                    }
                    "__timeout__" | "__cancel__" => {
                        calls += 1;
                        if let Some(id) = id.as_u64() {
                            pending.insert(id);
                        }
                    }
                    "__counts__" => write_result(
                        &mut stdout,
                        id,
                        json!({
                            "content": [],
                            "structuredContent": {
                                "calls": calls,
                                "cancellations": cancellations
                            },
                            "isError": false
                        }),
                    ),
                    "__child_exit__" => std::process::exit(17),
                    "__disconnect__" => std::process::exit(0),
                    _ => write_text(&mut stdout, id, text, false),
                }
            }
            "notifications/cancelled" => {
                if message["params"]["requestId"]
                    .as_u64()
                    .is_some_and(|id| pending.remove(&id))
                {
                    cancellations += 1;
                }
            }
            _ => {
                if let Some(id) = message.get("id") {
                    write_error(&mut stdout, id.clone(), -32601, "method not found");
                }
            }
        }
    }
}

fn write_text(stdout: &mut impl Write, id: Value, text: &str, is_error: bool) {
    write_result(
        stdout,
        id,
        json!({
            "content": [{"type": "text", "text": text}],
            "isError": is_error
        }),
    );
}

fn write_result(stdout: &mut impl Write, id: Value, result: Value) {
    write_json(
        stdout,
        json!({"jsonrpc": "2.0", "id": id, "result": result}),
    );
}

fn write_error(stdout: &mut impl Write, id: Value, code: i64, message: &str) {
    write_json(
        stdout,
        json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}}),
    );
}

fn write_json(stdout: &mut impl Write, message: Value) {
    serde_json::to_writer(&mut *stdout, &message).expect("test response should serialize");
    stdout
        .write_all(b"\n")
        .expect("test response should be written");
    stdout.flush().expect("test response should be flushed");
}
