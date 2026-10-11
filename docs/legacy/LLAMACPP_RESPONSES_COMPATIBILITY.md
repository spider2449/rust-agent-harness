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

## Windows live certification

RAH verified this exact combination and restricted Responses/function-tool
subset on Windows:

- RAH base commit: `7d06a3578d43907dd0a687af31b42be680b276ad` (Task 107).
- Codex: `codex-cli 0.149.0` from the certified full package.
- llama.cpp: b10621, source `c1d0e7a004015f23bc0233470b747b596f29b264`.
- Windows asset: `llama-b10621-bin-win-cuda-12.4-x64.zip`, SHA-256
  `81C2FF62E14B549CD5C766CCDD5C61F09E821A171655C3047BDCCFDDC2D1A1E2`.
- `llama-server.exe` SHA-256:
  `0F706D509CE7937504D527F49CC1E68A0E45EE60CFC4153B3FF16F76A0F4E3AC`.
- Model: `Ornith-1.5-35B-Q4_K_M.gguf` (Ornith 1.5 35B, `Q4_K_M`), SHA-256
  `CA6EA26329C88B78FFD90A85163BE2E746C2FAFD1024F56DB47E499F117F9A7F`.
- Endpoint and alias: `http://127.0.0.1:8080/v1`, `rah-local-model`.

The server used the model's native Jinja chat template and a 16,384-token
context. Direct `/v1/responses` chat and one ordinary `type = function` tool
call passed before the RAH gate.

RAH then verified the explicit effective model `rah-local-model` and provider
`rah-llamacpp`, streamed assistant text, terminal completion, and clean Codex
app-server shutdown. The chat marker was `RAH_LLAMACPP_CHAT_OK`.

The Generic Tool Bridge verification advertised only the ordinary `echo`
function with `PermissionLevel::None`. It observed exactly one
`ToolRequested`, `ToolStarted`, and `ToolFinished` event, one actual registry
execution, no replay, model continuation after the tool result, terminal
completion, and `RAH_LLAMACPP_TOOL_OK`.

This certification does not establish compatibility for every GGUF, every
llama.cpp build, all Responses features, hosted tools, namespaces, MCP, or
repository-edit tool selection. The optional repository-edit live gate was not
run.
