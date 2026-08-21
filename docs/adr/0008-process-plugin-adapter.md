# ADR 0008 — Process plugin adapter

Status: Accepted

## Context

`docs/RAH_V0.2_PROCESS_PLUGIN_RESEARCH.md` evaluated how future plugins can
provide tools without entering RAH runtime internals or bypassing the existing
tool, permission, and sandbox boundaries. Recommendation B adds an isolated
process adapter while preserving every architecture-defining public contract.

The first implementation is deliberately an echo-only prototype. It proves the
process, protocol, identity, permission, resource-bound, and lifecycle boundary
without creating a general plugin platform.

## Decision

RAH accepts Recommendation B with these rules:

1. Process plugins are external processes exposing ordinary RAH `Tool` values.
2. Plugin ownership lives in the isolated `rah-tools-plugin` crate.
3. `rah-runtime-codex` does not know whether a `Tool` is plugin-backed.
4. Process plugins use a RAH-owned, versioned JSON-RPC 2.0 protocol.
5. The initial transport is bounded newline-delimited JSON over stdio.
6. Rust DLL, SO, and dylib loading are not part of this design.
7. Host-configured plugin identity is authoritative.
8. Plugin-reported identity must match the configured identity.
9. Tools are discovered only after a successful versioned handshake.
10. Stable RAH tool names use `plugin.<plugin_id>.<remote_tool_name>`.
11. External tool permissions are assigned only by the existing
    `ExternalToolPermissionPolicy`.
12. A missing host permission assignment fails closed.
13. Plugin metadata, names, descriptions, schemas, and manifests cannot grant
    or escalate RAH permissions.
14. Process ownership and supervision are not described as OS sandboxing.
15. The child environment is cleared and minimized, then populated only from an
    explicit allowlist.
16. A plugin working directory must not implicitly be the RAH workspace.
17. Request queues, outstanding requests, message sizes, results, stderr, and
    retained diagnostics are bounded.
18. Cancellation is best-effort and does not imply rollback.
19. Timed-out, disconnected, or otherwise uncertain calls are never
    automatically replayed.
20. Automatic process restart is deferred from the prototype.
21. No `PluginManager` is required for the prototype.
22. No architecture-defining RAH public contract changes are authorized.

The prototype pins RAH process-plugin protocol version `1`, uses stdio only,
configures plugin ID `test`, discovers remote tool `echo`, maps it to
`plugin.test.echo`, and requires the explicit host assignment
`plugin:test:echo -> PermissionLevel::None`.

## Consequences

- `rah-tools-plugin` depends on `rah-tools` and `rah-protocol`; neither runtime
  nor Codex crates depend on the plugin adapter.
- Initialization validates the exact protocol version, configured identity, and
  reported plugin version before sending `initialized` and discovering tools.
- Plugin-backed tools enter the unchanged `ToolRegistry` and generic Codex tool
  bridge as ordinary RAH tools.
- The adapter owns direct child launch, bounded protocol correlation, best-effort
  cancellation, graceful shutdown, forced termination when necessary, and
  process reaping.
- Cleared environment and isolated working directory reduce accidental ambient
  authority but do not confine arbitrary child syscalls. A future OS-enforced
  sandbox requires a separate design.
- Valid plugin-declared tool failures remain completed `ToolOutput` values with
  `is_error: true`; transport and protocol failures become sanitized
  `ToolError::Execution` values.
- Raw plugin stderr is bounded host diagnostic data only. It never becomes a
  `ToolOutput`, model-visible error, or authorization input.
- Cancellation and timeout retire request IDs and ignore only known late
  responses. Unknown or duplicate responses invalidate the connection.
- Calls are sent at most once. Existing proxies fail after a crash or disconnect
  until the host explicitly constructs a new adapter.
