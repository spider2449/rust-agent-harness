#!/usr/bin/env python3
"""A RAH process plugin (protocol v4) written in Python, standard library only.

It adds a view to the Web UI (ADR 0063): a to-do board with a form that adds
an item and a button that clears the list. Items are kept in the host's
`storage` service, which this plugin declares in `requires`, so they survive
a restart. After every change it sends `ui/changed`, so every window showing
the board renders it again.

Mount it with:

    [[plugin]]
    use = "process"
    id = "todo"
    config = { program = "python", args = ["examples/process-plugins/todo_board.py"] }

The board is the panel `todo/board`. It is not a tool: the model cannot see
or change it. Messages are newline-delimited JSON-RPC 2.0 on stdin and
stdout; stderr is for diagnostics only.
"""

import json
import sys
from collections import deque

PROTOCOL = 4
VIEW = "board"
KEY = "items"
MAX_ITEMS = 100
MAX_ITEM_CHARS = 200


class InvalidInput(Exception):
    """The user's input was refused; nothing changed."""


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

    def items(self):
        stored = self.call_host("storage", "get", {"key": KEY})
        value = stored.get("value") if stored else None
        return value if isinstance(value, list) else []

    def save(self, items):
        self.call_host("storage", "put", {"key": KEY, "value": items})
        # Every window showing the board renders it again.
        self.send({"method": "ui/changed", "params": {"view": VIEW}})

    def render(self):
        items = self.items()
        nodes = [{"type": "text", "text": f"{len(items)} item(s)."}]
        if items:
            nodes.append({"type": "list", "items": items})
        nodes.append({
            "type": "form",
            "action": "add",
            "submit": "Add",
            "fields": [{"name": "item", "label": "New item"}],
        })
        if items:
            nodes.append({"type": "button", "action": "clear", "label": "Clear"})
        return {"nodes": nodes}

    def act(self, action, values):
        items = self.items()
        if action == "add":
            item = (values or {}).get("item") if isinstance(values, dict) else None
            if not isinstance(item, str) or not item.strip():
                raise InvalidInput("type an item first")
            if len(item) > MAX_ITEM_CHARS or len(items) >= MAX_ITEMS:
                raise InvalidInput(f"at most {MAX_ITEMS} items of {MAX_ITEM_CHARS} characters")
            self.save(items + [item.strip()])
            return {"message": f"Added “{item.strip()}”.", "rerender": True}
        if action == "clear":
            self.save([])
            return {"message": "Cleared.", "rerender": True}
        raise LookupError(action)

    def run(self):
        while (message := self.receive()) is not None:
            method, request_id = message.get("method"), message.get("id")
            params = message.get("params") or {}
            if method == "initialize":
                print(f"todo_board: initialized as {params.get('plugin_id')}", file=sys.stderr)
                self.send({"id": request_id, "result": {
                    "protocol": PROTOCOL,
                    "version": "0.1.0",
                    "provides": ["ui-views"],
                    "requires": ["storage"],
                    "views": [{"id": VIEW, "title": "To-do"}],
                }})
            elif method in ("views/render", "views/act"):
                if params.get("view") != VIEW:
                    self.send({"id": request_id, "error": {
                        "code": -32602, "message": "no such view", "data": {"kind": "unknown_view"}}})
                    continue
                try:
                    if method == "views/render":
                        result = self.render()
                    else:
                        result = self.act(params.get("action"), params.get("input"))
                    self.send({"id": request_id, "result": result})
                except InvalidInput as error:
                    self.send({"id": request_id, "error": {
                        "code": -32602, "message": str(error), "data": {"kind": "invalid_input"}}})
                except LookupError as error:
                    self.send({"id": request_id, "error": {
                        "code": -32602, "message": f"no action {error}", "data": {"kind": "unknown_action"}}})
                except Exception as error:  # Shown to the user as a failure.
                    self.send({"id": request_id, "error": {"code": -32000, "message": str(error)}})
            elif method == "shutdown":
                self.send({"id": request_id, "result": {}})
                return
            elif request_id is not None and method is not None:
                self.send({"id": request_id, "error": {"code": -32601, "message": f"unknown method {method}"}})
            # Notifications such as `$/cancel` need no answer: calls run to completion here.


if __name__ == "__main__":
    Plugin().run()
