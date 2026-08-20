# ADR 0005 — Codex uses the app-server process boundary

Status: Accepted

## Context

ADR-0001 makes `AgentRuntime` a RAH-owned abstraction, ADR-0002 confines Codex
integration to `rah-runtime-codex`, and ADR-0003 requires executable capabilities
to pass through RAH's `Tool` and `ToolRegistry` boundary. Task 019 evaluated the
current Codex integration surfaces and recorded its findings in
`docs/CODEX_INTEGRATION_SPIKE.md`.

The current Codex Rust crates expose a large, tightly coupled, and unsupported
Rust integration surface. Codex app-server instead provides a documented headless
process protocol for thread lifecycle, streamed events, approvals, and
cancellation. Its built-in command, file-change, and MCP execution paths do not,
however, inherently pass through RAH's tool, policy, and sandbox boundaries.

## Decision

For RAH v0.1:

1. Codex integration uses a version-pinned `codex app-server` subprocess.
2. The adapter communicates with app-server over stdio using the app-server
   JSON-RPC protocol.
3. RAH does not directly depend on `codex-core`, `codex-protocol`,
   `codex-app-server`, or any other Codex Rust crate.
4. All Codex protocol, transport, process-lifecycle, identifier, and event details
   remain private to `rah-runtime-codex`. No Codex type crosses a public RAH
   boundary.
5. `CodexRuntime` is an optional implementation of RAH's `AgentRuntime`; it is not
   an implementation of `ModelBackend`.
6. Codex built-in command execution, file changes, and MCP execution are disabled
   or unsupported unless every action can be mediated through RAH's
   `ToolRegistry`, permission policy, and sandbox path.
7. Post-execution Codex events must never be translated into
   `AgentEvent::ToolRequested`, `AgentEvent::ToolStarted`, or
   `AgentEvent::ToolFinished` in a way that falsely implies RAH authorized or
   executed the action.
8. The first restricted `CodexRuntime` does not support interactive Codex
   approvals. Such support requires a separately designed, RAH-owned approval
   response contract.
9. `AgentRuntime::cancel` maps an active Codex turn to `turn/interrupt`; the
   adapter confirms cancellation from the terminal turn notification.
10. RAH `SessionId` values remain independent from Codex thread identifiers. The
    adapter privately owns and maintains their mapping.

Changing the process boundary, enabling Codex-owned executable capabilities, or
introducing an interactive approval-response contract requires a later explicit
architecture decision.

## Consequences

- Codex remains replaceable and optional, and upstream Rust implementation changes
  remain outside the RAH dependency graph.
- `rah-runtime-codex` owns executable discovery and version checks, stdio framing,
  JSON-RPC correlation, protocol translation, child-process lifecycle, and the
  RAH-to-Codex session mapping.
- RAH accepts a runtime dependency on a compatible Codex executable instead of a
  compile-time dependency on Codex Rust crates.
- The initial adapter has a deliberately restricted capability set. Unsupported
  Codex tool or approval requests must fail safely rather than bypass RAH policy.
- Cancellation is turn-oriented even though the public RAH operation is
  session-oriented, so the adapter must track the active turn for each mapped
  session.
- Generated protocol fixtures may be used privately by `rah-runtime-codex`, but
  they do not become RAH public types.
- This decision narrows ADR-0002 for v0.1: ADR-0002 permits Codex dependencies only
  in `rah-runtime-codex`, while this ADR chooses not to add those Rust dependencies.

## Implementation status

Implemented for v0.1 by Tasks 020 through 024. The implementation preserves this
decision without amendment: no Codex Rust dependency was added, Codex-owned tools
remain unsupported, and the public `AgentRuntime` contract was not changed.
