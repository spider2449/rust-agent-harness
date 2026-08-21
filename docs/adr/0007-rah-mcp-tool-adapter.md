# ADR 0007 — RAH MCP tool adapter

Status: Accepted

## Context

`docs/RAH_V0.2_MCP_RESEARCH.md` evaluated how MCP server tools can enter RAH's
existing tool execution path without transferring execution or authorization
authority to Codex or to an MCP server. Recommendation B adds an isolated MCP
adapter crate and preserves every architecture-defining RAH public contract.

The echo-only prototype needs a deterministic local transport contract. It pins
MCP revision `2025-06-18`, whose initialization lifecycle matches the prototype
requirements, and implements only stdio. Supporting another protocol revision
or transport requires an explicit compatibility decision.

## Decision

RAH accepts Recommendation B with these rules:

1. MCP-backed tools are ordinary RAH `Tool` implementations.
2. MCP ownership lives in the isolated `rah-tools-mcp` crate.
3. `rah-runtime-codex` must not know whether a `Tool` is MCP-backed.
4. Codex-owned MCP remains disabled.
5. `ToolRegistry` remains execution authority.
6. RAH permission configuration remains authorization authority.
7. MCP server metadata must never grant or escalate RAH permissions.
8. RAH must not claim local sandbox authority over actions performed inside an
   external MCP server unless RAH actually enforces that boundary.
9. The initial implementation uses the pinned MCP `2025-06-18` protocol
   contract.
10. The initial prototype supports stdio only.
11. Streamable HTTP is deferred.
12. MCP `tools/list` discovery is adapted into RAH `ToolDefinition` values.
13. MCP `tools/call` is adapted from a RAH `ToolCall` through the ordinary
    `Tool::execute` boundary.
14. MCP results are translated into RAH `ToolOutput` or `ToolError` values.
15. No architecture-defining RAH public contract changes are authorized.

## Consequences

- `rah-tools-mcp` owns MCP protocol translation, request correlation, stdio
  process lifecycle, timeout, cancellation, and disconnect handling.
- The initial adapter discovers immutable proxy tools named
  `mcp.<configured_server_id>.<remote_tool_name>` and preserves the remote input
  schema unchanged.
- The configured server ID and permission are RAH-owned. Server name,
  descriptions, schemas, annotations, and other metadata cannot alter
  authorization. The echo prototype is registered with
  `PermissionLevel::None` only.
- A valid MCP result with `isError: true` becomes a completed RAH
  `ToolOutput` with `is_error: true`. Protocol, transport, timeout,
  cancellation, malformed-result, and disconnect failures become sanitized
  `ToolError::Execution` values. Invalid local input becomes
  `ToolError::InvalidInput`.
- Text content maps to `ToolContent::Text`; `structuredContent` maps to
  `ToolContent::Json`. Unsupported result content fails closed.
- Timeout or cancellation sends `notifications/cancelled`, ignores late
  responses, and never automatically replays an uncertain `tools/call`.
- The adapter owns and reaps its local child process. Process ownership alone is
  not described as sandboxing the MCP server's internal actions.
- No filesystem tool, process-execution tool, HTTP transport, automatic
  approval, Codex MCP configuration, or live model test is introduced by the
  prototype.
