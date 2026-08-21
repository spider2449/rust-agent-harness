# RAH v0.2 Process Plugin Architecture Research

Status: Research only; no implementation is authorized by this document
Date: 2026-08-21

## Executive conclusion

RAH can add language-neutral process plugins without changing any
architecture-defining public contract. The process-plugin implementation should
live in an isolated `rah-tools-plugin` crate. It should start one explicitly
configured child process, negotiate a versioned RAH-owned JSON-RPC 2.0 protocol
over stdio, discover remote tools, assign permissions exclusively from trusted
host policy, and return immutable `Arc<dyn Tool>` proxies for ordinary
registration in `ToolRegistry`.

The required execution path remains:

```text
Codex
 -> Generic Codex Tool Bridge
 -> ToolRegistry
 -> PluginTool
 -> RAH Process Plugin Adapter
 -> external plugin process
 -> plugin result
 -> ToolOutput
 -> ToolRegistry
 -> Generic Codex Tool Bridge
 -> Codex continuation
```

`rah-runtime-codex` sees only `ToolDefinition`, `ToolCall`, `ToolOutput`,
`ToolError`, and `ToolRegistry`. It must not import `rah-tools-plugin`, inspect a
tool's concrete type, generate plugin names, or implement plugin lifecycle logic.

The prototype is an architecture and transport proof, not an OS sandbox. A child
process inherits authority from its launch context even when RAH gives its proxy
tool `PermissionLevel::None`. Current RAH path policy and `ProcessSandbox` cannot
prevent that process from using filesystem, process, or network APIs directly.
The prototype must therefore use a trusted deterministic fixture and state this
limitation prominently.

## 1. Current repository review

### 1.1 RAH-owned tool contracts

The current boundaries are already suitable:

| Mechanism | Current behavior | Process-plugin use |
| --- | --- | --- |
| `Tool` | `Send + Sync` trait returning a neutral definition and asynchronously executing `ToolInput` in `ToolContext` | Implement once per discovered remote tool as `PluginTool`. |
| `ToolRegistry` | Stores `Arc<dyn Tool>`, rejects duplicate `ToolName`, returns sorted definitions, and dispatches calls by name | Register plugin proxies without plugin-specific registry behavior. |
| `ToolDefinition` | Serializable name, description, JSON input schema, and host-visible `PermissionLevel` | Build an immutable definition after discovery and host permission assignment. |
| `ToolCall` | RAH-owned call ID, stable tool name, and untrusted JSON input | The registry selects the proxy; the proxy validates and translates the input. |
| `ToolOutput` | Ordered text or JSON content plus `is_error` | Translate a successful plugin response into this exact type. |
| `ToolError` | Duplicate, unknown, invalid-input, and execution failures | Map local validation to `InvalidInput`; map sanitized remote, protocol, timeout, and transport failures to `Execution`. |
| `PermissionLevel` | Neutral `None`, `Read`, `Write`, or `Execute` classification | Take only an explicit trusted-host assignment. Plugin data never chooses it. |

`ToolRegistry::execute` currently consumes the `ToolCall` and passes only its
`ToolInput` to `Tool::execute`. A proxy therefore does not observe the RAH
`ToolCallId`. This is not a contract gap: the adapter can generate a private
remote execution identity for each actual proxy invocation, while the caller
that owns a `ToolCall` remains responsible for deduplicating that call before
registry dispatch.

### 1.2 External permission policy

`ExternalToolIdentity` is intentionally opaque and non-empty.
`ExternalToolPermissionPolicy` is a trusted-host map with a critical distinction:
an absent assignment is not `PermissionLevel::None`. The MCP adapter already
demonstrates the correct sequence:

```text
discover remote name
 -> construct ExternalToolIdentity
 -> require permission_for(identity) == Some(...)
 -> construct ToolDefinition
 -> register proxy
```

Process plugins should reuse these types unchanged. They must not copy or fork
the permission model into a manifest-specific permission system.

### 1.3 Workspace and sandbox boundaries

`WorkspacePolicy` canonicalizes paths and rejects path and symlink escapes. Its
documentation correctly says that path validation is not OS isolation.
`ProcessSandbox` validates a subprocess working directory against one workspace,
uses direct program/argument invocation, supports a timeout, captures output,
and kills on dropped execution. It also correctly refuses `ReadOnly` and
`WorkspaceWrite`, because the local executor can enforce only `FullAccess`.

These are reusable security principles, but neither existing type should be
coupled directly into the prototype plugin adapter:

- a long-lived bidirectional plugin is not a one-shot `Sandbox::execute` call;
- `ProcessSandbox` captures output only after process completion and therefore
  cannot serve supervised IPC;
- making the plugin working directory pass `WorkspacePolicy` would validate a
  path but would not confine process filesystem access;
- launching the plugin inside the RAH workspace would grant an unnecessary
  ambient path and should not be the default.

Future enforced OS isolation may be injected into the adapter's process-launch
implementation, but the public `Sandbox` contract need not change for this
prototype.

### 1.4 Current MCP process supervision

`rah-tools-mcp` is the closest implementation reference. It already owns:

- direct child spawning with piped stdin/stdout and `kill_on_drop`;
- pinned protocol initialization and `tools/list` discovery;
- immutable proxy construction and `ToolOutput` / `ToolError` mapping;
- request correlation with monotonically increasing private IDs;
- timeout and dropped-future cancellation;
- ignoring late responses whose pending entry was removed;
- no automatic replay or reconnect;
- graceful stdin close, bounded shutdown wait, forced kill, and reap.

The process-plugin adapter should reuse these concepts and test patterns, not
depend on `rah-tools-mcp` or reuse its MCP wire DTOs. The current narrow MCP
prototype also exposes gaps that must not be copied:

- it uses `mpsc::unbounded_channel`;
- it uses `BufRead::lines`, which does not bound a line before allocation;
- it discards stderr rather than draining and retaining bounded diagnostics;
- it inherits the parent environment and current working directory;
- unknown and duplicate response IDs are silently ignored;
- it has no explicit maximum result, schema, or tool-list size.

### 1.5 Generic Codex Tool Bridge

The bridge snapshots definitions from an ordinary `ToolRegistry`, owns private
Codex aliases where a RAH name cannot be advertised directly, checks the current
definition and allowed permission at execution time, creates a RAH `ToolCall`,
and invokes `ToolRegistry::execute`. It owns Codex-side routing, replay handling,
cancellation, and translation back to Codex.

Nothing in that path depends on built-in or MCP concrete tool types. A
`plugin.test.echo` proxy will follow the same path. Plugin remote names and
protocol request IDs remain invisible to the bridge; Codex aliasing remains
private to `rah-runtime-codex`.

### 1.6 Existing plugin extension document and accepted ADRs

`RAH_ARCHITECTURE_PLUGIN_EXTENSION.md` already fixes the central direction:
plugins expose tools through process IPC, do not own runtime internals, do not
bypass policy, remain language-neutral, and are interchangeable with built-in
and MCP tools at `ToolRegistry`.

The accepted ADRs reinforce rather than conflict with this spike:

- ADR 0001 keeps `AgentRuntime` RAH-owned.
- ADR 0002 confines Codex integration and types to its adapter.
- ADR 0003 makes `Tool` / `ToolRegistry` the extension boundary.
- ADR 0004 is unaffected; a tool plugin is not a model inference engine.
- ADR 0005 keeps Codex executable capabilities behind RAH authority.
- ADR 0006 requires the generic Codex bridge to dispatch only through the
  RAH-owned registry, permission, and applicable sandbox path.
- ADR 0007 establishes the directly analogous isolated external-tool adapter,
  immutable proxies, host-owned permissions, no replay, and truthful sandbox
  claims.

## 2. Ownership and dependency direction

The implementation should add, but this task does not create:

```text
rah-tools-plugin -> rah-tools -> rah-protocol
                 -> async/runtime and serialization dependencies
```

Direct `rah-protocol` use from `rah-tools-plugin` is acceptable for constructing
neutral tool definitions and outputs, matching `rah-tools-mcp`. No dependency
edge should point from `rah-tools` to the adapter.

```text
rah-runtime-codex -> rah-tools
rah-runtime-codex -X-> rah-tools-plugin   (production dependency forbidden)
rah-runtime       -X-> rah-tools-plugin
rah-core          -X-> rah-tools-plugin
rah-protocol      -X-> any RAH crate
```

A crate-local integration example or test may use `rah-tools-plugin` as a
development dependency of `rah-runtime-codex`, as the current live MCP bridge
example does, but production code must not acquire that edge.

## 3. First plugin model

The first model is deliberately narrow:

```text
explicit trusted-host configuration
 -> one language-neutral executable process
 -> versioned RAH-owned protocol over supervised stdio
 -> one immutable PluginTool proxy per accepted discovered tool
```

Excluded from v0.2 prototype scope are Rust dynamic libraries, Rust ABI
coupling, in-process plugin code, DLL/SO/dylib loading, sockets, plugin search
paths, installation, marketplace behavior, dependency resolution, hot reload,
and general runtime hooks.

## 4. Minimum manifest and configuration

### 4.1 Trust distinction

There are three different sources of data:

1. **Host configuration** is trusted and selects a specific manifest, expected
   plugin ID, permission assignments, environment values, and working directory.
2. **Manifest data** is declarative launch metadata. It is not permission and is
   trusted only to the extent that the host explicitly selected it.
3. **Process-reported metadata** is untrusted protocol input and can never grant
   permission or change configured identity.

The prototype should not automatically scan for manifests or executables.

### 4.2 Proposed minimum manifest

Use one explicit TOML manifest with this conceptual shape:

```toml
manifest_version = 1
plugin_id = "test"
plugin_version = "0.1.0"
rah_plugin_protocol = "1"
executable = "./rah-plugin-echo"
args = []

[metadata]
display_name = "RAH deterministic echo fixture"
```

Required fields are `manifest_version`, `plugin_id`, `plugin_version`,
`rah_plugin_protocol`, and `executable`. `args` defaults to empty. Human-facing
metadata is optional and never enters authorization decisions.

Environment and working directory should be host launch-policy configuration,
not plugin requests that the host automatically honors. If serialized beside
the manifest for convenience, they must remain explicitly host-owned fields.
The manifest should have no `permissions` or `required_permissions` field in the
prototype. A future informational capability declaration may help a human decide
whether to enable a plugin, but it still must not grant authority.

Resolve relative executable paths against the manifest directory, canonicalize
the result before spawn, require a regular file, and invoke it directly without
a shell. Arguments are literal argument-vector entries.

### 4.3 Static declaration versus discovery

Use dynamic discovery after a successful handshake only. Static tool lists add
two sources of truth, stale-definition handling, and mismatch policy without
improving the echo prototype. The manifest may later contain optional hashes or
expected tool names for audit/pinning, but it should not duplicate descriptions,
schemas, or permissions now.

## 5. Plugin identity and tool permission model

Validate configured plugin IDs as 1-64 lowercase ASCII letters, digits, `_`, or
`-`. Validate remote tool names with the same component grammar. This keeps
canonical identities and names unambiguous without provider-specific escaping.

Keep the three identity layers distinct:

| Identity | Authority and purpose |
| --- | --- |
| Host-configured plugin ID | Authoritative namespace and permission-policy input. It cannot change after configuration. |
| Manifest and process-reported ID | Expected consistency evidence only. Both must exactly match the configured ID or startup fails. |
| Process executable identity | Canonical executable path plus optional future file hash/signature evidence. It identifies launched bytes for audit, but it neither chooses the plugin namespace nor grants permission. |

A process cannot rename itself into another configured plugin namespace. A valid
signature or executable hash, if added later, would authenticate bytes or a
publisher; it still would not replace host permission assignment.

Use this canonical `ExternalToolIdentity` namespace:

```text
plugin:<configured_plugin_id>:<remote_tool_name>
```

For the prototype that identity is `plugin:test:echo`. The host assigns:

```text
plugin:test:echo -> PermissionLevel::None
```

The public RAH tool name is independently:

```text
plugin.<configured_plugin_id>.<remote_tool_name>
```

Therefore the prototype registers `plugin.test.echo`. `ToolRegistry` remains the
final collision authority. The adapter retains the remote name privately for
wire calls. It never creates Codex aliases.

Permission assignment occurs after discovery and before proxy construction. If
any discovered tool lacks an exact host assignment, initialization fails closed
and no proxy from that process is returned. This all-or-nothing rule avoids a
configuration typo silently hiding one remote capability while registering
others. Different remote identities may receive different levels.

Plugin names, descriptions, schemas, capabilities, requested permissions, and
contradictory metadata are ignored for authorization. In particular:

- no assignment means failure, not `PermissionLevel::None`;
- a name such as `read_file` does not imply `Read`;
- a description claiming safety does not imply `None`;
- plugin-reported `permission` fields are rejected as unexpected or ignored as
  metadata, never merged with host policy;
- only the host assignment is copied into `ToolDefinition.permission`.

## 6. RAH plugin protocol v1

### 6.1 Transport and framing

JSON-RPC 2.0 over stdio is sufficient for v0.2. It is widely implementable,
supports correlation and notifications, and matches the required lifecycle
without adopting MCP semantics. RAH owns every method and DTO.

Use newline-delimited UTF-8 JSON with exactly one compact JSON object per line.
Embedded newlines remain JSON string escapes. NDJSON is sufficient for the
prototype only with an incremental bounded reader; `read_line` / `lines` without
a pre-allocation limit is not acceptable. A future protocol may adopt
length-prefix framing without changing RAH public tool contracts.

### 6.2 Envelope and request IDs

Every message contains `"jsonrpc":"2.0"`. Host requests use positive unsigned
64-bit integer IDs that increase monotonically for one connection and are never
reused. The plugin must echo the exact numeric ID in one response. Requests have
`method` and object `params`; notifications omit `id`; responses contain exactly
one of `result` or `error` and no `method`.

Request IDs correlate wire messages only. They are not RAH `ToolCallId` values
and are not remote execution identities.

### 6.3 Initialization and version negotiation

The host sends `initialize` first:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "initialize",
  "params": {
    "protocol_versions": ["1"],
    "configured_plugin_id": "test",
    "host": {"name": "rah-tools-plugin", "version": "0.2.0"},
    "capabilities": {"cancellation": true}
  }
}
```

The plugin responds with one selected version and its identity:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "protocol_version": "1",
    "plugin": {"id": "test", "version": "0.1.0"},
    "capabilities": {"cancellation": true}
  }
}
```

The selected version must be one offered by the host and must equal the
manifest's pinned version for the prototype. Complex semantic-version ranges are
unnecessary. The reported plugin ID must exactly match the configured ID; the
reported version must match the manifest version. Mismatch terminates startup.
The configured ID remains authoritative even after a matching response.

After validation, the host sends an `initialized` notification. The plugin must
not send responses, requests, or other notifications before this point.

### 6.4 Tool discovery

The host sends `tools/list` with empty object params. The result is:

```json
{
  "tools": [{
    "name": "echo",
    "description": "Returns the supplied value.",
    "input_schema": {"type": "object"}
  }]
}
```

Tool names must be unique and valid components. Descriptions must be bounded
UTF-8 strings. `input_schema` must be a JSON object and a structurally valid
schema suitable for the existing neutral `ToolDefinition`; the prototype need
not implement full JSON Schema semantic validation. Unknown fields are rejected
in protocol v1 where practical so misspelled security-relevant fields do not go
unnoticed.

There is no discovery refresh, list-changed notification, or mutable definition
in the prototype. Restart or configuration change reconstructs proxies and a new
registry snapshot.

### 6.5 Tool invocation

For every actual `PluginTool::execute`, the adapter creates a random or
monotonic connection-unique opaque `execution_id` and a separate JSON-RPC request
ID, then sends:

```json
{
  "jsonrpc": "2.0",
  "id": 7,
  "method": "tools/call",
  "params": {
    "execution_id": "01J...",
    "name": "echo",
    "arguments": {"value": "hello"}
  }
}
```

The response result is deliberately RAH-shaped but protocol-private:

```json
{
  "content": [
    {"type": "text", "value": "hello"},
    {"type": "json", "value": {"echo": "hello"}}
  ],
  "is_error": false
}
```

A valid `is_error:true` result is a completed `ToolOutput`, not a transport
failure. Invalid local input becomes `ToolError::InvalidInput`. JSON-RPC remote
errors, protocol violations, disconnects, timeouts, crashes, and malformed or
oversized results become sanitized `ToolError::Execution`.

### 6.6 Cancellation and shutdown

Cancellation is a host notification:

```json
{
  "jsonrpc": "2.0",
  "method": "tools/cancel",
  "params": {"execution_id": "01J...", "reason": "RAH request ended"}
}
```

It is sent when the proxy future is dropped or its timeout expires, if the
process remains connected and advertised cancellation. The local request becomes
terminal immediately; cancellation acknowledgement is not required. A late
response for that cancelled request ID is recognized and ignored.

Graceful shutdown uses a `shutdown` request. Once its response is received, the
host closes stdin and expects process exit. The plugin must not initiate
shutdown. No separate `exit` notification is needed for v1.

### 6.7 JSON-RPC error format

Use the standard error object:

```json
{"code": -32602, "message": "invalid params", "data": {"kind": "invalid_input"}}
```

Reserve standard JSON-RPC codes for parse, invalid request, method, and params
errors. RAH plugin-specific codes occupy a documented adapter-local range, for
example `-32099` through `-32000`. `message` and `data` are untrusted diagnostics
and must be size-limited, sanitized, and mapped to stable host-side error text
before crossing `ToolError`.

The plugin sends no host-directed requests in protocol v1. Any request from the
plugin, unexpected method, unsolicited notification, or response with an
unknown ID is a protocol violation and closes the connection.

## 7. Process lifecycle and failure behavior

The adapter owns this exact lifecycle:

```text
configured
 -> validate manifest and host policy
 -> resolve executable and isolated cwd
 -> construct allowlisted environment
 -> spawn with piped stdin/stdout/stderr and kill-on-drop
 -> handshake within timeout
 -> validate version and identity
 -> send initialized
 -> discover bounded tool set
 -> require host permission for every tool
 -> construct immutable Tool proxies
 -> return Vec<Arc<dyn Tool>>
 -> execute bounded calls
 -> shutdown
 -> close stdin
 -> wait, force terminate if needed, and reap
```

Failure policy:

| Failure | Required behavior |
| --- | --- |
| Invalid config or manifest | Fail before spawn with an adapter-local typed error. |
| Spawn failure | Return a sanitized startup error; no tools are registered. |
| Handshake timeout | Terminate and reap; return initialization failure. |
| Protocol or identity mismatch | Terminate and reap; register nothing. |
| Malformed or oversized stdout | Treat as protocol violation, fail pending calls, terminate, and reap. |
| Unexpected EOF or pipe error | Fail all pending calls as disconnected; reap. |
| Child crash | Fail all current calls, record bounded diagnostics and exit status, reap, and do not restart. |
| Stderr flood | Continue draining while retaining only bounded sanitized diagnostics. |
| Shutdown timeout | Force terminate, wait again, reap, and report adapter shutdown failure. |
| Drop without explicit shutdown | Signal/kill through owned supervisor and arrange reaping; never leave a detached child. |

Startup must be transactional: tools are returned only after the complete list
has been validated and permission-assigned. The host registers them afterward.
If registry registration then collides, the host shuts down the adapter and does
not retain a partial generation.

## 8. Diagnostics and stderr

Pipe and continuously drain stderr so a full pipe cannot deadlock the child.
Use a fixed-capacity byte ring with a prototype default of 64 KiB per process
and a maximum accepted stderr record of 8 KiB. Once full, discard older bytes
and record a truncation counter. Memory use must remain constant regardless of
plugin output rate.

Before diagnostics enter tracing:

- decode lossily as UTF-8;
- escape control characters other than normal line boundaries;
- cap every emitted field;
- redact configured secret values and common credential-shaped assignments;
- attach configured plugin ID and process exit status as structured fields;
- never treat plugin text as a tracing target, field name, or format string.

Sanitization reduces accidental disclosure but cannot prove that arbitrary text
contains no secret. Raw stderr is therefore host diagnostic data only. It is
never included in model-visible `ToolError`, `ToolOutput`, or Codex continuation.
`ToolError` should say, for example, `process plugin disconnected`; operators can
correlate a private trace event for bounded detail.

## 9. Backpressure and resource limits

All untrusted-boundary queues and payloads must be bounded. Recommended
prototype defaults are intentionally conservative and configurable only within
hard maxima:

| Resource | Prototype limit | Limit behavior |
| --- | ---: | --- |
| Concurrent outstanding protocol requests | 32 per process | Reject a new call locally as busy; do not enqueue or spawn work. |
| Supervisor command queue | 64 commands | `try_send` failure closes or rejects the initiating operation; never wait while holding scarce capacity. |
| Stdout JSON message | 1 MiB before newline | Protocol violation; terminate connection. |
| Outbound JSON message | 1 MiB serialized | Reject locally before writing. |
| Tool result | 1 MiB serialized within message limit | Fail the call and close on oversized wire input. |
| Discovered tools | 128 | Initialization failure. |
| Description | 16 KiB per tool | Initialization failure. |
| Input schema | 256 KiB per tool, within aggregate message limit | Initialization failure. |
| Stderr retention | 64 KiB per process | Ring-buffer truncation; continue draining. |
| JSON nesting | Serde's recursion limit, plus shallow DTO validation | Reject parse/validation failure. |
| Retired request IDs | 64, at least twice max outstanding | Bounded tombstones for late-cancel recognition. |

The stdout reader must read bounded chunks and search for newline without ever
growing beyond 1 MiB. If the limit is crossed before a delimiter, terminate
without attempting to parse the prefix.

Cancellation commands need reserved queue capacity or a separate small bounded
control channel so saturated call traffic cannot prevent cancellation or
shutdown. Shutdown always takes priority. Pending-map size is independently
checked even if queue capacity is available.

## 10. Response validation and correlation

Treat every plugin byte as untrusted. Validate envelope shape before method DTOs.
Reject malformed UTF-8/JSON, non-object messages, wrong JSON-RPC version, invalid
ID types, both/neither result and error, unexpected methods, duplicate tool
names, invalid schemas, invalid content variants, and values exceeding limits.

Correlation states are:

```text
pending -> completed
pending -> cancelled_or_timed_out
```

- One response transitions `pending` to `completed` and resolves one waiter.
- A second response for a completed ID is a duplicate protocol violation.
- A response for an ID retired by cancellation or timeout is a permitted late
  response and is ignored.
- A response for an ID never issued, or too old to remain in the bounded retired
  set, is unsolicited and terminates the connection.
- A response whose execution identity contradicts the pending request, if the
  response includes that field, is a protocol violation.

Closing on correlation violations is intentionally strict because continuing
would risk routing a result to the wrong RAH call.

## 11. Exactly-once, replay, timeout, and cancellation

Three identities have different owners:

| Identity | Owner and purpose |
| --- | --- |
| RAH `ToolCallId` | Runtime/caller correlation and RAH events; not visible to `PluginTool::execute`. |
| JSON-RPC request ID | Adapter connection correlation; one request/response pair. |
| Remote execution ID | Adapter-generated identity for one plugin execution; used for cancellation and plugin-side deduplication if implemented. |

The adapter guarantees at-most-one wire `tools/call` send for one invocation of
`PluginTool::execute`. It cannot guarantee globally exactly-once side effects:
the child may act before a timeout, crash, or disconnect prevents delivery of
the response. The plugin may use `execution_id` to suppress duplicates within
its own process, but RAH does not rely on that for automatic retry.

Rules for v0.2:

- never automatically replay a timed-out, cancelled, disconnected, or otherwise
  uncertain `tools/call`;
- never reinterpret a retry by an upstream caller as safe merely because the
  input is equal;
- mark timeout/disconnect errors as uncertain in private diagnostics while
  exposing concise sanitized tool errors;
- ignore late responses only for known cancelled/timed-out IDs;
- do not carry execution identities across process generations;
- a newly spawned process, if introduced later, may serve only new calls.

RAH cancellation drops/aborts the registry execution future. The proxy's guard
sends `tools/cancel`, removes local pending state, and makes the local request
terminal. This is best effort. The plugin may already have completed or started
a side effect, may ignore cancellation, or may crash before receiving it.
Cancellation is not rollback and must never be described as such.

## 12. Restart policy

Restart and request retry are separate decisions. The prototype does neither.
A crash fails all current calls, invalidates the adapter generation, and causes
future calls through existing proxies to fail disconnected. The host must
explicitly construct a new adapter, repeat handshake/discovery/permission
assignment, construct new proxies, and register them in a new registry context.

Automatic bounded restart for new calls only may be researched later with a
circuit breaker and generation identity. It is unnecessary for proving the
boundary and creates definition-change and lifecycle complexity, so it is
deferred.

## 13. Environment and working-directory policy

### 13.1 Environment

Start with `Command::env_clear()`. Resolve the executable to an absolute path
before clearing the environment so `PATH` is unnecessary. Add only explicit
host-configured key/value pairs after validating names. Never forward the full
RAH environment and never support wildcard or prefix inheritance in the
prototype.

On platforms where a minimal runtime variable is technically required, use a
small documented platform allowlist, such as `SystemRoot` on Windows, and copy
only those exact variables. `PATH`, API keys, tokens, cloud credentials, proxy
variables, SSH variables, user profiles, and RAH model credentials are absent by
default. Set a non-secret `RAH_PLUGIN_PROTOCOL=1` value if useful. Trace variable
names, not values.

The deterministic echo fixture should run with an empty environment except for
documented platform necessities. Tests must inspect its received environment to
prove that a sentinel parent secret and unrelated variables are absent.

### 13.2 Working directory

Never inherit the RAH process cwd. The default is a host-created empty dedicated
temporary directory outside the repository, canonicalized before spawn and
owned for the adapter generation. An explicitly configured cwd must be an
absolute canonical directory selected by trusted host policy; the manifest
cannot silently choose the RAH workspace.

The echo fixture receives the dedicated empty directory. Cleanup occurs only
after the child is reaped. Working-directory isolation reduces accidental access
and path coupling; it does not stop the process from opening absolute paths.

## 14. Security boundary and relationship to RAH sandboxing

RAH controls:

- which manifest and executable it starts;
- direct arguments, cwd, and environment supplied at launch;
- which discovered tools receive host permission and enter the registry;
- protocol, payload, concurrency, timeout, and diagnostic limits;
- whether calls are forwarded and how results are translated;
- child lifecycle, termination, and reaping.

RAH does not currently control:

- OS filesystem access by arbitrary plugin code;
- subprocess creation by the plugin;
- network access by the plugin;
- native syscalls, user-level credentials, or other inherited token authority;
- rollback of side effects after timeout or cancellation.

`ExternalToolPermissionPolicy` authorizes exposure and invocation through RAH; it
does not confine the child. `WorkspacePolicy` validates paths selected by RAH; it
does not confine arbitrary child syscalls. `ProcessSandbox` currently proves
only `FullAccess` execution and cannot truthfully isolate the plugin.

Consequently the prototype documentation must say `supervised process`, never
`sandboxed plugin`. Running third-party untrusted plugins safely requires a
future platform-specific launcher or container/OS sandbox that can actually
enforce filesystem, process, network, and credential restrictions. That work is
separate from the protocol and proxy boundary.

## 15. Plugin adapter versus PluginManager

The prototype does not need a `PluginManager`. Use a small ownership shape:

```text
PluginConfig + PluginManifest
 -> PluginAdapter / PluginProcess
 -> Vec<Arc<dyn Tool>>
 -> host registers tools into ToolRegistry
```

`PluginAdapter` owns one supervisor and immutable proxies. Explicit `shutdown`
and drop behavior own process cleanup. A manager becomes justified only when RAH
needs multi-plugin discovery, dependency ordering, coordinated generations,
restart policy, or administrative APIs. None is required for echo.

## 16. Relationship to MCP

Both adapters should use the same RAH-facing design:

- external identity plus host permission assignment;
- immutable `Tool` proxies;
- ordinary registry registration and dispatch;
- private request correlation and transport DTOs;
- bounded timeouts, best-effort cancellation, no uncertain replay;
- owned child lifecycle and truthful sandbox statements;
- `ToolOutput` / sanitized `ToolError` translation.

The RAH plugin protocol is nevertheless not MCP. It has RAH-owned versioning,
identity verification, capabilities, result shape, validation rules, limits, and
lifecycle policy. `rah-tools-plugin` should not depend on `rah-tools-mcp`, and no
shared protocol/client abstraction should be created for the prototype. A small
amount of adapter-local Tokio process and JSON-RPC code is safer than a generic
transport abstraction whose real common contract is not yet known. Shared
bounded framing may be extracted later only after two hardened implementations
demonstrate identical requirements.

## 17. Minimal echo prototype design

The later implementation should contain:

- `rah-tools-plugin` with manifest validation, protocol-private DTOs, bounded
  stdio supervisor, diagnostics buffer, `PluginAdapter`, and `PluginTool`;
- a crate-local deterministic `rah-plugin-echo` test executable implementing
  protocol v1 and exposing only remote `echo`;
- host configuration for plugin ID `test`, explicit
  `plugin:test:echo -> PermissionLevel::None`, empty environment, isolated temp
  cwd, short deterministic timeouts, and all bounds enabled;
- generic bridge configuration with `allowed_permissions` explicitly containing
  `PermissionLevel::None` and no broader level;
- a `ToolRegistry` containing the returned `plugin.test.echo` proxy;
- a deterministic fake-Codex bridge test proving the generic bridge is unchanged;
- an opt-in live Codex example only if separately authorized and useful after
  deterministic coverage.

Expected end-to-end path:

```text
real or fake Codex requests plugin.test.echo
 -> existing generic bridge resolves its private Codex alias
 -> bridge creates ToolCall and checks the existing permission allowlist
 -> unchanged ToolRegistry dispatches PluginTool
 -> adapter sends one tools/call to the local fixture
 -> fixture returns deterministic text or JSON
 -> adapter constructs ToolOutput
 -> bridge returns the result for Codex continuation
```

The fixture intentionally performs no filesystem, shell, network, or credential
operation. However, without a real OS sandbox the host cannot technically prevent
a malicious replacement process from doing so. `PermissionLevel::None`, cleared
environment, and isolated cwd are necessary least-privilege measures, not a
proof of confinement.

## 18. Deterministic prototype test plan

All normal tests use the local fixture, no network, credentials, model, GPU, or
live Codex process.

### Manifest, identity, and startup

- valid manifest, spawn, handshake, and `initialized` ordering;
- unsupported manifest version and plugin protocol mismatch;
- configured/manifest/reported plugin identity mismatch;
- plugin version mismatch and executable spawn failure;
- handshake timeout terminates and reaps;
- executable receives literal arguments without shell interpolation.

### Discovery, permission, naming, and registry

- discovery maps `echo` to `plugin.test.echo` deterministically;
- explicit `plugin:test:echo` assignment supplies the definition permission;
- unconfigured discovered tool fails closed with no proxies returned;
- two discovered tools can receive different host permissions;
- contradictory plugin permission metadata cannot override host policy;
- duplicate/invalid remote names and invalid/oversized schemas fail startup;
- final RAH-name collision is rejected by unchanged `ToolRegistry`;
- registry dispatch executes the proxy while registry remains plugin-agnostic.

### Results and errors

- text result maps to `ToolContent::Text`;
- JSON result maps to `ToolContent::Json`;
- valid remote tool error maps to `ToolOutput { is_error: true }`;
- JSON-RPC error maps to sanitized `ToolError::Execution`;
- non-object local arguments fail as `ToolError::InvalidInput` if the discovered
  schema requires an object;
- unsupported content and oversized result fail closed.

### Protocol robustness and bounds

- malformed JSON, invalid UTF-8, excessive nesting, and oversized stdout message;
- unexpected method/request/notification from plugin;
- unknown response ID terminates the connection;
- duplicate response terminates the connection;
- known late response after cancellation is ignored;
- 33rd concurrent request is rejected at a 32-request limit;
- bounded command queue cannot allocate without limit and reserves shutdown and
  cancellation capacity;
- stderr flood is drained, retained at no more than 64 KiB, marked truncated,
  sanitized, and absent from `ToolError`.

### Timeout, cancellation, replay, and failure

- call timeout sends one cancellation and no second `tools/call`;
- dropping the execution future sends cancellation and makes it terminal;
- cancellation is not reported as rollback;
- late success after cancellation does not resolve another call;
- child crash and unexpected EOF fail every pending call;
- disconnect after possible execution is reported uncertain and never replayed;
- a second call through a dead generation fails without restart;
- explicit shutdown response, stdin close, exit, and reap;
- shutdown timeout forces termination and reap;
- drop cleanup leaves no child process.

### Environment, cwd, and bridge isolation

- `env_clear` removes a sentinel parent secret and proxy/credential variables;
- only exact configured environment keys and platform necessities are present;
- child cwd is the dedicated empty directory and not the repository;
- relative manifest executable resolution does not change child cwd;
- architecture test proves `rah-runtime-codex` has no production dependency on
  `rah-tools-plugin`;
- existing generic Codex bridge tests pass unchanged;
- a bridge integration test uses only `Arc<ToolRegistry>` and observes text/JSON
  continuation without importing a plugin type in production bridge code.

## 19. Public contract review

No implementation change is required to any requested public contract:

| Contract | Change required? | Reason |
| --- | --- | --- |
| `AgentRuntime` | No | It continues to consume a generic registry and manage agent lifecycle. |
| `Tool` | No | A remote proxy implements the existing trait. |
| `ToolRegistry` | No | It already accepts trait objects and rejects name collisions. |
| `ToolDefinition` | No | Name, description, schema, and host permission are sufficient. |
| `ToolCall` | No | The caller owns call deduplication; proxy-local execution identity is private. |
| `ToolOutput` | No | Text, JSON, and completed error results are representable. |
| `ToolError` | No | Invalid input and sanitized execution failures are sufficient. |
| `PermissionLevel` | No | Existing provider-neutral levels remain host assigned. |
| `AgentEvent` | No | Existing bridge/runtime emits lifecycle events around registry dispatch. |
| `Sandbox` | No | The prototype does not claim enforcement it cannot provide; future process isolation can remain adapter-internal or receive a separate researched launcher. |

Private adapter types such as manifest DTOs, protocol envelopes, request IDs,
execution IDs, supervisor commands, diagnostic buffers, and process-generation
state must not enter these public contracts.

## 20. ADR plan

Implementation should first propose `docs/adr/0008-process-plugin-adapter.md` for
human acceptance. The ADR is warranted because this work selects a plugin
execution model, RAH-owned protocol compatibility strategy, security claims,
permission namespace, and lifecycle policy. It should record the isolated crate,
stdio JSON-RPC v1, dynamic discovery, host-owned permissions, no replay/restart,
bounded resources, environment/cwd defaults, and absence of OS sandbox claims.

This research task does not create or accept ADR 0008. The existing convention
is research document first, accepted ADR second, implementation third, as shown
by the tool-bridge and MCP work.

## Final recommendation

B. PROCEED WITH ISOLATED PLUGIN ADAPTER CRATE
Existing public contracts are sufficient; add `rah-tools-plugin`.
