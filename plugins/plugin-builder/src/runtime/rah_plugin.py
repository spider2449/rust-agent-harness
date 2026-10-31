"""RAH process plugin runtime (protocol 4), standard library only.

Written by plugin-builder next to `plugin.py`; it is replaced on every save,
so do not edit it. It speaks newline-delimited JSON-RPC 2.0 on stdin and
stdout, answers `initialize` with `manifest.json` from this directory and
sends each request to the handler registered for it:

    from rah_plugin import Plugin, InvalidInput

    plugin = Plugin(__file__)

    @plugin.tool("my-plugin.greet")
    def greet(args, ctx):
        name = args.get("name")
        if not isinstance(name, str):
            raise InvalidInput("`name` must be a string")
        return f"Hello, {name}!"

    plugin.run()

Handlers receive a `Context` with `session`, `config` (the mount entry's
`plugin` value), `call_host(service, method, params)` and
`view_changed(view)`. Diagnostics go to stderr; stdout is the protocol.
"""

import json
import os
import sys
from collections import deque

PROTOCOL = 4


class InvalidInput(Exception):
    """The input was refused and nothing was done: the host records no effect."""


class HostError(Exception):
    """A `service/call` failed; `code` is the JSON-RPC error code."""

    def __init__(self, code, message):
        super().__init__(message)
        self.code = code


class _Refused(Exception):
    def __init__(self, kind, message):
        super().__init__(message)
        self.kind = kind


class Context:
    def __init__(self, plugin, session):
        self._plugin = plugin
        self.session = session
        self.config = plugin.config

    def call_host(self, service, method, params=None):
        """Calls a host service the manifest `requires`; returns its result."""
        return self._plugin._call_host(service, method, params or {})

    def view_changed(self, view):
        """Asks every window showing `view` to render it again."""
        self._plugin._send({"method": "ui/changed", "params": {"view": view}})


class Plugin:
    def __init__(self, entry_file):
        directory = os.path.dirname(os.path.abspath(entry_file))
        with open(os.path.join(directory, "manifest.json"), encoding="utf-8") as file:
            self.manifest = json.load(file)
        self.config = None
        self._tools = {}
        self._hooks = {}
        self._events = {}
        self._views = {}
        self._actions = {}
        self._queued = deque()
        self._next_id = 0

    # Registration -------------------------------------------------------

    def tool(self, name):
        """Registers `handler(args, ctx)` for a tool of the manifest."""
        return self._register(self._tools, name)

    def hook(self, name):
        """Registers `handler(input, ctx)` for a hook of the manifest."""
        return self._register(self._hooks, name)

    def event(self, name):
        """Registers `handler(data, ctx)` for an event of the manifest."""
        return self._register(self._events, name)

    def view(self, view_id):
        """Registers `render(ctx)` for a view; it returns a list of nodes."""
        return self._register(self._views, view_id)

    def action(self, view_id, action):
        """Registers `handler(input, ctx)` for a button or form of a view."""
        return self._register(self._actions, (view_id, action))

    def _register(self, table, key):
        def decorate(handler):
            table[key] = handler
            return handler

        return decorate

    # Transport ----------------------------------------------------------

    def _send(self, message):
        message["jsonrpc"] = "2.0"
        sys.stdout.write(json.dumps(message) + "\n")
        sys.stdout.flush()

    def _receive(self):
        if self._queued:
            return self._queued.popleft()
        line = sys.stdin.readline()
        return json.loads(line) if line else None

    def _call_host(self, service, method, params):
        self._next_id += 1
        request_id = f"rah-plugin-{self._next_id}"
        self._send({
            "id": request_id,
            "method": "service/call",
            "params": {"service": service, "method": method, "params": params},
        })
        while True:
            line = sys.stdin.readline()
            if not line:
                raise HostError(-32000, "the host closed the connection")
            message = json.loads(line)
            if message.get("id") == request_id and "method" not in message:
                if "error" in message:
                    error = message["error"]
                    raise HostError(error.get("code", -32000), error.get("message", ""))
                return message.get("result")
            self._queued.append(message)

    # Dispatch -----------------------------------------------------------

    def _check(self):
        declared = {
            "tool": {tool["name"] for tool in self.manifest.get("tools", [])},
            "hook": {hook["hook"] for hook in self.manifest.get("hooks", [])},
            "event": set(self.manifest.get("events", [])),
            "view": {view["id"] for view in self.manifest.get("views", [])},
        }
        handled = {
            "tool": set(self._tools),
            "hook": set(self._hooks),
            "event": set(self._events),
            "view": set(self._views),
        }
        problems = []
        for kind in declared:
            for name in sorted(declared[kind] - handled[kind]):
                problems.append(f"{kind} `{name}` is in manifest.json but has no handler")
            for name in sorted(handled[kind] - declared[kind]):
                problems.append(f"{kind} `{name}` has a handler but is not in manifest.json")
        for view_id, action in self._actions:
            if view_id not in declared["view"]:
                problems.append(f"action `{action}` belongs to unknown view `{view_id}`")
        if problems:
            for problem in problems:
                print(f"rah_plugin: {problem}", file=sys.stderr)
            sys.exit(1)

    @staticmethod
    def _tool_result(value):
        if isinstance(value, dict) and isinstance(value.get("content"), list):
            return value
        if isinstance(value, str):
            return {"content": [{"type": "text", "text": value}]}
        return {"content": [{"type": "json", "json": value}]}

    def _handle(self, method, params):
        session = params.get("session")
        if method == "tools/call":
            handler = self._tools.get(params.get("name"))
            if handler is None:
                raise InvalidInput(f"unknown tool `{params.get('name')}`")
            arguments = params.get("arguments")
            if arguments is None:
                arguments = {}
            return self._tool_result(handler(arguments, Context(self, session)))
        if method == "hooks/call":
            handler = self._hooks.get(params.get("hook"))
            if handler is None:
                return None
            return handler(params.get("input"), Context(self, None))
        if method == "views/render":
            render = self._views.get(params.get("view"))
            if render is None:
                raise _Refused("unknown_view", f"unknown view `{params.get('view')}`")
            nodes = render(Context(self, session))
            return nodes if isinstance(nodes, dict) else {"nodes": nodes or []}
        if method == "views/act":
            handler = self._actions.get((params.get("view"), params.get("action")))
            if handler is None:
                raise _Refused("unknown_action", f"unknown action `{params.get('action')}`")
            return handler(params.get("input"), Context(self, session)) or {}
        raise _Refused(None, f"unknown method `{method}`")

    def run(self):
        """Serves the host until it shuts the plugin down."""
        self._check()
        while (message := self._receive()) is not None:
            method, request_id = message.get("method"), message.get("id")
            params = message.get("params") or {}
            if method == "initialize":
                self.config = params.get("config")
                manifest = dict(self.manifest)
                manifest["protocol"] = PROTOCOL
                self._send({"id": request_id, "result": manifest})
            elif method == "shutdown":
                self._send({"id": request_id, "result": {}})
                return
            elif method == "event":
                handler = self._events.get(params.get("event"))
                if handler is not None:
                    try:
                        handler(params.get("data"), Context(self, None))
                    except Exception as error:  # Events have no answer.
                        print(f"rah_plugin: event handler failed: {error}", file=sys.stderr)
            elif request_id is None:
                continue  # Notifications such as `$/cancel`: calls run to completion here.
            else:
                try:
                    self._send({"id": request_id, "result": self._handle(method, params)})
                except InvalidInput as error:
                    self._send({"id": request_id, "error": {
                        "code": -32602, "message": str(error), "data": {"kind": "invalid_input"}}})
                except _Refused as error:
                    fault = {"code": -32601, "message": str(error)}
                    if error.kind:
                        fault["data"] = {"kind": error.kind}
                    self._send({"id": request_id, "error": fault})
                except Exception as error:  # Reported as an execution failure.
                    self._send({"id": request_id, "error": {"code": -32000, "message": str(error)}})
