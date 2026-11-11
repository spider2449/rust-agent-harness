# LLM stream buffer reuse

Reduce repeated allocations on the streaming response path in the Responses
and Chat Completions provider plugins. Reuse line and event buffers across
frames without changing framing, UTF-8 validation, error handling, size limits,
or public service contracts.

## Acceptance criteria

- Both SSE parsers retain line capacity across events.
- Chat Completions retains data capacity across JSON and DONE frames.
- Deterministic tests cover buffer reuse, chunk boundaries and existing errors.
- Run provider tests, formatting and diff checks.

This change reduces local allocation overhead; it does not claim a measured
improvement in provider inference time or end-to-end response latency.
