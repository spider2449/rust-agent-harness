# llama.cpp Responses compatibility

RAH's `rah-llamacpp` provider preset is certified only for the restricted
Responses subset used by `rah-runtime-codex`:

- streamed assistant text;
- ordinary `type = function` tools;
- function calls and function-call outputs; and
- multi-step continuation after a tool result.

This is intentionally narrower than a claim of complete OpenAI Responses API
compatibility. llama.cpp v0.3.0 skips non-function Responses tools. RAH does
not depend on those tools because the restricted Codex runtime disables MCP,
apps, web search, image viewing, shell execution, and unified execution, while
the Generic Tool Bridge advertises only ordinary function tools.

The following llama.cpp Responses surfaces are out of scope for this adapter:

- `previous_response_id`;
- arbitrary non-function Responses tools; and
- complete hosted-tool parity.

The adapter sends the full conversation input supplied by Codex 0.149.0 and
does not introduce `previous_response_id` behavior.
