#!/usr/bin/env python3
"""A RAH process plugin (protocol v2) written in Python, standard library only.

Tools:
  words.count           counts words, lines and characters of `text`.
  words.session_inputs  lists the user inputs of a session (default: the
                        calling one) by calling the host's `session` service,
                        which this plugin declares in `requires`.

Mount it with:

    [[plugin]]
    use = "process"
    id = "word-tools"
    config = { program = "python", args = ["examples/process-plugins/word_tools.py"] }

and assign `words.*` in `policy-authority`. Messages are newline-delimited
JSON-RPC 2.0 on stdin and stdout; stderr is for diagnostics only.
"""

import json
import sys
from collections import deque

PROTOCOL = 2
TOOLS = [
    {
        "name": "words.count",
        "description": "Counts the words, lines and characters of `text`.",
        "input_schema": {
            "type": "object",
            "properties": {"text": {"type": "string"}},
            "required": ["text"],
            "additionalProperties": False,
        },
        "permission": "none",
    },
    {
        "name": "words.session_inputs",
        "description": "Lists the user inputs of a session (default: the current one).",
        "input_schema": {
            "type": "object",
            "properties": {"session": {"type": "string"}},
            "additionalProperties": False,
        },
        "permission": "read",
    },
]


class InvalidInput(Exception):
    """Nothing was done: the host records no effect."""


class Plugin:
    def __init__(self):
        self.queued = deque()
        self.next_id = 0

    def send(self, message):
        message["jsonrpc"] = "2.0"
        sys.stdout.write(json.dumps(message) + "\n")
        sys.stdout.flush()

    def receive(self):
        if self.queued:
            return self.queued.popleft()
        line = sys.stdin.readline()
        return json.loads(line) if line else None

    def call_host(self, service, method, params):
        """Calls a host service; other messages that arrive meanwhile wait."""
        self.next_id += 1
        request_id = f"py-{self.next_id}"
        self.send({
            "id": request_id,
            "method": "service/call",
            "params": {"service": service, "method": method, "params": params},
        })
        while True:
            line = sys.stdin.readline()
            if not line:
                raise RuntimeError("the host closed the connection")
            message = json.loads(line)
            if message.get("id") == request_id and "method" not in message:
                if "error" in message:
                    raise RuntimeError(message["error"]["message"])
                return message["result"]
            self.queued.append(message)

    def call_tool(self, name, arguments, session):
        if name == "words.count":
            text = arguments.get("text")
            if not isinstance(text, str):
                raise InvalidInput("`text` must be a string")
            result = {"words": len(text.split()), "lines": len(text.splitlines()), "chars": len(text)}
            return {"content": [{"type": "json", "json": result}]}
        if name == "words.session_inputs":
            target = arguments.get("session", session)
            records = self.call_host("session", "read", {"session": target})
            inputs = [r["entry"]["input"] for r in records if r["entry"].get("kind") == "turn_started"]
            return {"content": [{"type": "json", "json": {"session": target, "inputs": inputs}}]}
        raise InvalidInput(f"unknown tool `{name}`")

    def run(self):
        while (message := self.receive()) is not None:
            method, request_id = message.get("method"), message.get("id")
            params = message.get("params") or {}
            if method == "initialize":
                print(f"word_tools: initialized as {params.get('plugin_id')}", file=sys.stderr)
                self.send({"id": request_id, "result": {
                    "protocol": PROTOCOL,
                    "version": "0.1.0",
                    "provides": ["tools"],
                    "requires": ["session"],
                    "tools": TOOLS,
                }})
            elif method == "tools/call":
                try:
                    result = self.call_tool(params["name"], params.get("arguments") or {}, params.get("session"))
                    self.send({"id": request_id, "result": result})
                except InvalidInput as error:
                    self.send({"id": request_id, "error": {
                        "code": -32602, "message": str(error), "data": {"kind": "invalid_input"}}})
                except Exception as error:  # Reported as an execution failure.
                    self.send({"id": request_id, "error": {"code": -32000, "message": str(error)}})
            elif method == "shutdown":
                self.send({"id": request_id, "result": {}})
                return
            elif request_id is not None and method is not None:
                self.send({"id": request_id, "error": {"code": -32601, "message": f"unknown method {method}"}})
            # Notifications such as `$/cancel` need no answer: calls run to completion here.


if __name__ == "__main__":
    Plugin().run()
