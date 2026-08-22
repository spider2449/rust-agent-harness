# Task 033: MCP prerequisite hardening

## Scope

Harden only `rah-tools-mcp` as a future trusted-profile provider.  Preserve
the existing RAH `Tool` / `ToolRegistry` boundary and do not compose MCP into a
trusted profile.

## Design

1. Canonicalize and revalidate a host-selected native executable; reject
   relative paths, wrappers on Windows, and symlink/reparse paths where the
   platform exposes them.
2. Spawn the child with an adapter-owned temporary cwd and a cleared
   environment (Windows keeps only `SystemRoot` when present), direct argv,
   and bounded host-only stderr capture.
3. Replace unbounded protocol work with bounded command, outstanding-request,
   message, result, stderr, and response limits.  Protocol/resource failures
   terminate the generation and fail pending work without replay.
4. Make startup, initialize, discovery, call, and shutdown timeouts explicit.
5. Require discovery to match the exact host-assigned remote name set.  An
   optional expected-tool declaration additionally pins a canonical JSON input
   schema and permission.  Discovery is fully validated before immutable RAH
   proxies are returned.

## Validation

Run focused MCP fixture tests, then workspace fmt/check/test/clippy, diff
check, metadata, and final status/diff inspection.

## Non-goals

No trusted-profile MCP composition, network MCP, generic subprocess API,
automatic restart/replay, profile schema, or PermissionLevel changes.
