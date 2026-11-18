"""Local Rust LSP queries over MCP stdio. Python standard library only.

Fixed rust-analyzer executable and workspace, no source edits or command tool.
Cargo runs offline; build scripts and proc macros are disabled. Index/cache
writes may still occur. This is process supervision, not OS isolation.
"""
import argparse
import json
import os
from pathlib import Path
import queue
import subprocess
import sys
import threading
import time
from urllib.parse import unquote

LIMIT = 16 * 1024 * 1024
METHODS = {"hover": "textDocument/hover", "definition": "textDocument/definition",
           "references": "textDocument/references", "symbols": "textDocument/documentSymbol"}


def uri_key(uri):
    value = unquote(uri)
    return value.lower() if os.name == "nt" else value


class Lsp:
    def __init__(self, root, executable, cancelled=None):
        self.root = root.resolve(strict=True)
        self.events = queue.Queue(maxsize=1024)
        self.stop = threading.Event()
        self.pending = {}
        self.busy = set()
        self.diagnostics = {}
        self.versions = {}
        self.texts = {}
        self.sequence = 0
        self.quiescent = False
        self.cancelled = cancelled or (lambda: False)
        env = {**os.environ, "CARGO_NET_OFFLINE": "true"}
        self.process = subprocess.Popen([str(executable.resolve(strict=True))], cwd=self.root,
                                        env=env, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                        stderr=subprocess.DEVNULL)
        self.reader = threading.Thread(target=self.read)
        self.reader.start()
        try:
            self.request("initialize", {"processId": os.getpid(), "rootUri": self.root.as_uri(),
                "workspaceFolders": [{"uri": self.root.as_uri(), "name": "workspace"}],
                "capabilities": {"window": {"workDoneProgress": True},
                    "experimental": {"serverStatusNotification": True},
                    "workspace": {"configuration": True},
                    "textDocument": {"synchronization": {"didSave": True}}},
                "initializationOptions": {"cargo": {"buildScripts": {"enable": False}},
                                           "procMacro": {"enable": False}}}, time.monotonic() + 30)
            self.send("initialized", {})
        except BaseException:
            self.close()
            raise

    def enqueue(self, event):
        while not self.stop.is_set():
            try:
                self.events.put(event, timeout=.1)
                return
            except queue.Full:
                pass

    def read(self):
        try:
            while not self.stop.is_set():
                headers = {}
                count = 0
                while True:
                    line = self.process.stdout.readline(8193)
                    count += len(line)
                    if not line:
                        raise EOFError("rust-analyzer disconnected")
                    if count > 8192:
                        raise ValueError("LSP headers too large")
                    if line == b"\r\n":
                        break
                    key, value = line.decode("ascii").split(":", 1)
                    headers[key.lower()] = value.strip()
                size = int(headers["content-length"])
                if not 0 < size <= LIMIT:
                    raise ValueError("LSP message too large")
                data = self.process.stdout.read(size)
                if len(data) != size:
                    raise EOFError("incomplete LSP frame")
                self.enqueue(json.loads(data))
        except Exception as error:
            self.enqueue(error)

    def write(self, message):
        data = json.dumps(message).encode("utf-8")
        if len(data) > LIMIT:
            raise ValueError("LSP request too large")
        self.process.stdin.write(f"Content-Length: {len(data)}\r\n\r\n".encode() + data)
        self.process.stdin.flush()

    def send(self, method, params):
        self.write({"jsonrpc": "2.0", "method": method, "params": params})

    def pump(self, timeout):
        deadline = time.monotonic() + max(0, timeout)
        while True:
            if self.cancelled():
                raise InterruptedError("LSP query cancelled; result unknown")
            try:
                message = self.events.get(timeout=min(.1, max(0, deadline - time.monotonic())))
                break
            except queue.Empty:
                if time.monotonic() >= deadline:
                    raise
        if isinstance(message, Exception):
            raise message
        method, params = message.get("method"), message.get("params", {})
        if method and "id" in message:
            result = None
            if method == "workspace/configuration":
                result = [None for _ in params.get("items", [])]
            elif method == "workspace/workspaceFolders":
                result = [{"uri": self.root.as_uri(), "name": "workspace"}]
            allowed = {"workspace/configuration", "workspace/workspaceFolders",
                       "window/workDoneProgress/create", "client/registerCapability",
                       "client/unregisterCapability", "workspace/diagnostic/refresh"}
            reply = {"result": result} if method in allowed else {"error": {
                "code": -32601, "message": "Unsupported server request"}}
            self.write({"jsonrpc": "2.0", "id": message["id"], **reply})
        elif method == "experimental/serverStatus":
            self.quiescent = params.get("quiescent") is True
        elif method == "$/progress":
            token, kind = params.get("token"), params.get("value", {}).get("kind")
            if kind == "begin":
                self.busy.add(token)
            elif kind == "end":
                self.busy.discard(token)
        elif method == "textDocument/publishDiagnostics":
            key = uri_key(params["uri"])
            if key in self.versions and (params.get("version") is None or
                                        params["version"] >= self.versions[key]):
                self.diagnostics[key] = params.get("diagnostics", [])
        elif not method and "id" in message:
            self.pending[message["id"]] = message

    def request(self, method, params, deadline):
        self.sequence += 1
        ident = self.sequence
        self.write({"jsonrpc": "2.0", "id": ident, "method": method, "params": params})
        while ident not in self.pending:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise TimeoutError(f"LSP request {method} timed out; result unknown")
            self.pump(remaining)
        response = self.pending.pop(ident)
        if "error" in response:
            raise RuntimeError(str(response["error"]))
        return response.get("result")

    def query(self, arguments):
        path = (self.root / arguments["path"]).resolve(strict=True)
        if not path.is_relative_to(self.root) or path.suffix.lower() != ".rs":
            raise ValueError("path must be a Rust file within this workspace")
        if path.stat().st_size > 1024 * 1024:
            raise ValueError("file exceeds 1 MiB")
        operation = arguments["operation"]
        if operation not in {*METHODS, "diagnostics"}:
            raise ValueError("unknown operation")
        position = {}
        if operation in {"hover", "definition", "references"}:
            for key in ("line", "character"):
                value = arguments.get(key)
                if type(value) is not int or value < 0:
                    raise ValueError(f"{key} must be a zero-based nonnegative integer (UTF-16)")
                position[key] = value
        text = path.read_text(encoding="utf-8")
        uri, key = path.as_uri(), uri_key(path.as_uri())
        deadline = time.monotonic() + 45
        # Drain queued publications before starting a new document version.
        while True:
            if time.monotonic() >= deadline:
                raise TimeoutError("LSP event drain timed out; result unknown")
            try:
                self.pump(0)
            except queue.Empty:
                break
        if self.texts.get(key) != text or operation == "diagnostics":
            version = self.versions.get(key, 0) + 1
            self.versions[key] = version
            self.texts[key] = text
            self.diagnostics.pop(key, None)
            document = {"uri": uri, "version": version}
            if version == 1:
                self.send("textDocument/didOpen", {"textDocument": {
                    **document, "languageId": "rust", "text": text}})
            else:
                self.send("textDocument/didChange", {"textDocument": document,
                                                      "contentChanges": [{"text": text}]})
        # Navigation has its own request/response lifecycle. A server need not
        # publish diagnostics for unchanged text, and background cargo checks
        # do not gate hover, definitions, references or document symbols.
        if operation != "diagnostics":
            # Cold-start syntax queries can work before Cargo metadata is loaded,
            # while semantic queries return null. Wait for analyzer readiness,
            # not for diagnostics (which may never be republished).
            while not self.quiescent:
                remaining = deadline - time.monotonic()
                if remaining <= 0:
                    raise TimeoutError("Rust analyzer not ready; result unknown")
                try:
                    self.pump(remaining)
                except queue.Empty as error:
                    raise TimeoutError("Rust analyzer not ready; result unknown") from error
            params = {"textDocument": {"uri": uri}}
            if position:
                params["position"] = position
            if operation == "references":
                params["context"] = {"includeDeclaration": True}
            return self.request(METHODS[operation], params, deadline)
        self.send("textDocument/didSave", {"textDocument": {"uri": uri}})
        while True:
            ready = key in self.diagnostics and not self.busy
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise TimeoutError("LSP not settled; result unknown")
            try:
                self.pump(min(.3, remaining) if ready else remaining)
            except queue.Empty:
                if ready and remaining >= .3:
                    break
                raise TimeoutError("LSP not settled; result unknown")
        return {"diagnostics": self.diagnostics[key], "scope": "latest file publication, not compilation proof"}

    def close(self):
        self.stop.set()
        if self.process.poll() is None:
            if os.name == "nt":
                subprocess.run([str(Path(os.environ["SystemRoot"]) / "System32/taskkill.exe"),
                                "/PID", str(self.process.pid), "/T", "/F"],
                               stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=10, check=False)
            if self.process.poll() is None:
                self.process.kill()
        self.process.wait(timeout=10)
        self.reader.join(timeout=10)
        self.process.stdin.close()
        self.process.stdout.close()


TOOL = {"name": "rust_lsp", "description": "Query local Rust diagnostics, hover, definitions, references or symbols. Paths are relative to the configured workspace; positions are zero-based UTF-16. No source edits. Cargo offline, build scripts/proc macros disabled; cache writes possible.",
        "inputSchema": {"type": "object", "properties": {
            "operation": {"type": "string", "enum": ["diagnostics", *METHODS]},
            "path": {"type": "string"}, "line": {"type": "integer", "minimum": 0},
            "character": {"type": "integer", "minimum": 0}},
            "required": ["operation", "path"], "additionalProperties": False}}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, required=True)
    parser.add_argument("--server", type=Path, required=True)
    args = parser.parse_args()
    client = None
    inbox = queue.Queue(maxsize=128)
    disconnected = threading.Event()
    cancelled = set()
    active = [None]

    def receive():
        try:
            while line := sys.stdin.buffer.readline(LIMIT + 1):
                if len(line) > LIMIT:
                    raise ValueError("MCP request too large")
                message = json.loads(line)
                if message.get("method") == "notifications/cancelled":
                    cancelled.add(message.get("params", {}).get("requestId"))
                else:
                    while not disconnected.is_set():
                        try:
                            inbox.put(message, timeout=.1)
                            break
                        except queue.Full:
                            pass
        finally:
            disconnected.set()

    input_reader = threading.Thread(target=receive, daemon=True)
    input_reader.start()
    try:
        while not disconnected.is_set() or not inbox.empty():
            try:
                message = inbox.get(timeout=.1)
            except queue.Empty:
                continue
            if "id" not in message:
                continue
            active[0] = message["id"]
            method, params = message.get("method"), message.get("params", {})
            reply = {"jsonrpc": "2.0", "id": message["id"]}
            if method == "initialize":
                reply["result"] = {"protocolVersion": "2024-11-05", "capabilities": {"tools": {}},
                                   "serverInfo": {"name": "rah-rust-lsp", "version": "1.0.0"}}
            elif method == "ping":
                reply["result"] = {}
            elif method == "tools/list":
                reply["result"] = {"tools": [TOOL]}
            elif method == "tools/call":
                try:
                    if params.get("name") != TOOL["name"]:
                        raise ValueError("unknown tool")
                    if client is None:
                        client = Lsp(args.root, args.server, lambda: disconnected.is_set() or active[0] in cancelled)
                    result = json.dumps(client.query(params.get("arguments", {})), ensure_ascii=False)
                    if len(result) > 100000:
                        raise ValueError("result exceeds 100000 characters; narrow the query")
                    reply["result"] = {"content": [{"type": "text", "text": result}]}
                except Exception as error:
                    reply["result"] = {"isError": True, "content": [{"type": "text", "text": str(error) or "LSP timed out; result unknown"}]}
                    if client is not None:
                        client.close()
                        client = None
            else:
                reply["error"] = {"code": -32601, "message": "Method not found"}
            sys.stdout.buffer.write(json.dumps(reply, ensure_ascii=False).encode("utf-8") + b"\n")
            sys.stdout.buffer.flush()
            cancelled.discard(active[0])
            active[0] = None
    finally:
        disconnected.set()
        if client is not None:
            client.close()
        input_reader.join(timeout=1)


if __name__ == "__main__":
    main()
