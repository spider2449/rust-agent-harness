//! A process plugin used by the host's tests (protocol v3).
//!
//! Arguments: `--requires <service>`, `--hook <name>[:<priority>]` and
//! `--event <name>` (all repeatable), and `--mode <mode>`, where mode is
//! `normal`, `v2` (answers protocol 2), `old-protocol` or `provides-model`.
//! With `initialize.config.flip_file`, the first start creates that file and
//! the next start reports a different manifest.
//!
//! Hooks act on `text` markers: `tool.before_invoke` decides by the call's
//! `text` argument (`allow`, `deny`, `ask`, `malformed`, `hang`),
//! `tool.after_invoke` rewrites `fixture.echo` output (`hang-after` does not
//! answer) and `model.before_request` adds a system item unless the last
//! user text is `hang` or `bad-image`.

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
    events: Vec<Value>,
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
    let hooks: Vec<Value> = flag("--hook")
        .iter()
        .map(|spec| match spec.split_once(':') {
            Some((hook, priority)) => {
                json!({ "hook": hook, "priority": priority.parse::<i32>().unwrap() })
            }
            None => json!({ "hook": spec }),
        })
        .collect();
    let events = flag("--event");
    eprintln!("fixture started in mode {mode}");
    let mut plugin = Plugin {
        lines: std::io::stdin().lock().lines(),
        queued: VecDeque::new(),
        next: 0,
        cancelled: Vec::new(),
        events: Vec::new(),
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
                let protocol = match mode.as_str() {
                    "old-protocol" => 1,
                    "v2" => 2,
                    _ => 3,
                };
                json!({
                    "protocol": protocol,
                    "version": "0.1.0",
                    "provides": [provides],
                    "requires": requires,
                    "hooks": hooks,
                    "events": events,
                    "tools": [
                        tool("fixture.echo", "none"),
                        tool("fixture.call", "read"),
                        tool("fixture.invalid", "write"),
                        tool("fixture.slow", "write"),
                        tool("fixture.cancelled", "none"),
                        tool("fixture.exit", "execute"),
                        tool("fixture.events", "none"),
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
                    "fixture.events" => json!({
                        "content": [{ "type": "json", "json": plugin.events }]
                    }),
                    _ => std::process::exit(3),
                }
            }
            Some("hooks/call") => match hook(&params) {
                Some(result) => result,
                None => continue,
            },
            Some("event") => {
                plugin.events.push(params);
                continue;
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

/// Answers `hooks/call`; `None` leaves the request unanswered.
fn hook(params: &Value) -> Option<Value> {
    let input = &params["input"];
    match params["hook"].as_str().unwrap() {
        "tool.before_invoke" => match input["call"]["input"]["text"].as_str() {
            Some("allow") => Some(json!({ "decision": "allow" })),
            Some("deny") => Some(json!({ "decision": "deny", "reason": "fixture says no" })),
            Some("ask") => Some(json!({ "decision": "ask", "reason": "fixture asks" })),
            Some("malformed") => Some(json!({ "decision": "maybe" })),
            Some("hang") => None,
            _ => Some(Value::Null),
        },
        "tool.after_invoke" if input["call"]["name"] == "fixture.echo" => {
            let text = input["output"]["content"][0]["text"].as_str().unwrap();
            if text == "hang-after" {
                return None;
            }
            let filtered = format!("filtered {text} ({})", input["effect"].as_str().unwrap());
            Some(json!({ "output": { "content": [{ "type": "text", "text": filtered }] } }))
        }
        "tool.after_invoke" => Some(Value::Null),
        "model.before_request" => {
            let mut request = input.clone();
            let last = request["items"]
                .as_array()
                .unwrap()
                .iter()
                .rev()
                .find(|item| item["type"] == "user")
                .and_then(|item| item["text"].as_str())
                .map(str::to_owned);
            let added = match last.as_deref() {
                Some("hang") => return None,
                Some("bad-image") => json!({
                    "type": "tool_result",
                    "call_id": "c1",
                    "output": {
                        "content": [{ "type": "image", "value": { "media_type": "image/svg+xml", "data": "AA==" } }],
                        "is_error": false
                    }
                }),
                _ => json!({ "type": "system", "text": "from fixture" }),
            };
            request["items"].as_array_mut().unwrap().push(added);
            Some(json!({ "request": request }))
        }
        _ => Some(Value::Null),
    }
}
