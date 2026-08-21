# ADR 0006 — Codex dynamic tools bridge into RAH execution

Status: Accepted

## Context

ADR-0005 established the Codex app-server process boundary and kept Codex-owned
execution capabilities disabled. Research in
`docs/RAH_V0.2_TOOL_BRIDGE_RESEARCH.md` evaluated the version-pinned experimental
dynamic-tool protocol as a way for Codex to request execution through RAH without
transferring execution or authorization authority to Codex.

## Decision

RAH accepts Recommendation B for an explicitly enabled v0.2 RAH Tool Bridge
with these rules:

1. Codex dynamic tools are untrusted requests, never execution authority.
2. The RAH-owned `ToolRegistry` remains the execution authority.
3. RAH-owned permission policy remains the authorization authority.
4. RAH `Sandbox` remains authoritative for tools to which it applies.
5. Codex-owned shell, file, MCP, web, image, and app capabilities remain disabled.
6. Codex approval requests remain denied and are never approved automatically.
7. Dynamic tools use the version-pinned experimental app-server contract.
8. Eligible registered RAH tool definitions are snapshotted for each Codex
   thread and translated generically.
9. One RAH-owned app-server connection is the sole responder for a tool-bearing
   thread.
10. Provider-specific identifiers, aliases, protocol DTOs, and routing remain
    private to `rah-runtime-codex`.
11. No architecture-defining RAH public contract changes are authorized.

The first implementation advertised only the RAH `EchoTool`. The promoted bridge
uses the same private execution path for every tool in its host-supplied
`ToolRegistry`; enabling tools or permissions remains an explicit host decision.
Use of Codex's version-pinned experimental dynamic-tool protocol remains an
adapter-local compatibility risk.

## Consequences

- A model-requested dynamic tool call is translated into a real RAH `ToolCall`,
  checked against RAH permission policy, and dispatched only through
  `ToolRegistry`.
- Only actual RAH-owned execution produces RAH tool lifecycle events. Codex
  dynamic-tool notifications do not produce duplicate events.
- The adapter opts into Codex's experimental API only in explicit bridge mode and
  retains the exact Codex executable and schema version pin.
- The adapter privately owns request correlation, thread and turn routing,
  aliasing, deduplication, cancellation, and response translation.
- Unknown, malformed, misrouted, replayed, denied, cancelled, or disconnected
  calls fail closed without enabling another Codex capability.
- Enabling additional host capabilities, interactive approval, multiple
  responders, or a non-experimental production bridge requires a later explicit
  decision and implementation task.
