# RAH v0.1 Architecture

## Ownership boundaries

RAH public boundaries use only RAH-owned neutral types. `rah-protocol` is the
dependency-bottom crate and contains serializable identifiers, messages, events,
tool descriptions, tool calls, and tool outputs. Provider and runtime adapters
translate at their private edges.

```text
rah-protocol
  ^       ^        ^         ^
  |       |        |         |
model   tools   session   sandbox
  \       |        |        /
   \      |        |       /
          rah-runtime
                ^
                |
       rah-runtime-codex
                ^
                |
       codex app-server process
```

`AgentRuntime`, `ModelBackend`, `Tool`, `ToolRegistry`, `SessionStore`, and
`Sandbox` remain independent extension points. External tools must adapt into the
same `Tool` and `ToolRegistry` path as built-in tools; plugins do not own runtime,
policy, session, or sandbox internals.

## Deterministic runtime

`MinimalTestRuntime` proves the provider-neutral loop using `MockBackend`. A host
may explicitly allow permission levels for this deterministic setup. The default
allows only `PermissionLevel::None`; the manifest end-to-end demo explicitly adds
`Read`, while `FsReadTool` separately enforces its configured workspace boundary.

## Codex runtime adapter

`rah-runtime-codex` owns five private layers:

```text
CodexRuntime
 -> session/thread/turn translation
 -> correlated connection actor
 -> private JSON-RPC parsing
 -> stdio transport
 -> owned codex app-server child
```

The executable must report `codex-cli 0.148.0`. The adapter generates the installed
app-server schema locally and verifies the required initialize, thread, turn,
message-delta, completion, resume, and interrupt payload fields against the
captured contract before spawning a session process.

Each RAH `SessionId` is generated independently. The private map stores the Codex
thread ID and active turn ID. `start` performs `thread/start` followed by
`turn/start`; `resume` performs only `thread/resume` and never invents input or
starts a new turn; `cancel` sends `turn/interrupt` and waits for an interrupted
`turn/completed` notification.

One owned connection task serializes writes, correlates responses, broadcasts
notifications, retains subprocess failures, and rejects server requests. Dropping
an active turn stream queues an interrupt. Dropping the final connection owner
aborts the connection task, which drops and terminates the child transport.

Unknown additive notifications are logged and ignored. Unknown correlation,
malformed framing, unsafe server requests, or lifecycle ambiguity fail through a
typed adapter error or terminal RAH failure event.

## Conformance and architecture gates

Generic deterministic conformance helpers cover the observable contracts of
`ModelBackend`, `Tool`, `SessionStore`, and `AgentRuntime`. Codex-specific contract
tests use fake subprocess transport and captured JSON without exposing private IDs.

The architecture test fails when forbidden Codex Rust dependencies appear, when
provider dependencies enter core crates, when `rah-protocol` gains an upward RAH
dependency, when app-server details escape the adapter, or when the tool-security
regression evidence is removed.
