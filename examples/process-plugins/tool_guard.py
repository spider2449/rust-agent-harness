#!/usr/bin/env python3
"""A RAH process plugin (protocol v3) with hooks, standard library only.

It provides no tools. It attaches to two hooks:

  tool.before_invoke  denies a tool call whose arguments contain one of the
                      configured words. Otherwise it gives no decision, so
                      the policy (for example `policy-authority`) decides.
  tool.after_invoke   replaces the configured words in text and JSON output
                      with `[redacted]`.

and counts `session.appended` events, reporting them on stderr.

Mount it with:

    [[plugin]]
    use = "process"
    id = "tool-guard"
    config = { program = "python", args = ["examples/process-plugins/tool_guard.py"], plugin = { block = ["password"] } }

Its `tool.before_invoke` priority is 10, so it runs before
`policy-authority` (priority 0). It only denies; a hook that answered
`allow` at this priority would bypass the policy. If the plugin fails to
answer in time, the host denies the call.
"""

import json
import sys

PROTOCOL = 3


class Plugin:
    def __init__(self):
        self.block = []
        self.appended = 0

    def send(self, message):
        message["jsonrpc"] = "2.0"
        sys.stdout.write(json.dumps(message) + "\n")
        sys.stdout.flush()

    def before_invoke(self, request):
        arguments = json.dumps(request["call"]["input"]).lower()
        for word in self.block:
            if word.lower() in arguments:
                return {"decision": "deny", "reason": f"the arguments mention `{word}`"}
        return None

    def redact(self, value):
        """Returns `value` with the blocked words replaced in every string."""
        if isinstance(value, str):
            for word in self.block:
                value = value.replace(word, "[redacted]")
            return value
        if isinstance(value, list):
            return [self.redact(item) for item in value]
        if isinstance(value, dict):
            return {key: self.redact(item) for key, item in value.items()}
        return value

    def after_invoke(self, completion):
        content = []
        for item in completion["output"]["content"]:
            if item["type"] == "text":
                item = {**item, "text": self.redact(item["text"])}
            elif item["type"] == "json":
                item = {**item, "json": self.redact(item["json"])}
            content.append(item)  # Images pass unchanged.
        if content == completion["output"]["content"]:
            return None  # Keeps the output as it is.
        return {"output": {**completion["output"], "content": content}}

    def run(self):
        for line in sys.stdin:
            message = json.loads(line)
            method, request_id = message.get("method"), message.get("id")
            params = message.get("params") or {}
            if method == "initialize":
                config = params.get("config") or {}
                self.block = [str(word) for word in config.get("block", [])]
                print(f"tool_guard: blocking {self.block}", file=sys.stderr)
                self.send({"id": request_id, "result": {
                    "protocol": PROTOCOL,
                    "version": "0.1.0",
                    "hooks": [
                        {"hook": "tool.before_invoke", "priority": 10},
                        {"hook": "tool.after_invoke"},
                    ],
                    "events": ["session.appended"],
                }})
            elif method == "hooks/call":
                hook, data = params["hook"], params["input"]
                if hook == "tool.before_invoke":
                    result = self.before_invoke(data)
                elif hook == "tool.after_invoke":
                    result = self.after_invoke(data)
                else:
                    result = None
                self.send({"id": request_id, "result": result})
            elif method == "event":
                self.appended += 1
                print(f"tool_guard: {self.appended} session entries", file=sys.stderr)
            elif method == "shutdown":
                self.send({"id": request_id, "result": {}})
                return
            elif request_id is not None and method is not None:
                self.send({"id": request_id, "error": {"code": -32601, "message": f"unknown method {method}"}})


if __name__ == "__main__":
    Plugin().run()
