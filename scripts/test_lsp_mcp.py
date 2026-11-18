"""Deterministic bridge checks; set RAH_TEST_LSP for the local-toolchain smoke."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import threading
import queue
import unittest
import time
from unittest.mock import patch

import lsp_mcp


class BridgeTests(unittest.TestCase):
    def test_repeated_navigation_without_diagnostics_and_with_background_work(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            file = root / "main.rs"
            file.write_text("fn main() {}")
            client = object.__new__(lsp_mcp.Lsp)
            client.root = root
            client.events = queue.Queue()
            client.cancelled = lambda: False
            client.pending, client.diagnostics, client.versions, client.texts = {}, {}, {}, {}
            client.busy = {"background-check"}
            client.sequence = 0
            client.quiescent = False
            sent = []

            def write(message):
                sent.append(message)
                if message.get("method") == "textDocument/didOpen":
                    client.events.put({"method": "experimental/serverStatus", "params": {"health": "ok", "quiescent": True}})
                if "id" in message:
                    self.assertTrue(client.quiescent)
                    client.events.put({"id": message["id"], "result": {"answer": message["method"]}})

            client.write = write
            for operation in ["symbols", "hover", "hover", "definition", "references"]:
                result = client.query({"path": "main.rs", "operation": operation, "line": 0, "character": 3})
                self.assertEqual(result, {"answer": lsp_mcp.METHODS[operation]})
            self.assertEqual([m["method"] for m in sent if "id" not in m], ["textDocument/didOpen"])
            file.write_text("fn renamed() {}")
            client.query({"path": "main.rs", "operation": "symbols"})
            change = sent[-2]
            self.assertEqual(change["method"], "textDocument/didChange")
            self.assertEqual(change["params"]["textDocument"]["version"], 2)
            self.assertEqual(change["params"]["contentChanges"], [{"text": "fn renamed() {}"}])
            self.assertEqual(client.diagnostics, {})

    def test_request_deadline_survives_continuous_notifications(self):
        client = object.__new__(lsp_mcp.Lsp)
        client.sequence, client.pending = 0, {}
        client.write = lambda message: None
        client.pump = lambda timeout: None
        with patch.object(lsp_mcp.time, "monotonic", side_effect=[0, 1, 2]):
            with self.assertRaisesRegex(TimeoutError, "textDocument/hover timed out"):
                client.request("textDocument/hover", {}, 2)

    def test_catalog_and_handshake_without_starting_a_server(self):
        requests = [{"jsonrpc": "2.0", "id": 1, "method": "initialize"},
                    {"jsonrpc": "2.0", "method": "notifications/initialized"},
                    {"jsonrpc": "2.0", "id": 2, "method": "tools/list"},
                    {"jsonrpc": "2.0", "id": 3, "method": "unknown"}]
        result = subprocess.run([sys.executable, "-B", str(Path(lsp_mcp.__file__)),
                                 "--root", ".", "--server", "missing-server"],
                                input="".join(json.dumps(r) + "\n" for r in requests),
                                capture_output=True, text=True, timeout=10, check=True)
        messages = [json.loads(line) for line in result.stdout.splitlines()]
        self.assertEqual([m["id"] for m in messages], [1, 2, 3])
        self.assertEqual(messages[1]["result"]["tools"][0]["name"], "rust_lsp")
        self.assertEqual(messages[2]["error"]["code"], -32601)

    def test_workspace_escape_and_bad_positions_are_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp) / "workspace"
            root.mkdir()
            outside = Path(tmp) / "outside.rs"
            outside.write_text("fn main() {}")
            inside = root / "main.rs"
            inside.write_text("fn main() {}")
            client = object.__new__(lsp_mcp.Lsp)
            client.root = root
            with self.assertRaisesRegex(ValueError, "within this workspace"):
                client.query({"path": "../outside.rs", "operation": "symbols"})
            with self.assertRaisesRegex(ValueError, "zero-based"):
                client.query({"path": "main.rs", "operation": "hover", "line": -1})

    def test_progress_and_stale_diagnostics(self):
        client = object.__new__(lsp_mcp.Lsp)
        client.events = queue.Queue()
        client.cancelled = lambda: False
        client.busy = set()
        client.diagnostics = {}
        client.versions = {lsp_mcp.uri_key("file:///a.rs"): 2}
        for token, kind in [(1, "begin"), ("1", "begin"), (1, "end")]:
            client.events.put({"method": "$/progress", "params": {"token": token, "value": {"kind": kind}}})
            client.pump(0)
        self.assertEqual(client.busy, {"1"})
        client.events.put({"method": "textDocument/publishDiagnostics", "params": {
            "uri": "file:///a.rs", "version": 1, "diagnostics": ["stale"]}})
        client.pump(0)
        self.assertEqual(client.diagnostics, {})

    def test_cancel_interrupts_an_idle_server_wait(self):
        client = object.__new__(lsp_mcp.Lsp)
        client.cancelled = lambda: True
        with self.assertRaises(InterruptedError):
            client.pump(45)

    def test_stdio_cancellation_and_disconnect(self):
        # Replace only the subprocess-facing LSP connection. Exercise the actual
        # MCP input thread and dispatcher with a request that cannot finish alone.
        code = '''
import lsp_mcp, time
class Waiting:
    def __init__(self, root, server, cancelled): self.cancelled = cancelled
    def query(self, args):
        while not self.cancelled(): time.sleep(.01)
        raise InterruptedError("cancelled")
    def close(self): pass
lsp_mcp.Lsp = Waiting
lsp_mcp.main()
'''
        process = subprocess.Popen([sys.executable, "-B", "-c", code, "--root", ".", "--server", "unused"],
                                   cwd=Path(lsp_mcp.__file__).parent, stdin=subprocess.PIPE,
                                   stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        replies = queue.Queue()
        reader = threading.Thread(target=lambda: replies.put(process.stdout.readline()))
        reader.start()
        try:
            for message in [
                {"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": "rust_lsp", "arguments": {}}},
                {"jsonrpc": "2.0", "method": "notifications/cancelled", "params": {"requestId": 1}},
            ]:
                process.stdin.write(json.dumps(message) + "\n")
            process.stdin.flush()
            response = json.loads(replies.get(timeout=5))
            self.assertTrue(response["result"]["isError"])
            self.assertIn("cancelled", response["result"]["content"][0]["text"])
            process.stdin.write(json.dumps({"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": "rust_lsp", "arguments": {}}}) + "\n")
            process.stdin.close()
            self.assertEqual(process.wait(timeout=5), 0)
        finally:
            if process.poll() is None:
                process.kill()
                process.wait(timeout=5)
            reader.join(timeout=5)
            process.stdout.close()
            process.stderr.close()

    @unittest.skipUnless(os.environ.get("RAH_TEST_LSP"), "requires explicit local rust-analyzer")
    def test_live_repository_navigation_over_mcp(self):
        root = Path(lsp_mcp.__file__).resolve().parent.parent
        process = subprocess.Popen([sys.executable, "-B", str(Path(lsp_mcp.__file__)),
                                    "--root", str(root), "--server", os.environ["RAH_TEST_LSP"]],
                                   stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                   text=True, encoding="utf-8")
        replies = queue.Queue()

        def read():
            for line in process.stdout:
                replies.put(json.loads(line))

        reader = threading.Thread(target=read)
        reader.start()

        def request(ident, method, params):
            process.stdin.write(json.dumps({"jsonrpc": "2.0", "id": ident, "method": method, "params": params}) + "\n")
            process.stdin.flush()
            response = replies.get(timeout=85)
            self.assertEqual(response["id"], ident)
            self.assertNotIn("error", response)
            self.assertFalse(response["result"].get("isError"), response)
            return response["result"]

        try:
            request(1, "initialize", {"protocolVersion": "2024-11-05", "capabilities": {},
                                      "clientInfo": {"name": "smoke", "version": "1"}})
            for ident, operation in enumerate(["symbols", "hover", "hover", "definition", "references", "hover", "symbols", "hover"], 2):
                started = time.monotonic()
                result = request(ident, "tools/call", {"name": "rust_lsp", "arguments": {
                    "operation": operation, "path": "crates/rah-protocol/src/lib.rs", "line": 23, "character": 15}})
                value = json.loads(result["content"][0]["text"])
                if operation == "hover":
                    self.assertIn("RAH_PROTOCOL_VERSION", json.dumps(value))
                elif operation in ("symbols", "definition"):
                    self.assertTrue(value)
                print(f"MCP {operation}: {time.monotonic() - started:.3f}s", flush=True)
        finally:
            process.stdin.close()
            try:
                process.wait(timeout=15)
            finally:
                if process.poll() is None:
                    process.kill()
                    process.wait(timeout=5)
                reader.join(timeout=5)
                process.stdout.close()
                process.stderr.close()
        self.assertEqual(process.returncode, 0)
        self.assertFalse(reader.is_alive())

    @unittest.skipUnless(os.environ.get("RAH_TEST_LSP"), "requires explicit local rust-analyzer")
    def test_live_queries_and_correction(self):
        with tempfile.TemporaryDirectory(prefix="rah mcp lsp ") as tmp:
            root = Path(tmp)
            (root / "Cargo.toml").write_text("[package]\nname='mcp-fixture'\nversion='0.1.0'\nedition='2021'\n")
            (root / "src").mkdir()
            file = root / "src/main.rs"
            file.write_text('fn value() -> u32 { 1 }\nfn main() { let _: u32 = "wrong"; let _ = value(); }\n')
            client = lsp_mcp.Lsp(root, Path(os.environ["RAH_TEST_LSP"]))
            try:
                result = client.query({"path": "src/main.rs", "operation": "diagnostics"})
                self.assertTrue(any("mismatched types" in d["message"] for d in result["diagnostics"]))
                for operation in ["symbols", "hover", "definition", "references"]:
                    result = client.query({"path": "src/main.rs", "operation": operation, "line": 0, "character": 4})
                    self.assertTrue(result, operation)
                file.write_text('fn main() {}\n')
                result = client.query({"path": "src/main.rs", "operation": "diagnostics"})
                self.assertEqual(result["diagnostics"], [])
            finally:
                client.close()
            self.assertIsNotNone(client.process.poll())
            self.assertFalse(client.reader.is_alive())


if __name__ == "__main__":
    unittest.main()
