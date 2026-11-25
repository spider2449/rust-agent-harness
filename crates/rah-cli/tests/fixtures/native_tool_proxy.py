"""Loopback-only adapter for the ignored native grammar evaluation.

Run llama.cpp on a dedicated port with the Qwen3-Coder chat template first.
Run this proxy with that upstream port and pass its printed port to
RAH_SMALL_MODEL_PORT for live_small_model_native_grammar. The proxy forwards
native tool deltas unchanged and does not interpret assistant text.
"""

import argparse
import json
from http.client import HTTPConnection
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer


def make_proxy(upstream_port):
    if not 1 <= upstream_port <= 65535 or upstream_port == 8080:
        raise ValueError("use an isolated upstream port")
    class Handler(BaseHTTPRequestHandler):
        def do_POST(self):
            if self.path != "/v1/chat/completions":
                self.send_error(404)
                return
            try:
                length = int(self.headers.get("Content-Length", "0"))
                if not 0 < length <= 512 * 1024:
                    raise ValueError("unexpected request length")
                body = json.loads(self.rfile.read(length))
                tools = body.get("tools", [])
                if len(tools) > 1:
                    raise ValueError("the benchmark allows one tool per action stage")
                if tools:
                    body["tool_choice"] = "required"
                    body["parallel_tool_calls"] = False
                    body["max_tokens"] = min(body.get("max_tokens", 256), 256)
            except (ValueError, AttributeError, TypeError) as error:
                self.send_error(400, str(error))
                return

            upstream = HTTPConnection("127.0.0.1", upstream_port, timeout=125)
            try:
                try:
                    upstream.request("POST", self.path, body=json.dumps(body).encode("utf-8"),
                                     headers={"Content-Type": "application/json"})
                    response = upstream.getresponse()
                except (OSError, ValueError):
                    self.send_error(502, "isolated llama.cpp is unavailable")
                    return
                with response:
                    self.send_response(response.status)
                    self.send_header("Content-Type", response.getheader("Content-Type", "application/json"))
                    self.end_headers()
                    for line in response:
                        self.wfile.write(line)
                        self.wfile.flush()
            except OSError:
                pass
            finally:
                upstream.close()

    return ThreadingHTTPServer(("127.0.0.1", 0), Handler)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("upstream_port", type=int)
    args = parser.parse_args()
    with make_proxy(args.upstream_port) as proxy:
        print(proxy.server_port, flush=True)
        proxy.serve_forever()
