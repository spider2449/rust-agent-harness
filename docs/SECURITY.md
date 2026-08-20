# RAH v0.1 Security Model

## Trust boundary

Model output is untrusted. A model request never authorizes execution. The only
supported execution path is:

```text
parsed ToolCall
 -> ToolRegistry
 -> host permission decision
 -> Sandbox / workspace policy
 -> Tool
 -> ToolOutput
```

Tools cannot grant themselves permissions or bypass the registry. Future MCP and
process-plugin tools must enter through the same boundary.

## Workspace files and subprocesses

`FsReadTool` canonicalizes paths through `WorkspacePolicy`, rejects traversal and
outside-workspace paths, limits bytes, and rejects non-UTF-8/binary input.

`ShellExecTool` uses a program plus argument vector, validates its working
directory, captures stdout/stderr/exit status, and supports timeout through the
sandbox abstraction. These controls are policy and process boundaries; RAH does
not claim that path checking alone is strong operating-system isolation.

## Restricted Codex adapter

The adapter pins the executable and schema contract before use. It owns stdin,
stdout, bounded retained stderr, JSON-RPC correlation, abnormal-exit reporting,
shutdown, and active-turn interruption.

For v0.1, restricted threads force no-approval and read-only settings and disable
known shell, unified execution, web search, image-view, app, dynamic-tool, and MCP
surfaces in adapter configuration. Every server-initiated request, including
command/file/permission approval, dynamic tool, and MCP elicitation requests,
receives an explicit unsupported error. Nothing is accepted automatically.

If a command, file-change, MCP, or dynamic-tool item is nevertheless observed, the
RAH stream fails. Such post-execution items are never translated into
`ToolRequested`, `ToolStarted`, or `ToolFinished`, because that would falsely imply
RAH authorization. Codex sandbox settings are defense in depth and are not a
replacement for RAH's policy and sandbox contracts.

## Known limitations

- The restricted adapter must not be used to enable Codex-owned tools.
- Interactive Codex approvals require a future RAH-owned response contract.
- Version pinning limits compatibility; upgrading Codex requires recapturing and
  reviewing schema fixtures and rerunning the deterministic adapter suite.
- The broadcast event buffer is bounded. A lagging consumer receives a terminal
  failure instead of silently losing lifecycle or security-relevant events.
- The normal test suite proves adapter translation and lifecycle behavior with a
  fake transport; it does not claim live model, credential, or platform-sandbox
  validation.

Security-sensitive changes should run the full workspace checks and the
`rah-runtime-codex` architecture test before review.
