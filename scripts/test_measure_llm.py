"""Offline checks for the opt-in LLM measurement tool."""

import io
import itertools
import json
from pathlib import Path
import runpy
import unittest
from unittest.mock import patch
from urllib.parse import urlsplit


MEASURE = runpy.run_path(str(Path(__file__).with_name("measure-llm.py")))["measure"]


class Response(io.BytesIO):
    status = 200


class Connection:
    def __init__(self, response):
        self.response = response
        self.closed = False
        self.body = None

    def connect(self):
        pass

    def request(self, method, path, body, headers):
        self.body = json.loads(body)

    def getresponse(self):
        return self.response

    def close(self):
        self.closed = True


def stream(*events):
    return b"".join(b"data: " + json.dumps(event).encode() + b"\n\n" for event in events) + b"data: [DONE]\n\n"


class MeasurementTests(unittest.TestCase):
    def measure(self, payload, status=200, context_lines=0):
        response = Response(payload)
        response.status = status
        connection = Connection(response)
        clock = itertools.count(0, 0.001)
        with patch("http.client.HTTPConnection", return_value=connection), \
                patch("time.perf_counter", side_effect=lambda: next(clock)):
            try:
                result = MEASURE(urlsplit("http://127.0.0.1:8080"), "test", 128, context_lines)
            finally:
                self.assertTrue(connection.closed)
        return result, connection.body

    def test_reasoning_and_content_are_timed_separately_without_retaining_text(self):
        result, body = self.measure(stream(
            {"choices": [{"delta": {"reasoning_content": "private reasoning"}}]},
            {"choices": [{"delta": {"content": "answer"}, "finish_reason": "stop"}]},
            {"choices": [], "usage": {"prompt_tokens": 80, "completion_tokens": 12,
                                      "prompt_tokens_details": {"cached_tokens": 50}},
             "timings": {"prompt_ms": 10, "predicted_ms": 20, "cache_n": 50,
                         "unexpected": "ignore", "predicted_n": True}},
        ), context_lines=2)
        self.assertLess(result["connect_ms"], result["headers_ms"])
        self.assertLess(result["first_reasoning_ms"], result["first_text_ms"])
        self.assertEqual(result["first_delta_ms"], result["first_reasoning_ms"])
        self.assertEqual(result["reasoning_fragments"], 1)
        self.assertEqual(result["text_fragments"], 1)
        self.assertEqual(result["cached_prompt_tokens"], 50)
        self.assertEqual(result["server_timings"], {"prompt_ms": 10, "predicted_ms": 20, "cache_n": 50})
        self.assertNotIn("private reasoning", json.dumps(result))
        self.assertIn("Record 000001", body["messages"][0]["content"])

    def test_content_only_without_usage_is_supported(self):
        result, body = self.measure(stream({"choices": [{
            "delta": {"content": "answer"}, "finish_reason": "length",
        }], "timings": None}))
        self.assertIsNone(result["first_reasoning_ms"])
        self.assertIsNone(result["completion_tokens"])
        self.assertEqual(result["first_delta_ms"], result["first_text_ms"])
        self.assertNotIn("Record", body["messages"][0]["content"])

    def test_eof_without_done_fails(self):
        with self.assertRaisesRegex(RuntimeError, "without DONE"):
            self.measure(b"")

    def test_done_without_finish_fails(self):
        with self.assertRaisesRegex(RuntimeError, "finish reason"):
            self.measure(stream())

    def test_bad_json_fails(self):
        with self.assertRaises(json.JSONDecodeError):
            self.measure(b"data: invalid\n\n")

    def test_http_error_fails(self):
        with self.assertRaisesRegex(RuntimeError, "HTTP 503"):
            self.measure(b"", status=503)

    def test_oversized_line_fails(self):
        with self.assertRaisesRegex(RuntimeError, "limits"):
            self.measure(b"x" * 262145)


if __name__ == "__main__":
    unittest.main()
