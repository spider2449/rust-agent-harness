//! A process plugin used by the host's tests (protocol v2).
//!
//! Arguments: `--requires <service>` (repeatable) and `--mode <mode>`, where
//! mode is `normal`, `old-protocol` or `provides-model`. With
//! `initialize.config.flip_file`, the first start creates that file and the
//! next start reports a different manifest.

use std::{
    collections::VecDeque,
    io::{BufRead, Write},
};

use serde_json::{Value, json};

struct Plugin {
    lines: std::io::Lines<std::io::StdinLock<'static>>,
    queued: VecDeque<Value>,
    next: u64,
    cancelled: Vec<Value>,
}

fn send(message: &Value) {
    let mut out = std::io::stdout().lock();
    writeln!(out, "{message}").unwrap();
    out.flush().unwrap();
}

fn tool(name: &str, permission: &str) -> Value {
    json!({
        "name": name,
        "description": format!("fixture {name}"),
        "input_schema": { "type": "object" },
        "permission": permission
    })
}

impl Plugin {
    fn next_message(&mut self) -> Option<Value> {
        if let Some(message) = self.queued.pop_front() {
            return Some(message);
        }
        let line = self.lines.next()?.ok()?;
        Some(serde_json::from_str(&line).expect("the host sends JSON"))
    }

    /// Calls a host service and waits for its answer, queueing other
    /// messages.
    fn call_host(&mut self, service: &Value, method: &Value, params: Value) -> Value {
        self.next += 1;
        let id = format!("fixture-{}", self.next);
        send(&json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "service/call",
            "params": { "service": service, "method": method, "params": params }
        }));
        loop {
            let message = self.lines.next().unwrap().unwrap();
            let message: Value = serde_json::from_str(&message).unwrap();
            if message.get("id") == Some(&json!(id)) && message.get("method").is_none() {
                return message;
            }
            self.queued.push_back(message);
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let flag = |name: &str| {
        args.windows(2)
            .filter(|pair| pair[0] == name)
            .map(|pair| pair[1].clone())
            .collect::<Vec<_>>()
    };
    let mode = flag("--mode").pop().unwrap_or_else(|| "normal".into());
    let mut requires = flag("--requires");
    eprintln!("fixture started in mode {mode}");
    let mut plugin = Plugin {
        lines: std::io::stdin().lock().lines(),
        queued: VecDeque::new(),
        next: 0,
        cancelled: Vec::new(),
    };
    while let Some(message) = plugin.next_message() {
        let id = message.get("id").cloned();
        let params = message.get("params").cloned().unwrap_or(Value::Null);
        let result = match message.get("method").and_then(Value::as_str) {
            Some("initialize") => {
                if let Some(flip) = params["config"]["flip_file"].as_str() {
                    if std::path::Path::new(flip).exists() {
                        requires = vec!["session".into()];
                    } else {
                        std::fs::write(flip, "started").unwrap();
                    }
                }
                let provides = if mode == "provides-model" {
                    "model"
                } else {
                    "tools"
                };
                json!({
                    "protocol": if mode == "old-protocol" { 1 } else { 2 },
                    "version": "0.1.0",
                    "provides": [provides],
                    "requires": requires,
                    "tools": [
                        tool("fixture.echo", "none"),
                        tool("fixture.call", "read"),
                        tool("fixture.invalid", "write"),
                        tool("fixture.slow", "write"),
                        tool("fixture.cancelled", "none"),
                        tool("fixture.exit", "execute"),
                    ]
                })
            }
            Some("tools/call") => {
                let arguments = &params["arguments"];
                match params["name"].as_str().unwrap() {
                    "fixture.echo" => json!({
                        "content": [
                            { "type": "text", "text": arguments["text"] },
                            { "type": "json", "json": { "session": params["session"] } }
                        ]
                    }),
                    "fixture.call" => {
                        let answer = plugin.call_host(
                            &arguments["service"],
                            &arguments["method"],
                            arguments["params"].clone(),
                        );
                        json!({ "content": [{ "type": "json", "json": answer }] })
                    }
                    "fixture.invalid" => {
                        send(&json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "error": { "code": -32602, "message": "bad text", "data": { "kind": "invalid_input" } }
                        }));
                        continue;
                    }
                    "fixture.slow" => continue,
                    "fixture.cancelled" => json!({
                        "content": [{ "type": "json", "json": plugin.cancelled }]
                    }),
                    _ => std::process::exit(3),
                }
            }
            Some("$/cancel") => {
                plugin.cancelled.push(params["id"].clone());
                continue;
            }
            Some("shutdown") => {
                send(&json!({ "jsonrpc": "2.0", "id": id, "result": {} }));
                return;
            }
            _ => continue,
        };
        send(&json!({ "jsonrpc": "2.0", "id": id, "result": result }));
    }
}
