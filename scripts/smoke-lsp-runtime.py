"""Explicit local subprocess smoke checks; no model, package install or downloads.

Checks LSP initialization and a deliberate diagnostic for each bundled language.
PATH excludes host development tools. Rust uses a detached file (syntax check),
not a Cargo project; this does not prove project/toolchain completeness.
"""

import argparse
import importlib.util
import json
import os
from pathlib import Path
import queue
import subprocess
import tempfile
import threading
import time

spec = importlib.util.spec_from_file_location("prepare_lsp", Path(__file__).with_name("prepare-lsp-runtime.py"))
runtime = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runtime)


def smoke(bundle, name, extension, language, source):
    with tempfile.TemporaryDirectory(prefix="rah lsp smoke ") as tmp:
        workspace = Path(tmp)
        file = workspace / ("example" + extension)
        file.write_text(source, encoding="utf-8")
        argv = [str(bundle / arg) if not arg.startswith("--") else arg for arg in runtime.PROGRAMS[name]]
        env = {key: value for key, value in os.environ.items() if key.upper() not in {"PATH", "NODE_PATH", "NODE_OPTIONS", "PYTHONPATH", "VIRTUAL_ENV"}}
        env["PATH"] = str(Path(os.environ.get("SystemRoot", "C:/Windows")) / "System32")
        env["CARGO_NET_OFFLINE"] = "true"
        messages = queue.Queue()
        with tempfile.TemporaryFile() as errors:
            process = subprocess.Popen(argv, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=errors, cwd=workspace, env=env)

            def read():
                try:
                    while True:
                        headers = {}
                        while True:
                            line = process.stdout.readline()
                            if not line:
                                raise EOFError("language server exited")
                            if line == b"\r\n":
                                break
                            key, value = line.decode("ascii").split(":", 1)
                            headers[key.lower()] = value.strip()
                        messages.put(json.loads(process.stdout.read(int(headers["content-length"]))))
                except Exception as error:
                    messages.put(error)

            reader = threading.Thread(target=read)
            reader.start()

            def send(message):
                data = json.dumps(message).encode("utf-8")
                process.stdin.write(f"Content-Length: {len(data)}\r\n\r\n".encode() + data)
                process.stdin.flush()

            def receive(predicate):
                deadline = time.monotonic() + 60
                while time.monotonic() < deadline:
                    message = messages.get(timeout=max(0.01, deadline - time.monotonic()))
                    if isinstance(message, Exception):
                        raise message
                    if "method" in message and "id" in message:
                        result = None
                        if message["method"] == "workspace/configuration":
                            result = [None for _ in message.get("params", {}).get("items", [])]
                        send({"jsonrpc": "2.0", "id": message["id"], "result": result})
                    elif predicate(message):
                        return message
                raise TimeoutError("No expected LSP response")

            options = {}
            if name == "rust":
                options = {"detachedFiles": [str(file)], "cargo": {"buildScripts": {"enable": False}}, "procMacro": {"enable": False}, "checkOnSave": False}
            if name == "typescript":
                options = {"tsserver": {"path": str(bundle / "node_modules/typescript/lib/tsserver.js")}, "disableAutomaticTypingAcquisition": True}
            try:
                send({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"processId": os.getpid(), "rootUri": workspace.as_uri(), "workspaceFolders": [{"uri": workspace.as_uri(), "name": "smoke"}], "capabilities": {"workspace": {"configuration": True}, "textDocument": {"publishDiagnostics": {"relatedInformation": False}, "synchronization": {"didSave": True}}}, "initializationOptions": options}})
                response = receive(lambda m: m.get("id") == 1)
                if "error" in response:
                    raise ValueError(response["error"])
                send({"jsonrpc": "2.0", "method": "initialized", "params": {}})
                send({"jsonrpc": "2.0", "method": "textDocument/didOpen", "params": {"textDocument": {"uri": file.as_uri(), "languageId": language, "version": 1, "text": source}}})
                send({"jsonrpc": "2.0", "method": "textDocument/didSave", "params": {"textDocument": {"uri": file.as_uri()}}})
                diagnostic = receive(lambda m: m.get("method") == "textDocument/publishDiagnostics" and m.get("params", {}).get("diagnostics"))
                print(f"PASS {language}: {len(diagnostic['params']['diagnostics'])} diagnostic(s)", flush=True)
                send({"jsonrpc": "2.0", "id": 2, "method": "shutdown", "params": None})
                receive(lambda m: m.get("id") == 2)
            except Exception:
                errors.seek(0)
                print(errors.read()[-4000:].decode("utf-8", "replace"), flush=True)
                raise
            finally:
                if process.poll() is None:
                    # Keep the parent alive until its tsserver children are
                    # terminated too; Windows can otherwise retain the cwd.
                    if os.name == "nt":
                        subprocess.run([str(Path(env["PATH"]) / "taskkill.exe"), "/PID", str(process.pid), "/T", "/F"], capture_output=True, timeout=15, check=False)
                    if process.poll() is None:
                        process.kill()
                process.wait(timeout=10)
                reader.join(timeout=10)
                process.stdin.close()
                process.stdout.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("bundle", type=Path)
    args = parser.parse_args()
    bundle = args.bundle.resolve(strict=True)
    runtime.check(bundle)
    for case in [
        ("typescript", ".ts", "typescript", 'const value: number = "wrong";\n'),
        ("typescript", ".js", "javascript", '// @ts-check\n/** @type {number} */\nlet value = "wrong";\n'),
        ("pyright", ".py", "python", 'value: int = "wrong"\n'),
        ("clangd", ".cpp", "cpp", 'int main() { return missing_name; }\n'),
        ("clangd", ".c", "c", 'int main(void) { return missing_name; }\n'),
        ("rust", ".rs", "rust", 'fn main() { let = ; }\n'),
    ]:
        smoke(bundle, *case)


if __name__ == "__main__":
    main()
