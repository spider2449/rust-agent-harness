"""Measure a loopback Chat Completions stream without third-party packages.

This sends real inference requests. It measures the server directly, not RAH
or browser rendering. Repeated prompts can benefit from server prompt caching.
"""

import argparse
import http.client
import ipaddress
import json
import time
from urllib.parse import urlsplit


def measure(origin, model, max_tokens, context_lines=0):
    connection = http.client.HTTPConnection(origin.hostname, origin.port, timeout=120)
    prompt = "Count from 1 to 100, separated by commas. Output only the numbers."
    if context_lines:
        records = "\n".join(
            f"Record {index:06d}: alpha beta gamma delta epsilon zeta."
            for index in range(context_lines)
        )
        prompt = f"Reference records (ignore these when answering):\n{records}\n\n{prompt}"
    body = json.dumps({
        "model": model,
        "messages": [{"role": "user", "content": prompt}],
        "max_tokens": max_tokens,
        "temperature": 0,
        "stream": True,
        "stream_options": {"include_usage": True},
    })
    started = time.perf_counter()
    connected = None
    headers = None
    first = None
    first_reasoning = None
    first_delta = None
    fragments = 0
    reasoning_fragments = 0
    timings = {}
    usage = None
    finish = None
    total_bytes = 0
    try:
        connection.connect()
        connected = time.perf_counter() - started
        connection.request("POST", "/v1/chat/completions", body, {
            "Content-Type": "application/json", "Accept": "text/event-stream",
        })
        response = connection.getresponse()
        headers = time.perf_counter() - started
        if response.status != 200:
            raise RuntimeError(f"server returned HTTP {response.status}")
        while True:
            line = response.readline(262145)
            total_bytes += len(line)
            if len(line) > 262144 or total_bytes > 16 * 1024 * 1024:
                raise RuntimeError("stream exceeded measurement limits")
            if not line:
                raise RuntimeError("stream ended without DONE")
            if time.perf_counter() - started > 120:
                raise TimeoutError("measurement exceeded 120 seconds")
            if not line.startswith(b"data:"):
                continue
            data = line[5:].strip()
            if data == b"[DONE]":
                break
            event = json.loads(data)
            # Keep numeric timing diagnostics, never provider reasoning text.
            for key in ("prompt_n", "prompt_ms", "prompt_per_second", "predicted_n",
                        "predicted_ms", "predicted_per_second", "cache_n"):
                value = (event.get("timings") or {}).get(key)
                if isinstance(value, (int, float)) and not isinstance(value, bool):
                    timings[key] = value
            if event.get("usage"):
                usage = event["usage"]
            for choice in event.get("choices", []):
                delta = choice.get("delta", {})
                now = time.perf_counter() - started
                reasoning = delta.get("reasoning_content") or delta.get("reasoning")
                if reasoning:
                    first_reasoning = first_reasoning if first_reasoning is not None else now
                    reasoning_fragments += 1
                if reasoning or delta.get("content"):
                    first_delta = first_delta if first_delta is not None else now
                if delta.get("content"):
                    first = first if first is not None else now
                    fragments += 1
                if choice.get("finish_reason"):
                    finish = choice["finish_reason"]
        elapsed = time.perf_counter() - started
        if finish is None:
            raise RuntimeError("stream completed without a finish reason")
        tokens = usage.get("completion_tokens") if usage else None
        return {
            "context_lines": context_lines,
            "connect_ms": round(connected * 1000, 2),
            "headers_ms": round(headers * 1000, 2),
            "first_delta_ms": round(first_delta * 1000, 2) if first_delta is not None else None,
            "first_reasoning_ms": round(first_reasoning * 1000, 2) if first_reasoning is not None else None,
            "first_text_ms": round(first * 1000, 2) if first is not None else None,
            "total_ms": round(elapsed * 1000, 2),
            "text_fragments": fragments,
            "reasoning_fragments": reasoning_fragments,
            "prompt_tokens": usage.get("prompt_tokens") if usage else None,
            "cached_prompt_tokens": (usage.get("prompt_tokens_details") or {}).get("cached_tokens") if usage else None,
            "completion_tokens": tokens,
            "tokens_per_second_including_prefill": round(tokens / elapsed, 2) if tokens else None,
            "finish_reason": finish,
            "server_timings": timings,
        }
    finally:
        connection.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--origin", default="http://127.0.0.1:8080")
    parser.add_argument("--model", required=True)
    parser.add_argument("--max-tokens", type=int, default=128)
    parser.add_argument("--runs", type=int, default=3)
    parser.add_argument("--context-lines", type=int, default=0,
                        help="prepend 0..4096 synthetic reference records to test prompt processing")
    args = parser.parse_args()
    origin = urlsplit(args.origin)
    local = origin.hostname == "localhost"
    if not local:
        try:
            local = ipaddress.ip_address(origin.hostname or "").is_loopback
        except ValueError:
            pass
    if (origin.scheme != "http" or not local or origin.username or origin.password
            or origin.path not in ("", "/") or origin.query or origin.fragment):
        parser.error("origin must be an HTTP loopback origin")
    if not 1 <= args.runs <= 10 or not 1 <= args.max_tokens <= 4096:
        parser.error("runs must be 1..10 and max-tokens must be 1..4096")
    if not 0 <= args.context_lines <= 4096:
        parser.error("context-lines must be 0..4096")
    for run in range(1, args.runs + 1):
        print(json.dumps({"run": run, **measure(origin, args.model, args.max_tokens, args.context_lines)}), flush=True)


if __name__ == "__main__":
    main()
