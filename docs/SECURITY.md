# RAH v0.2 Security Model

## Trust and authorization boundary

Model output and external-provider metadata are untrusted. A tool request or
declaration never authorizes execution. The supported path is:

```text
parsed ToolCall
 -> ToolRegistry
 -> host permission decision
 -> Sandbox / workspace policy where applicable
 -> Tool
 -> ToolOutput
```

`ExternalToolIdentity` is an opaque RAH-owned key for one tool discovered from
an external provider. `ExternalToolPermissionPolicy` maps those identities to
host-selected RAH `PermissionLevel` values. It is default-deny: absence is not
`PermissionLevel::None`, and an unassigned external tool fails before
registration. Duplicate assignments are rejected. MCP server and process-plugin
metadata cannot grant or escalate permissions.

The external assignment becomes the tool definition's required permission. The
runtime or Generic Codex Tool Bridge still checks that requirement against the
host's allowed permission levels before `ToolRegistry` dispatch. Permission
ownership therefore remains with the host at both composition and execution.

## Built-in filesystem and subprocess tools

`FsReadTool` canonicalizes paths through `WorkspacePolicy`, rejects traversal and
outside-workspace paths, limits bytes, and rejects non-UTF-8/binary input.

`ShellExecTool` uses a program plus argument vector, validates its working
directory, captures stdout/stderr/exit status, and supports timeout through the
sandbox abstraction. These controls are policy and process boundaries; RAH does
not claim that path checks or process supervision provide strong OS isolation.
Because `ShellExecTool` accepts model-selected process details, ADR 0009 leaves
it unsuitable for live model exposure.

The deterministic v0.3 Execute prototype instead uses a capability-specific
`HostExecutionTool`. Its immutable `HostExecutionPolicy` selects one canonical
native executable, renders exact or typed argv, fixes cwd beneath a canonical
host root, clears and explicitly rebuilds the environment, closes stdin, fixes
the timeout, and enforces bounded concurrent stdout/stderr reads. Execute
permission remains a separate required runtime gate. Output overflow and timeout
attempt termination and return bounded structured error results; neither means
rollback. Windows uses best-effort Job Object ownership and Unix uses a
best-effort process group. These mechanisms supervise processes but do not
provide filesystem or network isolation.

## MCP process boundary

`rah-tools-mcp` directly launches an explicitly configured local MCP executable
without shell-string interpolation. The adapter owns the pinned stdio protocol,
request correlation, timeouts, cooperative cancellation, shutdown, termination,
and process reaping. Discovered definitions and results are validated and
translated into neutral RAH types; unsupported result content fails closed.

The MCP server is a separate process with its own possible filesystem, process,
and network authority. Owning and supervising that process does not sandbox its
internal actions. Cancellation may stop waiting and sends the protocol's
notification, but it is not rollback: the server may already have caused side
effects. Timed-out, cancelled, disconnected, or otherwise uncertain
`tools/call` operations are not automatically replayed.

## Process Plugin process boundary

`rah-tools-plugin` launches an explicitly selected executable directly and uses
RAH process-plugin protocol version `1` over bounded NDJSON stdio. It validates
configured/reported identity and version before discovery. The host controls
every permission assignment.

The adapter applies bounded plugin IPC, including limits for queued commands,
outstanding requests, message bytes, result bytes, discovered metadata, and
retired request tracking. Plugin stderr is drained into a bounded, lossy,
control-escaped, host-only diagnostic tail; it is never tool output,
model-visible data, or authorization input.

The inherited environment is cleared. Only `RAH_PLUGIN_PROTOCOL`, Windows
`SystemRoot` where required to launch the child, and explicit host-allowlisted
name/value pairs are provided. Each generation receives a newly created isolated
temporary working directory rather than the RAH workspace, and the adapter
removes it after process termination.

Cancellation is best effort and is not rollback. A plugin can finish or begin an
external side effect before cancellation is observed. Each tool call is sent at
most once; timed-out, disconnected, and otherwise uncertain external calls are
never automatically replayed. Existing proxies fail after disconnection until
the host explicitly creates a new adapter.

Environment minimization, an isolated cwd, resource bounds, and child-process
supervision reduce accidental ambient authority and denial-of-service exposure.
They are not OS sandboxing and do not prevent arbitrary child syscalls,
filesystem access, subprocess creation, or network access.

## Restricted Codex adapter

The adapter pins the executable and schema contract before use. It owns stdin,
stdout, bounded retained stderr, JSON-RPC correlation, abnormal-exit reporting,
shutdown, and active-turn interruption.

Restricted mode disables Codex-owned shell, unified execution, file, MCP, web,
image, app, and approval surfaces. Generic bridge mode enables only the
version-pinned dynamic-tool request transport used to reach RAH's registry.
Codex `mcp_servers` remains empty. MCP elicitation and all approval requests are
rejected, while command, file-change, and MCP tool items fail the RAH stream.
They never become RAH tool lifecycle events because RAH did not authorize them.

Codex sandbox settings are defense in depth, not a replacement for RAH policy,
registry, tool, or sandbox contracts.

## Known limitations

- Interactive Codex approvals are unsupported.
- The Codex adapter and external protocols are exactly pinned compatibility
  boundaries.
- Cancellation across any external process boundary cannot undo side effects.
- The broadcast event buffer is bounded; a lagging consumer receives a terminal
  failure instead of silently losing security-relevant events.
- Deterministic tests validate translation, policy, and lifecycle behavior; they
  do not claim live model, credential, third-party server, or platform-sandbox
  validation.
