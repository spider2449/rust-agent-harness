# Task 034: MCP hardening fixture matrix

## Scope

This task verifies the Task 033 `rah-tools-mcp` hardening boundary with the
repository-owned deterministic stdio fixture. It does not compose MCP into a
trusted profile or expand MCP authority.

## Verified behavior

- Each child receives an adapter-created temporary cwd, not the parent cwd;
  the fixture observes it through a test-owned file and the adapter removes it
  after the child is reaped.
- The inherited environment is cleared. A parent secret sentinel is absent in
  the child. On Windows `SystemRoot` remains present exactly when it was
  present in the parent so a native executable can launch.
- Initialize and discovery each use the 2-second host timeout. Tool calls use
  the configured timeout (30 seconds by default), and shutdown waits 500 ms
  before termination/reaping. Direct process spawn has no separate async
  startup handshake; spawn failure is reported synchronously before initialize.
- Bounded framing rejects a line once it exceeds 1 MiB without accumulating the
  entire frame. The default limits are 32 outstanding requests, 64 queued
  commands, 1 MiB protocol messages, 1 MiB MCP result/RAH output, and a 64 KiB
  stderr diagnostic tail. Retired request IDs retain at most 64 IDs.
- Discovery is atomic and accepts only the exact host-assigned remote name set.
  Expected schemas compare recursively normalized JSON values: object key order
  is ignored, while array order and every schema value remain significant.

## Fixture modes

The local `rah-mcp-echo-server` supports test-only modes for cwd/environment
observation, hangs during initialize/discovery, oversized frames and results,
stderr flooding, malformed/missing/extra/duplicate/schema-drift discovery,
late responses, and child exits at lifecycle stages. Arguments selecting modes
are supplied solely by the test host configuration.

## Regression fix

The original bounded control queue had a fixed capacity of eight. With more
than eight valid concurrent timeout/cancellation paths, a `Cancel` notification
could be dropped, leaving a pending request and its permit live. The control
queue now has `max_outstanding + 1` capacity, preserving boundedness while
ensuring every admitted outstanding request can retire plus one stop signal.
